#!/usr/bin/env python3
"""Tests for scripts/agent-cost.py, run against the scripted stand-in agents.

    python scripts/test-agent-cost.py --mage-parse PATH   (or $MAGE_PARSE_BIN)

No model is called. A handful of tasks keeps it quick; the full stand-in run is
documented in benchmarks/AGENT_COST.md. The Rust-arm tests need `rustc` on PATH
and are skipped without it.
"""
import importlib.util
import json
import os
import shutil
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("agent_cost", os.path.join(HERE, "agent-cost.py"))
ac = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ac)

STANDIN = os.path.join(HERE, "agent_wrappers", "agent_cost_standin.py")
SUBSET = "algo-001,basic-001"
BIN = None
HAVE_RUSTC = shutil.which("rustc") is not None


def standin(mode, extra=""):
    return '"%s" "%s" %s %s' % (sys.executable.replace("\\", "/"), STANDIN.replace("\\", "/"), mode, extra)


class Base(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = tempfile.mkdtemp(prefix="agent-cost-test-")

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.root, ignore_errors=True)

    def run_driver(self, agent, name, arms="mage", only=SUBSET, extra=()):
        out = os.path.join(self.root, name)
        argv = ["--mage-parse", BIN, "--agent", agent, "--arms", arms, "--only", only, "--out", out,
                "--scratch", os.path.join(self.root, "scratch")] + list(extra)
        stdout = sys.stdout
        sys.stdout = open(os.devnull, "w")
        try:
            ac.main(argv)
        finally:
            sys.stdout.close()
            sys.stdout = stdout
        with open(os.path.join(out, "summary.json"), encoding="utf-8") as f:
            return out, json.load(f)

    def transcript(self, out, ep):
        with open(os.path.join(out, "transcripts", ep + ".jsonl"), encoding="utf-8") as f:
            return [json.loads(l) for l in f]


class Translation(unittest.TestCase):
    def test_mechanical_cases(self):
        t = ac.translate_input
        self.assertEqual(t("factorial(5)")[0], "factorial(5)")
        self.assertEqual(t("add(-1, 1)")[0], "add(-1, 1)")
        self.assertEqual(t('greet("MAGE")')[0], 'greet("MAGE")')
        self.assertEqual(t("color_name(Color.Red)")[0], "color_name(Color::Red)")
        self.assertEqual(t("@Range{start:0,end:10}.contains(5)")[0], "Range{start: 0, end: 10}.contains(5)")

    def test_refuses_what_is_not_mechanical(self):
        for s in ("v x = 1; x", "f(fn(x) => x)", "f([1, 2])", "f(1.5)", "a - 1", "f(1) + 2"):
            self.assertIsNone(ac.translate_input(s)[0], s)

    def test_task_set_and_pairing(self):
        tasks = ac.accepted_tasks(os.path.join(ac.ROOT, "benchmarks", "tasks"),
                                  os.path.join(ac.ROOT, "benchmarks", "task-acceptance-baseline.txt"))
        base = ac.ta.baseline_ids(os.path.join(ac.ROOT, "benchmarks", "task-acceptance-baseline.txt"))
        self.assertTrue(tasks)
        self.assertFalse({t["id"] for t in tasks} & base)
        by = {t["id"]: ac.translate_cases(t)[0] for t in tasks}
        if "ds-013" in by:   # closure input and a struct-valued expected: not mechanical
            self.assertIsNone(by["ds-013"])


class MageArm(Base):
    def test_oracle_solves_everything_and_accounting_adds_up(self):
        out, doc = self.run_driver(standin("oracle"), "oracle")
        s = doc["arms"]["mage"]
        self.assertEqual((s["attempted"], s["solved"]), (2, 2))
        for e in doc["episodes"]:
            self.assertEqual(sum(e["tokens"].values()), e["tokens_total"])
            self.assertEqual(e["attempts"], 1)
        self.assertEqual(sum(s["by_category"].values()), s["tokens_total"])
        self.assertEqual(round(s["tokens_total"] / 2, 1), s["tokens_per_solved"])
        self.assertGreater(s["by_category"]["prompt"], 0)
        self.assertGreater(s["by_category"]["final_source"], 0)

    def test_lookup_uses_tools_and_costs_more(self):
        _, base = self.run_driver(standin("oracle"), "o2")
        out, doc = self.run_driver(standin("lookup-then-oracle"), "lookup")
        s = doc["arms"]["mage"]
        self.assertEqual(s["solved"], 2)
        self.assertEqual(s["tool_calls"], {"check": 2, "ontology_index": 2, "ontology_query": 4})
        self.assertGreater(s["tokens_per_solved"], base["arms"]["mage"]["tokens_per_solved"])
        self.assertGreater(s["by_category"]["tool_results"], 0)
        self.assertGreater(s["by_category"]["diagnostics"], 0)
        for e in doc["episodes"]:
            self.assertEqual(sum(e["tokens"].values()), e["tokens_total"])

    def test_attempt_cap_and_feedback(self):
        out, doc = self.run_driver(standin("always-wrong"), "wrong", only="algo-001")
        e = doc["episodes"][0]
        self.assertEqual((e["outcome"], e["attempts"], e["solved"]), ("failed", 3, False))
        out, doc = self.run_driver(standin("always-wrong"), "wrong2", only="algo-001", extra=["--max-attempts", "2"])
        self.assertEqual(doc["episodes"][0]["attempts"], 2)
        msgs = [r["msg"] for r in self.transcript(out, "algo-001.mage")[:-1] if r["dir"] == "to_agent"]
        results = [m for m in msgs if m["type"] == "result"]
        self.assertEqual([m["attempts_left"] for m in results], [1, 0])
        self.assertIn("not accepted", results[0]["diagnostics"])
        self.assertIn("E0001", results[1]["diagnostics"])   # the broken second submission: checker diagnostics

    def test_second_attempt_can_solve(self):
        _, doc = self.run_driver(standin("wrong-then-oracle"), "wto", only="algo-001")
        e = doc["episodes"][0]
        self.assertEqual((e["outcome"], e["attempts"]), ("solved", 2))
        self.assertEqual(doc["arms"]["mage"]["submissions_histogram"], {"2": 1})

    def test_tool_outside_allowlist_is_a_violation(self):
        _, doc = self.run_driver(standin("tool-violation"), "viol", only="algo-001")
        e = doc["episodes"][0]
        self.assertEqual((e["outcome"], e["violation"], e["solved"]), ("violation", "run_tests", False))

    def test_garbage_is_a_protocol_error(self):
        _, doc = self.run_driver(standin("garbage"), "garb", only="algo-001")
        self.assertEqual(doc["episodes"][0]["outcome"], "protocol_error")

    def test_wall_clock_cap(self):
        _, doc = self.run_driver(standin("slow"), "slow", only="algo-001", extra=["--wall", "2"])
        self.assertEqual(doc["episodes"][0]["outcome"], "timeout")

    def test_turn_and_token_caps(self):
        _, doc = self.run_driver(standin("lookup-then-oracle"), "turns", only="algo-001", extra=["--max-turns", "2"])
        self.assertEqual(doc["episodes"][0]["outcome"], "turn_cap")
        _, doc = self.run_driver(standin("lookup-then-oracle"), "tokcap", only="algo-001", extra=["--max-tokens", "50"])
        self.assertEqual(doc["episodes"][0]["outcome"], "token_cap")

    def test_expected_values_never_reach_the_agent(self):
        log = os.path.join(self.root, "spy.log")
        out, _ = self.run_driver(standin("spy", '--log "%s"' % log.replace("\\", "/")), "spy", only=SUBSET + ",basic-003")
        tasks = {t["id"]: t for t in ac.accepted_tasks(os.path.join(ac.ROOT, "benchmarks", "tasks"),
                                                       os.path.join(ac.ROOT, "benchmarks", "task-acceptance-baseline.txt"))}
        with open(log, encoding="utf-8") as f:
            wire = f.read()
        checked = 0
        for tid in SUBSET.split(",") + ["basic-003"]:
            sent = "\n".join(json.dumps(r["msg"], ensure_ascii=False) + " " + r["msg"].get("prompt", "")
                             for r in self.transcript(out, tid + ".mage")[:-1] if r["dir"] == "to_agent")
            for c in tasks[tid]["tests"]:
                exp = c["expected"]
                if len(ac.ta.canon(exp)) >= 5 and exp not in tasks[tid]["task"] and exp not in c["input"]:
                    checked += 1
                    self.assertNotIn(exp, sent, tid)
                    self.assertNotIn(exp, wire, tid)
        self.assertGreater(checked, 0)

    def test_failed_submission_feedback_has_no_expected_values(self):
        # a type-correct wrong answer to factorial: feedback must be the generic line
        src = "+f factorial(n: i64) -> i64 { n }"
        task = [t for t in ac.accepted_tasks(os.path.join(ac.ROOT, "benchmarks", "tasks"),
                                             os.path.join(ac.ROOT, "benchmarks", "task-acceptance-baseline.txt"))
                if t["id"] == "algo-001"][0]

        class A:
            mage_parse, fuel, rustc = BIN, 5000000, "rustc"
        j = ac.Judge(A, task, "mage", os.path.join(self.root, "j"))
        os.makedirs(j.scratch, exist_ok=True)
        ok, diag = j.judge(src)
        self.assertFalse(ok)
        for c in task["tests"]:
            self.assertNotIn(c["expected"], diag)   # "120", "3628800"
        self.assertNotIn("got", diag)

    def test_deterministic(self):
        o1, d1 = self.run_driver(standin("lookup-then-oracle"), "det1")
        o2, d2 = self.run_driver(standin("lookup-then-oracle"), "det2")
        for rel in ["summary.json", "summary.md"] + ["transcripts/" + f for f in sorted(os.listdir(os.path.join(o1, "transcripts")))]:
            with open(os.path.join(o1, rel), "rb") as f1, open(os.path.join(o2, rel), "rb") as f2:
                a, b = f1.read(), f2.read()
            self.assertEqual(a, b, rel)

    def test_environment_is_scrubbed_and_usage_is_opt_in(self):
        os.environ["STANDIN_USAGE"] = "1"
        os.environ["SECRET_TEST_KEY"] = "do-not-leak"
        try:
            env = ac.agent_env([])
            self.assertNotIn("SECRET_TEST_KEY", env)
            self.assertNotIn("STANDIN_USAGE", env)
            self.assertIn("PATH", {k.upper() for k in env})
            self.assertEqual(ac.agent_env(["STANDIN_USAGE"])["STANDIN_USAGE"], "1")
            _, plain = self.run_driver(standin("oracle"), "nousage", only="algo-001")
            self.assertNotIn("reported", plain["episodes"][0])
            _, rep = self.run_driver(standin("oracle"), "usage", only="algo-001", extra=["--agent-env", "STANDIN_USAGE"])
            self.assertEqual(rep["episodes"][0]["reported"]["total"], 2)
            self.assertEqual(rep["arms"]["mage"]["reported_tokens_per_solved"], 2.0)
        finally:
            del os.environ["STANDIN_USAGE"], os.environ["SECRET_TEST_KEY"]


@unittest.skipUnless(HAVE_RUSTC, "rustc not on PATH")
class RustArm(Base):
    def test_oracle_and_pairing(self):
        out, doc = self.run_driver(standin("oracle"), "rust-oracle", arms="mage,rust", only=SUBSET + ",ds-011")
        self.assertEqual(doc["arms"]["rust"]["solved"], doc["arms"]["rust"]["attempted"])
        self.assertEqual(doc["paired"]["n"], 3)
        for e in doc["episodes"]:
            self.assertEqual(sum(e["tokens"].values()), e["tokens_total"])

    def test_rust_tools_are_check_only(self):
        agent = os.path.join(self.root, "ontology_agent.py")
        with open(agent, "w", newline="\n") as f:
            f.write('import sys\nsys.stdin.readline()\nprint(\'{"type":"tool","name":"ontology_index","args":{}}\', flush=True)\nsys.stdin.readline()\n')
        _, doc = self.run_driver('"%s" "%s"' % (sys.executable.replace("\\", "/"), agent.replace("\\", "/")),
                                 "rust-viol", arms="rust", only="algo-001")
        self.assertEqual(doc["episodes"][0]["outcome"], "violation")

    def test_rust_feedback_is_compiler_errors_only(self):
        out, doc = self.run_driver(standin("always-wrong"), "rust-wrong", arms="rust", only="algo-001", extra=["--max-attempts", "2"])
        e = doc["episodes"][0]
        self.assertEqual((e["outcome"], e["attempts"]), ("failed", 2))
        results = [r["msg"] for r in self.transcript(out, "algo-001.rust")[:-1] if r["msg"].get("type") == "result"]
        self.assertIn("not accepted", results[0]["diagnostics"])
        self.assertIn("error", results[1]["diagnostics"])
        for m in results:
            self.assertNotIn("3628800", m["diagnostics"])
            self.assertNotIn("assert_eq", m["diagnostics"])


if __name__ == "__main__":
    args = sys.argv[1:]
    BIN = os.environ.get("MAGE_PARSE_BIN")
    if "--mage-parse" in args:
        i = args.index("--mage-parse")
        BIN = args[i + 1]
        del args[i:i + 2]
    if not BIN:
        sys.exit("--mage-parse PATH or $MAGE_PARSE_BIN is required")
    unittest.main(argv=[sys.argv[0]] + args)

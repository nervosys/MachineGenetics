#!/usr/bin/env python3
"""What does it cost an AI agent, in tokens, to produce a correct program?

One *episode* is one (task, arm) pair. An *agent command* (`--agent "<cmd>"`)
is started once per episode and spoken to over JSON lines on stdin/stdout. The
driver never calls a model and holds no API key; whatever sits behind the agent
command does, and is the operator's business (see benchmarks/AGENT_COST.md).

Arms
----
mage  the agent writes MAGE and may call ONLY the compiler's self-describing
      tools: ontology_index, ontology_query, describe, check.
rust  the agent writes Rust and may call ONLY check (a rustc compile).

Protocol (JSON lines; every message is one line)
------------------------------------------------
driver -> agent  {"type":"task", ...}            once, first
                 {"type":"tool_result","name":N,"ok":B,"content":S}
                 {"type":"result","accepted":false,"attempts_left":K,"diagnostics":S}
                 {"type":"end","reason":R}       best effort, then stdin closes
agent  -> driver {"type":"tool","name":N,"args":{...}}
                 {"type":"final","source":S}     a submission, judged once
                 optional on any reply: "usage":{"input_tokens":I,"output_tokens":O}

Judging reuses scripts/task-acceptance.py: the MAGE arm uses its `judge`
(type-check + every test case, strict comparison); the Rust arm builds the same
`{input, expected}` cases into a `#[test]` harness, for the cases that can be
translated mechanically (see `translate_input`), and excludes the rest.

Token accounting uses the compiler's built-in BPE (`mage-parse --bpe-count -`),
which is MAGE's own 4096-merge vocabulary and NOT any model's tokenizer. If the
agent reports `usage`, that is recorded as the primary figure and the BPE count
as the secondary. The text counted for each message is its content, not its JSON
envelope: the task prompt, `name(args)` of a tool call, a tool result's content,
the submitted source, the diagnostics text, the verdict text.
"""
import argparse
import collections
import importlib.util
import json
import os
import queue
import re
import shlex
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)

_spec = importlib.util.spec_from_file_location("task_acceptance", os.path.join(HERE, "task-acceptance.py"))
ta = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ta)

MAGE_TOOLS = ("ontology_index", "ontology_query", "describe", "check")
RUST_TOOLS = ("check",)
TOOLS = {"mage": MAGE_TOOLS, "rust": RUST_TOOLS}
CATEGORIES = ("prompt", "tool_calls", "tool_results", "diagnostics", "final_source", "verdict")

TOOL_OUT_CAP = 6000   # characters of any tool result or diagnostics text
RUSTC_TIMEOUT = 120
RUST_RUN_TIMEOUT = 20

# What the agent command's environment is reduced to (cf. arena/src/command.rs).
KEPT_ENV = ("PATH", "SYSTEMROOT", "TEMP", "TMP", "PATHEXT") if os.name == "nt" else ("PATH",)


# ── task set ────────────────────────────────────────────────────────────────
def accepted_tasks(tasks_dir, baseline):
    """Tasks the acceptance check accepts: every task not in the shrink-only
    baseline. Derived at run time, not hard-coded."""
    base = ta.baseline_ids(baseline)
    return [t for t in ta.load_tasks(tasks_dir) if t["id"] not in base]


_SIG_MAGE = re.compile(r"\+f\s[^{;]*")
_SIG_RUST = re.compile(r"\bpub\s+fn\s[^{;]*")


def signatures(source, arm):
    rx = _SIG_MAGE if arm == "mage" else _SIG_RUST
    return [" ".join(m.group(0).split()) for m in rx.finditer(source)]


# ── mechanical MAGE -> Rust translation of test cases ───────────────────────
_RTOK = re.compile(r'\s*(?:(?P<str>"(?:[^"\\\n]|\\.)*")|(?P<int>-?\d+)|(?P<id>[A-Za-z_][A-Za-z_0-9]*)|(?P<p>[(),.{}:@]))')
_RUST_EXPECTED = re.compile(r'^(?:-?\d+|true|false|"(?:[^"\\\n]|\\.)*")$')
_NOT_RUST = {"fn", "v", "m", "ret", "loop", "if", "match"}


def _tokens(s):
    pos, toks = 0, []
    s = s.strip()
    while pos < len(s):
        m = _RTOK.match(s, pos)
        if not m:
            return None
        toks.append((m.lastgroup, m.group(m.lastgroup)))
        pos = m.end()
    return toks


def translate_input(src):
    """Translate a MAGE test input to a Rust expression, or return
    (None, reason). Only what is mechanical: integer/string literals, calls,
    method calls, `Enum.Variant` -> `Enum::Variant`, `@Name{f:v,..}` struct
    literals -> `Name{f: v, ..}`. Anything else (closures, statements, floats,
    lists, sigils, operators) is refused, never guessed."""
    toks = _tokens(src)
    if not toks:
        return None, "input does not tokenise as the supported subset"
    out = []
    for i, (k, v) in enumerate(toks):
        prev = toks[i - 1] if i else (None, None)
        nxt = toks[i + 1] if i + 1 < len(toks) else (None, None)
        if k == "id":
            if v in _NOT_RUST:
                return None, "keyword `%s`" % v
            out.append(v)
        elif k == "int":
            if v.startswith("-") and prev[1] not in ("(", ",", ":"):
                return None, "binary operator"
            out.append(v)
        elif k == "str":
            out.append(v)
        elif v == "@":
            if not (nxt[0] == "id" and nxt[1][:1].isupper() and i + 2 < len(toks) and toks[i + 2][1] == "{"):
                return None, "`@` that is not a struct literal"
        elif v == ".":
            if nxt[0] != "id":
                return None, "`.` not followed by a name"
            if prev[0] == "id" and prev[1][:1].isupper() and nxt[1][:1].isupper():
                out.append("::")
            elif nxt[1][:1].islower() and (prev[0] in ("id", "str") or prev[1] in (")", "}")):
                out.append(".")
            else:
                return None, "unsupported `.`"
        elif v == ",":
            out.append(", ")
        elif v == ":":
            out.append(": ")
        else:
            out.append(v)
    return "".join(out), ""


def translate_cases(task):
    """-> (cases, reasons). cases is a list of (rust_input, rust_expected), or
    None when any case is untranslatable (the task is then excluded from the
    Rust arm). reasons are per-case strings for the failures."""
    cases, reasons = [], []
    for i, c in enumerate(task["tests"]):
        rin, why = translate_input(c["input"])
        if rin is None:
            reasons.append("case %d input: %s" % (i + 1, why))
            continue
        exp = c["expected"].strip()
        if not _RUST_EXPECTED.match(exp):
            reasons.append("case %d expected is not an integer, bool or string literal" % (i + 1))
            continue
        cases.append((rin, exp))
    if reasons or not task.get("rust_equivalent", {}).get("rs_source", "").strip():
        return None, reasons or ["no rust reference"]
    return cases, []


def harness(source, cases):
    body = ["#[cfg(test)]", "mod __ac_harness {", "    use super::*;"]
    for i, (rin, exp) in enumerate(cases):
        body.append("    #[test] fn case_%d() { assert_eq!(%s, %s); }" % (i, rin, exp))
    body.append("}")
    return source.rstrip("\n") + "\n\n" + "\n".join(body) + "\n"


# ── token counting ──────────────────────────────────────────────────────────
class Bpe:
    """`mage-parse --bpe-count -`, cached by text. MAGE's own vocabulary."""

    def __init__(self, binary):
        self.binary, self.cache = binary, {}

    def count(self, text):
        if not text:
            return 0
        if text not in self.cache:
            p = subprocess.run([self.binary, "--bpe-count", "-"], input=text.encode("utf-8"),
                               capture_output=True, timeout=60)
            if p.returncode != 0:
                raise RuntimeError("--bpe-count failed: " + p.stderr.decode("utf-8", "replace")[:200])
            self.cache[text] = int(p.stdout.decode().split()[0])
        return self.cache[text]


# ── tools ───────────────────────────────────────────────────────────────────
def cap(s):
    s = s.replace("\r\n", "\n")
    return s if len(s) <= TOOL_OUT_CAP else s[:TOOL_OUT_CAP] + "\n...[truncated %d chars]" % (len(s) - TOOL_OUT_CAP)


class Judge:
    """Per-episode tool and judging implementation. Everything runs in
    `scratch`; agent source is only ever written there."""

    def __init__(self, a, task, arm, scratch):
        self.a, self.task, self.arm, self.scratch = a, task, arm, scratch
        self.rust_cases = translate_cases(task)[0] if arm == "rust" else None

    # -- tools
    def tool(self, name, args):
        """-> (ok, content, category). `name` is already on the allowlist."""
        if not isinstance(args, dict):
            return False, "args must be an object", "tool_results"
        if self.arm == "rust":
            return self._rust_check(args)
        if name == "ontology_index":
            return self._mp(["--ontology-index"])
        if name == "ontology_query":
            term, kind, budget = args.get("term"), args.get("kind"), args.get("budget")
            if not isinstance(term, str) or not term or term.startswith("-") or len(term) > 100:
                return False, "term must be a non-empty string (max 100 chars, not starting with '-')", "tool_results"
            argv = ["--ontology-query", term]
            if kind is not None:
                if not isinstance(kind, str) or not re.fullmatch(r"[a-z_]{1,24}", kind):
                    return False, "kind must be a class name from ontology_index", "tool_results"
                argv.append("--kind=" + kind)
            if budget is not None:
                if isinstance(budget, bool) or not isinstance(budget, int) or not 20 <= budget <= 4000:
                    return False, "budget must be an integer 20..4000", "tool_results"
                argv.append("--budget=%d" % budget)
            return self._mp(argv)
        if name == "describe":
            flag = args.get("flag")
            if not isinstance(flag, str) or not re.fullmatch(r"[A-Za-z0-9_=.-]{1,40}", flag) or flag == "-":
                return False, "flag must be a mode name such as `check`", "tool_results"
            return self._mp(["--describe", flag])
        return self._mage_check(args)

    def _mp(self, argv):
        rc, out, err = ta.run([self.a.mage_parse] + argv, self.scratch, 60)
        return rc == 0, cap(out if rc == 0 else (err or out)), "tool_results"

    def _write(self, args, fname):
        src = args.get("source")
        if not isinstance(src, str):
            return None
        path = os.path.join(self.scratch, fname)
        with open(path, "w", encoding="utf-8", newline="\n") as f:
            f.write(src.rstrip("\n") + "\n")
        return path

    def _mage_check_json(self, path):
        rc, out, err = ta.run([self.a.mage_parse, "--check", path, "--json", "--compact"], self.scratch, 60)
        try:
            j = json.loads(out)
        except ValueError:
            return None, "check produced no output: " + (err.strip()[:200] or "rc=%s" % rc)
        j.pop("file", None)   # the scratch path is neither useful to the agent nor deterministic
        return j, ""

    def _mage_check(self, args):
        path = self._write(args, "submission.mg")
        if path is None:
            return False, "source must be a string", "diagnostics"
        j, why = self._mage_check_json(path)
        if j is None:
            return False, why, "diagnostics"
        return True, cap(json.dumps(j, sort_keys=True, separators=(",", ":"))), "diagnostics"

    def _rustc(self, argv):
        return ta.run([self.a.rustc, "--edition", "2021", "--color", "never", "-A", "warnings"] + argv,
                      self.scratch, RUSTC_TIMEOUT)

    def _rust_check(self, args):
        path = self._write(args, "src.rs")
        if path is None:
            return False, "source must be a string", "diagnostics"
        rc, out, err = self._rustc(["--crate-type", "lib", "--emit=metadata", "-o", "check.rmeta", "src.rs"])
        if rc == 0:
            return True, "ok: compiles", "diagnostics"
        return True, cap(err.strip() or out.strip() or "rustc failed rc=%s" % rc), "diagnostics"

    # -- judging
    def judge(self, source):
        """-> (accepted, diagnostics text for the agent). The text never carries
        an expected value: when the submission compiles, the agent is only told
        it is not accepted."""
        if self.arm == "rust":
            return self._judge_rust(source)
        return self._judge_mage(source)

    def _judge_mage(self, source):
        path = self._write({"source": source}, "submission.mg")
        j, why = self._mage_check_json(path)
        if j is None:
            return False, why
        if not j.get("ok") or j.get("error_count"):
            errs = [d for d in j.get("diagnostics", []) if d.get("severity") == "error"]
            return False, cap(json.dumps({"diagnostics": errs}, sort_keys=True, separators=(",", ":")))
        host = sorted({e for f in j.get("effects", []) for k in ("declared", "inferred") for e in f.get(k, [])
                       if e in ta.HOST_EFFECTS})
        if host:
            return False, "not accepted: host effects (%s) are not permitted in this benchmark" % ",".join(host)
        r = ta.judge(self.a.mage_parse, self.task, self.scratch, self.a.fuel, source=source)
        if r["status"] == "ok":
            return True, ""
        return False, "not accepted: the program type-checks but does not satisfy the task"

    def _judge_rust(self, source):
        with open(os.path.join(self.scratch, "src.rs"), "w", encoding="utf-8", newline="\n") as f:
            f.write(source.rstrip("\n") + "\n")
        rc, out, err = self._rustc(["--crate-type", "lib", "--emit=metadata", "-o", "check.rmeta", "src.rs"])
        if rc != 0:
            return False, cap(err.strip() or out.strip() or "rustc failed rc=%s" % rc)
        with open(os.path.join(self.scratch, "t.rs"), "w", encoding="utf-8", newline="\n") as f:
            f.write(harness(source, self.rust_cases))
        exe = os.path.join(self.scratch, "t.exe" if os.name == "nt" else "t")
        rc, out, err = self._rustc(["--test", "-o", exe, "t.rs"])
        if rc != 0:
            # The error text would quote the harness, and so the expected values.
            return False, "not accepted: the program compiles but does not fit the required items"
        rc, out, err = ta.run([exe, "--test-threads=1"], self.scratch, RUST_RUN_TIMEOUT)
        if rc == 0:
            return True, ""
        return False, "not accepted: the program compiles but does not satisfy the task"


# ── the agent process ───────────────────────────────────────────────────────
def agent_argv(cmd):
    """shlex-split the command; a relative argument that names an existing file
    is made absolute because the agent runs in a different directory."""
    argv = shlex.split(cmd, posix=True)
    out = []
    for x in argv:
        if not x.startswith("-") and not os.path.isabs(x) and os.path.exists(x):
            x = os.path.abspath(x)
        out.append(x)
    return out


def agent_env(extra):
    env = {k: v for k, v in os.environ.items() if k.upper() in KEPT_ENV}
    for name in extra:
        if name in os.environ:
            env[name] = os.environ[name]
    return env


def kill_tree(p):
    try:
        if os.name == "nt":
            subprocess.run(["taskkill", "/F", "/T", "/PID", str(p.pid)], capture_output=True)
        else:
            import signal
            os.killpg(p.pid, signal.SIGKILL)
    except Exception:
        pass
    try:
        p.kill()
    except Exception:
        pass


class Agent:
    def __init__(self, argv, env, cwd, errpath):
        kw = {} if os.name == "nt" else {"start_new_session": True}
        self.err = open(errpath, "wb")
        self.p = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=self.err, **kw)
        self.q = queue.Queue()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.p.stdout:
            self.q.put(line)
        self.q.put(None)

    def send(self, obj):
        try:
            self.p.stdin.write((json.dumps(obj, sort_keys=True, ensure_ascii=False) + "\n").encode("utf-8"))
            self.p.stdin.flush()
            return True
        except (BrokenPipeError, OSError, ValueError):
            return False

    def recv(self, timeout):
        """-> line bytes, None on EOF, or raises queue.Empty on timeout."""
        return self.q.get(timeout=max(timeout, 0.001))

    def close(self):
        try:
            self.p.stdin.close()
        except Exception:
            pass
        try:
            self.p.wait(timeout=2)
        except subprocess.TimeoutExpired:
            kill_tree(self.p)
            self.p.wait()
        try:
            self.p.stdout.close()
        except Exception:
            pass
        self.err.close()


# ── one episode ─────────────────────────────────────────────────────────────
def render_prompt(task, arm, sigs, calls, tools, limits):
    lang = "MAGE" if arm == "mage" else "Rust (edition 2021)"
    L = ["Write a complete program fragment in %s that solves the task below. Reply with {\"type\":\"final\",\"source\":...}." % lang,
         "", "Task: " + task["task"], ""]
    if sigs:
        L += ["Required items (signatures as the tests use them):"] + ["  " + s for s in sigs] + [""]
    L += ["The tests will evaluate these expressions against your source (inputs only; expected values are not shown):"]
    L += ["  " + c for c in calls] + [""]
    if arm == "mage":
        L += ["MAGE is not a language you can assume you know. Its compiler describes itself through the tools below."]
    else:
        L += ["No documentation is provided."]
    L += ["Tools (reply {\"type\":\"tool\",\"name\":N,\"args\":{...}}; any other tool ends the episode):"]
    L += ["  %s %s - %s" % (t["name"], t["args"], t["about"]) for t in tools]
    L += ["", "Limits: %d submissions, %d agent turns, %d tokens, %d seconds. A submission that is not accepted comes back with diagnostics."
          % (limits["max_attempts"], limits["max_turns"], limits["max_tokens"], limits["wall_s"])]
    return "\n".join(L)


TOOL_DOCS = {
    "ontology_index": ("{}", "the classes of the compiler's ontology and how many entries each holds"),
    "ontology_query": ('{"term":S,"kind"?:S,"budget"?:N}', "look up a term in the ontology, optionally restricted to a class, within a token budget"),
    "describe": ('{"flag":S}', "expand one compiler mode's documentation, e.g. check"),
    "check": ('{"source":S}', "type-check a candidate; returns the compiler's diagnostics (no tests are run)"),
}
TOOL_DOCS_RUST_CHECK = ('{"source":S}', "compile a candidate with rustc; returns the compile errors (no tests are run)")


def run_episode(a, task, arm, bpe, workdir):
    tid = task["id"]
    src_ref = task["solution"]["rdx_source"] if arm == "mage" else task["rust_equivalent"]["rs_source"]
    sigs = signatures(src_ref, arm)
    if arm == "mage":
        calls = [c["input"] for c in task["tests"]]
    else:
        calls = [rin for rin, _ in translate_cases(task)[0]]
    allow = TOOLS[arm]
    tools = [{"name": n, "args": (TOOL_DOCS_RUST_CHECK if arm == "rust" else TOOL_DOCS[n])[0],
              "about": (TOOL_DOCS_RUST_CHECK if arm == "rust" else TOOL_DOCS[n])[1]} for n in allow]
    limits = {"max_attempts": a.max_attempts, "max_turns": a.max_turns, "max_tokens": a.max_tokens, "wall_s": a.wall}
    ep = os.path.join(workdir, "%s.%s" % (tid, arm))
    os.makedirs(ep, exist_ok=True)
    agent_cwd = os.path.join(ep, "agent")
    os.makedirs(agent_cwd, exist_ok=True)
    judge = Judge(a, task, arm, os.path.join(ep, "work"))
    os.makedirs(judge.scratch, exist_ok=True)

    tokens = collections.Counter()
    reported = {"input": 0, "output": 0, "n": 0}
    transcript, seq = [], [0]
    tool_counts = collections.Counter()
    state = {"attempts": 0, "turns": 0, "outcome": None, "solved": False}
    t0 = time.monotonic()

    def log(direction, category, text, msg):
        n = bpe.count(text)
        tokens[category] += n
        seq[0] += 1
        transcript.append({"seq": seq[0], "dir": direction, "category": category, "tokens": n, "msg": msg})
        return n

    def total():
        return sum(tokens.values())

    task_msg = {"type": "task", "id": tid, "arm": arm, "prompt": render_prompt(task, arm, sigs, calls, tools, limits),
                "description": task["task"], "signatures": sigs, "call_sites": calls, "tools": tools,
                "limits": limits, "attempt": 1}
    agent = Agent(agent_argv(a.agent), agent_env(a.agent_env), agent_cwd, os.path.join(ep, "agent.stderr"))
    log("to_agent", "prompt", task_msg["prompt"], task_msg)
    if not agent.send(task_msg):
        state["outcome"] = "agent_exit"
    deadline = t0 + a.wall

    while state["outcome"] is None:
        if total() >= a.max_tokens:
            state["outcome"] = "token_cap"
            break
        if state["turns"] >= a.max_turns:
            state["outcome"] = "turn_cap"
            break
        try:
            line = agent.recv(deadline - time.monotonic())
        except queue.Empty:
            state["outcome"] = "timeout"
            break
        if line is None:
            state["outcome"] = "agent_exit"
            break
        state["turns"] += 1
        text = line.decode("utf-8", "replace").strip()
        try:
            msg = json.loads(text)
            assert isinstance(msg, dict) and msg.get("type") in ("tool", "final")
        except (ValueError, AssertionError):
            log("from_agent", "tool_calls", text[:2000], {"raw": text[:2000]})
            state["outcome"] = "protocol_error"
            break
        usage = msg.get("usage")
        if isinstance(usage, dict) and all(isinstance(usage.get(k), int) for k in ("input_tokens", "output_tokens")):
            reported["input"] += usage["input_tokens"]
            reported["output"] += usage["output_tokens"]
            reported["n"] += 1
        if msg["type"] == "tool":
            name, args = msg.get("name"), msg.get("args", {})
            log("from_agent", "tool_calls", "%s(%s)" % (name, json.dumps(args, sort_keys=True, ensure_ascii=False)), msg)
            if name not in allow:
                state["outcome"] = "violation"
                state["violation"] = str(name)[:60]
                break
            tool_counts[name] += 1
            ok, content, cat = judge.tool(name, args)
            res = {"type": "tool_result", "name": name, "ok": ok, "content": content}
            log("to_agent", cat, content, res)
            if not agent.send(res):
                state["outcome"] = "agent_exit"
            continue
        # final
        src = msg.get("source")
        log("from_agent", "final_source", src if isinstance(src, str) else "", msg)
        if not isinstance(src, str):
            state["outcome"] = "protocol_error"
            break
        state["attempts"] += 1
        accepted, diag = judge.judge(src)
        if accepted:
            note = {"type": "end", "reason": "accepted"}
            log("to_agent", "verdict", "accepted", note)
            agent.send(note)
            state["outcome"], state["solved"] = "solved", True
            break
        left = a.max_attempts - state["attempts"]
        res = {"type": "result", "accepted": False, "attempts_left": left, "diagnostics": diag}
        if left <= 0:
            log("to_agent", "diagnostics", diag, res)
            note = {"type": "end", "reason": "not accepted; no attempts left"}
            log("to_agent", "verdict", "not accepted; no attempts left", note)
            agent.send(res)
            agent.send(note)
            state["outcome"] = "failed"
            break
        log("to_agent", "diagnostics", diag, res)
        if not agent.send(res):
            state["outcome"] = "agent_exit"

    elapsed = time.monotonic() - t0
    if state["outcome"] in ("timeout", "violation", "protocol_error", "token_cap", "turn_cap"):
        kill_tree(agent.p)
    agent.close()
    stderr_tail = ""
    if state["outcome"] in ("agent_exit", "protocol_error"):
        try:
            with open(os.path.join(ep, "agent.stderr"), "rb") as f:
                stderr_tail = f.read()[-300:].decode("utf-8", "replace").strip()
        except OSError:
            pass

    rec = {"id": "%s/%s" % (tid, arm), "task": tid, "arm": arm, "category": task["category"],
           "outcome": state["outcome"], "solved": state["solved"], "attempts": state["attempts"],
           "turns": state["turns"], "tokens": {c: tokens[c] for c in CATEGORIES}, "tokens_total": total(),
           "tool_counts": dict(sorted(tool_counts.items()))}
    if reported["n"]:
        rec["reported"] = {"input_tokens": reported["input"], "output_tokens": reported["output"],
                           "total": reported["input"] + reported["output"], "replies_with_usage": reported["n"]}
    if state.get("violation"):
        rec["violation"] = state["violation"]
    if stderr_tail:
        rec["agent_stderr_tail"] = stderr_tail
    if a.timestamps:
        rec["elapsed_s"] = round(elapsed, 3)
    tdir = os.path.join(a.out, "transcripts")
    os.makedirs(tdir, exist_ok=True)
    with open(os.path.join(tdir, "%s.%s.jsonl" % (tid, arm)), "w", encoding="utf-8", newline="\n") as f:
        for r in transcript:
            f.write(json.dumps(r, sort_keys=True, ensure_ascii=False) + "\n")
        f.write(json.dumps({"episode_end": rec}, sort_keys=True, ensure_ascii=False) + "\n")
    shutil.rmtree(ep, ignore_errors=True)
    return rec


# ── summaries ───────────────────────────────────────────────────────────────
def arm_stats(recs):
    n = len(recs)
    solved = [r for r in recs if r["solved"]]
    tot = sum(r["tokens_total"] for r in recs)
    s = {"attempted": n, "solved": len(solved), "solve_rate": round(len(solved) / n, 4) if n else None,
         "tokens_total": tot,
         "tokens_per_solved": round(tot / len(solved), 1) if solved else None,
         "median_tokens_per_episode": statistics.median(r["tokens_total"] for r in recs) if n else None,
         "by_category": {c: sum(r["tokens"][c] for r in recs) for c in CATEGORIES},
         "submissions_histogram": dict(sorted(collections.Counter(str(r["attempts"]) for r in recs).items())),
         "outcomes": dict(sorted(collections.Counter(r["outcome"] for r in recs).items())),
         "tool_calls": dict(sorted(sum((collections.Counter(r["tool_counts"]) for r in recs), collections.Counter()).items()))}
    rep = [r for r in recs if "reported" in r]
    if rep and len(rep) == n:
        rt = sum(r["reported"]["total"] for r in recs)
        s["reported_tokens_total"] = rt
        s["reported_tokens_per_solved"] = round(rt / len(solved), 1) if solved else None
        s["reported_median_per_episode"] = statistics.median(r["reported"]["total"] for r in recs)
    return s


def summarise(recs, paired_ids, meta):
    arms = sorted({r["arm"] for r in recs})
    doc = {"meta": meta, "arms": {}, "paired": None}
    for arm in arms:
        doc["arms"][arm] = arm_stats([r for r in recs if r["arm"] == arm])
    if len(arms) > 1:
        both = sorted({r["task"] for r in recs if r["arm"] == "mage"} & {r["task"] for r in recs if r["arm"] == "rust"} & set(paired_ids))
        doc["paired"] = {"tasks": both, "n": len(both),
                         "arms": {arm: arm_stats([r for r in recs if r["arm"] == arm and r["task"] in both]) for arm in arms}}
    return doc


def fmt(v, nd=1):
    if v is None:
        return "-"
    return ("%.*f" % (nd, v)) if isinstance(v, float) and not float(v).is_integer() else str(int(v)) if isinstance(v, (int, float)) else str(v)


def table(arms):
    L = ["| arm | attempted | solved | solve rate | tokens / solved | median tokens / episode | prompt | tool calls | tool results | diagnostics | final source | verdict | submissions (count:episodes) |",
         "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|"]
    for arm, s in arms.items():
        c = s["by_category"]
        hist = " ".join("%s:%d" % kv for kv in s["submissions_histogram"].items())
        L.append("| %s | %d | %d | %s | %s | %s | %d | %d | %d | %d | %d | %d | %s |" % (
            arm, s["attempted"], s["solved"], fmt(s["solve_rate"] * 100 if s["solve_rate"] is not None else None) + "%",
            fmt(s["tokens_per_solved"]), fmt(s["median_tokens_per_episode"]), c["prompt"], c["tool_calls"],
            c["tool_results"], c["diagnostics"], c["final_source"], c["verdict"], hist))
    rep = [(arm, s) for arm, s in arms.items() if "reported_tokens_per_solved" in s]
    if rep:
        L += ["", "Provider-reported usage (primary when present):", "",
              "| arm | reported tokens / solved | reported median / episode |", "|---|---:|---:|"]
        for arm, s in rep:
            L.append("| %s | %s | %s |" % (arm, fmt(s["reported_tokens_per_solved"]), fmt(s["reported_median_per_episode"])))
    return L


def markdown(doc):
    m = doc["meta"]
    L = ["# Agent cost: tokens to a correct program", "",
         "Agent command: `%s`. Tasks %d (MAGE arm), %d paired with Rust. Limits: %s." % (
             m["agent"], m["tasks_mage"], m["tasks_paired"], json.dumps(m["limits"], sort_keys=True)),
         "Token counts are BPE tokens of MAGE's own 4096-merge vocabulary, not any model's tokenizer.", "",
         "## All episodes run", ""] + table(doc["arms"])
    if doc["paired"]:
        L += ["", "## Paired tasks only (%d tasks, both arms)" % doc["paired"]["n"], ""] + table(doc["paired"]["arms"])
    return "\n".join(L) + "\n"


# ── main ────────────────────────────────────────────────────────────────────
def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--mage-parse", default=os.environ.get("MAGE_PARSE_BIN"), help="mage-parse binary (default $MAGE_PARSE_BIN)")
    ap.add_argument("--agent", help="the agent command (shlex-split, no shell); started once per episode")
    ap.add_argument("--agent-env", action="append", default=[], metavar="NAME",
                    help="pass this one environment variable through to the agent (repeatable); everything else is dropped")
    ap.add_argument("--arms", default="mage,rust")
    ap.add_argument("--only", help="comma-separated task ids")
    ap.add_argument("--tasks", default=os.path.join(ROOT, "benchmarks", "tasks"))
    ap.add_argument("--baseline", default=os.path.join(ROOT, "benchmarks", "task-acceptance-baseline.txt"))
    ap.add_argument("--rustc", default="rustc")
    ap.add_argument("--out", default="agent-cost-out", help="output directory (transcripts/, summary.json, summary.md)")
    ap.add_argument("--scratch", help="scratch parent directory (default: system temp)")
    ap.add_argument("--max-attempts", type=int, default=3)
    ap.add_argument("--max-turns", type=int, default=20)
    ap.add_argument("--max-tokens", type=int, default=60000, help="per-episode cap on counted BPE tokens")
    ap.add_argument("--wall", type=float, default=120.0, help="per-episode wall-clock cap, seconds")
    ap.add_argument("--fuel", type=int, default=5000000)
    ap.add_argument("--timestamps", action="store_true", help="add wall times to outputs (off by default: outputs are then byte-stable)")
    ap.add_argument("--list", action="store_true", help="print the task set and the Rust pairing, then exit")
    a = ap.parse_args(argv)
    if not a.mage_parse:
        ap.error("--mage-parse or $MAGE_PARSE_BIN is required")
    tasks = accepted_tasks(a.tasks, a.baseline)
    if a.only:
        want = set(a.only.split(","))
        tasks = [t for t in tasks if t["id"] in want]
    paired = {t["id"]: translate_cases(t) for t in tasks}
    if a.list:
        for t in tasks:
            cases, why = paired[t["id"]]
            print("%-9s %-15s rust=%s" % (t["id"], t["category"], "paired" if cases else "excluded: " + "; ".join(why)))
        print("%d tasks, %d paired with rust" % (len(tasks), sum(1 for c, _ in paired.values() if c)))
        return 0
    if not a.agent:
        ap.error("--agent is required")
    arms = [x for x in a.arms.split(",") if x]
    for x in arms:
        if x not in TOOLS:
            ap.error("unknown arm " + x)
    if "rust" in arms and not shutil.which(a.rustc):
        ap.error("rustc not found (--rustc)")
    a.out = os.path.abspath(a.out)
    bpe = Bpe(a.mage_parse)
    parent = a.scratch or tempfile.gettempdir()
    os.makedirs(parent, exist_ok=True)
    recs = []
    workdir = tempfile.mkdtemp(prefix="agent-cost-", dir=parent)
    try:
        for t in sorted(tasks, key=lambda t: t["id"]):
            for arm in arms:
                if arm == "rust" and not paired[t["id"]][0]:
                    continue
                r = run_episode(a, t, arm, bpe, workdir)
                recs.append(r)
                print("%-9s %-5s %-14s attempts=%d tokens=%d" % (t["id"], arm, r["outcome"], r["attempts"], r["tokens_total"]),
                      flush=True)
    finally:
        shutil.rmtree(workdir, ignore_errors=True)
    meta = {"agent": a.agent, "limits": {"max_attempts": a.max_attempts, "max_turns": a.max_turns,
                                         "max_tokens": a.max_tokens, "wall_s": a.wall},
            "tasks_mage": len(tasks), "tasks_paired": sum(1 for c, _ in paired.values() if c),
            "tokenizer": "mage-parse --bpe-count (MAGE 4096-merge vocabulary; not a model tokenizer)"}
    doc = summarise(recs, [i for i, (c, _) in paired.items() if c], meta)
    doc["episodes"] = sorted(recs, key=lambda r: r["id"])
    os.makedirs(a.out, exist_ok=True)
    with open(os.path.join(a.out, "summary.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump(doc, f, indent=1, sort_keys=True)
        f.write("\n")
    md = markdown(doc)
    with open(os.path.join(a.out, "summary.md"), "w", encoding="utf-8", newline="\n") as f:
        f.write(md)
    print()
    print(md)
    return 0


if __name__ == "__main__":
    sys.exit(main())

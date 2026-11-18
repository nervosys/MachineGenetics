#!/usr/bin/env python3
"""Mechanical acceptance check for the benchmark tasks (benchmarks/tasks/*.json).

For every task: does the MAGE reference solution (`solution.rdx_source`) parse,
does it type-check (`mage-parse --check --json`), and does it pass the task's
own `tests` array when each case is *evaluated* (`mage-parse --eval`)?

The corpus's "99 of 99" figure counts **parsing only**. This script measures the
two stronger things: check-clean, and tests-pass. See benchmarks/ACCEPTANCE.md.

How one test case is run
------------------------
The solution is written to a temp file and a wrapper function is appended:

    +f __ta_case() { <input> }

`input` is spliced verbatim as the wrapper's body, so statements such as
`v s = Stack.new(); s.push(1); s.pop()` work, and the last expression is the
value. The file is run as

    mage-parse --eval FILE __ta_case --fuel N

with the working directory set to an empty scratch directory. `--fuel` bounds
the run (termination) and makes it deterministic; exhausting it is an error,
never a pass. The value printed on the last line of stdout is the "got" string.

How a case is judged
--------------------
`canon(got) == canon(expected)`, where canon removes every whitespace character
that is OUTSIDE a double-quoted string literal and changes nothing else. So
`[1, 2]` equals `[1,2]` and `Pair { first: 2 }` equals `Pair{first:2}`, but
`32.0` does not equal `32`, `"a"` does not equal `a`, `?Some(2)` does not equal
`Some(2)`, and string contents are compared byte for byte. There is no other
normalisation, and none is to be added to make a case pass: a case that fails
for a formatting reason is a failure and is reported with both strings.

Case statuses
-------------
pass        ran, canon(got) == canon(expected)
fail        ran, produced a value, and it differs
error       could not be evaluated: eval error, input does not parse as MAGE,
            fuel exhausted, timeout.  Counts as a failure (never a pass).
unrunnable  not attempted, with a reason, because a mechanical verdict would be
            meaningless or unsafe:
              - the solution declares a host-reaching effect (fs, net, env,
                llm, tools, timer): --eval would really perform it;
              - the expected string is not a value literal (placeholder such
                as `<contents>`, `...`, or prose);
              - the input carries a trailing comment naming outside setup
                (`// with PORT=3000`).

A struct/enum-valued case is refused by the fuel-bounded evaluator ("takes and
returns plain data"); only then is it re-run without --fuel, with a 30 s
wall-clock limit. A timeout (retried once) is an error, never a pass.

Task status: check (any check error) > test (any fail/error case) >
unrunnable (any unrunnable case, none failing) > ok. A task is *accepted* when
it is check-clean and every case passes.
"""
import argparse
import collections
import json
import os
import re
import subprocess
import sys
import tempfile

HOST_EFFECTS = ("fs", "net", "env", "llm", "tools", "timer")
WRAPPER = "__ta_case"
EVAL_TIMEOUT = 60
UNMETERED_TIMEOUT = 30
CHECK_TIMEOUT = 60


# ── comparison ──────────────────────────────────────────────────────────────
def canon(s):
    """Strip whitespace outside double-quoted strings; nothing else."""
    out, in_str, esc = [], False, False
    for ch in s.strip():
        if in_str:
            out.append(ch)
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
        elif ch == '"':
            in_str = True
            out.append(ch)
        elif not ch.isspace():
            out.append(ch)
    return "".join(out)


def comment_start(s):
    """Index of a `//` outside string literals, or -1."""
    in_str, esc = False, False
    for i, ch in enumerate(s):
        if in_str:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
        elif ch == '"':
            in_str = True
        elif s.startswith("//", i):
            return i
    return -1


def lenient(s):
    """INFORMATIONAL ONLY. Maps the notations the evaluator and the corpus
    disagree about onto one form, to say how many failures are notation-only:
    `?Some(`->`Some(`, `?None`->`None`, `R.Ok(`->`Ok(`, `R.Err(`->`Err(`,
    `3.0`->`3`. Never used to decide pass/fail."""
    c = canon(s)
    parts = re.split(r'("(?:[^"\\]|\\.)*")', c)
    for i in range(0, len(parts), 2):
        p = parts[i]
        p = p.replace("?Some(", "Some(").replace("?None", "None").replace("R.Ok(", "Ok(").replace("R.Err(", "Err(")
        parts[i] = re.sub(r"(\d)\.0(?![\d])", r"\1", p)
    return "".join(parts)


# ── is `expected` a value literal? ─────────────────────────────────────────
_TOK = re.compile(
    r'\s*(?:(?P<str>b?"(?:[^"\\]|\\.)*")|(?P<num>-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?)'
    r'|(?P<id>\??[A-Za-z_][A-Za-z_0-9]*)|(?P<p>\.\.\.|[()\[\]{},:.]))')


def _tokens(s):
    pos, toks = 0, []
    s = s.rstrip()
    while pos < len(s):
        m = _TOK.match(s, pos)
        if not m:
            return None
        toks.append((m.lastgroup, m.group(m.lastgroup)))
        pos = m.end()
    return toks


def expected_is_value(s):
    """True when `s` parses as a value literal (so it can be compared)."""
    toks = _tokens(s)
    if not toks:
        return False
    i = [0]

    def peek():
        return toks[i[0]] if i[0] < len(toks) else (None, None)

    def eat(v=None):
        t = peek()
        if t[0] is None or (v is not None and t[1] != v):
            raise ValueError
        i[0] += 1
        return t

    def seq(close, item):
        if peek()[1] == close:
            eat(close)
            return
        while True:
            item()
            if peek()[1] == ",":
                eat(",")
                if peek()[1] == close:
                    break
                continue
            break
        eat(close)

    def value():
        k, v = peek()
        if k in ("str", "num"):
            eat()
            return
        if v == "(":
            eat("(")
            eat(")")
            return
        if v == "[":
            eat("[")
            seq("]", value)
            return
        if v == "{":
            eat("{")
            seq("}", pair)
            return
        if k == "id":
            eat()
            while peek()[1] == ".":
                eat(".")
                if peek()[0] != "id":
                    raise ValueError
                eat()
            if peek()[1] == "(":
                eat("(")
                seq(")", value)
            elif peek()[1] == "{":
                eat("{")
                seq("}", pair)
            return
        raise ValueError

    def pair():
        k, v = peek()
        if k not in ("str", "id"):
            raise ValueError
        eat()
        eat(":")
        value()

    try:
        value()
        return i[0] == len(toks)
    except ValueError:
        return False


# ── running the compiler ───────────────────────────────────────────────────
def run(cmd, cwd, timeout):
    # One retry on timeout: a loaded machine can stall a process that takes a
    # second; a program that genuinely does not terminate times out twice.
    # stdin is closed so nothing can wait on it.
    for attempt in (0, 1):
        try:
            p = subprocess.run(cmd, cwd=cwd, capture_output=True, timeout=timeout,
                               stdin=subprocess.DEVNULL)
            break
        except subprocess.TimeoutExpired:
            if attempt:
                return None, "", "timeout after %ds (twice)" % timeout
    dec = lambda b: b.decode("utf-8", "replace").replace("\r\n", "\n")
    return p.returncode, dec(p.stdout), dec(p.stderr)


def first_name(msg):
    m = re.search(r"`([^`]+)`", msg)
    return m.group(1) if m else ""


def check_task(binary, path, scratch):
    rc, out, err = run([binary, "--check", path, "--json", "--compact"], scratch, CHECK_TIMEOUT)
    try:
        j = json.loads(out)
    except ValueError:
        return {"parses": False, "check_ok": False, "codes": {}, "first": "check produced no JSON: " + (err.strip()[:80] or "rc=%s" % rc)}
    diags = [d for d in j.get("diagnostics", []) if d.get("severity") == "error"]
    parses = not any(d.get("category") == "SyntaxError" for d in diags)
    first = ""
    if diags:
        d = diags[0]
        first = "%s %s" % (d["code"], first_name(d["message"]) or ascii_(d["message"])[:40])
    return {"parses": parses, "check_ok": bool(j.get("ok")) and not diags,
            "codes": dict(sorted(j.get("counts", {}).get("by_code", {}).items())), "first": first}


def ascii_(s):
    return s.encode("ascii", "replace").decode("ascii")


def one_line(s, n=70):
    s = " ".join(ascii_(s).split())
    return s if len(s) <= n else s[: n - 3] + "..."


def run_case(binary, src, case, effects, scratch, fuel, idx):
    inp, exp = case["input"], case["expected"]
    res = {"input": inp, "expected": exp, "status": "", "got": None, "reason": ""}
    host = [e for e in effects if e in HOST_EFFECTS]
    if host:
        res.update(status="unrunnable", reason="solution declares host effect(s) %s; eval would perform them" % ",".join(host))
        return res
    if not expected_is_value(exp):
        res.update(status="unrunnable", reason="expected is not a value literal: %s" % one_line(exp, 40))
        return res
    cm = comment_start(inp)
    if cm >= 0:
        res.update(status="unrunnable", reason="input carries a setup comment: %s" % one_line(inp[cm:], 40))
        return res
    path = os.path.join(scratch, "case%d.mg" % idx)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(src.rstrip("\n") + "\n\n+f %s() { %s }\n" % (WRAPPER, inp))
    rc, out, err = run([binary, "--eval", path, WRAPPER, "--fuel", str(fuel)], scratch, EVAL_TIMEOUT)
    if rc != 0 and "takes and returns plain data" in err:
        # The fuel-bounded evaluator only returns ints/floats/bools/strings/
        # lists/tuples; a struct or enum result is refused. Re-run unmetered
        # (still wall-clock bounded) so such cases can be judged at all.
        res["unmetered"] = True
        rc, out, err = run([binary, "--eval", path, WRAPPER], scratch, UNMETERED_TIMEOUT)
    if rc != 0:
        msg = [l for l in err.splitlines() if "eval error" in l] or [err.strip() or "rc=%s" % rc]
        why = msg[0].replace("eval error: ", "")
        m = re.match(r"parse error at (\d+):(\d+): (.*)", why)
        if m:
            # Line numbers are in the spliced file; the wrapper is its last line.
            where = "input" if int(m.group(1)) >= len(src.rstrip("\n").split("\n")) + 2 else "solution"
            why = "%s does not parse: %s" % (where, m.group(3))
        res.update(status="error", reason=one_line(why, 90))
        return res
    lines = [l for l in out.splitlines() if l.strip()]
    got = lines[-1] if lines else ""
    res["got"] = got
    if canon(got) == canon(exp):
        res["status"] = "pass"
    else:
        res.update(status="fail", reason="expected %s got %s" % (json.dumps(exp), json.dumps(got)))
        # Informational only; never turns a fail into a pass.
        res["notation_only"] = lenient(got) == lenient(exp)
    return res


def judge(binary, task, scratch, fuel, source=None):
    """Judge `source` (default: the task's own reference) against the task's
    tests. `source` is the one addition made for scripts/agent-cost.py, which
    judges an agent's submission with exactly the functions used for the
    reference; nothing about how a case is run or compared depends on it."""
    src = task["solution"]["rdx_source"] if source is None else source
    path = os.path.join(scratch, "solution.mg")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(src.rstrip("\n") + "\n")
    chk = check_task(binary, path, scratch)
    effects = task["solution"].get("effects", [])
    cases = [run_case(binary, src, c, effects, scratch, fuel, i) for i, c in enumerate(task.get("tests", []))]
    n = len(cases)
    counts = collections.Counter(c["status"] for c in cases)
    bad = [(i, c) for i, c in enumerate(cases) if c["status"] in ("fail", "error")]
    if not chk["check_ok"]:
        status, reason = "check", chk["first"]
    elif bad:
        i, c = bad[0]
        status = "test"
        reason = "%d/%d failing; first case %d: %s" % (len(bad), n, i + 1, c["reason"] if c["status"] == "fail" else "error " + c["reason"])
    elif counts["unrunnable"]:
        status = "unrunnable"
        first = next(c for c in cases if c["status"] == "unrunnable")
        reason = "%d/%d case(s): %s" % (counts["unrunnable"], n, first["reason"])
    elif n == 0:
        status, reason = "unrunnable", "task has no tests"
    else:
        status, reason = "ok", ""
    testdim = ("fail" if bad else "unrunnable" if counts["unrunnable"] or n == 0 else "pass")
    return {"id": task["id"], "category": task["category"], "parses": chk["parses"],
            "check_ok": chk["check_ok"], "check_codes": chk["codes"], "test_status": testdim,
            "cases": {k: counts[k] for k in ("pass", "fail", "error", "unrunnable")},
            "status": status, "reason": reason, "case_detail": cases}


def load_tasks(tasks_dir):
    """Every task of every `*.json` in `tasks_dir`, in file-name order."""
    tasks = []
    for fn in sorted(os.listdir(tasks_dir)):
        if fn.endswith(".json"):
            with open(os.path.join(tasks_dir, fn), encoding="utf-8") as f:
                tasks.extend(json.load(f))
    return tasks


def baseline_ids(path):
    """Task ids listed in the shrink-only baseline (the tasks NOT accepted)."""
    ids = set()
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.strip() and not line.startswith("#"):
                ids.add(line.split()[0])
    return ids


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--mage-parse", required=True, help="path to the mage-parse binary")
    ap.add_argument("--tasks", default="benchmarks/tasks", help="directory of task JSON files")
    ap.add_argument("--json", metavar="PATH", help="write machine-readable results here")
    ap.add_argument("--baseline-out", metavar="PATH", help="write the baseline lines (tasks not accepted) here")
    ap.add_argument("--fuel", type=int, default=5000000, help="--fuel per case (default 5000000)")
    ap.add_argument("--only", help="comma-separated task ids")
    ap.add_argument("-q", "--quiet", action="store_true", help="no per-task lines")
    a = ap.parse_args()

    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    tasks_dir = a.tasks if os.path.isabs(a.tasks) else os.path.join(root, a.tasks)
    tasks = load_tasks(tasks_dir)
    if a.only:
        want = set(a.only.split(","))
        tasks = [t for t in tasks if t["id"] in want]

    results = []
    with tempfile.TemporaryDirectory(prefix="task-acceptance-") as scratch:
        for t in tasks:
            r = judge(a.mage_parse, t, scratch, a.fuel)
            results.append(r)
            if not a.quiet:
                print("%-9s %-10s check=%-5s tests=%s %s" % (
                    r["id"], r["status"], "ok" if r["check_ok"] else "ERR",
                    "/".join("%s:%d" % (k, v) for k, v in r["cases"].items() if v) or "none", r["reason"]))

    cats = collections.OrderedDict()
    for r in results:
        c = cats.setdefault(r["category"], collections.Counter())
        c["tasks"] += 1
        c["parse"] += r["parses"]
        c["check_clean"] += r["check_ok"]
        c["all_pass"] += r["test_status"] == "pass"
        c["some_fail"] += r["test_status"] == "fail"
        c["unrunnable"] += r["test_status"] == "unrunnable"
        c["accepted"] += r["status"] == "ok"
    cols = ("tasks", "parse", "check_clean", "all_pass", "some_fail", "unrunnable", "accepted")
    tot = collections.Counter()
    print()
    print("%-22s" % "category" + "".join("%12s" % c for c in cols))
    for cat, c in cats.items():
        tot.update(c)
        print("%-22s" % cat + "".join("%12d" % c[k] for k in cols))
    print("%-22s" % "TOTAL" + "".join("%12d" % tot[k] for k in cols))
    ncase = collections.Counter()
    for r in results:
        ncase.update(r["cases"])
    print("cases: " + " ".join("%s=%d" % (k, ncase[k]) for k in ("pass", "fail", "error", "unrunnable")))

    if a.json:
        doc = {"summary": {"by_category": {k: dict(v) for k, v in cats.items()}, "total": dict(tot), "cases": dict(ncase)},
               "tasks": results}
        with open(a.json, "w", encoding="utf-8", newline="\n") as f:
            json.dump(doc, f, indent=1, sort_keys=True)
            f.write("\n")
    if a.baseline_out:
        with open(a.baseline_out, "w", encoding="utf-8", newline="\n") as f:
            for r in sorted(results, key=lambda r: r["id"]):
                if r["status"] != "ok":
                    f.write("%s %s %s\n" % (r["id"], r["status"], r["reason"]))


if __name__ == "__main__":
    main()

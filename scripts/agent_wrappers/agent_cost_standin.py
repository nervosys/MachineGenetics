#!/usr/bin/env python3
"""Scripted stand-in agents for scripts/agent-cost.py. They call no model.

    agent_cost_standin.py MODE [--tasks DIR] [--log FILE]

They exist to test the harness, never to measure anything: `oracle` reads the
reference solution out of the corpus (the driver never sends it), so its token
counts are a floor for the protocol, not the cost of any model.

MODE
  oracle              final answer = the reference (MAGE arm: rdx_source, Rust arm: rs_source)
  lookup-then-oracle  ontology_index, two ontology_query calls, check, then the reference
                      (Rust arm: check, then the reference)
  wrong-then-oracle   one wrong submission, then the reference (exercises the attempt loop)
  always-wrong        wrong, syntactically broken, wrong, ... never the reference
  tool-violation      calls `run_tests`, which is not on the allowlist
  garbage             replies with something that is not a protocol message
  slow                sleeps far past any wall-clock cap
  spy                 oracle, and appends every line it receives to --log FILE
"""
import glob
import json
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))


def load_refs(tasks_dir):
    refs = {}
    for fn in sorted(glob.glob(os.path.join(tasks_dir, "*.json"))):
        with open(fn, encoding="utf-8") as f:
            for t in json.load(f):
                refs[t["id"]] = {"mage": t["solution"]["rdx_source"], "rust": t["rust_equivalent"]["rs_source"]}
    return refs


WRONG = {"mage": "+f zz_wrong() -> i32 { 0 }\n", "rust": "pub fn zz_wrong() -> i32 { 0 }\n"}
BROKEN = {"mage": "+f zz_broken( -> { \n", "rust": "pub fn zz_broken( -> { \n"}


def say(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)
    mode = args[0]
    tasks_dir = os.path.join(os.path.dirname(os.path.dirname(HERE)), "benchmarks", "tasks")
    log = None
    i = 1
    while i < len(args):
        if args[i] == "--tasks":
            tasks_dir = args[i + 1]
            i += 2
        elif args[i] == "--log":
            log = args[i + 1]
            i += 2
        else:
            sys.exit("unknown argument " + args[i])
    refs = load_refs(tasks_dir)

    first = sys.stdin.readline()
    if log:
        with open(log, "a", encoding="utf-8", newline="\n") as f:
            f.write(first)
    task = json.loads(first)
    arm, ref = task["arm"], refs[task["id"]][task["arm"]]

    if mode == "slow":
        time.sleep(3600)
        return
    if mode == "garbage":
        sys.stdout.write("this is not json\n")
        sys.stdout.flush()
        return
    if mode == "tool-violation":
        say({"type": "tool", "name": "run_tests", "args": {"source": ref}})
        return

    plan = []   # messages to send, in order, each after reading one reply
    if mode in ("oracle", "spy"):
        plan = [{"type": "final", "source": ref}]
    elif mode == "wrong-then-oracle":
        plan = [{"type": "final", "source": WRONG[arm]}, {"type": "final", "source": ref}]
    elif mode == "lookup-then-oracle":
        if arm == "mage":
            plan = [{"type": "tool", "name": "ontology_index", "args": {}},
                    {"type": "tool", "name": "ontology_query", "args": {"term": "fn"}},
                    {"type": "tool", "name": "ontology_query", "args": {"term": "match", "budget": 300}},
                    {"type": "tool", "name": "check", "args": {"source": ref}},
                    {"type": "final", "source": ref}]
        else:
            plan = [{"type": "tool", "name": "check", "args": {"source": ref}}, {"type": "final", "source": ref}]
    elif mode == "always-wrong":
        plan = [{"type": "final", "source": WRONG[arm]}, {"type": "final", "source": BROKEN[arm]}] * 4
    else:
        sys.exit("unknown mode " + mode)

    step = 0
    usage = {"input_tokens": 1, "output_tokens": 1}
    while step < len(plan):
        msg = dict(plan[step])
        if os.environ.get("STANDIN_USAGE"):
            msg["usage"] = usage
        say(msg)
        step += 1
        line = sys.stdin.readline()
        if not line:
            return
        if log:
            with open(log, "a", encoding="utf-8", newline="\n") as f:
                f.write(line)
        reply = json.loads(line)
        if reply.get("type") == "end":
            return
    # plan exhausted: wait for the driver to close
    while sys.stdin.readline():
        pass


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Evaluator for germline's self-modification workload (germline/src/selfmod.rs).

Usage:
    python3 scripts/selfmod-eval.py [--no-time] MANIFEST BINARY

SelfModWorkload appends the candidate's built compiler as the last argument, so
configure the evaluator argv as `python3 <this script> [--no-time] <manifest>`.
Both this script and the manifest must lie outside every writable prefix (the
workload refuses a config where they do not). Freeze them, with the corpus, by
`scripts/freeze-corpora.sh`, so the judge does not move with the tree.

MANIFEST is JSON; paths are relative to the manifest's directory:

    {
      "good":   ["ok/a.mg", ...],                       # must check clean
      "bad":    {"bad/use_after_move.mg": ["E0382"]},   # must raise these codes
      "target": ["backlog/feature.mg", ...],            # should check clean, may not yet
      "sha256": {"ok/a.mg": "<hex>", ...}               # optional: pins each file
    }

With `sha256`, every named file must be pinned and match, or the script exits
non-zero. The workload's suite digest covers this manifest's bytes, so pinning
makes it cover the corpus too: an edited corpus file is refused, not measured.

Each file is checked with `BINARY --check --json FILE`. Prints one JSON object
on the last stdout line, the axes SelfModWorkload reads:

    safety       fraction of `good` files with no error (and exit status 0).
                 A new error on a known-good program is a regression.
    correctness  fraction of `bad` files whose expected codes all appear.
                 Codes, not positions: some diagnostics carry no span.
    capability   fraction of `target` files that check clean, the thing a
                 candidate improves; when `target` is empty, the mean of
                 safety and correctness.
    efficiency   1 / (1 + total seconds of checking). Wall-clock, so it varies
                 between runs on one machine; `--no-time` leaves it out.

A file whose check cannot be run or parsed counts as failed on its axis; the
script itself exits non-zero only when the manifest is unusable, which the
workload records as an evaluator failure (capability 0).
"""

import hashlib
import json
import os
import subprocess
import sys
import time

TIMEOUT_S = 60


def die(msg):
    print(f"selfmod-eval: {msg}", file=sys.stderr)
    sys.exit(2)


def check(binary, path):
    """(ok, codes) for one file; ok is False when the check could not be read."""
    try:
        r = subprocess.run([binary, "--check", "--json", path], capture_output=True, text=True, timeout=TIMEOUT_S)
    except (OSError, subprocess.TimeoutExpired) as e:
        print(f"selfmod-eval: {path}: {e}", file=sys.stderr)
        return None
    # The report is pretty-printed over many lines; fall back to the last line
    # in case anything precedes it.
    try:
        report = json.loads(r.stdout)
    except ValueError:
        line = next((l for l in reversed(r.stdout.splitlines()) if l.strip()), "")
        try:
            report = json.loads(line)
        except ValueError:
            print(f"selfmod-eval: {path}: output is not JSON", file=sys.stderr)
            return None
    if not isinstance(report, dict):
        return None
    counts = report.get("counts")
    if isinstance(counts, dict) and isinstance(counts.get("by_code"), dict):
        codes = {c for c, n in counts["by_code"].items() if n}
        errors = counts.get("error", 0)
    else:
        diags = report.get("diagnostics") or []
        codes = {d.get("code") for d in diags if isinstance(d, dict) and d.get("code")}
        errors = report.get("error_count", sum(1 for d in diags if isinstance(d, dict) and d.get("severity") == "error"))
    return {"exit": r.returncode, "errors": errors, "codes": codes}


def clean(res):
    return res is not None and res["exit"] == 0 and res["errors"] == 0


def frac(hits, n):
    return hits / n if n else 1.0


def main(argv):
    no_time = "--no-time" in argv
    args = [a for a in argv if a != "--no-time"]
    if len(args) != 2:
        die("usage: selfmod-eval.py [--no-time] MANIFEST BINARY")
    manifest_path, binary = args
    try:
        with open(manifest_path, encoding="utf-8") as f:
            m = json.load(f)
    except (OSError, ValueError) as e:
        die(f"cannot read manifest {manifest_path}: {e}")
    root = os.path.dirname(os.path.abspath(manifest_path))
    good = m.get("good", [])
    bad = m.get("bad", {})
    target = m.get("target", [])
    if not isinstance(good, list) or not isinstance(bad, dict) or not isinstance(target, list):
        die("manifest fields: good (list), bad (object of file -> codes), target (list)")
    if not good and not bad and not target:
        die("the manifest names no files")
    pins = m.get("sha256")
    if pins is not None and not isinstance(pins, dict):
        die("manifest field sha256 must be an object of file -> hex digest")
    for rel in list(good) + list(bad) + list(target):
        path = os.path.join(root, rel)
        if not os.path.isfile(path):
            die(f"manifest names a missing file: {rel}")
        if pins is not None:
            with open(path, "rb") as f:
                digest = hashlib.sha256(f.read()).hexdigest()
            if pins.get(rel) != digest:
                die(f"{rel} does not match its pinned sha256")

    started = time.monotonic()
    safety = frac(sum(clean(check(binary, os.path.join(root, f))) for f in good), len(good))
    hits = 0
    for f, want in bad.items():
        res = check(binary, os.path.join(root, f))
        if res is not None and set(want) <= res["codes"]:
            hits += 1
    correctness = frac(hits, len(bad))
    if target:
        capability = frac(sum(clean(check(binary, os.path.join(root, f))) for f in target), len(target))
    else:
        capability = (safety + correctness) / 2
    seconds = time.monotonic() - started

    axes = {"capability": capability, "safety": safety, "correctness": correctness}
    if not no_time:
        axes["efficiency"] = 1.0 / (1.0 + seconds)
    print(json.dumps(axes, sort_keys=True))


if __name__ == "__main__":
    main(sys.argv[1:])

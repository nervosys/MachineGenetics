#!/usr/bin/env python3
"""Evaluator for germline's self-modification workload (germline/src/selfmod.rs).

Usage:
    python3 scripts/selfmod-eval.py [--no-time] MANIFEST BINARY
    python3 scripts/selfmod-eval.py --pin-eval FLAG MANIFEST BINARY

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
      "sha256": {"ok/a.mg": "<hex>", ...},              # optional: pins each file
      "fmt_flag": "--fmt-expand",                       # optional: enables `fidelity`
      "eval": {"flag": "--eval",                        # optional: enables `semantics`
               "expect": {"ok/a.mg": "<sha256 of stdout>", ...},
               "target": {"backlog/b.mg": "<sha256>",        # optional: run-time backlog
                          "backlog/t.mg": {"fails_with": "integer overflow in"}, ...}}
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
                 safety and correctness. With `eval.target`, each of those
                 files is one more item, met when `BINARY <flag> FILE` exits 0
                 with exactly the pinned stdout: a run-time backlog, whose
                 expected output is written from the specification, not
                 recorded from a binary. An entry {"fails_with": TEXT} is met
                 instead when the run exits non-zero and TEXT appears in its
                 stderr: a trap the specification requires.
    efficiency   1 / (1 + total seconds of checking). Wall-clock, so it varies
                 between runs on one machine; `--no-time` leaves it out.
    fidelity     only with `fmt_flag`: of the programs that check clean (`good`,
                 and `target` files that now do), the fraction whose formatted
                 output (`BINARY <fmt_flag> FILE`) keeps every numeric literal
                 outside comments (as often as the source has it; a canonical
                 spelling may add one, as `f32x4` becomes `Simd[f32, 4]`). A change that
                 parses new syntax and then drops part of it (a discriminant
                 `A = 1` formatted as `A`) passes `--check` and fails this.
                 Use it as a guard axis.
    semantics    only with `eval`: the fraction of `expect` files for which
                 `BINARY <flag> FILE` exits 0 and prints exactly the pinned
                 output (sha256 of stdout, line endings normalised). Checking
                 and formatting say nothing about what a program computes; a
                 candidate that breaks the evaluator passes both and fails
                 this. Use it as a guard axis.

`--pin-eval FLAG MANIFEST BINARY` prints the `eval` object for the manifest's
`good` files that BINARY runs with exit 0, to paste into the manifest before
freezing it. A program that fails to run is left out rather than pinned as
failing, so a candidate that makes it run is not counted as a regression.

A file whose check cannot be run or parsed counts as failed on its axis; the
script itself exits non-zero only when the manifest is unusable, which the
workload records as an evaluator failure (capability 0).
"""

import collections
import hashlib
import json
import os
import re
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


NUMBER = re.compile(r"(?<![A-Za-z_0-9.])\d+(?:\.\d+)?")


def literals(text):
    return collections.Counter(NUMBER.findall(re.sub(r"//[^\n]*", "", text)))


def faithful(binary, fmt_flag, path):
    """Whether formatting `path` keeps its numeric literals."""
    try:
        r = subprocess.run([binary, fmt_flag, path], capture_output=True, text=True, timeout=TIMEOUT_S)
        with open(path, encoding="utf-8") as f:
            src = f.read()
    except (OSError, subprocess.TimeoutExpired, UnicodeDecodeError) as e:
        print(f"selfmod-eval: {path}: {e}", file=sys.stderr)
        return False
    # Nothing lost; a canonical spelling may add one (`f32x4` -> `Simd[f32, 4]`).
    return r.returncode == 0 and not (literals(src) - literals(r.stdout))


def clean(res):
    return res is not None and res["exit"] == 0 and res["errors"] == 0


def run_output(binary, flag, path):
    """(exit status, sha256 of stdout with CRLF normalised), or None."""
    try:
        r = subprocess.run([binary, flag, path], capture_output=True, timeout=TIMEOUT_S)
    except (OSError, subprocess.TimeoutExpired) as e:
        print(f"selfmod-eval: {path}: {e}", file=sys.stderr)
        return None
    return r.returncode, hashlib.sha256(r.stdout.replace(b"\r\n", b"\n")).hexdigest()


def fails_with(binary, flag, path, text):
    """Whether running `path` exits non-zero with `text` in its stderr."""
    try:
        r = subprocess.run([binary, flag, path], capture_output=True, timeout=TIMEOUT_S)
    except (OSError, subprocess.TimeoutExpired) as e:
        print(f"selfmod-eval: {path}: {e}", file=sys.stderr)
        return False
    return r.returncode != 0 and text.encode("utf-8") in r.stderr


def pin_eval(flag, manifest_path, binary):
    with open(manifest_path, encoding="utf-8") as f:
        m = json.load(f)
    root = os.path.dirname(os.path.abspath(manifest_path))
    expect = {}
    for rel in m.get("good", []):
        got = run_output(binary, flag, os.path.join(root, rel))
        if got is not None and got[0] == 0:
            expect[rel] = got[1]
    print(json.dumps({"flag": flag, "expect": expect}, indent=1, sort_keys=True))


def frac(hits, n):
    return hits / n if n else 1.0


def main(argv):
    if argv[:1] == ["--pin-eval"]:
        if len(argv) != 4 or not argv[1].startswith("--"):
            die("usage: selfmod-eval.py --pin-eval FLAG MANIFEST BINARY")
        pin_eval(argv[1], argv[2], argv[3])
        return
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
    clean_files = []
    good_clean = 0
    for f in good:
        if clean(check(binary, os.path.join(root, f))):
            good_clean += 1
            clean_files.append(f)
    safety = frac(good_clean, len(good))
    hits = 0
    for f, want in bad.items():
        res = check(binary, os.path.join(root, f))
        if res is not None and set(want) <= res["codes"]:
            hits += 1
    correctness = frac(hits, len(bad))
    target_clean = [f for f in target if clean(check(binary, os.path.join(root, f)))]
    clean_files.extend(target_clean)
    seconds = time.monotonic() - started

    ev = m.get("eval")
    eval_target = {}
    if ev is not None:
        if not isinstance(ev, dict) or not isinstance(ev.get("flag"), str) or not ev["flag"].startswith("--") \
                or not isinstance(ev.get("expect", {}), dict) or not isinstance(ev.get("target", {}), dict) \
                or not (ev.get("expect") or ev.get("target")):
            die("manifest field eval must be {\"flag\": \"--eval\", \"expect\": {FILE: SHA256, ...}, "
                "\"target\": {FILE: SHA256, ...}} with at least one file in expect or target")
        eval_target = ev.get("target", {})
    strays = sorted(set(eval_target) - set(target))
    if strays:
        die(f"eval target names files not in target (so not pinned): {', '.join(strays)}")
    met = 0
    for rel, want in eval_target.items():
        path = os.path.join(root, rel)
        if isinstance(want, dict):
            text = want.get("fails_with")
            if not isinstance(text, str) or not text or set(want) != {"fails_with"}:
                die(f"eval target {rel}: an object entry must be {{\"fails_with\": TEXT}}")
            met += fails_with(binary, ev["flag"], path, text)
        else:
            got = run_output(binary, ev["flag"], path)
            met += got is not None and got == (0, want)
    items = len(target) + len(eval_target)
    if items:
        capability = frac(len(target_clean) + met, items)
    else:
        capability = (safety + correctness) / 2
    axes = {"capability": capability, "safety": safety, "correctness": correctness}
    fmt_flag = m.get("fmt_flag")
    if fmt_flag is not None:
        if not isinstance(fmt_flag, str) or not fmt_flag.startswith("--"):
            die("manifest field fmt_flag must be a flag such as --fmt-expand")
        kept = sum(faithful(binary, fmt_flag, os.path.join(root, f)) for f in clean_files)
        axes["fidelity"] = frac(kept, len(clean_files))
    if ev is not None and ev.get("expect"):
        strays = sorted(set(ev["expect"]) - set(good))
        if strays:
            die(f"eval expect names files not in good (so not pinned): {', '.join(strays)}")
        same = 0
        for rel, want in ev["expect"].items():
            got = run_output(binary, ev["flag"], os.path.join(root, rel))
            same += got is not None and got == (0, want)
        axes["semantics"] = frac(same, len(ev["expect"]))
    if not no_time:
        axes["efficiency"] = 1.0 / (1.0 + seconds)
    print(json.dumps(axes, sort_keys=True))


if __name__ == "__main__":
    main(sys.argv[1:])

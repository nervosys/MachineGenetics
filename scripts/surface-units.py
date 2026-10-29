#!/usr/bin/env python3
"""Which printed surface is cheaper, by unit? (IMPLEMENTATION_PLAN.md S2.)

Every tracked `.mg` file is printed twice with the built `mage-parse`:

    sigil    `--fmt-compact`   (the token-minimal surface)
    keyword  `--fmt-expand`    (the human surface)

and each surface is measured in four units:

    bytes    UTF-8 bytes of the printed file
    tokens   MAGE lexer tokens: `mage-parse --count-tokens`, the count the arena's
             `Program::token_len` charges (Eof included, as there)
    bpe      cl100k BPE tokens, from agentic-eval's `tokens_of` example
             (set TOKENS_OF to its built binary; otherwise Python `tiktoken` if
             importable; otherwise the unit is reported unavailable, not estimated)
    bits     description length under one arena learner, `ng:4:14:0.02`, trained
             on the train split of one surface and scored on the held-out split of
             that surface. Total bits = bpb x bytes of the held-out files.

Split: files sorted by path (`git ls-files '*.mg'`); every 5th file, the 5th,
10th, ..., is held out, the rest train. The split is the same for both surfaces.

Usage (from the repository root, after building both binaries in release):

    cargo build --release --manifest-path prototype/Cargo.toml
    cargo build --release --manifest-path arena/Cargo.toml
    TOKENS_OF=<path to tokens_of> python scripts/surface-units.py [--json OUT]

The binaries are located with `cargo metadata`, so CARGO_TARGET_DIR is honoured;
nothing is read from a stale `target/` directory.
"""

import argparse
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

LEARNER = "ng:4:14:0.02"
# The arena's text path evaluates at most this many held-out bytes (BASELINE_EVAL_BYTES).
EVAL_CAP = 64 << 10
HOLDOUT_EVERY = 5


def die(msg):
    print(f"surface-units: {msg}", file=sys.stderr)
    sys.exit(1)


def repo_root():
    out = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True)
    return Path(out.stdout.strip())


def target_dir(manifest):
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", str(manifest)],
        capture_output=True,
        text=True,
    )
    if out.returncode != 0:
        die(f"cargo metadata failed for {manifest}: {out.stderr.strip()}")
    return Path(json.loads(out.stdout)["target_directory"])


def find_bin(manifest, name, profile):
    exe = name + (".exe" if os.name == "nt" else "")
    path = target_dir(manifest) / profile / exe
    if not path.is_file():
        die(f"missing {path}; build with: cargo build --{profile} --manifest-path {manifest.as_posix()}")
    return path


def run(args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if r.returncode != 0:
        die(f"{args[0]} {' '.join(str(a) for a in args[1:3])} exited {r.returncode}: {r.stderr.strip() or r.stdout.strip()}")
    return r.stdout


def median_ratio(sig, kw):
    """Median of per-file sigil/keyword ratios, over files whose keyword figure is non-zero."""
    return statistics.median(s / k for s, k in zip(sig, kw) if k)


def bpe_counts(paths, tokens_of):
    """cl100k counts for `paths`, in order, or None when no BPE source is available."""
    if tokens_of:
        out = run([tokens_of, *paths])
        counts = []
        exact = None
        for line in out.splitlines():
            if line.startswith("tokenizer exact:"):
                exact = line
                continue
            m = re.match(r"^\s*(\d+)\s+\d+\s{3}(.*)$", line)
            if m:
                counts.append(int(m.group(1)))
        if len(counts) != len(paths):
            die(f"tokens_of returned {len(counts)} counts for {len(paths)} files")
        print(f"bpe: {tokens_of} ({exact})", file=sys.stderr)
        return counts
    try:
        import tiktoken  # noqa: PLC0415

        enc = tiktoken.get_encoding("cl100k_base")
        print("bpe: Python tiktoken cl100k_base", file=sys.stderr)
        return [len(enc.encode(Path(p).read_text(encoding="utf-8"), disallowed_special=())) for p in paths]
    except Exception as e:  # noqa: BLE001 - any failure means unavailable, reported as such
        print(f"bpe: unavailable (TOKENS_OF unset; tiktoken: {e.__class__.__name__})", file=sys.stderr)
        return None


def train_and_score(arena, train, heldout, json_path):
    """Train the learner on `train`, score `heldout`; return (bpb, bytes_trained)."""
    run(
        [
            arena, "physics-baseline", "--system", "none", "--learners", LEARNER,
            "--text-train", train, "--text-heldout", heldout, "--json", json_path,
        ]
    )
    data = json.loads(Path(json_path).read_text(encoding="utf-8"))
    last = data["runs"][0]["checkpoints"][-1]
    return last["heldout_bpb"], last["bytes_trained"]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--profile", default="release", choices=["release", "debug"])
    ap.add_argument("--work", help="scratch directory (default: a fresh temporary one)")
    ap.add_argument("--json", help="also write every number to this JSON file")
    args = ap.parse_args()

    root = repo_root()
    os.chdir(root)
    mage = find_bin(root / "prototype" / "Cargo.toml", "mage-parse", args.profile)
    arena = find_bin(root / "arena" / "Cargo.toml", "arena", args.profile)
    tokens_of = os.environ.get("TOKENS_OF") or None
    if tokens_of and not Path(tokens_of).is_file():
        die(f"TOKENS_OF={tokens_of} is not a file")

    files = sorted(
        line for line in run(["git", "ls-files", "*.mg"]).splitlines() if line
    )
    if not files:
        die("no tracked .mg files")
    held_idx = {i for i in range(len(files)) if i % HOLDOUT_EVERY == HOLDOUT_EVERY - 1}
    print(f"surface-units: {len(files)} files, {len(held_idx)} held out, mage-parse {mage}, arena {arena}", file=sys.stderr)

    work = Path(args.work) if args.work else Path(tempfile.mkdtemp(prefix="surface-units-"))
    work.mkdir(parents=True, exist_ok=True)
    surfaces = {"sigil": "--fmt-compact", "keyword": "--fmt-expand"}
    printed = {s: [] for s in surfaces}
    for s, flag in surfaces.items():
        (work / s).mkdir(exist_ok=True)
        for i, src in enumerate(files):
            out = work / s / f"{i:03d}.mg"
            run([mage, flag, root / src, out])
            printed[s].append(out)

    # Bytes and lexer tokens, per file, for both surfaces.
    res = {s: {"bytes": [], "tokens": []} for s in surfaces}
    for s in surfaces:
        for p in printed[s]:
            res[s]["bytes"].append(p.stat().st_size)
            res[s]["tokens"].append(int(run([mage, "--count-tokens", p]).strip()))

    # cl100k, if any BPE source is present.
    bpe_available = True
    for s in surfaces:
        counts = bpe_counts([str(p) for p in printed[s]], tokens_of)
        if counts is None:
            bpe_available = False
            break
        res[s]["bpe"] = counts

    # Learner bits: one train corpus and one held-out corpus per surface, plus one
    # run per held-out file for the per-file median.
    held_list = sorted(held_idx)
    bits = {}
    for s in surfaces:
        train_bytes = b"".join(printed[s][i].read_bytes() for i in range(len(files)) if i not in held_idx)
        held_bytes = {i: printed[s][i].read_bytes() for i in held_list}
        whole = b"".join(held_bytes[i] for i in held_list)
        if len(whole) > EVAL_CAP:
            die(f"{s}: held-out corpus is {len(whole)} bytes, over the {EVAL_CAP}-byte evaluation window")
        train_p = work / f"{s}-train.bin"
        held_p = work / f"{s}-heldout.bin"
        train_p.write_bytes(train_bytes)
        held_p.write_bytes(whole)
        bpb, trained = train_and_score(arena, train_p, held_p, work / f"{s}-whole.json")
        # One scoring run per held-out file. A zero-byte file has no bits to score
        # and is charged 0; the learner cannot be given an empty corpus.
        jobs = {i: held_bytes[i] for i in held_list if held_bytes[i]}

        def score_one(item, train_p=train_p, s=s):
            i, data = item
            one = work / f"{s}-one-{i:03d}.bin"
            one.write_bytes(data)
            f_bpb, _ = train_and_score(arena, train_p, one, work / f"{s}-one-{i:03d}.json")
            return i, f_bpb * len(data)

        with ThreadPoolExecutor(max_workers=os.cpu_count() or 4) as pool:
            per_file_bits = dict(pool.map(score_one, jobs.items()))
        per_file = [per_file_bits.get(i, 0.0) for i in held_list]
        bits[s] = {
            "bpb": bpb,
            "bytes_trained": trained,
            "train_bytes": len(train_bytes),
            "heldout_bytes": len(whole),
            "total_bits": bpb * len(whole),
            "per_file_bits": per_file,
        }
        print(f"bits: {s} bpb {bpb:.4f} over {len(whole)} held-out bytes, trained on {trained} bytes", file=sys.stderr)

    # Table.
    def tot(s, u):
        return sum(res[s][u])

    rows = []
    for u in ["bytes", "tokens"] + (["bpe"] if bpe_available else []):
        sig, kw = res["sigil"][u], res["keyword"][u]
        rows.append((u, f"{sum(sig)}", f"{sum(kw)}", sum(sig) / sum(kw), median_ratio(sig, kw), len(sig)))
    s_bits = bits["sigil"]["total_bits"]
    k_bits = bits["keyword"]["total_bits"]
    bits_med = median_ratio(bits["sigil"]["per_file_bits"], bits["keyword"]["per_file_bits"])
    rows.append(("bits (held-out)", f"{s_bits:.0f}", f"{k_bits:.0f}", s_bits / k_bits, bits_med, len(held_list)))

    print("| unit | sigil total | keyword total | sigil/keyword | per-file median sigil/keyword | files |")
    print("|---|---|---|---|---|---|")
    for u, a, b, r, m, n in rows:
        print(f"| {u} | {a} | {b} | {r:.3f} | {m:.3f} | {n} |")
    if not bpe_available:
        print("| bpe (cl100k) | unavailable | unavailable | - | - | - |")
    print()
    print(
        f"learner {LEARNER}: sigil bpb {bits['sigil']['bpb']:.4f} (train {bits['sigil']['train_bytes']} B, "
        f"held-out {bits['sigil']['heldout_bytes']} B); keyword bpb {bits['keyword']['bpb']:.4f} "
        f"(train {bits['keyword']['train_bytes']} B, held-out {bits['keyword']['heldout_bytes']} B)"
    )

    if args.json:
        Path(args.json).write_text(
            json.dumps(
                {
                    "files": files,
                    "held_out": [files[i] for i in held_list],
                    "learner": LEARNER,
                    "bytes": {s: res[s]["bytes"] for s in surfaces},
                    "tokens": {s: res[s]["tokens"] for s in surfaces},
                    "bpe": {s: res[s].get("bpe") for s in surfaces},
                    "bits": bits,
                },
                indent=1,
            ),
            encoding="utf-8",
        )
    if not args.work:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()

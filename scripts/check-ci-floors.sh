#!/usr/bin/env bash
# The floors the ontology publishes as `ci_floors`, actually enforced.
#
# `MAGE_ONTOLOGY.json` publishes six of them, and its own doc comment said they
# were "Read from `.github/workflows/ci.yml`". **That file contains none of
# them** — no reliability-bench job, no heal threshold, no token-ratio gate.
# `UNIFICATION.md` went further and described a "new CI step" that parses the
# `**Total**` row of `benchmarks/TOKEN_REPORT.md` and fails above 1.100. There
# was no such step.
#
# So an agent reading the ontology believed six regressions were gated, and
# none were. Two of the six were not even true on a default run: the
# file-oracle structural-heal contribution is 1, not >= 2, and the stage-3
# refine smoke is 0 unless a wrapper command is supplied.
#
# This script measures what can be measured here and enforces it. The floors
# are stated once, in `ci_floors_thresholds` below, and the ontology entry for
# each names this file — so the published claim and the enforcement move
# together.
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

# ── The floors ────────────────────────────────────────────────────────
MIN_PARSE=98        # file-oracle parse rate, out of a 100-task corpus
MIN_HEAL=40         # perturbed-oracle pattern-heal successes
MAX_LEX_RATIO=1.100 # native-lexer total ratio, MAGE vs Rust

fail=0

# `reliability-bench` **writes `benchmarks/RELIABILITY_REPORT.md`** every time
# it runs, and that file is tracked. So this check used to modify the working
# tree as a side effect of checking — the exact thing §3 below saves and
# restores `TOKEN_REPORT.md` to avoid, applied to one artifact of this script
# and not the other. The consequence is worse here than there: the report
# embeds p50/p95/p99 **latencies**, so the rewrite is load-dependent. Running
# the suite on a busy machine rewrote them 30/247/348 → 51/551/1089 µs, and a
# subsequent `git add -A` would have committed timing noise as though it were a
# measurement.
#
# Restored, not compared. `TOKEN_REPORT.md` gets a staleness check because it
# is byte-stable; this one deliberately does not, because a latency table would
# be permanently red — the distinction is whether the artifact records a
# measurement or a timing.
RELIABILITY_REPORT=benchmarks/RELIABILITY_REPORT.md
reliability_saved=""
if [ -f "$RELIABILITY_REPORT" ]; then
    reliability_saved="$(mktemp)"
    cp "$RELIABILITY_REPORT" "$reliability_saved"
fi

restore_reliability_report() {
    if [ -n "$reliability_saved" ] && [ -f "$reliability_saved" ]; then
        cp "$reliability_saved" "$RELIABILITY_REPORT"
        rm -f "$reliability_saved"
        reliability_saved=""
    fi
}

# On every exit path, including a floor breach: the script must not leave a
# tracked file rewritten just because it failed.
trap restore_reliability_report EXIT

run_bench() {
    cargo run --release --quiet --manifest-path prototype/Cargo.toml \
        --bin reliability-bench -- "$@" 2>&1 || true
}

# ── 1. file-oracle parse rate ─────────────────────────────────────────
#
# The bench exits 1 on its documented-correct result (one corpus task does not
# parse cleanly and recovers via structural-heal), so its status is not the
# signal — the numbers it prints are.
oracle="$(run_bench)"
parse="$(printf '%s' "$oracle" | grep -oE 'parse [0-9]+/' | grep -oE '[0-9]+' | head -1)"
if [ -z "${parse:-}" ]; then
    echo "  x  could not read the file-oracle parse rate from the bench" >&2
    fail=1
elif [ "$parse" -lt "$MIN_PARSE" ]; then
    echo "  x  MIN_PARSE: file-oracle parse $parse/100, floor >= $MIN_PARSE" >&2
    fail=1
else
    echo "  ok MIN_PARSE: file-oracle parse $parse/100 (floor >= $MIN_PARSE)"
    echo "floor_parse=$parse" >&2
fi

# ── 2. perturbed pattern-heal recoveries ──────────────────────────────
perturbed="$(run_bench --agent perturbed)"
heal="$(printf '%s' "$perturbed" | grep -oE 'succeeded [0-9]+/' | grep -oE '[0-9]+' | head -1)"
if [ -z "${heal:-}" ]; then
    echo "  x  could not read the pattern-heal count from the bench" >&2
    fail=1
elif [ "$heal" -lt "$MIN_HEAL" ]; then
    echo "  x  MIN_HEAL: perturbed pattern-heal $heal, floor >= $MIN_HEAL" >&2
    fail=1
else
    echo "  ok MIN_HEAL: perturbed pattern-heal $heal (floor >= $MIN_HEAL)"
    echo "floor_heal=$heal" >&2
fi

# ── 3. native-lexer token ratio ───────────────────────────────────────
#
# Measured fresh, and the committed report checked for staleness.
#
# This used to read the ratio out of the *checked-in* `TOKEN_REPORT.md` and
# nothing else. That file is generated, nothing regenerated it, and it was
# stale — two categories had drifted by a few bytes. A floor read from a stale
# artifact is not a floor; it is a record of a floor that once held.
#
# The bench *writes* `TOKEN_REPORT.md` as a side effect, which makes the
# obvious fix wrong: run the bench, then compare the file against the bench's
# own stdout, and the two agree by construction. That was this script's first
# version of the check, and it passed when handed a deliberately stale
# report — the same vacuous-comparison shape catalogued in HANDOFF.md. What
# has to be compared is the file *as committed* against the file the bench
# produces, so the committed copy is saved first and put back afterwards: a
# check that rewrites a tracked file is not a check either.
#
# The bench's exit status used to be ignored here, and this note explained why:
# it returned non-zero whenever a task's *claimed* `token_count` disagreed with
# measurement by more than 10 %, and 150 claims across the 100 tasks do — the
# subject of `benchmarks/FINDINGS.md` §1, a finding rather than a regression.
# A status that is red on every run carries no information, so this script read
# the ratio out of stdout and dropped the status on the floor.
#
# Since 2026-08-19 the bench ratchets that set against
# `benchmarks/token-claims-baseline.txt` and reports only *movement*, so the
# status now distinguishes "the corpus is as known" from "something changed",
# and is honoured below. Exit 2 is a tool failure (unreadable corpus,
# unwritable report); `|| true` was swallowing that too, which meant a bench
# that could not run at all still let this check pass.
# `set -o errexit` is on (line 20). A bare `bench_out="$(... )"` whose command
# fails kills the script *at the assignment*, so the verdict below never
# prints and CI reports a bare exit 1 with no reason — which is why the
# original wrote `|| true`. The fix is to keep the status, not to discard it.
bench_status=0
REPORT=benchmarks/TOKEN_REPORT.md
saved="$(mktemp)"
cp "$REPORT" "$saved"
bench_out="$(cargo run --quiet --release --manifest-path prototype/Cargo.toml \
                 --bin token-bench 2>&1)" || bench_status=$?
ratio="$(printf '%s\n' "$bench_out" | awk '/native lexers:/ { for (i = 1; i <= NF; i++) if ($i ~ /^ratio=/) { sub(/^ratio=/, "", $i); print $i; exit } }')"
if cmp -s "$REPORT" "$saved"; then
    stale=""
else
    stale="yes"
fi
cp "$saved" "$REPORT"
rm -f "$saved"

if [ "$bench_status" -eq 2 ]; then
    echo "  x  token-bench could not run (exit 2); the ratio below is meaningless" >&2
    printf '%s\n' "$bench_out" | tail -3 >&2
    fail=1
elif [ "$bench_status" -ne 0 ]; then
    # Exit 1 now means the claim set moved against the baseline, in either
    # direction: a new disagreement, or a baseline entry that has stopped
    # disagreeing and should be deleted. Both are edits someone must make.
    echo "  x  token-bench: the corpus claim set moved against its baseline" >&2
    printf '%s\n' "$bench_out" | grep -E '^(  x|  \+|token-bench:)' | head -8 >&2
    fail=1
elif [ -z "${ratio:-}" ]; then
    echo "  x  could not read the native-lexer ratio from the bench" >&2
    fail=1
elif awk -v r="$ratio" -v m="$MAX_LEX_RATIO" 'BEGIN { exit !(r > m) }'; then
    echo "  x  native-lexer ratio $ratio, ceiling <= $MAX_LEX_RATIO" >&2
    fail=1
elif [ -n "$stale" ]; then
    # Not a floor breach — a stale artifact, which is how the floor came to be
    # enforced against old data in the first place.
    echo "  x  benchmarks/TOKEN_REPORT.md differs from a fresh run; regenerate it:" >&2
    echo "     cargo run --release --manifest-path prototype/Cargo.toml --bin token-bench" >&2
    fail=1
else
    echo "  ok native-lexer ratio $ratio (ceiling <= $MAX_LEX_RATIO, report current)"
fi

echo
if [ "$fail" -ne 0 ]; then
    echo "FAILED - a published CI floor is not met." >&2
    exit 1
fi
echo "OK — every enforceable published floor holds."

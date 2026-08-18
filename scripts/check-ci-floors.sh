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
# The bench's exit status is deliberately ignored. It returns non-zero
# whenever a task's *claimed* `token_count` disagrees with the measurement by
# more than 10 %, and 150 claims across the 100 tasks do — which is the subject
# of `benchmarks/FINDINGS.md` §1, a finding rather than a regression.
REPORT=benchmarks/TOKEN_REPORT.md
saved="$(mktemp)"
cp "$REPORT" "$saved"
bench_out="$(cargo run --quiet --release --manifest-path prototype/Cargo.toml \
                 --bin token-bench 2>&1 || true)"
ratio="$(printf '%s\n' "$bench_out" | awk '/native lexers:/ { for (i = 1; i <= NF; i++) if ($i ~ /^ratio=/) { sub(/^ratio=/, "", $i); print $i; exit } }')"
if cmp -s "$REPORT" "$saved"; then
    stale=""
else
    stale="yes"
fi
cp "$saved" "$REPORT"
rm -f "$saved"

if [ -z "${ratio:-}" ]; then
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

#!/usr/bin/env bash
# The benchmark tasks must be *accepted*, not just parsed — or be listed in the
# baseline below, which may only shrink.
#
# Why this exists: every documented task figure ("98/99", "99/100") counts
# **parsing only**. Run the compiler's own checker over the 99 MAGE reference
# solutions and 47 do not type-check; run the tasks' own `tests` through
# `--eval` and 18 tasks pass. "Parses" had been read as "is a valid, solved
# task", and nothing measured the difference. `scripts/task-acceptance.py`
# measures it; `benchmarks/ACCEPTANCE.md` says what the failures are and whose
# fault each group is (the task's, or the compiler/evaluator's).
#
# A task is *accepted* when its reference is check-clean and every one of its
# test cases evaluates to the expected value (strict comparison — see the
# docstring of the script; nothing is loosened to make a case pass).
#
# The baseline, `benchmarks/task-acceptance-baseline.txt`, has one line per task
# that is NOT accepted:  `<id> <check|test|unrunnable> <first reason>`.
# This script compares the id and the status word only (the reason is a hint
# and changes when a diagnostic is reworded):
#
#   - a task not in the baseline that is not accepted      -> fail (regression)
#   - a task in the baseline that is now accepted          -> fail (shrink it)
#   - a task whose status word changed (e.g. check -> test) -> fail (refresh):
#     moving from `check` to `test` means the reference now type-checks, which
#     is progress that must be recorded in the same commit.
#
# Compiler: `$MAGE_PARSE_BIN` if set, else resolved through cargo
# (scripts/find-mage-parse.sh) like every other checker.
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

if [ -n "${MAGE_PARSE_BIN:-}" ]; then
    BIN="$MAGE_PARSE_BIN"
else
    . scripts/find-mage-parse.sh
    BIN="$(build_and_find_mage_parse)"
fi

BASELINE=benchmarks/task-acceptance-baseline.txt
OUT="$(mktemp -d)"
trap 'rm -rf "$OUT"' EXIT

# Capture, then match. A pipe into `grep -q` under `pipefail` reports failure
# when the writer takes SIGPIPE.
python scripts/task-acceptance.py --mage-parse "$BIN" -q \
    --json "${TASK_ACCEPTANCE_JSON:-$OUT/result.json}" \
    --baseline-out "$OUT/measured.txt" > "$OUT/summary.txt"
tr -d '\r' < "$OUT/measured.txt" > "$OUT/measured.lf"
cat "$OUT/summary.txt"

if [ ! -f "$BASELINE" ]; then
    {
        echo "# Tasks that are NOT accepted: the MAGE reference does not type-check (check),"
        echo "# or a test case fails/errors (test), or no case could be run mechanically"
        echo "# (unrunnable). One line per task: <id> <status> <first reason>. Sorted."
        echo "# Only the id and the status word are compared; the reason is a hint."
        echo "# May only shrink: fix a task, then delete its line. See benchmarks/ACCEPTANCE.md."
        cat "$OUT/measured.lf"
    } > "$BASELINE"
    echo "baseline written to $BASELINE"
    exit 0
fi

tr -d '\r' < "$BASELINE" > "$OUT/baseline.lf"

fail=0
while read -r id status _rest; do
    [ -n "${id:-}" ] || continue
    case "$id" in \#*) continue ;; esac
    now=$(awk -v t="$id" '$1 == t { print $2 }' "$OUT/measured.lf")
    if [ -z "$now" ]; then
        echo "  +  $id: baselined as $status, now accepted - remove its line from $BASELINE"
        fail=1
    elif [ "$now" != "$status" ]; then
        echo "  x  $id: baseline says $status, now $now - refresh the baseline in this commit"
        fail=1
    fi
done < "$OUT/baseline.lf"

while read -r id status _rest; do
    [ -n "${id:-}" ] || continue
    if ! awk -v t="$id" '$1 == t { found = 1 } END { exit !found }' "$OUT/baseline.lf"; then
        echo "  x  $id: not accepted ($status), not in the baseline"
        fail=1
    fi
done < "$OUT/measured.lf"

total_now=$(awk 'NF { n++ } END { print n+0 }' "$OUT/measured.lf")
total_was=$(awk 'NF && $1 !~ /^#/ { n++ } END { print n+0 }' "$OUT/baseline.lf")
echo "Tasks not accepted: $total_now (baseline $total_was)."

if [ "$fail" -ne 0 ]; then
    echo
    echo "The set may only shrink. If you fixed tasks (or the compiler), regenerate"
    echo "the baseline in the same commit:  rm $BASELINE && bash $0"
    exit 1
fi

echo "OK - no task regressed, and none improved without the baseline saying so."

#!/usr/bin/env bash
# Every `#[ignore]`d test must be run by some CI job, or it is not a test.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# `eval_bench` — the harness that certifies the evaluator at 73/73 exact — was
# red for **76 commits**. Three independent things hid it: the test is
# `#[ignore]`d so `cargo test` skips it, it was additionally reachable only via
# a `--bench` flag nobody passed, and the branch it lived on was never pushed.
# Each of those is defensible on its own. Together they meant the number
# published in three documents had no runner at all.
#
# `#[ignore]` is the right tool for a slow harness. What it must not be is a
# way to remove a test from every schedule while it still reads, in the source
# and in the counts, as a test that exists. `cargo test` reports it as
# "ignored", which looks like a decision someone is making each run, and is
# not — nobody is deciding anything.
#
# Running this the first time found `perf_report` in exactly that state: it
# produces the ABL artifact-scaling figures in MEASUREMENTS.md and
# ARCHITECTURE_DSL.md, `cargo test` compiles it so it could not rot into a
# build error unnoticed, and no job had ever executed it. A runtime panic would
# have surfaced only when someone next tried to regenerate those numbers —
# exactly when a broken harness costs the most. It is now a CI step; it takes
# three seconds.
#
# ─────────────────────────────────────────────────────────────────────────────
# HOW IT DECIDES
# ─────────────────────────────────────────────────────────────────────────────
#
# An `#[ignore]`d test is satisfied if its function name appears anywhere in
# `.github/workflows/ci.yml`. That is deliberately loose: this check's job is
# to notice a test with *no* runner, not to prove the runner is correct. A
# name that appears in a comment would satisfy it — and that is the right
# trade, because the failure it guards against is absence, not subtlety.
#
# Files that no `mod` declaration reaches are skipped, because their tests
# cannot run for a prior reason and `check-orphan-sources.sh` already reports
# them. Double-reporting the same defect through two instruments makes both
# noisier and neither more informative. That skip list is read from
# `orphan-sources-baseline.txt`, so the two checks stay consistent by
# construction: fix an orphan and its tests become this check's problem
# automatically.
#
# There is no baseline file. The live population is small and currently fully
# covered, so the honest state is simply green — a baseline would exist only to
# hold exemptions that do not exist. If that changes, add one; do not weaken
# this.
#
# Usage:
#     bash scripts/check-ignored-tests.sh

set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

CRATES="prototype RecursiveMachineIntelligence ribosome germline forge framework/framewerx"
WORKFLOW=.github/workflows/ci.yml
ORPHANS=scripts/orphan-sources-baseline.txt

if [ ! -f "$WORKFLOW" ]; then
    echo "  x  $WORKFLOW is missing; nothing can be said about what CI runs" >&2
    exit 1
fi

fail=0
checked=0
skipped=0

for crate in $CRATES; do
    src="$crate/src"
    [ -d "$src" ] || continue

    while IFS= read -r f; do
        # A test in a file nothing compiles is already reported elsewhere.
        if [ -f "$ORPHANS" ] && grep -qxF "$f" "$ORPHANS" 2>/dev/null; then
            skipped=$((skipped + 1))
            continue
        fi

        # `#[ignore]` followed by the next `fn` — attribute lines only, so a
        # doc comment that mentions `#[ignore]` is not mistaken for one.
        names="$(awk '
            /^[[:space:]]*#\[ignore/ { seen = 1; next }
            seen && /fn [a-zA-Z_]/ {
                match($0, /fn [a-zA-Z_][a-zA-Z0-9_]*/)
                print substr($0, RSTART + 3, RLENGTH - 3)
                seen = 0
                next
            }
            seen && !/^[[:space:]]*#\[/ { seen = 0 }
        ' "$f")"

        [ -n "$names" ] || continue

        while IFS= read -r name; do
            [ -n "$name" ] || continue
            checked=$((checked + 1))
            if ! grep -qF -- "$name" "$WORKFLOW"; then
                echo "  x  $f: \`$name\` is #[ignore]d and no CI job names it" >&2
                echo "       Nothing runs it — not \`cargo test\`, which skips ignored" >&2
                echo "       tests, and not CI. Add a step, or delete the test." >&2
                fail=1
            fi
        done <<< "$names"
    done < <(find "$src" -name '*.rs' -type f | sort)
done

if [ "$fail" -ne 0 ]; then
    exit 1
fi

echo "  ok all $checked #[ignore]d test(s) are run by a CI job ($skipped file(s) skipped: no \`mod\` reaches them, see check-orphan-sources.sh)."

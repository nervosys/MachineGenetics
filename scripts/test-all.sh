#!/usr/bin/env bash
# Build and test all five MAGE crates.
#
# The repository is five separate Cargo workspaces on purpose (see
# ARCHITECTURE.md §"Repository layout"), so a root `cargo test` does nothing.
# This is the single entry point that covers everything CI covers:
#
#     rmi (cpu)   1,380 tests
#     prototype   1,172 tests
#     ribosome      164 tests
#     germline      112 tests
#     forge          52 tests
#     -------------------------
#     total       2,880 tests, 0 warnings
#
# Usage:
#   scripts/test-all.sh            # debug
#   scripts/test-all.sh --release  # optimized
#   scripts/test-all.sh --bench    # + eval_bench (73/73) and perf_report; implies --release
#   scripts/test-all.sh --cuda     # + prototype --features cuda (1,071 tests; needs an
#                                  #   NVIDIA driver to exercise kernels, CPU-falls-back
#                                  #   without one). CI only compile-checks this path.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE=()
BENCH=0
CUDA=0
CHECKDOCS=0

for arg in "$@"; do
    case "$arg" in
        --release) PROFILE=(--release) ;;
        --bench)   BENCH=1; PROFILE=(--release) ;;
        --cuda)    CUDA=1 ;;
        --check-docs) CHECKDOCS=1 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

failed=()
declare -A COUNTS

run_crate() {
    local name="$1" manifest="$2"; shift 2
    printf '\n=== %s ===\n' "$name"
    # Tee rather than re-measure: the counts `--check-docs` verifies must come
    # from the run just displayed. A second measurement could disagree with the
    # one the operator watched, which would make the check itself untrustworthy.
    local log
    log="$(mktemp)"
    if ! cargo test --manifest-path "$REPO/$manifest" "${PROFILE[@]}" "$@" 2>&1 | tee "$log"; then
        failed+=("$name")
    fi
    COUNTS["$name"]="$(grep -oE '^test result: ok\. [0-9]+ passed' "$log" \
        | grep -oE '[0-9]+' | awk '{s+=$1} END {print s+0}')"
    rm -f "$log"
}

# rmi is feature-gated: the default feature set pulls in GPU backends that need
# a toolchain CI does not have, so `cpu` is the portable, always-buildable one.
run_crate rmi       RecursiveMachineIntelligence/Cargo.toml --no-default-features --features cpu
run_crate prototype prototype/Cargo.toml
run_crate ribosome  ribosome/Cargo.toml
run_crate germline  germline/Cargo.toml
run_crate forge     forge/Cargo.toml

if [ "$CUDA" -eq 1 ]; then
    run_crate 'prototype (cuda)' prototype/Cargo.toml --features cuda
    # Feeds --check-docs so the documented CUDA figure is verified against the
    # hardware run that just happened, not left to drift as 1,269 did.
    COUNTS[cuda]=${COUNTS["prototype (cuda)"]:-0}
    unset 'COUNTS[prototype (cuda)]'
fi

if [ "$BENCH" -eq 1 ]; then
    printf '\n=== measurement harnesses ===\n'
    for harness in eval_bench perf_report; do
        blog="$(mktemp)"
        if ! cargo test --manifest-path "$REPO/prototype/Cargo.toml" --release "$harness" \
                -- --ignored --nocapture 2>&1 | tee "$blog"; then
            failed+=("prototype::$harness")
        fi
        # `[eval-bench] correctness: 73/73 programs exact` — feed the numerator
        # to --check-docs so the "73/73" claimed in four documents is verified
        # against the run that just produced it.
        if [ "$harness" = "eval_bench" ]; then
            n="$(grep -oE 'correctness: [0-9]+/' "$blog" | grep -oE '[0-9]+' | head -1)"
            [ -n "$n" ] && COUNTS[eval_exact]="$n"
        fi
        rm -f "$blog"
    done

    # reliability-bench is a binary rather than a test, and it **exits 1 on its
    # documented-correct result**: one of the 100 tasks does not parse cleanly
    # and recovers via structural-heal, which it reports as a failure exit. So
    # its status is deliberately not treated as pass/fail here — the numbers it
    # prints are the measurement, and --check-docs compares those. Worth knowing
    # before wiring this into CI, where a nonzero exit would read as broken.
    printf '\n=== reliability-bench ===\n'
    rlog="$(mktemp)"
    cargo run --release --quiet --manifest-path "$REPO/prototype/Cargo.toml" \
        --bin reliability-bench 2>&1 | tee "$rlog" || true
    lex="$(grep -oE 'lex [0-9]+/' "$rlog" | grep -oE '[0-9]+' | head -1)"
    parse="$(grep -oE 'parse [0-9]+/' "$rlog" | grep -oE '[0-9]+' | head -1)"
    eff="$(grep -oE 'effective pass: +[0-9]+/' "$rlog" | grep -oE '[0-9]+' | head -1)"
    [ -n "$lex" ] && COUNTS[rb_lex]="$lex"
    [ -n "$parse" ] && COUNTS[rb_parse]="$parse"
    [ -n "$eff" ] && COUNTS[rb_effective]="$eff"
    rm -f "$rlog"
fi

echo
if [ "${#failed[@]}" -gt 0 ]; then
    echo "FAILED: ${failed[*]}" >&2
    exit 1
fi
echo 'All crates green.'

if [ "$CHECKDOCS" -eq 1 ]; then
    # Sum the five crates by name, not every key in COUNTS: a --cuda run adds a
    # `cuda` entry that re-tests prototype, and folding it into the total would
    # count that crate twice and make the documented total unreachable.
    total=0
    for k in rmi prototype ribosome germline forge; do
        total=$((total + ${COUNTS[$k]:-0}))
    done
    echo
    {
        for k in "${!COUNTS[@]}"; do echo "$k=${COUNTS[$k]}"; done
        echo "total=$total"
        # The CI job count is a claim in HANDOFF.md that nothing checked, so a
        # rewrite could — and did — change "10 jobs" to "11" by counting the
        # `push`/`pull_request` trigger keys along with the jobs. Measured here
        # from the workflow itself: keys indented two spaces *after* `jobs:`.
        echo "ci_jobs=$(awk '/^jobs:/{i=1;next} i&&/^  [a-z0-9_-]+:$/{n++} END{print n+0}' "$REPO/.github/workflows/ci.yml")"
        # `.mg` sources checked vs skipped as sketches. HANDOFF.md states both,
        # and both moved this session (96/30 -> 101/25) when `framewerx` was
        # rewritten. Measured by running the checker, so the claim cannot drift
        # from the list that produces it.
        mg_line="$(bash "$REPO/scripts/check-mg-sources.sh" 2>/dev/null | grep -E '^Checked [0-9]+ \.mg')"
        echo "mg_checked=$(printf '%s' "$mg_line" | awk '{print $2}')"
        # Field 5, not 4: the line reads "Checked 101 .mg files; 25 skipped",
        # so `$4` is "files;". The pin caught it on its first run.
        echo "mg_sketches=$(printf '%s' "$mg_line" | awk '{print $5}')"
        # Documentation entry points actually executed. `--check` and `--eval`
        # are independent oracles and the blocks had only ever been checked;
        # running them found thirteen registered builtins with no arm in the
        # evaluator.
        echo "$(bash "$REPO/scripts/check-doc-evals.sh" 2>/dev/null | grep -E '^doc_evals=')"
        echo "$(bash "$REPO/scripts/check-doc-blocks.sh" 2>&1 >/dev/null | grep -E '^doc_blocks=')"
    # Invoked through `bash` rather than executed directly. The mode bit is now
    # set in git, but it is the kind of thing a Windows checkout drops silently
    # — this failed in CI with a bare "Permission denied" after passing on the
    # machine it was written on. Belt and braces: the mode is correct *and* not
    # relied upon.
    } | bash "$REPO/scripts/check-doc-counts.sh" || exit 1
fi

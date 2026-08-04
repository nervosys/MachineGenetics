#!/usr/bin/env bash
# Build and test all five MAGE crates.
#
# The repository is five separate Cargo workspaces on purpose (see
# ARCHITECTURE.md §"Repository layout"), so a root `cargo test` does nothing.
# This is the single entry point that covers everything CI covers:
#
#     rmi (cpu)   1,380 tests
#     prototype   1,038 tests
#     ribosome      155 tests
#     germline      112 tests
#     forge          52 tests
#     -------------------------
#     total       2,737 tests, 0 warnings
#
# Usage:
#   scripts/test-all.sh            # debug
#   scripts/test-all.sh --release  # optimized
#   scripts/test-all.sh --bench    # + eval_bench (73/73) and perf_report; implies --release
#   scripts/test-all.sh --cuda     # + prototype --features cuda (1,269 tests; needs an
#                                  #   NVIDIA driver to exercise kernels, CPU-falls-back
#                                  #   without one). CI only compile-checks this path.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE=()
BENCH=0
CUDA=0

for arg in "$@"; do
    case "$arg" in
        --release) PROFILE=(--release) ;;
        --bench)   BENCH=1; PROFILE=(--release) ;;
        --cuda)    CUDA=1 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

failed=()

run_crate() {
    local name="$1" manifest="$2"; shift 2
    printf '\n=== %s ===\n' "$name"
    if ! cargo test --manifest-path "$REPO/$manifest" "${PROFILE[@]}" "$@"; then
        failed+=("$name")
    fi
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
fi

if [ "$BENCH" -eq 1 ]; then
    printf '\n=== measurement harnesses ===\n'
    for harness in eval_bench perf_report; do
        if ! cargo test --manifest-path "$REPO/prototype/Cargo.toml" --release "$harness" -- --ignored --nocapture; then
            failed+=("prototype::$harness")
        fi
    done
fi

echo
if [ "${#failed[@]}" -gt 0 ]; then
    echo "FAILED: ${failed[*]}" >&2
    exit 1
fi
echo 'All crates green.'

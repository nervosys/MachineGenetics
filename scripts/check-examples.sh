#!/usr/bin/env bash
# Pin the shipped examples: each must typecheck, evaluate, and print the answer
# recorded here.
#
# ─────────────────────────────────────────────────────────────────────────────
# THE STATE THIS RECORDS
# ─────────────────────────────────────────────────────────────────────────────
#
# On 2026-08-05, **11 of 12 examples under `examples/` failed `--check`**, and
# nothing anywhere checked them: CI runs `cargo test`, and the examples are data,
# not tests. The diagnosis at the time — ten shared a `use std::x;` line, Rust's
# `::` where MAGE wants `.` — was only the first error in each file. A mechanical
# conversion was tried and every example then failed deeper on some construct
# that had never existed (`data`, pipeline `|>`, `handle`). They were aspirational
# Rust, not MAGE, and were rewritten rather than converted.
#
# All twelve now typecheck and run. Rewriting them found **eleven compiler and
# evaluator bugs**, every one by running a probe rather than by reading source.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THE OUTPUT IS PINNED AND NOT JUST THE EXIT STATUS
# ─────────────────────────────────────────────────────────────────────────────
#
# "Typechecks" turned out to be a weak bar, and "runs" not much stronger. Two
# examples ran cleanly and printed the *wrong answer*:
#
#   * `cli-tool` searched with `contains`, which is element membership, not
#     substring search — so its grep matched nothing and reported `0`;
#   * `autonomous-pipeline` filtered its worklist on readiness instead of on
#     what had been placed, and declared three of five tasks unplaceable on an
#     acyclic graph.
#
# Neither is visible from an exit status. Both are obvious the moment the output
# is compared against what the example claims to demonstrate. So the answer is
# what gets pinned.
#
# Regenerating after an intentional change:
#     scripts/check-examples.sh --print   # emits an EXPECTED block to paste in
#
# Read the diff before pasting it. The whole point is that a changed answer is a
# claim someone has to look at, not a file to be re-blessed.
#
# It fails when an example stops checking, stops evaluating, or changes what it
# prints — and, because that is the failure this repository kept finding, when
# an example exists with no recorded answer at all.
#
# Usage: scripts/check-examples.sh [--print] [path-to-mage-parse]

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO"

PRINT=0
if [ "${1:-}" = "--print" ]; then
    PRINT=1
    shift
fi

MP="${1:-}"
if [ -z "$MP" ]; then
    for c in prototype/target/release/mage-parse prototype/target/release/mage-parse.exe \
             prototype/target/debug/mage-parse prototype/target/debug/mage-parse.exe; do
        [ -x "$c" ] && MP="$c" && break
    done
fi
if [ -z "$MP" ] || [ ! -x "$MP" ]; then
    echo "check-examples: no mage-parse binary; build it first" >&2
    exit 2
fi

# What `--eval <example>/src/main.mg main` must print, verbatim. Every one of
# these was read against what the example says it demonstrates, not merely
# captured from a run that did not crash.
declare -A EXPECTED=(
    [agent-swarm]='"critic: 1 task(s), cost 150 | fixer: 1 task(s), cost 148 | reader: 2 task(s), cost 130; approved: T1,T4; total cost 428"'
    [autonomous-pipeline]='"order spec -> schema -> handlers -> verify -> emit; unplaceable 0; cycle schema,spec; ran 3 used 1500 skipped schema,spec; <4 tokens for handlers for user management>; <cached spec>; emit(emit,200) verify(verify,400) handlers(generate,900) schema(generate,600) spec(plan,150)"'
    [cli-tool]='"-i alpha -> alpha beta | ALPHA delta; -c beta -> 2; no pattern -> usage error"'
    [cost-aware-optimizer]='"3 benchmark samples; x86-64: size | aarch64: balanced | riscv64: balanced | wasm32: balanced; x86-64: size | aarch64: size | riscv64: no candidate within budget | wasm32: balanced"'
    [data-structures]='"points=3, total_distance=15, closest=3"'
    [effects-showcase]='"statuses=3 missing=0; 3 configured, live ok; up.example@1700000017; audited 21 chars; recorded 21 chars; ok/warn/error; 3 samples, worst 503"'
    [hello-world]='"Hello, MAGE! (your name has 4 letters)"'
    [http-client]='"1: user 1 Ada (active=true); 2: rate limited, retry in 30s; 3: not found; 4: decode: expected 3 fields, got 1"'
    [live-compiler]='"live r2 passing 4; rollbacks 1; after explicit rollback r1 passing 3; type: handler:9 unresolved placeholder; revert placeholder@70; no repair proposed"'
    [multilang-bindings]='"int32_t mg_checksum(uint8_t* data, int32_t seed); /* caller frees */ const char* mg_describe(int32_t code); /* callee frees */ || namespace mage { int32_t checksum(std::vector<uint8_t> data, int32_t seed); } // std::unique_ptr namespace mage { std::string_view describe(int32_t code); } // raw || def mage_checksum(data: bytes, seed: int) -> int: ...  # refcounted def mage_describe(code: int) -> str: ...  # refcounted || (func $checksum (param i32 i32 i32) (result i32)) ;; caller frees in memory0 (func $describe (param i32) (result i32 i32)) ;; callee frees in memory0 || wasm arity checksum:2->3 describe:1->1 || wrote 137 bytes"'
    [safe-plugin-host]='"linter start; fs.read -> mode=strict; net.fetch -> 200 OK; fs.write -> approval required for fs.write; proc.spawn -> denied proc.spawn; fs.read -> budget exhausted at 2"'
    [swarm-code-review]='"7 finding(s); parser.mg:12 block x4 | types.mg:41 warn x2; blockers parser.mg:12; blocked; parser.mg=5 types.mg=2"'
)

fail=0
checked=0
check_failed=()
eval_failed=()
output_changed=()
unrecorded=()

if [ "$PRINT" = "1" ]; then
    echo "declare -A EXPECTED=("
fi

for dir in examples/*/; do
    src="$dir/src/main.mg"
    [ -f "$src" ] || continue
    name="$(basename "$dir")"
    checked=$((checked + 1))

    # Capture first, then match. `"$MP" --check … | grep -q` under
    # `set -o pipefail` reports failure even when grep *succeeds*: grep -q exits
    # at the first match, mage-parse takes SIGPIPE, and pipefail surfaces that
    # as the pipeline's status. Every broken example then read as passing.
    #
    # `scripts/purge-video-from-history.sh` carries a comment about this exact
    # trap, written after hitting it twice. Knowing about a footgun evidently
    # does not stop one standing on it.
    out="$("$MP" --check "$src" 2>&1)"
    if printf '%s' "$out" | grep -q "error:"; then
        check_failed+=("$name")
        continue
    fi

    got="$("$MP" --eval "$src" main 2>&1 | tail -1)"

    if [ "$PRINT" = "1" ]; then
        printf "    [%s]='%s'\n" "$name" "$got"
        continue
    fi

    # An eval error is printed to stdout like any other line, so the exit status
    # cannot be trusted to distinguish "ran" from "died".
    case "$got" in
        *"eval error"*|*"Error reading"*) eval_failed+=("$name: $got"); continue ;;
    esac

    want="${EXPECTED[$name]:-}"
    if [ -z "$want" ]; then
        unrecorded+=("$name")
    elif [ "$got" != "$want" ]; then
        output_changed+=("$name")
        printf '  %s\n    want: %s\n    got:  %s\n' "$name" "$want" "$got" >&2
    fi
done

if [ "$PRINT" = "1" ]; then
    echo ")"
    exit 0
fi

echo "Checked $checked examples against ${#EXPECTED[@]} recorded answers."

if [ "${#check_failed[@]}" -gt 0 ]; then
    echo "These no longer typecheck:" >&2
    printf '  %s\n' "${check_failed[@]}" >&2
    fail=1
fi

if [ "${#eval_failed[@]}" -gt 0 ]; then
    echo "These typecheck but do not run:" >&2
    printf '  %s\n' "${eval_failed[@]}" >&2
    fail=1
fi

if [ "${#output_changed[@]}" -gt 0 ]; then
    echo "These print something other than the recorded answer (see above)." >&2
    echo "Decide which is correct before regenerating with --print." >&2
    fail=1
fi

if [ "${#unrecorded[@]}" -gt 0 ]; then
    echo "These examples have no recorded answer — add one to EXPECTED:" >&2
    printf '  %s\n' "${unrecorded[@]}" >&2
    fail=1
fi

[ "$fail" -eq 0 ] && echo "OK — all $checked examples typecheck, run, and print what they should."
exit "$fail"

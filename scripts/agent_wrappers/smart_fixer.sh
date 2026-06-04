#!/usr/bin/env bash
# smart_fixer.sh - deterministic Stage-3 refine wrapper.
#
# Acts as an "LLM in the loop" without an actual LLM. Tries the inverse
# of every perturbation the bench applies (drop last `;`, drop last `}`,
# duplicate `;`, swap let<->mut, etc.) and returns the first candidate
# that parses cleanly via `MechGen-parse --check`.
#
# Protocol (per scripts/agent_wrappers/README.md):
#   propose mode (RDX_BENCH_MODE=propose) : stdin = task description.
#     We can't synthesize MechGen from natural language - emit a stub
#     that the mechanical recovery pipeline rejects so the bench
#     advances straight to refine.
#   refine  mode (RDX_BENCH_MODE=refine)  : stdin = broken source,
#     RDX_PARSE_ERROR = parse-error message. Try reverse-mutations,
#     return first that parses, fall back to broken source.

set -uo pipefail

MODE="${RDX_BENCH_MODE:-propose}"
PARSE_ERR="${RDX_PARSE_ERROR:-}"
# MechGen-parse binary location (release build).
MGP="prototype/target/release/MechGen-parse.exe"
if [ ! -x "$MGP" ]; then
    MGP="prototype/target/release/MechGen-parse"
fi

INPUT=$(cat)

# Helper: does the given source parse cleanly?
parses_ok() {
    local tmp
    tmp=$(mktemp --suffix=.mg)
    printf '%s' "$1" > "$tmp"
    local out
    out=$("$MGP" --check "$tmp" 2>&1)
    rm -f "$tmp"
    echo "$out" | grep -q "parse error" && return 1
    echo "$out" | grep -q "Lex error" && return 1
    return 0
}

if [ "$MODE" = "propose" ]; then
    # Stub - any parseable source the mechanical pipeline accepts but
    # that won't match the task. Bench will fail propose, fall through
    # to refine where we do the actual work.
    printf '+f stub() -> i32 { 0 }\n'
    exit 0
fi

# Refine mode. Try a sequence of candidate fixes.
try_candidate() {
    local cand="$1"
    if parses_ok "$cand"; then
        printf '%s' "$cand"
        exit 0
    fi
}

# 1. Append `;` (drop-last-semi reversal).
try_candidate "${INPUT};"

# 2. Append `}` (drop-last-brace reversal). Multi-brace via loop.
candidate="$INPUT"
for _ in 1 2 3 4; do
    candidate="${candidate}
}"
    try_candidate "$candidate"
done

# 3. Append `)` (drop-last-paren reversal).
try_candidate "${INPUT})"
try_candidate "${INPUT}))"

# 4. Append `;` then `}` (combined drop).
try_candidate "${INPUT};
}"

# 5. Swap let<->mut back. Try both directions.
swapped=$(printf '%s' "$INPUT" | sed -e 's/ let / TMP_MUT_SWAP /g' -e 's/ mut / let /g' -e 's/ TMP_MUT_SWAP / mut /g')
try_candidate "$swapped"

# 6. Delete the first stray `,` right after `{`. Common shape: `{ , item }`.
no_stray=$(printf '%s' "$INPUT" | sed -e 's/{[[:space:]]*,/{/' )
try_candidate "$no_stray"

# 7. Collapse duplicate `;;` to single `;`.
collapsed=$(printf '%s' "$INPUT" | sed -e 's/;;/;/g')
try_candidate "$collapsed"

# 8. Truncated source: try appending `()` placeholder + balanced closes.
opens=$(printf '%s' "$INPUT" | tr -cd '{(' | wc -c | tr -d ' ')
closes=$(printf '%s' "$INPUT" | tr -cd '})' | wc -c | tr -d ' ')
diff=$((opens - closes))
if [ "$diff" -gt 0 ]; then
    suffix=""
    for _ in $(seq 1 "$diff"); do
        suffix="${suffix}}"
    done
    try_candidate "${INPUT}()${suffix}"
    try_candidate "${INPUT}${suffix}"
fi

# 9. Combined: collapse `;;`, swap let/mut, append closers.
combo=$(printf '%s' "$INPUT" | sed -e 's/;;/;/g')
if [ "$diff" -gt 0 ]; then
    suffix=""
    for _ in $(seq 1 "$diff"); do
        suffix="${suffix}}"
    done
    try_candidate "${combo}${suffix}"
fi

# Give up - emit broken source unchanged. Bench records refine_succeeded=false.
printf '%s' "$INPUT"

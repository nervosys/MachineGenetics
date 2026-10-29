#!/usr/bin/env bash
# Run the arena over several seeds and config variants, in parallel, and leave
# one JSON report per (variant, seed) pair for `arena measure` to read (plan M3).
#
# Usage:
#   scripts/arena-seeds.sh --out DIR --seeds 1-6 --jobs 4 \
#       --variant shape="--novelty shape" --variant behaviour="--novelty behaviour" \
#       -- --heldout README.md,germline/src/gate.rs --rounds 6 --agents grammar:1,mutator:1,attacker:1
#
#   arena measure DIR        # loss-to-loss fits and compute multipliers (ARENA.md)
#
# --seeds takes ranges and lists: 1-6, 1,3,5, or 1-3,7.
# --variant name="<extra args>" may repeat. Its words are split on spaces, so an
#   argument with a space in it cannot be passed.
# Everything after `--` goes to every run. The runner sets --seed and --json, so
# those must not appear there.
#
# Each run writes DIR/<variant>-<seed>.json and DIR/<variant>-<seed>.log. A run
# whose JSON already exists is skipped, so a rerun resumes.
#
# Every run is `--cost fuel` unless its arguments already set `--cost`. Under
# fuel, two runs of one seed differ only in energy and time, so held-out and
# proxy bits/byte are deterministic (ARENA.md, "The held-out comparison").
#
# The release binary is built once, into CARGO_TARGET_DIR when it is set.

set -euo pipefail

usage() {
    sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
}

die() {
    echo "arena-seeds: $*" >&2
    exit 2
}

OUT=""
SEEDS=""
JOBS=1
VARIANT_NAMES=()
VARIANT_ARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --out) [[ $# -ge 2 ]] || die "--out needs a directory"; OUT="$2"; shift 2 ;;
        --seeds) [[ $# -ge 2 ]] || die "--seeds needs a value"; SEEDS="$2"; shift 2 ;;
        --jobs) [[ $# -ge 2 ]] || die "--jobs needs a value"; JOBS="$2"; shift 2 ;;
        --variant)
            [[ $# -ge 2 ]] || die "--variant needs name=\"<args>\""
            spec="$2"
            shift 2
            [[ "$spec" == *=* ]] || die "--variant wants name=\"<args>\", got: $spec"
            name="${spec%%=*}"
            [[ "$name" =~ ^[A-Za-z0-9_-]+$ ]] || die "variant name '$name' must be letters, digits, _ or -"
            VARIANT_NAMES+=("$name")
            VARIANT_ARGS+=("${spec#*=}")
            ;;
        --) shift; break ;;
        -h|--help) usage; exit 0 ;;
        *) die "unknown option '$1' (arena arguments go after --)" ;;
    esac
done
ARENA_ARGS=("$@")

[[ -n "$OUT" ]] || die "--out DIR is required"
[[ -n "$SEEDS" ]] || die "--seeds is required, e.g. 1-6"
[[ "$JOBS" =~ ^[1-9][0-9]*$ ]] || die "--jobs must be a positive integer, got '$JOBS'"
[[ ${#VARIANT_NAMES[@]} -ge 1 ]] || die "at least one --variant name=\"<args>\" is required"

for a in "${ARENA_ARGS[@]}"; do
    case "$a" in
        --seed|--seed=*|--json|--json=*) die "$a is set by the runner, not by the arena arguments" ;;
    esac
done

# Expand the seed spec into one seed per line.
SEED_LIST=()
IFS=',' read -r -a SEED_PARTS <<< "$SEEDS"
for part in "${SEED_PARTS[@]}"; do
    if [[ "$part" =~ ^([0-9]+)-([0-9]+)$ ]]; then
        lo=${BASH_REMATCH[1]}
        hi=${BASH_REMATCH[2]}
        (( lo <= hi )) || die "seed range '$part' runs backwards"
        for ((s = lo; s <= hi; s++)); do SEED_LIST+=("$s"); done
    elif [[ "$part" =~ ^[0-9]+$ ]]; then
        SEED_LIST+=("$part")
    else
        die "cannot read seed '$part'; use 1-6, 1,3,5, or 1-3,7"
    fi
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$OUT"

# Build once. A workspace of its own, so it builds from arena/.
echo "arena-seeds: building the release arena binary (log: $OUT/build.log)"
(cd "$ROOT/arena" && cargo build --release --bin arena) > "$OUT/build.log" 2>&1 \
    || die "the build failed; see $OUT/build.log"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/arena/target}"
BIN="$TARGET_DIR/release/arena"
[[ -x "$BIN.exe" ]] && BIN="$BIN.exe"
[[ -x "$BIN" ]] || die "no binary at $BIN after the build"

has_cost() {
    local a
    for a in "$@"; do
        case "$a" in
            --cost|--cost=*) return 0 ;;
        esac
    done
    return 1
}

run_job() {
    # $1 json path, $2 log path, $3 seed, then the full argument list.
    local json="$1" log="$2" seed="$3"
    shift 3
    "$BIN" --seed "$seed" --json "$json.partial" "$@" > "$log" 2>&1 \
        && mv -f "$json.partial" "$json"
}

LAUNCHED=()
SKIPPED=0
FUEL_ADDED=0
for vi in "${!VARIANT_NAMES[@]}"; do
    vname="${VARIANT_NAMES[$vi]}"
    read -r -a VARG <<< "${VARIANT_ARGS[$vi]}" || true
    for seed in "${SEED_LIST[@]}"; do
        json="$OUT/$vname-$seed.json"
        log="$OUT/$vname-$seed.log"
        if [[ -s "$json" ]]; then
            SKIPPED=$((SKIPPED + 1))
            continue
        fi
        job_args=("${ARENA_ARGS[@]}" "${VARG[@]}")
        if ! has_cost "${job_args[@]}"; then
            job_args+=(--cost fuel)
            FUEL_ADDED=1
        fi
        while (( $(jobs -rp | wc -l) >= JOBS )); do
            wait -n || true
        done
        run_job "$json" "$log" "$seed" "${job_args[@]}" &
        LAUNCHED+=("$json")
    done
done
wait || true

if [[ $FUEL_ADDED -eq 1 ]]; then
    echo "arena-seeds: note: --cost fuel added to the runs that did not set --cost (held-out bits/byte are deterministic under fuel)"
fi

FAILED=0
if [[ ${#LAUNCHED[@]} -gt 0 ]]; then
    for json in "${LAUNCHED[@]}"; do
        if [[ ! -s "$json" ]]; then
            FAILED=$((FAILED + 1))
            echo "arena-seeds: FAILED $(basename "$json") (see ${json%.json}.log)" >&2
        fi
    done
fi
echo "arena-seeds: ${#LAUNCHED[@]} run, $SKIPPED skipped (already in $OUT), $FAILED failed, $OUT"
[[ $FAILED -eq 0 ]] || exit 1

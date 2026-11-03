#!/usr/bin/env bash
# The knowledge-base gate (NEUROSYMBOLIC_PLAN.md, Phase 8): admit or refuse a
# proposal to change `kb/*.mg`.
#
#   scripts/kb-gate.sh <proposal.json>
#
# A proposal is admitted when it passes, in order:
#
#   kb_base, kb_author, kb_edit, kb_schema
#       `kb-gate admit` (prototype/src/kb_proposal.rs): the proposal applies to
#       the knowledge base at HEAD, and the changed files load, which checks
#       the schema and every invariant.
#   kb_conformance
#       The prototype and arena test suites pass against the changed files.
#       The knowledge base is compiled in (`include_str!`), so the suites run
#       in a worktree of HEAD with the changed files written into it: what is
#       tested is what would be committed.
#   kb_cost
#       A fixed arena run (`--cost fuel`, so it is deterministic) spends no
#       more fuel under the changed knowledge base than under HEAD's.
#
# Each refusal is a line `refused <rule>: <why>`, the rule a `GateRule` in
# kb/rules/refusals.mg. Exit 0 admitted, 2 refused, 1 the gate itself failed.
# The gate stops at the first stage that refuses.
#
# The worktree and its build directory are temporary unless KB_GATE_TREE and
# KB_GATE_TARGET name them; a build of both crates needs some gigabytes.
# Nothing is committed: an admitted proposal is applied by whoever commits it.

set -u

proposal=${1:?usage: scripts/kb-gate.sh <proposal.json>}
proposal=$(cd "$(dirname "$proposal")" && pwd)/$(basename "$proposal")
root=$(git rev-parse --show-toplevel) || exit 1
tree=${KB_GATE_TREE:-$(mktemp -d)/tree}
export CARGO_TARGET_DIR=${KB_GATE_TARGET:-$tree-target}
logs=$tree-logs
mkdir -p "$logs"

cleanup() { git -C "$root" worktree remove --force "$tree" >/dev/null 2>&1; }
trap cleanup EXIT

say() { echo "kb-gate: $*" >&2; }
fail() { say "$*"; exit 1; }

# The fixed benchmark, and the fuel it spent: the sum over its agents.
bench() {
    "$CARGO_TARGET_DIR/debug/arena" --heldout README.md,germline/src/gate.rs \
        --rounds 4 --programs 16 --agents grammar:1,uniform:1,mutator:1 \
        --cost fuel --meter-gpus none --seed 1 --json "$logs/bench-$1.json" \
        > "$logs/bench-$1.log" 2>&1 || fail "the benchmark did not run ($logs/bench-$1.log)"
    local report=$logs/bench-$1.json
    command -v cygpath >/dev/null && report=$(cygpath -m "$report")
    python -c "import json,sys; print(sum(a['fuel'] for a in json.load(open(sys.argv[1]))['agents']))" \
        "$report" || fail "the benchmark report has no fuel"
}

say "worktree of $(git -C "$root" rev-parse --short HEAD) at $tree"
git -C "$root" worktree add --detach "$tree" HEAD >/dev/null 2>&1 || fail "cannot make a worktree at $tree"
cd "$tree" || exit 1

say "building kb-gate and arena at HEAD"
cargo build --manifest-path prototype/Cargo.toml --bin kb-gate > "$logs/build-base.log" 2>&1 \
    || fail "kb-gate does not build ($logs/build-base.log)"
cargo build --manifest-path arena/Cargo.toml > "$logs/build-arena-base.log" 2>&1 \
    || fail "arena does not build at HEAD ($logs/build-arena-base.log)"

say "benchmark at HEAD"
base_fuel=$(bench base) || exit 1
# kb_base, kb_author, kb_edit, kb_schema: the changed files are written into the worktree.
"$CARGO_TARGET_DIR/debug/kb-gate" admit "$proposal" "$tree"
case $? in
    0) ;;
    2) exit 2 ;;
    *) fail "kb-gate admit failed" ;;
esac

say "conformance: the prototype and arena suites against the changed files"
refused=0
for crate in prototype arena; do
    if ! cargo test --manifest-path $crate/Cargo.toml > "$logs/test-$crate.log" 2>&1; then
        names=$(grep -E '^test .* \.\.\. FAILED$' "$logs/test-$crate.log" | sed -E 's/^test (.*) \.\.\. FAILED$/\1/')
        [ -n "$names" ] || names="the $crate suite does not build or run ($logs/test-$crate.log)"
        while IFS= read -r n; do echo "refused kb_conformance: $crate: $n"; done <<< "$names"
        refused=1
    fi
done
[ $refused = 0 ] || exit 2

say "benchmark under the changed knowledge base"
cargo build --manifest-path arena/Cargo.toml > "$logs/build-arena-new.log" 2>&1 \
    || fail "arena does not build under the change ($logs/build-arena-new.log)"
new_fuel=$(bench new) || exit 1
echo "benchmark fuel: $base_fuel at HEAD, $new_fuel changed"
if [ "$new_fuel" -gt "$base_fuel" ]; then
    echo "refused kb_cost: the benchmark spent $new_fuel fuel, $base_fuel at HEAD"
    exit 2
fi
echo "admitted: $(git -C "$tree" diff --stat -- kb | tail -1)"
say "logs in $logs"

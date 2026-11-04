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
#       A fixed arena benchmark (`--cost fuel`, so it is deterministic) spends
#       no more fuel under the changed knowledge base than under HEAD's: it is
#       refused by a one-sided sign test over the seeds, at p < 0.05, which on
#       eight seeds is a rise on at least seven. A seed that ties counts for
#       nothing, so a change that leaves sampling alone passes.
#       One seed cannot decide this: a change to what is sampled reshuffles the
#       draws, and with nothing changed the fuel of these runs differs by half
#       between seeds. Nor can the mean: fuel has a heavy tail, and one seed at
#       66,668 against a usual 25,000 hid a rise on seven of eight.
#       All the runs append to one evidence log, each opening with the version
#       of the knowledge base it ran under. KB_GATE_SEEDS replaces the seeds, to
#       check a verdict on seeds it has not seen.
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

seeds=${KB_GATE_SEEDS:-1 2 3 4 5 6 7 8}

# The fixed benchmark, one run per seed.
bench() {
    for seed in $seeds; do
        "$CARGO_TARGET_DIR/debug/arena" --heldout README.md,germline/src/gate.rs \
            --rounds 4 --programs 16 --agents grammar:1,uniform:1,mutator:1 \
            --cost fuel --meter-gpus none --seed "$seed" --json "$logs/bench-$1-$seed.json" --evidence "$logs/bench.jsonl" \
            > "$logs/bench-$1-$seed.log" 2>&1 || fail "the benchmark did not run ($logs/bench-$1-$seed.log)"
    done
}

# Compare the two benchmarks' fuel, the sum over each run's agents, seed by seed.
# Prints the comparison; exits 3 when a one-sided sign test finds a rise at p < 0.05.
compare_fuel() {
    local dir=$logs
    command -v cygpath >/dev/null && dir=$(cygpath -m "$dir")
    python -c '
import json, math, sys
d, seeds = sys.argv[1], sys.argv[2].split()
fuel = lambda arm, s: sum(a["fuel"] for a in json.load(open(f"{d}/bench-{arm}-{s}.json"))["agents"])
base, new = [fuel("base", s) for s in seeds], [fuel("new", s) for s in seeds]
rise = [b - a for a, b in zip(base, new)]
up, down = sum(r > 0 for r in rise), sum(r < 0 for r in rise)
n = up + down
p = sum(math.comb(n, k) for k in range(up, n + 1)) / 2 ** n if n else 1.0
median = sorted(rise)[len(rise) // 2]
print(f"benchmark fuel over {len(rise)} seeds: rose on {up}, fell on {down}, sign test p = {p:.3f}; upper median rise {median:+d}; rises {rise}")
sys.exit(3 if p < 0.05 else 0)
' "$dir" "$seeds"
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
bench base
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
bench new
comparison=$(compare_fuel)
case $? in
    0) echo "$comparison" ;;
    3) echo "$comparison"
       echo "refused kb_cost: fuel rose across the seeds, by a sign test at p < 0.05"
       exit 2 ;;
    *) fail "the benchmark reports have no fuel" ;;
esac
echo "admitted: $(git -C "$tree" diff --stat -- kb | tail -1)"
say "logs in $logs; evidence in $logs/bench.jsonl"

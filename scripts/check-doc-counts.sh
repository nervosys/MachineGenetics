#!/usr/bin/env bash
# Verify that every documented test count matches what the suites actually report.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# On 2026-08-04 four separate documented figures turned out to be wrong, each
# found by accident while doing something else:
#
#   * forge "235 tests" — accurate immediately before step 144, then never
#     updated through three steps that added 36 tests between them.
#   * GERMLINE.md "141 tests" — never matched the files it described; the real
#     number was 100 when it was written, and what it counted is unrecoverable.
#   * ARCHITECTURE.md "prototype 1,209" — the known over-count, corrected in
#     MEASUREMENTS.md months earlier and left stale in the layout table.
#   * MEASUREMENTS.md "forge 235 pass (ribosome 86, germline 141)" — two crate
#     splits out of date.
#
# The pattern is not carelessness about any one number. It is that a summary
# line is written once, while its subject is fresh, and nothing ever forces it
# to be looked at again. Docs asserting measured facts decay silently — which is
# exactly what DOCS.md says this repository refuses to tolerate ("when a design
# document and a measurement disagree, the measurement wins") while providing no
# way to notice the disagreement.
#
# This is that mechanism, and it is deliberately dumb: an explicit table of
# (file, pattern, crate) assertions checked against numbers the test run just
# produced. A pattern that stops matching is a FAILURE, not a skip — a claim
# that can no longer be located is a claim nobody is checking, and silently
# passing would recreate the exact problem this guards against.
#
# Usage:
#     scripts/test-all.sh --check-docs        # measure, then verify
#     printf 'ribosome=162\ntotal=2744\n' | scripts/check-doc-counts.sh
#
# Reads `crate=count` lines on stdin, plus `total=count`.

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO"

declare -A ACTUAL
# Counted explicitly rather than with `${#ACTUAL[@]}`: under `set -u` an
# associative array that has never been assigned is *unbound*, so the length
# expansion below aborted the line instead of yielding 0. The guard it was
# supposed to arm therefore never fired — running this script with no stdin
# printed "documentation disagrees with measurement" for all 74 claims when
# what actually happened was that nothing had been measured. A counter is
# assigned before the loop and so is always bound.
supplied=0
while IFS='=' read -r k v; do
    # Strip CR: this is invoked from PowerShell as well as bash, and a piped
    # string from PowerShell arrives CRLF-terminated. Without this the last
    # value carries a trailing \r and the comparison fails while reporting
    # "documented as 2744, measured 2744" — a mismatch of invisible bytes.
    k="${k//$'\r'/}"
    v="${v//$'\r'/}"
    if [ -n "${k:-}" ]; then
        ACTUAL["$k"]="$v"
        supplied=$((supplied + 1))
    fi
done

if [ "$supplied" -eq 0 ]; then
    echo "check-doc-counts: no counts on stdin; run via scripts/test-all.sh --check-docs" >&2
    exit 2
fi

# Fields are TAB-separated. Not `|`, because half these patterns match markdown
# table rows and are full of pipes — the first version of this script split on
# `|` and silently mangled every one of them.
#
#   file <TAB> extended-regex isolating the claim <TAB> crate key
#
# Each regex must match the number *and* enough context to prove it is a claim
# about that crate. A bare number would pass on any coincidence in the file.
#
# No comment lines inside the block: the reader below splits every line on tabs
# and would take a `#` line as a filename.
#
# The last HANDOFF entry is the right-hand end of the "1,066 → 1,091" range in
# the example-rewrite section. The left end is history and stays put, so the
# pattern deliberately starts *after* the arrow — `digits()` strips non-digits
# from the whole match, and a pattern spanning both numbers would compare
# against "1,0661,091".
CHECKS=$(cat <<'EOF'
scripts/test-all.sh	#     rmi \(cpu\) +[0-9,]+ tests	rmi
scripts/test-all.sh	#     prototype +[0-9,]+ tests	prototype
scripts/test-all.sh	#     ribosome +[0-9,]+ tests	ribosome
scripts/test-all.sh	#     germline +[0-9,]+ tests	germline
scripts/test-all.sh	#     forge +[0-9,]+ tests	forge
scripts/test-all.sh	#     total +[0-9,]+ tests	total
scripts/test-all.ps1	rmi \(cpu\) +[0-9,]+ tests	rmi
scripts/test-all.ps1	prototype +[0-9,]+ tests	prototype
scripts/test-all.ps1	ribosome +[0-9,]+ tests	ribosome
scripts/test-all.ps1	germline +[0-9,]+ tests	germline
scripts/test-all.ps1	forge +[0-9,]+ tests	forge
scripts/test-all.ps1	total +[0-9,]+ tests	total
ARCHITECTURE.md	\(\*\*[0-9,]+ tests\*\* green\)	prototype
ARCHITECTURE.md	`rmi` \| [0-9,]+ 	rmi
ARCHITECTURE.md	`mage-prototype` \| [0-9,]+ 	prototype
ARCHITECTURE.md	`ribosome` \| [0-9,]+ 	ribosome
ARCHITECTURE.md	`germline` \| [0-9,]+ 	germline
ARCHITECTURE.md	`forge` \| [0-9,]+ 	forge
MEASUREMENTS.md	MAGE prototype \| \*\*[0-9,]+ pass	prototype
MEASUREMENTS.md	ribosome \(build engine\) \| \*\*[0-9,]+ pass	ribosome
MEASUREMENTS.md	germline \(RSI control plane\) \| \*\*[0-9,]+ pass	germline
MEASUREMENTS.md	forge \(registry\) \| \*\*[0-9,]+ pass	forge
RIBOSOME.md	its own crate, [0-9,]+ tests	ribosome
RIBOSOME.md	ribosome/Cargo.toml +# [0-9,]+ tests	ribosome
GERMLINE.md	its own crate, [0-9,]+ tests	germline
GERMLINE.md	germline/Cargo.toml +# [0-9,]+ tests	germline
HANDOFF.md	\*\*[0-9,]+\*\* — rmi	total
HANDOFF.md	rmi [0-9,]+ ·	rmi
HANDOFF.md	prototype [0-9,]+ ·	prototype
HANDOFF.md	ribosome [0-9,]+ ·	ribosome
HANDOFF.md	germline [0-9,]+ ·	germline
HANDOFF.md	forge [0-9,]+ \|	forge
HANDOFF.md	→ [0-9,]+\*\*, all green	prototype
HANDOFF.md	\| CI \| [0-9,]+ jobs	ci_jobs
HANDOFF.md	\| [0-9,]+ checked	mg_checked
HANDOFF.md	[0-9,]+ listed sketches	mg_sketches
HANDOFF.md	[0-9,]+ MAGE blocks typecheck	doc_blocks
HANDOFF.md	[0-9,]+ documentation entry points	doc_evals
HANDOFF.md	file-oracle parse [0-9]+/	floor_parse
HANDOFF.md	pattern-heal [0-9]+,	floor_heal
benchmarks/STATUS.md	├─ sigils \([0-9]+\)	onto_sigils
benchmarks/STATUS.md	├─ keywords \([0-9]+\)	onto_keywords
benchmarks/STATUS.md	├─ types \([0-9]+\)	onto_types
benchmarks/STATUS.md	├─ ast_kinds \([0-9]+\)	onto_ast_kinds
benchmarks/STATUS.md	├─ ir_ops \([0-9]+\)	onto_ir_ops
benchmarks/STATUS.md	├─ op_families \([0-9]+\)	onto_op_families
benchmarks/STATUS.md	├─ layer_map \([0-9]+\)	onto_layer_map
benchmarks/STATUS.md	├─ rap_methods \([0-9]+\)	onto_rap_methods
benchmarks/STATUS.md	├─ heal_patterns \([0-9]+\)	onto_heal_patterns
benchmarks/STATUS.md	├─ recovery_stages \([0-9]+\)	onto_recovery_stages
benchmarks/STATUS.md	├─ examples \([0-9]+\)	onto_examples
benchmarks/STATUS.md	├─ framewerx_modules \([0-9]+\)	onto_framewerx_modules
benchmarks/STATUS.md	├─ cli_flags \([0-9]+\)	onto_cli_flags
benchmarks/STATUS.md	├─ bench_backends \([0-9]+\)	onto_bench_backends
benchmarks/STATUS.md	├─ effects \([0-9]+\)	onto_effects
benchmarks/STATUS.md	├─ wrapper_protocol \([0-9]+\)	onto_wrapper_protocol
benchmarks/STATUS.md	├─ project_layout \([0-9]+\)	onto_project_layout
benchmarks/STATUS.md	├─ ci_floors \([0-9]+\)	onto_ci_floors
benchmarks/STATUS.md	Ontology \([0-9]+ sections	onto_sections
UNIFICATION.md	in \*\*[0-9]+ sections\*\*	onto_sections
UNIFICATION.md	cli_flags \([0-9]+\)	onto_cli_flags
UNIFICATION.md	effects \([0-9]+\)	onto_effects
UNIFICATION.md	wrapper_protocol \([0-9]+\)	onto_wrapper_protocol
UNIFICATION.md	ci_floors \([0-9]+\)	onto_ci_floors
ARCHITECTURE.md	\*\*[0-9]+ crates transitively\*\*	ribosome_deps
RIBOSOME.md	— [0-9]+ crates transitively	ribosome_deps
EOF
)

# Checked only when a CUDA run supplied the number, since it takes a GPU to
# measure. Kept in the same mechanism rather than a second one: the documented
# CUDA figure went stale for exactly the same reason as the others — it was
# written once, before the library split, and never looked at again.
#
# Each pattern must contain **exactly one** number. `digits()` strips every
# non-digit from the whole match, so a pattern spanning a second number silently
# concatenates them: `dual 3090 Ti locally, 1,071 tests` became "30901071", and
# `1,071 passing, 0 failed` became "10710". Both looked like wildly wrong counts
# rather than a broken pattern, which is the confusing way for this to fail.
CUDA_CHECKS=$(cat <<'EOF'
scripts/test-all.sh	\([0-9,]+ tests; needs	cuda
scripts/test-all.ps1	\([0-9,]+ tests\)\. Needs	cuda
ARCHITECTURE.md	cuda \([0-9,]+ tests\)	cuda
ARCHITECTURE.md	hardware — [0-9,]+ tests on dual	cuda
MEASUREMENTS.md	hardware: \*\*[0-9,]+ passing	cuda
.github/workflows/ci.yml	locally, [0-9,]+ tests green	cuda
HANDOFF.md	\*\*[0-9,]+ passing\*\* on dual	cuda
EOF
)

# Checked only when a --bench run supplied the number. `eval_bench` prints
# "correctness: N/N programs exact"; the claim appears in four documents and had
# never been verified against a run until 2026-08-05, when it turned out to be
# right — which is not a reason to leave it unchecked, since "73/73" being
# correct today says nothing about the next time the corpus changes.
BENCH_CHECKS=$(cat <<'EOF'
ARCHITECTURE.md	eval_bench \([0-9]+/	eval_exact
README.md	computes \*\*[0-9]+/	eval_exact
DOCS.md	eval\.rs`, [0-9]+/	eval_exact
DIRECT_CODEGEN_STRATEGY.md	`--eval`, [0-9]+/	eval_exact
MEASUREMENTS.md	`lex [0-9]+/	rb_lex
MEASUREMENTS.md	· parse [0-9]+/	rb_parse
MEASUREMENTS.md	· effective [0-9]+/	rb_effective
EOF
)

# Prose summaries name every crate in one sentence — "prototype **1,038**, rmi
# **1,380**, …" — and markdown wraps them across lines with `> ` prefixes, so
# these are matched against a flattened copy of the file rather than line by
# line. The first version missed two claims purely because of that wrapping.
PROSE_FILES="ROADMAP.md MEASUREMENTS.md"

fail=0
checked=0
# A claim whose crate was never measured is a different failure from a claim
# that disagrees with its measurement, and the verdict has to say which —
# telling someone their docs are wrong when the run simply skipped a crate
# sends them to edit numbers that were correct.
missing=0

digits() { printf '%s' "$1" | tr -cd '0-9'; }

flatten() { sed 's/^> //' "$1" | tr '\n' ' '; }

compare() {
    local where="$1" key="$2" got="$3"
    local want="${ACTUAL[$key]:-}"
    if [ -z "$want" ]; then
        echo "  ?  $where: no measured value for '$key'" >&2
        missing=$((missing + 1))
        return
    fi
    checked=$((checked + 1))
    if [ "$got" != "$want" ]; then
        # Quoted, so a mismatch of invisible characters is visible rather than
        # reading as "2744 does not equal 2744".
        echo "  x  $where: '$key' documented as '$got', measured '$want'" >&2
        fail=1
    fi
}

echo "Checking documented test counts against this run..."

while IFS=$'\t' read -r file pattern key; do
    [ -n "${file:-}" ] || continue
    if [ ! -f "$file" ]; then
        echo "  !  missing file: $file" >&2
        fail=1
        continue
    fi
    hit="$(grep -oE -- "$pattern" "$file" | head -1)"
    if [ -z "$hit" ]; then
        echo "  !  $file: pattern for '$key' no longer matches - the claim moved or was reworded" >&2
        fail=1
        continue
    fi
    compare "$file" "$key" "$(digits "$hit")"
done <<< "$CHECKS"

if [ -n "${ACTUAL[cuda]:-}" ]; then
    while IFS=$'\t' read -r file pattern key; do
        [ -n "${file:-}" ] || continue
        if [ ! -f "$file" ]; then
            echo "  !  missing file: $file" >&2
            fail=1
            continue
        fi
        hit="$(grep -oE -- "$pattern" "$file" | head -1)"
        if [ -z "$hit" ]; then
            echo "  !  $file: CUDA count claim no longer matches - reworded?" >&2
            fail=1
            continue
        fi
        compare "$file (cuda)" "$key" "$(digits "$hit")"
    done <<< "$CUDA_CHECKS"
else
    echo "  -  CUDA counts not checked (no --cuda run supplied one)"
fi

if [ -n "${ACTUAL[eval_exact]:-}" ] || [ -n "${ACTUAL[rb_lex]:-}" ]; then
    while IFS=$'\t' read -r file pattern key; do
        [ -n "${file:-}" ] || continue
        if [ ! -f "$file" ]; then
            echo "  !  missing file: $file" >&2
            fail=1
            continue
        fi
        hit="$(grep -oE -- "$pattern" "$file" | head -1)"
        if [ -z "$hit" ]; then
            echo "  !  $file: bench-harness claim no longer matches - reworded?" >&2
            fail=1
            continue
        fi
        compare "$file (bench)" "$key" "$(digits "$hit")"
    done <<< "$BENCH_CHECKS"
else
    echo "  -  bench harnesses not checked (no --bench run supplied results)"
fi

for file in $PROSE_FILES; do
    if [ ! -f "$file" ]; then
        echo "  !  missing file: $file" >&2
        fail=1
        continue
    fi
    flat="$(flatten "$file")"
    for key in rmi prototype ribosome germline forge; do
        hit="$(printf '%s' "$flat" | grep -oE "$key \*\*[0-9,]+\*\*" | head -1)"
        if [ -z "$hit" ]; then
            echo "  !  $file: no '$key **N**' claim found - reworded?" >&2
            fail=1
            continue
        fi
        compare "$file" "$key" "$(digits "${hit#"$key"}")"
    done
    hit="$(printf '%s' "$flat" | grep -oE "\*\*[0-9,]+ (tests|passing)" | head -1)"
    if [ -z "$hit" ]; then
        echo "  !  $file: no total claim found" >&2
        fail=1
    else
        compare "$file (total)" total "$(digits "$hit")"
    fi
done

echo
if [ "$fail" -ne 0 ]; then
    echo "FAILED - documentation disagrees with measurement." >&2
    echo "The measurement wins; update the docs (see DOCS.md)." >&2
    exit 1
fi
if [ "$missing" -ne 0 ]; then
    # Still a failure — an unchecked claim is an unguarded claim — but not the
    # same one, and the remedy is to run the suite, not to edit the docs.
    echo "INCOMPLETE - $missing documented count(s) had nothing to compare against;" >&2
    echo "$checked matched. Run scripts/test-all.sh --check-docs so every crate is measured." >&2
    exit 1
fi
echo "OK - $checked documented counts match the measured run."

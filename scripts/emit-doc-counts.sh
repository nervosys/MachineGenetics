#!/usr/bin/env bash
# Measure every documented count that is not a crate's test total.
#
# **Why this is its own file.** `test-all.sh` and `test-all.ps1` both feed
# `check-doc-counts.sh`, and the bash side's comment already states the rule
# they were breaking: "One implementation of the check, in bash, rather than
# two that can drift." That was true of the *check*. The *measurements* were
# the half still duplicated -- and they had drifted all the way apart. The ps1
# emitted six keys (five crate counts plus `total`) against 91 pins, so
# `test-all.ps1 -CheckDocs` reported
#
#   INCOMPLETE - 46 documented count(s) had nothing to compare against
#
# and exited 1. It had done since the bash twin grew `doc_blocks`, `doc_evals`,
# `floor_*`, `onto_*`, `rmi_api_items`, `unsafe_*`, `mg_*` and the benchmark
# figures, and the ps1 grew none of them. CI runs the bash version, so nothing
# noticed. Filed as item 27; this is the fix.
#
# Crate test counts arrive on **stdin** as `key=value` lines, because only the
# harness that ran the suites knows them, and they differ per platform run.
# Everything else is measured here, once. Output is the union, on stdout, in
# the form `check-doc-counts.sh` consumes.
set -o errexit
set -o nounset
set -o pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Pass the caller's crate counts through untouched.
cat

# The CI job count is a claim in HANDOFF.md that nothing checked, so a
# rewrite could — and did — change "10 jobs" to "11" by counting the
# `push`/`pull_request` trigger keys along with the jobs. Measured here
# from the workflow itself: keys indented two spaces *after* `jobs:`.
echo "ci_jobs=$(awk '/^jobs:/{i=1;next} i&&/^  [a-z0-9_-]+:$/{n++} END{print n+0}' "$REPO/.github/workflows/ci.yml")"
# `ribosome`'s transitive dependency count. ARCHITECTURE.md and
# RIBOSOME.md both state it as evidence for the claim that the build
# engine privileges no language, and both said **39** while the measured
# figure was 28 — the number had never been checked, and the two
# documents were wrong together because one was copied from the other.
# Unique crate *names* on normal edges; six crates resolve at two
# versions, so a name-version count is 34, and the documents now say
# which they mean.
# Markdown documents at the repository root. `DOCS.md` opens by
# counting them and then indexes them one by one, so the number and the
# table can disagree — and a document added without an index entry is
# exactly what that opening sentence exists to prevent.
echo "root_docs=$(ls "$REPO"/*.md | grep -c .)"
# The `unsafe` inventory that SECURITY_AUDIT.md §3 publishes. That row
# has been wrong twice, and both times the omitted part was where a
# real defect lived: first it asserted the arena allocator did not
# exist ("1 audited unsafe in lib.rs, 3 in the CUDA FFI shim"), and
# reviewing what was actually there found four memory-safety defects;
# then it counted 9 items where there are 13, omitting the four
# `unsafe impl Send`/`Sync` — which is where the fifth was. A count in
# a security document asserting how much unsafe code exists is the
# last claim that should be typed in by hand.
#
# `grep -c` counts matching *lines*, which is the same unit the
# document quotes, and this is the same expression §3 tells a reader
# to run. Inside `echo` rather than bare, because `grep -c` exits 1 on
# a zero count and `set -e` would kill the script at a bare assignment.
# Documented `rmi/docs` API items. HANDOFF.md states this figure and
# nothing pinned it, so when the criterion tightened on 2026-08-25 —
# from "the name appears in src/" to "a definition exists" — the count
# moved 275 -> 269 with no mechanism to notice. Measured by running the
# checker, so the claim cannot drift from the check that produces it.
echo "rmi_api_items=$(bash "$REPO/scripts/check-rmi-api-doc.sh" 2>/dev/null | awk '/^documented items:/ {print $3}')"
unsafe_re='\bunsafe\s*(\{|fn |impl )'
echo "unsafe_memory_pool=$(grep -cE "$unsafe_re" "$REPO/RecursiveMachineIntelligence/src/runtime/memory_pool.rs")"
echo "unsafe_cuda_full=$(grep -cE "$unsafe_re" "$REPO/RecursiveMachineIntelligence/src/compute/cuda_full.rs")"
echo "unsafe_cuda_backend=$(grep -cE "$unsafe_re" "$REPO/prototype/src/cuda_backend.rs")"
echo "ribosome_deps=$(cargo tree --manifest-path "$REPO/ribosome/Cargo.toml"             -e normal --prefix none 2>/dev/null             | sed 's/ (\*)$//' | awk 'NF{print $1}' | sort -u | grep -c .)"
# `.mg` sources checked vs skipped as sketches. HANDOFF.md states both,
# and both moved this session (96/30 -> 101/25) when `framewerx` was
# rewritten. Measured by running the checker, so the claim cannot drift
# from the list that produces it.
mg_line="$(bash "$REPO/scripts/check-mg-sources.sh" 2>/dev/null | grep -E '^Checked [0-9]+ \.mg')"
echo "mg_checked=$(printf '%s' "$mg_line" | awk '{print $2}')"
# Field 5, not 4: the line reads "Checked 101 .mg files; 25 skipped",
# so `$4` is "files;". The pin caught it on its first run.
echo "mg_sketches=$(printf '%s' "$mg_line" | awk '{print $5}')"
# The three lines below each run a checker to *measure* what the docs
# claim, which makes `--check-docs` a few minutes slower. CI runs the
# same checkers as their own steps; this is the price of the claims
# being measured rather than typed in.
# Documentation entry points actually executed. `--check` and `--eval`
# are independent oracles and the blocks had only ever been checked;
# running them found thirteen registered builtins with no arm in the
# evaluator.
echo "$(bash "$REPO/scripts/check-doc-evals.sh" 2>/dev/null | grep -E '^doc_evals=')"
echo "$(bash "$REPO/scripts/check-doc-blocks.sh" 2>&1 >/dev/null | grep -E '^doc_blocks=')"
# Ontology section sizes, straight from the committed dump (CI proves
# it matches a fresh generation). Two documents quote these counts and
# five of them were stale — `cli_flags (17)` when the binary accepts
# 36, `heal_patterns (~13)` when there are 34, `keywords (12)` when
# there are 102.
# The reliability floors, measured by running the bench. HANDOFF.md
# states them in its status table; the ontology publishes the
# thresholds. Nothing enforced either until check-ci-floors.sh.
bash "$REPO/scripts/check-ci-floors.sh" 2>&1 >/dev/null | grep -E '^floor_' || true
# The figures README.md and ARCHITECTURE_DSL.md quote from the two
# benchmark scripts they cite. Those scripts ran nowhere until
# 2026-09-02, and once wired into CI they asserted only *structural*
# success — "check passed", "unsupported=[]" — so the numbers the
# documents actually quote could have drifted with the step still
# green. A guard wearing a stronger name than it earns. These pin the
# measured integers; the cited ratios (1.09x, 1.12x, 10.2x) are
# derived from them, so the parts are enough.
# Both scripts need the *release* binaries, and this function runs in
# whatever profile the caller asked for — debug by default. The first
# version of this omitted the builds and swallowed the result with
# `|| true`, so in CI the scripts printed "missing binary", emitted no
# keys, and `check-doc-counts.sh` failed downstream with six INCOMPLETE
# rows. It was right to: an unchecked claim is an unguarded claim. The
# `|| true` was the bug — it turned "the benchmark could not run" into
# silence at the one place that knew.
cargo build --release --quiet --manifest-path "$REPO/prototype/Cargo.toml" --bin mage-parse
cargo build --release --quiet --manifest-path "$REPO/forge/Cargo.toml"     --bin forge
bash "$REPO/benchmarks/capstone/run.sh"   2>&1 >/dev/null | grep -E '^capstone_'
bash "$REPO/benchmarks/constructs/run.sh" 2>&1 >/dev/null | grep -E '^constructs_'
python - "$REPO/MAGE_ONTOLOGY.json" <<'ONTO'
import json, sys
d = json.load(open(sys.argv[1], encoding='utf-8'))
for k, v in d['sections'].items():
    print('onto_%s=%d' % (k, len(v)))
print('onto_sections=%d' % len(d['sections']))
ONTO

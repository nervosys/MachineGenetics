#!/usr/bin/env bash
# Every declared diagnostic code must be reachable — a code nothing can emit is
# a promise to agents that no program keeps.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# `hir.rs` calls these codes "part of the agent contract: machine-matchable,
# never reused, and (where the concept maps) chosen to echo the equivalent
# rustc code so Rust-trained agents recognise them."
#
# `DiagnosticCategory::UseAfterMove` had a code, `E0382`, and **no producer
# anywhere in the crate**. The only path that could have made one —
# `heal::infer_category` — folded `"move"` into the borrow branch, so a
# diagnostic reading *use of moved value `x`* came back coded `E0502` while the
# code that names it could not be emitted at all. Three hundred lines above,
# the `move-after-use` fix table recognised the same message and offered move
# fixes for it, so an agent got move fixes carrying a borrow's code.
#
# Nothing noticed, and nothing could have: the enum compiled, `code()` was
# exhaustive, every test passed. The variant existed, the code existed, the
# documentation described it, and the set of programs that could produce it was
# empty. That is the shape this repository keeps finding — an absence that
# cannot fail loudly — and it is the only one of these where the instruments
# were already pointed at the right file and still saw nothing, because they
# were all asking whether the code was *correct* rather than whether it was
# *reachable*.
#
# ─────────────────────────────────────────────────────────────────────────────
# HOW IT DECIDES
# ─────────────────────────────────────────────────────────────────────────────
#
# For each variant of `DiagnosticCategory`, look for a `DiagnosticCategory::<V>`
# mention in the crate **outside** `hir.rs`, which is where the enum, its
# `code()` mapping and its documentation live. A mention there proves only that
# the variant was declared; a mention elsewhere is a pass, a healer or a bridge
# naming it, which is what makes it producible.
#
# **Test code does not count, and the first version of this script got that
# wrong.** It searched whole files, and the commit that fixed `UseAfterMove`
# also added tests asserting on it — so reverting the fix left the variant
# mentioned in four `assert_eq!`s and the check still passed. It would have
# reported the exact bug it was written for as fine. A variant named only
# inside `mod tests` is one the tests construct by hand; it says nothing about
# whether any pass emits it, which is the whole question.
#
# So each file is truncated at its first `#[cfg(test)]` or `mod tests {` before
# being searched. That is a heuristic: it assumes test modules come last, which
# is this repository's layout throughout, and it would miss a producer defined
# after one. Missing a producer makes this check *stricter*, not weaker — the
# error runs toward a false alarm rather than a false pass, which is the
# direction a reachability check has to fail in.
#
# It remains a weak check in the other axis: it does not prove a *program*
# exists that reaches the mention, so a producer behind a condition that is
# never true still passes.
#
# ── The second question: produced, or merely classified? ─────────────────────
#
# Being named somewhere is a low bar, and three variants clear it in a way worth
# separating. `BorrowConflict`, `UseAfterMove` and `SpecViolation` each have
# exactly one non-test mention in the crate, and all three are inside
# `heal::infer_category` — the function that guesses a category from a message
# someone else already wrote. No analysis pass emits them.
#
# That is not the same state as `TypeMismatch` (types.rs), `UnresolvedName`,
# `UnresolvedType`, `DuplicateDefinition` (resolve.rs), `UndeclaredEffect`
# (effects.rs) or `SyntaxError`, each of which some pass constructs on finding
# the condition. A classifier-only code says the compiler can *relay* such an
# error, not that it can *find* one — and for these three that is exactly right,
# because there is no ownership or borrow-checking phase and the contract
# verifier prints its own summary rather than emitting diagnostics.
#
# So the split is recorded in a baseline rather than judged. A variant moving
# from pass-produced to classifier-only is a real regression and fails; one
# moving the other way is progress and fails too, asking for the baseline entry
# to be dropped in the same commit — the same shape as
# `check-orphan-sources.sh`, and for the same reason: a baseline nobody prunes
# stops meaning anything.
#
# Two things it deliberately does not do:
#
#   * It does not read the `code()` match. That match is exhaustive, so the
#     compiler already guarantees every variant has a code. Re-checking it here
#     would be a second copy of a guarantee that cannot drift.
#   * It does not check for *unused* codes in the other direction (a code
#     string appearing nowhere as a category). `aci.rs` holds a lookup table of
#     rustc codes it explains without emitting, which is a legitimate second
#     vocabulary, and conflating the two would make this checker cry wolf.
#
# Usage:
#     bash scripts/check-diagnostic-codes.sh
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

HIR="prototype/src/hir.rs"

if [ ! -f "$HIR" ]; then
    echo "check-diagnostic-codes: $HIR not found — has the enum moved?" >&2
    exit 1
fi

# The variants, read from the enum body rather than from `code()`, so a variant
# added to the enum and forgotten in the match is still checked (the compiler
# would catch that one, but this should not depend on which error comes first).
variants="$(
    awk '/^pub enum DiagnosticCategory/{f=1; next} f && /^}/{exit} f' "$HIR" \
        | grep -oE '^\s{4}[A-Z][A-Za-z0-9]*' \
        | tr -d ' '
)"

if [ -z "$variants" ]; then
    echo "check-diagnostic-codes: found no variants — the enum's shape changed." >&2
    echo "  This check reads \`pub enum DiagnosticCategory\` in $HIR. If it moved" >&2
    echo "  or was renamed, point this script at it rather than deleting it: an" >&2
    echo "  empty result here must not read as success." >&2
    exit 1
fi

# Every .rs file in the crate except the one declaring the enum, with each
# file's test module cut off. `awk` stops at the first `#[cfg(test)]` or
# `mod tests {` line and prints nothing after it.
non_test_source="$(
    find prototype/src -name '*.rs' ! -name "$(basename "$HIR")" -print0 \
        | xargs -0 awk '/^[[:space:]]*#\[cfg\(test\)\]/{nextfile} /^[[:space:]]*(pub )?mod tests[[:space:]]*\{/{nextfile} {print}'
)"

if [ -z "$non_test_source" ]; then
    echo "check-diagnostic-codes: read no non-test source from prototype/src." >&2
    echo "  An empty haystack would make every variant look reachable-by-nothing" >&2
    echo "  or reachable-by-everything depending on the grep, so this stops here" >&2
    echo "  rather than reporting either." >&2
    exit 1
fi

# The same source again, minus the healer, which answers the second question:
# which variants some *pass* constructs rather than only the text classifier.
HEAL="prototype/src/heal.rs"
pass_source="$(
    find prototype/src -name '*.rs' ! -name "$(basename "$HIR")" ! -name "$(basename "$HEAL")" -print0 \
        | xargs -0 awk '/^[[:space:]]*#\[cfg\(test\)\]/{nextfile} /^[[:space:]]*(pub )?mod tests[[:space:]]*\{/{nextfile} {print}'
)"

BASELINE="scripts/diagnostic-codes-classifier-only.txt"

unreachable=""
classifier_only=""
n=0
for v in $variants; do
    n=$((n + 1))
    # Producers: any mention in non-test code outside the declaring file.
    #
    # A here-string rather than a pipe, and not a style choice: `grep -q` exits
    # on the first match, the writer upstream takes SIGPIPE, and `pipefail` then
    # reports the pipeline as failed *because* the match was found. The first
    # draft did exactly that and called all ten variants unreachable. It failed
    # toward the alarm rather than the false pass, which is the direction this
    # check has to break in, and is the only reason it was obvious.
    if ! grep -q "DiagnosticCategory::$v" <<< "$non_test_source"; then
        unreachable="$unreachable$v"$'\n'
    elif ! grep -q "DiagnosticCategory::$v" <<< "$pass_source"; then
        classifier_only="$classifier_only$v"$'\n'
    fi
done

if [ -n "$unreachable" ]; then
    echo "  x  diagnostic categor(ies) declared in $HIR that nothing outside it names:" >&2
    printf '%s' "$unreachable" | sed '/^$/d; s/^/       /' >&2
    echo "     Each has a stable code in \`code()\` and is documented as part of the" >&2
    echo "     agent contract, so an agent may match on it. A variant no pass, healer" >&2
    echo "     or bridge ever constructs is a code that cannot be emitted — the" >&2
    echo "     contract promises something no program delivers." >&2
    echo "     Either produce it where the condition is detected, or remove the" >&2
    echo "     variant. Keeping it costs nothing at compile time and misleads every" >&2
    echo "     agent that reads the contract." >&2
    exit 1
fi

found="$(printf '%s' "$classifier_only" | sed '/^$/d' | sort)"

if [ ! -f "$BASELINE" ]; then
    printf '%s\n' "$found" > "$BASELINE"
    echo "check-diagnostic-codes: no baseline; wrote $(printf '%s\n' "$found" | grep -c . ) entr(ies) to $BASELINE." >&2
fi

was="$(grep -vE '^[[:space:]]*(#|$)' "$BASELINE" | sort)"
new_only="$(comm -23 <(printf '%s\n' "$found") <(printf '%s\n' "$was"))"
now_produced="$(comm -13 <(printf '%s\n' "$found") <(printf '%s\n' "$was"))"

fail=0

if [ -n "$new_only" ]; then
    echo "  x  diagnostic categor(ies) that only \`heal::infer_category\` names, and are not baselined:" >&2
    printf '%s\n' "$new_only" | sed 's/^/       /' >&2
    echo "     No pass constructs these, so the compiler can relay such an error but" >&2
    echo "     cannot find one. Either emit it where the condition is detected, or add" >&2
    echo "     it to $BASELINE with a line saying which analysis is missing." >&2
    fail=1
fi

if [ -n "$now_produced" ]; then
    echo "  +  baseline entr(ies) now produced by a pass — remove them in this commit:" >&2
    printf '%s\n' "$now_produced" | sed 's/^/       /' >&2
    echo "     A baseline nobody prunes stops meaning anything." >&2
    fail=1
fi

if [ "$fail" -ne 0 ]; then
    exit 1
fi

classified="$(printf '%s\n' "$found" | grep -c . || true)"
echo "  ok all $n DiagnosticCategory variant(s) are named by non-test code outside $HIR ($classified reachable only via the healer's classifier, baselined)."

#!/usr/bin/env bash
# Find .rs files that no `mod` declaration reaches — source that is in the
# tree, reads as reviewed, and never reaches a compiler.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# `RecursiveMachineIntelligence/src/compute/cuda_full.rs` is 1,812 lines holding
# 16 `unsafe` blocks and 6 `#[test]`s. No `mod` declaration references it, so it
# has never been compiled, its tests have never run on any hardware, and its
# `unsafe` has never been checked by anything — while `SECURITY_AUDIT.md`
# described the crate's unsafe surface as "all reviewed".
#
# That is the failure mode HANDOFF.md calls rule 7: a file the instruments
# cannot see reads exactly like a file with nothing wrong. Every other check in
# this directory asks a question *about* code that compiles. Nothing asked
# whether the code compiles at all.
#
# Running it the first time found a second one nobody had mentioned:
# `compute/wasm.rs`, 208 lines — and `core/discoverability.rs` advertises a
# "WASM Backend" to callers, so the crate publishes a capability whose only
# implementation is a file no compiler reads.
#
# ─────────────────────────────────────────────────────────────────────────────
# HOW IT DECIDES
# ─────────────────────────────────────────────────────────────────────────────
#
# A file is reachable if some file in the same crate declares `mod <stem>`.
# That is a heuristic, not name resolution: it does not follow the module tree,
# so a `mod` in the wrong parent still counts as reachable.
#
# The reason it is trustworthy *here* is measured rather than assumed. Across
# the six crates it walks it reports exactly two files, both genuinely
# unreachable, and **zero** false positives — verified by reading every `mod`
# declaration in `compute/mod.rs` and confirming neither name appears. It also
# does not have to handle `#[path = "..."]`, which would defeat it: there is not
# one in the repository (`git grep '#\[path'` is empty). If that stops being
# true this check must be revisited, and this paragraph is the reason why.
#
# Per HANDOFF rule 9, a check that is often wrong is worse than a documented
# gap. This one is currently never wrong, and the conditions under which that
# stops holding are written above.
#
# Usage:
#     bash scripts/check-orphan-sources.sh          # verify against baseline
#     rm scripts/orphan-sources-baseline.txt && bash scripts/check-orphan-sources.sh
#                                                   # regenerate (then review!)

set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

CRATES="prototype RecursiveMachineIntelligence ribosome germline forge framework/framewerx"
BASELINE=scripts/orphan-sources-baseline.txt

found=""

for crate in $CRATES; do
    src="$crate/src"
    [ -d "$src" ] || continue

    # Every `mod` name declared anywhere in the crate, including `mod x { .. }`
    # inline forms and `#[cfg(..)] pub mod x;` (the attribute is on its own
    # line in this codebase, so the mod line still matches).
    mods="$(grep -rhoE '^[[:space:]]*(pub[[:space:]]+)?mod[[:space:]]+[a-zA-Z_][a-zA-Z0-9_]*' "$src" 2>/dev/null \
            | sed -E 's/.*mod[[:space:]]+//' | sort -u)"

    while IFS= read -r f; do
        base="$(basename "$f" .rs)"
        # Crate roots and directory roots are reachable by definition; a file
        # directly under src/bin/ is its own binary target.
        case "$base" in lib|main|mod) continue ;; esac
        case "$f" in */src/bin/*) continue ;; esac

        if ! printf '%s\n' "$mods" | grep -qx "$base"; then
            found="${found}${f}"$'\n'
        fi
    done < <(find "$src" -name '*.rs' -type f | sort)
done

found="$(printf '%s' "$found" | grep -v '^$' | sort || true)"

if [ ! -f "$BASELINE" ]; then
    {
        echo "# Source files no \`mod\` declaration reaches, and which therefore never"
        echo "# compile. One path per line, sorted. May only shrink."
        echo "#"
        echo "# Both entries are in the vendored \`rmi\` crate, which must stay syncable"
        echo "# against its own upstream — deleting or gating them is its owner's call"
        echo "# (HANDOFF item 15). They are recorded so that a *third* one fails."
        printf '%s\n' "$found"
    } > "$BASELINE"
    echo "check-orphan-sources: no baseline; wrote $(printf '%s\n' "$found" | grep -c . ) entr(ies) to $BASELINE." >&2
    echo "Review it before committing — this run blessed whatever it measured." >&2
    exit 1
fi

was="$(grep -vE '^[[:space:]]*(#|$)' "$BASELINE" | sort)"

new_orphans="$(comm -23 <(printf '%s\n' "$found") <(printf '%s\n' "$was"))"
gone="$(comm -13 <(printf '%s\n' "$found") <(printf '%s\n' "$was"))"

fail=0

if [ -n "$new_orphans" ]; then
    echo "  x  source files that no \`mod\` declaration reaches, and are not in the baseline:" >&2
    printf '%s\n' "$new_orphans" | sed 's/^/       /' >&2
    echo "     Either wire it up with a \`mod\` declaration, or delete it. A file that" >&2
    echo "     never compiles is not reviewed, however carefully it was written." >&2
    fail=1
fi

if [ -n "$gone" ]; then
    echo "  +  baseline entr(ies) now reachable — remove them in this commit:" >&2
    printf '%s\n' "$gone" | sed 's/^/       /' >&2
    fail=1
fi

if [ "$fail" -ne 0 ]; then
    exit 1
fi

n="$(printf '%s\n' "$found" | grep -c . || true)"
echo "  ok every .rs file is reachable by a \`mod\`, except $n known and baselined."

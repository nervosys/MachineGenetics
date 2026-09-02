#!/usr/bin/env bash
# Every Markdown document at the repository root must have an entry in
# `DOCS.md`, and every root document `DOCS.md` names must still exist.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# `DOCS.md` opens: "There are 22 Markdown documents at the repository root…
# **This index says which is which**, so nothing here has to be read to find
# out whether it is still true." That is a universal claim, and the only thing
# checking it was the `root_docs` count pin — which compares the *number* in
# that sentence against `ls *.md | wc -l`.
#
# A count catches adding a document and forgetting the number. It does not
# catch adding a document, updating the number, and forgetting the **entry** —
# which is the failure the sentence exists to prevent, and the one that leaves
# a reader unable to tell whether a file is current. Two documents swapped in
# the same commit would also net to zero.
#
# This is the same shape as rule 4: a claim that says "every" guarded by a
# check that counts. All 22 were verified indexed by hand on 2026-08-25 — this
# keeps it that way without the hand.
#
# ─────────────────────────────────────────────────────────────────────────────
# SCOPE, STATED
# ─────────────────────────────────────────────────────────────────────────────
#
# **Root documents only**, because that is what `DOCS.md` claims. 13 of the
# repository's documentation *directories* — `agent-guide/`, `cookbook/`,
# `internals/`, `migration-guide/`, `quick-start/` and others, 68 files between
# them — are indexed nowhere, and that is not a false claim because `DOCS.md`
# scopes itself to the root in its first sentence. It is worth knowing anyway:
# `internals/` turned out to document a compiler that was designed and not
# built (`check-internals-doc.sh`), and one reason it went unexamined for so
# long is that no index points at it.
#
# Matching is by filename substring: an entry that merely *mentions* a document
# satisfies this, rather than requiring a link or a description. That is
# deliberately loose — the failure guarded against is absence, not quality.
#
# Usage:
#     bash scripts/check-docs-index.sh

set -o nounset
set -o pipefail

cd "$(dirname "$0")/.." || exit 1

INDEX=DOCS.md

if [ ! -f "$INDEX" ]; then
    echo "  x  $INDEX is missing; there is no index to check" >&2
    exit 1
fi

fail=0
checked=0

# ── Direction 1: a root document with no entry ───────────────────────────────
for f in *.md; do
    [ -f "$f" ] || continue
    [ "$f" = "$INDEX" ] && continue
    checked=$((checked + 1))
    if ! grep -qF -- "$f" "$INDEX"; then
        echo "  x  $f is at the repository root and $INDEX does not name it" >&2
        echo "       \"This index says which is which\" — so a document it never" >&2
        echo "       mentions is one a reader cannot tell the status of. Add an" >&2
        echo "       entry saying whether it is current." >&2
        fail=1
    fi
done

if [ "$checked" -eq 0 ]; then
    echo "  x  no root Markdown documents found; the glob stopped matching" >&2
    exit 1
fi

# ── Direction 2: an entry whose document is gone ─────────────────────────────
#
# Names that look like a root document reference. Restricted to the ones that
# actually end in `.md` and contain no slash, so a link to `benchmarks/…` or
# `internals/…` is not mistaken for a root entry.
named="$(grep -oE '\b[A-Za-z0-9_-]+\.md\b' "$INDEX" | sort -u)"

while read -r n; do
    [ -n "$n" ] || continue
    [ "$n" = "$INDEX" ] && continue
    if [ ! -f "$n" ]; then
        echo "  x  $INDEX names \`$n\`, which is not a document at the root" >&2
        echo "       Either it was deleted or renamed and the index kept the old" >&2
        echo "       name, or it lives in a subdirectory and the reference lost" >&2
        echo "       its path. Both leave a reader following a dead pointer." >&2
        fail=1
    fi
done <<< "$named"

if [ "$fail" -ne 0 ]; then
    exit 1
fi

echo "  ok all $checked root document(s) have an entry in $INDEX, and every root document it names exists."

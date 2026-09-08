#!/usr/bin/env bash
# Run the differentiability pass over every tracked `.mg` source and aggregate.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# `DIFFERENTIABILITY.md` published "0 of 155 functions differentiable, 125 of
# them for want of a floating-point parameter" for a phase in which there was no
# way to run the pass. `prototype/src/differentiable.rs` was library-only: no
# CLI mode reached it, no script called it, and the figure could be reproduced
# only by writing a program. A number with no command beside it is the shape
# this repository has spent five sessions removing from its own documents, and
# it does not get an exception for being a number this repository liked.
#
# `mage-parse --differentiable` is the command; this is the aggregate over the
# corpus. Both figures in the document come from here.
#
# The summary lines it parses are a tested format contract —
# `differentiable::tests::the_summary_line_shape_is_a_contract` fails if the
# wording moves. Without that test this script would silently match nothing and
# report zeros, which is a checker that stops reaching its subject: the exact
# failure the pass's fourth state exists to name.
#
# Usage:
#     scripts/measure-differentiability.sh            # human table
#     scripts/measure-differentiability.sh --pins     # key=value, for pinning
#     scripts/measure-differentiability.sh --check    # CI: the document agrees
set -o errexit
set -o nounset
set -o pipefail

# Resolved before the `cd`, because `--check` re-invokes this script and a
# relative `$0` stops naming it once the working directory has moved.
SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"

cd "$(dirname "$0")/.."

DOC=DIFFERENTIABILITY.md

# `--check` is the reason the other two modes are worth having. A figure in a
# document with a command beside it still decays; what stops it is something
# that re-derives it and fails. The document carries this script's `--pins`
# output verbatim in a fenced block, so the comparison is a diff rather than a
# regex per number — one mechanism for all of them, and adding a figure to the
# block is enough to have it checked.
if [ "${1:-}" = "--check" ]; then
    fresh="$(bash "$SELF" --pins)"
    pinned="$(awk '
        /^\$ scripts\/measure-differentiability.sh --pins$/ { on = 1; next }
        on && /^```/ { exit }
        on { print }
    ' "$DOC")"
    if [ -z "$pinned" ]; then
        echo "  x  $DOC has no '\$ scripts/measure-differentiability.sh --pins'" >&2
        echo "     block, so nothing is pinned. Add one, or delete this check —" >&2
        echo "     a checker that cannot find its subject must not pass." >&2
        exit 1
    fi
    if [ "$fresh" != "$pinned" ]; then
        echo "  x  $DOC disagrees with the measurement:" >&2
        diff <(printf '%s\n' "$pinned") <(printf '%s\n' "$fresh") \
            | sed 's/^/     /' >&2 || true
        echo "     '<' is what the document says, '>' is what the pass reports." >&2
        exit 1
    fi
    # The prose leads with the three headline ratios. They are the numbers a
    # reader takes away, and a block that agrees while the sentence above it
    # does not is worse than no check at all.
    fail=0
    while IFS='=' read -r k v; do
        case "$k" in
            nets_total)      nets_total=$v ;;
            nets_differentiable) nets_diff=$v ;;
            trains_total)    trains_total=$v ;;
            trains_differentiable) trains_diff=$v ;;
            functions_total) fn_total=$v ;;
            functions_differentiable) fn_diff=$v ;;
        esac
    done <<< "$fresh"
    for claim in "$nets_diff of $nets_total nets" \
                 "$trains_diff of $trains_total train blocks" \
                 "$fn_diff of $fn_total functions"; do
        if ! grep -qF "$claim" "$DOC"; then
            echo "  x  $DOC does not state \"$claim\"" >&2
            fail=1
        fi
    done
    [ "$fail" -eq 0 ] || exit 1
    echo "OK — $DOC agrees with the differentiability pass."
    exit 0
fi

# Built every run, then located through cargo. This script is where the
# stale-binary failure `scripts/find-mage-parse.sh` documents was found: it
# reported "101 of 101 sources could not be parsed" while running a compiler
# three weeks old, which is a clean-looking zero from a command that never ran.
. scripts/find-mage-parse.sh
BIN="$(build_and_find_mage_parse)"

files=0
unparsed=0
report=""

while IFS= read -r f; do
    files=$((files + 1))
    if out="$("$BIN" --differentiable "$f" 2>/dev/null)"; then
        # `$(...)` strips the trailing newline, so without one added back the
        # last line of one file's report and the first line of the next
        # concatenate into a single line — and the joined line still starts
        # with `nets:`, so awk parsed it as a summary and dropped a header.
        # A silent off-by-one line, not an error.
        report="${report}${out}"$'\n'
    else
        # A file the front end cannot read is counted, not skipped. `stdlib/`
        # is 25 files of Rust behind a `.mg` extension; a corpus figure that
        # quietly excluded them would be measuring a smaller corpus than it
        # claimed to.
        unparsed=$((unparsed + 1))
    fi
done < <(git ls-files '*.mg' | sort)

printf '%s' "$report" | awk -v files="$files" -v unparsed="$unparsed" -v mode="${1:-}" '
/^(nets|trains|functions): / {
    k = substr($1, 1, length($1) - 1)
    total[k]  += $4
    smooth[k] += $7
    ae[k]     += $9
    unk[k]    += $12
    no[k]     += $14
    next
}
# Reasons, for the negative verdicts — the useful half of a "0 differentiable"
# result is which obligation failed. The per-file header carries the same em
# dash and is skipped, or the most common "reason" is a filename.
/^\/\// { next }
/ — / {
    line = $0
    sub(/^[^—]*— /, "", line)
    reason[line]++
}
END {
    order[1] = "nets"; order[2] = "trains"; order[3] = "functions"
    if (mode == "--pins") {
        printf "mg_files=%d\n", files
        printf "mg_unparsed=%d\n", unparsed
        for (i = 1; i <= 3; i++) {
            k = order[i]
            printf "%s_total=%d\n", k, total[k]
            printf "%s_differentiable=%d\n", k, smooth[k] + ae[k]
            printf "%s_smooth=%d\n", k, smooth[k]
            printf "%s_almost_everywhere=%d\n", k, ae[k]
            printf "%s_unknown=%d\n", k, unk[k]
            printf "%s_not=%d\n", k, no[k]
        }
        exit
    }
    printf "%d tracked .mg sources; %d could not be parsed.\n\n", files, unparsed
    printf "%-10s %7s %7s %7s %7s %7s %7s\n", \
           "subject", "total", "diff", "smooth", "a.e.", "unknown", "not"
    for (i = 1; i <= 3; i++) {
        k = order[i]
        printf "%-10s %7d %7d %7d %7d %7d %7d\n", \
               k, total[k], smooth[k] + ae[k], smooth[k], ae[k], unk[k], no[k]
    }
    print ""
    print "reasons, most common first:"
    n = asorti(reason, sorted, "@val_num_desc")
    for (i = 1; i <= n && i <= 12; i++) {
        printf "%5d  %s\n", reason[sorted[i]], sorted[i]
    }
}
'

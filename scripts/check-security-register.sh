#!/usr/bin/env bash
# `SECURITY_AUDIT.md` §1's accepted-risk register must agree with what
# `cargo audit` actually reports — in both directions.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# The document names this gap itself: "`cargo audit` in CI catches new
# *vulnerabilities*; nothing compares this table against the warnings actually
# reported." CI's `audit` job deliberately does not pass `-D warnings`, for a
# good reason — the remaining findings are `unmaintained`/`unsound` advisories
# with nowhere to move to, and a permanently red job is an ignored job. But the
# consequence is that the entire warning surface is printed and nothing reads
# it. The register is where a human wrote down what they decided about those
# warnings, and that writing has no mechanism behind it.
#
# It had already drifted **in both directions at once**, which is the whole
# argument for a two-directional check:
#
#   - RUSTSEC-2026-0097 (`rand`) was listed as an accepted risk after it had
#     stopped firing. An accepted-risk row that no longer applies is worse than
#     no row: it reads as a live decision someone is standing behind.
#   - RUSTSEC-2026-0190 (`anyhow`, dated 2026-06-25) was being reported and was
#     not in the table at all — for roughly two months, with a patched version
#     available the whole time.
#
# Neither is visible to a reader. Someone checking whether the accepted set was
# complete would have found it neither complete nor current, and the only way
# to find that out was to run the five surfaces by hand and diff them against
# the prose. That is what this does.
#
# ─────────────────────────────────────────────────────────────────────────────
# HOW IT DECIDES
# ─────────────────────────────────────────────────────────────────────────────
#
# The register rows are *read out of* `SECURITY_AUDIT.md` — not copied here.
# A check that keeps its own list of the thing it is checking can only ever
# report what its author already knew (see HANDOFF.md, "What the instruments
# taught", rule 4). Adding a row to the table is what tells this script the row
# exists.
#
# Each row's Status column is classified into what should be true of it:
#
#   FIXED / No longer applies  →  must NOT be reported by any surface
#   Accepted                   →  MUST be reported by some surface
#
# and a status this script cannot classify is a failure, not a pass. Then:
#
#   reported, not in the register    →  FAIL: an unrecorded finding
#   Accepted, reported nowhere       →  FAIL: a stale acceptance
#   FIXED/struck, reported again     →  FAIL: a regression
#
# The union across surfaces is what counts for the "Accepted" direction,
# because an advisory can be accepted on the strength of a single surface —
# RUSTSEC-2024-0436 (`paste`) is reported only by `rmi`'s lockfile, and
# excluding that surface would make a correct row look stale.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHAT THIS DOES NOT CHECK
# ─────────────────────────────────────────────────────────────────────────────
#
# It compares *identity*, not *rationale*. A row can name the right advisory
# and give a reason that stopped being true — "only under the non-default `gpu`
# feature" is a claim about a dependency graph, and nothing here re-derives it.
# That is deliberate: the check that would do it is a `cargo tree` reachability
# argument per row, which is a much weaker instrument with a false-positive
# class, and per rule 10 a weak instrument is worse than a documented gap.
#
# It also cannot say whether an advisory *should* have been accepted. That is
# the judgement the register exists to record.
#
# **`RecursiveMachineIntelligence/Cargo.lock` is git-ignored on purpose**, so a
# finding there may be a property of one developer's working copy rather than
# of this repository — the distinction that made a `crossbeam-epoch` hit there
# a false alarm while the identical hit in `prototype` was real. CI generates
# that lockfile fresh, which audits what a consumer would actually resolve.
# Findings on that surface are labelled `(git-ignored)` in the output so the
# difference is never silently lost.
#
# There is no baseline file. Every row currently agrees with every surface, so
# green is the honest state; a baseline would hold exemptions that do not
# exist. If that changes, add one rather than weakening this.
#
# Usage:
#     bash scripts/check-security-register.sh

set -o nounset
set -o pipefail

cd "$(dirname "$0")/.." || exit 1

DOC=SECURITY_AUDIT.md
# The four committed surfaces, then the git-ignored one. CI audits the same
# five; keep the two lists in step.
SURFACES="prototype forge ribosome germline RecursiveMachineIntelligence"
GIT_IGNORED_SURFACE=RecursiveMachineIntelligence
NPM_SURFACE=video

# An advisory id, e.g. RUSTSEC-2026-0190.
ID_RE='RUSTSEC-[0-9]\{4\}-[0-9]\{4\}'

fail=0

# ── Preconditions ────────────────────────────────────────────────────────────
#
# Each of these exits rather than skipping. A surface that silently goes
# unchecked reads exactly like a surface with nothing wrong (rule 8), and this
# script's entire subject is claims that nothing was verifying.

if [ ! -f "$DOC" ]; then
    echo "  x  $DOC is missing; there is no register to check" >&2
    exit 1
fi

if ! command -v cargo-audit >/dev/null 2>&1 && ! cargo audit --version >/dev/null 2>&1; then
    echo "  x  cargo-audit is not installed; the register cannot be checked" >&2
    echo "       cargo install cargo-audit --locked" >&2
    exit 1
fi

if ! command -v npm >/dev/null 2>&1; then
    echo "  x  npm is not on PATH; the $NPM_SURFACE/ surface cannot be checked" >&2
    echo "       That surface had a high-severity advisory while HANDOFF.md" >&2
    echo "       claimed \"0 npm\", so it is not one to skip quietly." >&2
    exit 1
fi

# ── Read the register out of the document ────────────────────────────────────
#
# Rows of §1's table only: from the `## 1.` heading to the next `##`, table
# lines whose *first column* names an advisory. Restricting to the first column
# matters — the Status prose cites other advisory ids, and the blockquotes
# around the table cite more.

section="$(awk '/^## 1\./ { inside = 1; next } /^## / { inside = 0 } inside' "$DOC")"

if [ -z "$section" ]; then
    echo "  x  $DOC has no \`## 1.\` section; the register moved or was renamed" >&2
    exit 1
fi

# "<id> <expectation>" per line, where expectation is present|absent.
register="$(printf '%s\n' "$section" | awk -F'|' '
    /^\|/ {
        # $1 is empty (leading pipe), $2 is the ID column, $5 the Status column.
        id = $2
        if (match(id, /RUSTSEC-[0-9][0-9][0-9][0-9]-[0-9][0-9][0-9][0-9]/) == 0) next
        id = substr(id, RSTART, RLENGTH)

        status = $5
        if (status ~ /No longer applies/) { print id, "absent";  next }
        if (status ~ /FIXED/)             { print id, "absent";  next }
        if (status ~ /Accepted/)          { print id, "present"; next }
        print id, "UNCLASSIFIED"
    }
')"

if [ -z "$register" ]; then
    echo "  x  §1 of $DOC lists no advisories; the table format changed" >&2
    echo "       Expected rows like: | RUSTSEC-2024-0436 | crate | sev | Accepted — … |" >&2
    exit 1
fi

while read -r id expectation; do
    [ -n "$id" ] || continue
    if [ "$expectation" = "UNCLASSIFIED" ]; then
        echo "  x  $DOC §1: \`$id\`'s Status column says none of FIXED," >&2
        echo "       Accepted, or \"No longer applies\", so this check cannot tell" >&2
        echo "       what should be true of it. An unclassifiable row must not" >&2
        echo "       read as a passing one — say which of the three it is." >&2
        fail=1
    fi
done <<< "$register"

registered_ids="$(printf '%s\n' "$register" | awk '{print $1}' | sort -u)"

# ── Run every surface ────────────────────────────────────────────────────────
#
# `cargo audit` exits nonzero when it finds a vulnerability, so its status
# cannot be used to detect a *failure to run*; the JSON body is the signal.

reported=""          # "<id> <surface>" per line
surfaces_checked=0

for surface in $SURFACES; do
    lock="$surface/Cargo.lock"
    if [ ! -f "$lock" ]; then
        if [ "$surface" = "$GIT_IGNORED_SURFACE" ]; then
            echo "  x  $lock does not exist. It is git-ignored, so generate one:" >&2
            echo "       ( cd $surface && cargo generate-lockfile )" >&2
        else
            echo "  x  $lock is missing, and it is a committed lockfile" >&2
        fi
        fail=1
        continue
    fi

    json="$(cargo audit --file "$lock" --json --color never 2>/dev/null)"

    if [ -z "$json" ]; then
        echo "  x  cargo audit produced no report for $lock" >&2
        echo "       Re-run it directly to see why; a surface that cannot be" >&2
        echo "       audited is not a surface with nothing wrong." >&2
        fail=1
        continue
    fi

    surfaces_checked=$((surfaces_checked + 1))

    ids="$(printf '%s\n' "$json" \
        | grep -o "\"id\":\"$ID_RE\"" \
        | sed 's/.*"\(RUSTSEC-[0-9-]*\)"/\1/' \
        | sort -u)"

    while read -r id; do
        [ -n "$id" ] || continue
        reported="$reported$id $surface"$'\n'
    done <<< "$ids"
done

reported_ids="$(printf '%s' "$reported" | awk '{print $1}' | sort -u)"

label_surfaces() {
    # Every surface reporting $1, with the git-ignored one marked as such.
    printf '%s' "$reported" | awk -v want="$1" -v ignored="$GIT_IGNORED_SURFACE" '
        $1 == want { printf "%s%s ", $2, ($2 == ignored ? " (git-ignored)" : "") }
    '
}

# ── Direction 1: reported, but not in the register ───────────────────────────

while read -r id; do
    [ -n "$id" ] || continue
    if ! printf '%s\n' "$registered_ids" | grep -qxF "$id"; then
        echo "  x  \`$id\` is reported by cargo audit and is not in $DOC §1" >&2
        echo "       Surfaces: $(label_surfaces "$id")" >&2
        echo "       Add a row saying what was decided about it — FIXED with the" >&2
        echo "       pin, or Accepted with the reason. This is how" >&2
        echo "       RUSTSEC-2026-0190 went unrecorded for two months while a" >&2
        echo "       patched version existed." >&2
        fail=1
    fi
done <<< "$reported_ids"

# ── Direction 2: in the register, but disagreeing with what is reported ──────

while read -r id expectation; do
    [ -n "$id" ] || continue
    [ "$expectation" = "UNCLASSIFIED" ] && continue

    if printf '%s\n' "$reported_ids" | grep -qxF "$id"; then
        is_reported=1
    else
        is_reported=0
    fi

    if [ "$expectation" = "present" ] && [ "$is_reported" -eq 0 ]; then
        echo "  x  \`$id\` is listed in $DOC §1 as an **accepted** risk, and no" >&2
        echo "       surface reports it any more." >&2
        echo "       An acceptance that has stopped applying is indistinguishable" >&2
        echo "       from one nobody re-checked. Strike it through with the reason" >&2
        echo "       it stopped firing, the way RUSTSEC-2026-0097 was." >&2
        fail=1
    fi

    if [ "$expectation" = "absent" ] && [ "$is_reported" -eq 1 ]; then
        echo "  x  \`$id\` is recorded in $DOC §1 as fixed or no longer applying," >&2
        echo "       and cargo audit is reporting it again." >&2
        echo "       Surfaces: $(label_surfaces "$id")" >&2
        echo "       Either the pin was lost in a \`cargo update\`, or a surface" >&2
        echo "       that did not carry it now does." >&2
        fail=1
    fi
done <<< "$register"

# ── The npm surface ──────────────────────────────────────────────────────────
#
# §1 records this as "0 of 294 packages" after the nanoid fix. The register has
# no npm rows, so anything at all here is unrecorded by construction. CI gates
# at `--audit-level=high`; this reads the *total*, because the register's claim
# is zero and a moderate finding is still a finding nobody wrote down.

npm_json="$(cd "$NPM_SURFACE" && npm audit --json 2>/dev/null)"

if [ -z "$npm_json" ]; then
    echo "  x  npm audit produced no report for $NPM_SURFACE/" >&2
    fail=1
else
    # Sum the severity buckets rather than reading `"total"`. There are two
    # `total` keys in npm's schema — `metadata.vulnerabilities.total` and
    # `metadata.dependencies.total` (293 here) — and telling them apart by
    # document order is a parse that reads the package count as a vulnerability
    # count the day the order changes. The severity names appear only in the
    # vulnerabilities object.
    # shellcheck disable=SC2020  # a character set is exactly what is wanted:
    # each of `,` `{` `}` becomes a newline, so every key lands on its own line.
    npm_total="$(printf '%s' "$npm_json" \
        | tr ',{}' '\n\n\n' \
        | grep -E '"(critical|high|moderate|low|info)":[[:space:]]*[0-9]+' \
        | sed 's/[^0-9]//g' \
        | awk '{ n += $1 } END { print (NR ? n : "") }')"

    if [ -z "$npm_total" ]; then
        echo "  x  could not read a vulnerability total out of npm audit's JSON" >&2
        echo "       for $NPM_SURFACE/. The output shape changed; fix the parse" >&2
        echo "       rather than dropping the surface." >&2
        fail=1
    elif [ "$npm_total" -ne 0 ]; then
        echo "  x  npm audit reports $npm_total vulnerabilit(y/ies) in $NPM_SURFACE/," >&2
        echo "       and $DOC §1 records that surface as zero." >&2
        echo "       Run: ( cd $NPM_SURFACE && npm audit )" >&2
        fail=1
    fi
fi

if [ "$fail" -ne 0 ]; then
    exit 1
fi

registered_count="$(printf '%s\n' "$registered_ids" | grep -c .)"
reported_count="$(printf '%s\n' "$reported_ids" | grep -c .)"

echo "  ok $DOC §1 agrees with cargo audit: $registered_count registered advisor(y/ies), $reported_count reported across $surfaces_checked surface(s), and $NPM_SURFACE/ at 0."

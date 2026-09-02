#!/usr/bin/env bash
# The RAP method surface agrees with itself, and with what documents it.
#
# `rap.rs` states its 37 methods twice: once as `"ns/verb" =>` dispatch arms,
# once in the `METHODS` list the server publishes for discovery. Nothing checked
# that those two agree. They do today — 37 and 37, exactly — but the failure
# modes are silent in both directions: an arm with no list entry is a method
# that works and cannot be discovered, and a list entry with no arm is a method
# an agent is told about and gets an error from. An agent protocol's whole point
# is that the advertised surface *is* the surface.
#
# `internals/07-rap-server.md` is the third statement of the same list, and it
# was documented at **3 of 37** until 2026-08-25. So the doc is checked too.
#
# Three directions:
#
#   1. a dispatch arm missing from `METHODS`
#   2. a `METHODS` entry with no dispatch arm
#   3. a served method the chapter never names
#
# And a fourth, ratcheted rather than enforced: a method-shaped name in the
# chapter that the server does not serve. Thirteen of those are legitimate —
# `internals/07` carries a table mapping names that were planned or documented
# elsewhere to what actually shipped (`skb/suggest` → `skb/query`, the five
# `query/*` editor-service methods → nothing), plus `namespace/verb`, which is
# the naming *convention* and not a method at all. Deleting that table would
# lose the most useful thing in the section. So they are baselined: the count
# may shrink, never grow, and a *new* phantom fails.
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

SRC=prototype/src/rap.rs
DOC=internals/07-rap-server.md
BASELINE=scripts/rap-planned-baseline.txt

for f in "$SRC" "$DOC"; do
    if [ ! -f "$f" ]; then
        echo "  x  $f is missing; this check cannot mean anything" >&2
        exit 1
    fi
done

report="$(python - "$SRC" "$DOC" <<'PY'
import io, re, sys

src = io.open(sys.argv[1], encoding='utf-8').read()
doc = io.open(sys.argv[2], encoding='utf-8').read()

# The dispatch arms: `"ns/verb" =>` at the head of a match arm. Anchored to the
# line so a method *name* appearing inside a string elsewhere is not counted —
# the mistake `check-rmi-api-doc.sh` made in its first version, where English
# words in comments satisfied it.
served = sorted(set(re.findall(r'^\s+"([a-z]+/[a-zA-Z_]+)" =>', src, re.M)))

m = re.search(r'METHODS[^=]*=\s*&?\[(.*?)\];', src, re.S)
advertised = sorted(set(re.findall(r'"([a-z]+/[a-zA-Z_]+)"', m.group(1)))) if m else []
if not m:
    print('NOLIST\t')

# Backticked in the chapter, which is how every method there is written.
documented = sorted(set(re.findall(r'`([a-z]+/[a-zA-Z_]+)`', doc)))

for name in served:
    if name not in advertised:
        print('UNLISTED\t%s' % name)
for name in advertised:
    if name not in served:
        print('UNSERVED\t%s' % name)
for name in served:
    if name not in documented:
        print('UNDOCUMENTED\t%s' % name)
for name in documented:
    if name not in served:
        print('PLANNED\t%s' % name)

print('COUNTED\t%d\t%d\t%d' % (len(served), len(advertised), len(documented)))
PY
)"

fail=0
planned=0
served_n=0
adv_n=0
doc_n=0

while IFS=$'\t' read -r kind a b c; do
    case "$kind" in
        NOLIST)
            echo "  x  no METHODS list found in $SRC — the discovery surface is unreadable" >&2
            fail=$((fail + 1))
            ;;
        UNLISTED)
            echo "  x  '$a' is dispatched but absent from METHODS — it works and cannot be discovered" >&2
            fail=$((fail + 1))
            ;;
        UNSERVED)
            echo "  x  '$a' is in METHODS but has no dispatch arm — advertised and it errors" >&2
            fail=$((fail + 1))
            ;;
        UNDOCUMENTED)
            echo "  x  '$a' is served and $DOC never names it" >&2
            fail=$((fail + 1))
            ;;
        PLANNED)
            planned=$((planned + 1))
            ;;
        COUNTED)
            served_n="$a"; adv_n="$b"; doc_n="$c"
            ;;
    esac
done <<< "$report"

if [ "$fail" -ne 0 ]; then
    echo >&2
    echo "     An agent protocol's advertised surface must be its real surface." >&2
    exit 1
fi

want="$(cat "$BASELINE" 2>/dev/null || echo 0)"
if [ "$planned" -gt "$want" ]; then
    echo "  x  $planned method-shaped name(s) in $DOC are not served (baseline $want)." >&2
    echo "     A new one means the chapter describes a method that does not exist." >&2
    echo "     If it is deliberately a *planned* name, say so where it appears and" >&2
    echo "     raise the baseline in the same commit." >&2
    exit 1
fi
if [ "$planned" -lt "$want" ]; then
    echo "  ok $planned planned-but-unserved name(s), down from $want — update $BASELINE" >&2
fi

echo "  ok RAP serves $served_n method(s); METHODS advertises the same $adv_n;" \
     "$DOC names all of them ($doc_n backticked, $planned planned and not served)."

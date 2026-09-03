#!/usr/bin/env bash
# The RAP method surface agrees with itself, with the ontology, and with the
# chapter that documents it.
#
# The same list is stated **six** times: the `"ns/verb" =>` dispatch arms in
# `rap.rs`, the `METHODS` list the server publishes, the `rap_methods` section
# of `MAGE_ONTOLOGY.json` (generated from a hand-maintained table in
# `ontology.rs`), and `internals/07-rap-server.md`. Nothing compared any of
# them.
#
# Three of the four agreed. The ontology did not: it omitted `rap/methods`, the
# method whose entire job is enumerating the methods, so an agent discovering
# the protocol through the ontology could not discover that discovery has an
# endpoint. Three documents also published the count as 37 while the server has
# always answered `METHODS.len()`, which is 38.
#
# The failure modes are silent in every direction. An arm with no `METHODS`
# entry is a method that works and cannot be found; a `METHODS` entry with no
# arm is one an agent is told about and gets an error from; an ontology omission
# is invisible to exactly the client that reads machine-readable discovery. For
# a protocol whose premise is that an agent learns what it can call, the
# advertised surface *is* the surface. The chapter was at **3 of 38** as
# recently as 2026-08-25.
#
# Five directions enforced, one ratcheted:
#
#   1. a dispatch arm missing from `METHODS`
#   2. a `METHODS` entry with no dispatch arm
#   3. a served method the ontology does not publish
#   4. an ontology entry with no dispatch arm
#   5. a served method the chapter never names
#   6. `MAGE_ONTOLOGY.md` section 5's two published counts
#
# And ratcheted: a method-shaped name in the chapter the server does not serve.
# Thirteen are legitimate -- `internals/07` maps names that were planned or
# documented elsewhere to what shipped (`skb/suggest` -> `skb/query`, the five
# `query/*` editor-service methods -> nothing), plus `agent/context`, labelled
# "(planned method)" where it appears, and `namespace/verb`, which is the naming
# *convention* and not a method. Deleting that table would lose the most useful
# paragraph in the section, so the count is baselined: it may shrink, never
# grow, and a new phantom fails.
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

SRC=prototype/src/rap.rs
ONTOMD=MAGE_ONTOLOGY.md
DOC=internals/07-rap-server.md
ONTO=MAGE_ONTOLOGY.json
BASELINE=scripts/rap-planned-baseline.txt

for f in "$SRC" "$DOC" "$ONTO" "$ONTOMD"; do
    if [ ! -f "$f" ]; then
        echo "  x  $f is missing; this check cannot mean anything" >&2
        exit 1
    fi
done

report="$(python - "$SRC" "$DOC" "$ONTO" "$ONTOMD" <<'PY'
import io, json, re, sys

# Hyphens are part of a method name. The first version of this pattern was
# `[a-zA-Z_]+`, which dropped `pipeline/recover-and-encode` out of every list
# at once — so the check reported "37 and 37 agree" when there were 38 of each.
# A false clean, in a script written to prevent false cleans. It was caught by
# comparing against a *fourth* source this script did not yet read.
NAME = r'[a-z]+/[a-zA-Z_][a-zA-Z_-]*'

src = io.open(sys.argv[1], encoding='utf-8').read()
doc = io.open(sys.argv[2], encoding='utf-8').read()
onto = json.load(io.open(sys.argv[3], encoding='utf-8'))

# The dispatch arms: `"ns/verb" =>` at the head of a match arm. Anchored to the
# line so a method *name* appearing inside a string elsewhere is not counted —
# the mistake `check-rmi-api-doc.sh` made in its first version, where English
# words in comments satisfied it.
served = sorted(set(re.findall(r'^\s+"(%s)" =>' % NAME, src, re.M)))

m = re.search(r'METHODS[^=]*=\s*&?\[(.*?)\];', src, re.S)
advertised = sorted(set(re.findall(r'"(%s)"' % NAME, m.group(1)))) if m else []
if not m:
    print('NOLIST\t')

# Backticked in the chapter, which is how every method there is written.
documented = sorted(set(re.findall(r'`(%s)`' % NAME, doc)))

# The fourth statement of the same list, and the one an agent actually reads to
# discover the protocol. Generated from `RAP_METHODS` in `ontology.rs`, a
# hand-maintained table, which had drifted: it omitted `rap/methods`, so
# discovery-by-ontology could not discover that discovery has an endpoint.
in_ontology = sorted({e['method'] for e in onto['sections']['rap_methods']})

for name in served:
    if name not in advertised:
        print('UNLISTED\t%s' % name)
for name in advertised:
    if name not in served:
        print('UNSERVED\t%s' % name)
for name in served:
    if name not in documented:
        print('UNDOCUMENTED\t%s' % name)
for name in served:
    if name not in in_ontology:
        print('UNPUBLISHED\t%s' % name)
for name in in_ontology:
    if name not in served:
        print('PHANTOM\t%s' % name)
for name in documented:
    if name not in served:
        print('PLANNED\t%s' % name)

# `MAGE_ONTOLOGY.md` section 5 is a *sixth* statement of the list, and a partial
# one on purpose. It said "RAP exposes 24 JSON-RPC 2.0 endpoints" above a table
# of 23 rows, against a server serving 38 — three numbers, none agreeing with
# another. Rather than duplicate the list a sixth time, both figures in that
# sentence are pinned to what is measured here.
md = io.open(sys.argv[4], encoding='utf-8').read()
sec = md[md.index('### 5. RAP'):md.index('### 6. Concurrency')]
m2 = re.search(r'RAP exposes \*\*(\d+)\*\* JSON-RPC', sec)
m3 = re.search(r'name \*\*(\d+)\*\* of', sec)
rows = len(set(re.findall(r'^\|\s*`(%s)`' % NAME, sec, re.M)))
if not m2 or not m3:
    print('MDSHAPE\t')
else:
    if int(m2.group(1)) != len(served):
        print('MDTOTAL\t%s\t%d' % (m2.group(1), len(served)))
    if int(m3.group(1)) != rows:
        print('MDROWS\t%s\t%d' % (m3.group(1), rows))

print('COUNTED\t%d\t%d\t%d\t%d' % (len(served), len(advertised), len(documented), len(in_ontology)))
PY
)"

fail=0
planned=0
served_n=0
adv_n=0
doc_n=0
onto_n=0

while IFS=$'\t' read -r kind a b c d; do
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
        MDSHAPE)
            echo "  x  cannot find the endpoint counts in $ONTOMD section 5 — the sentence changed shape" >&2
            fail=$((fail + 1))
            ;;
        MDTOTAL)
            echo "  x  $ONTOMD says RAP exposes $a endpoints; the server serves $b" >&2
            fail=$((fail + 1))
            ;;
        MDROWS)
            echo "  x  $ONTOMD claims to name $a methods; its tables hold $b rows" >&2
            fail=$((fail + 1))
            ;;
        UNPUBLISHED)
            echo "  x  '$a' is served and MAGE_ONTOLOGY.json does not publish it — discovery cannot find it" >&2
            fail=$((fail + 1))
            ;;
        PHANTOM)
            echo "  x  MAGE_ONTOLOGY.json publishes '$a', which has no dispatch arm" >&2
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
            served_n="$a"; adv_n="$b"; doc_n="$c"; onto_n="$d"
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
     "the ontology publishes $onto_n, and $DOC names all of them ($planned planned, not served)."

#!/usr/bin/env bash
# `skb/` is a generated export, and this proves it still matches the binary.
#
# Open item 23: the tree held **56 rules that nothing read**, while
# `builtin_rules()` served **255** from the binary. They were not even the same
# corpus — the files used `BR-`/`LT-`/`TS-`/`CC-` identifiers against the
# binary's `BOR-`/`LIF-`/`TYP-`/`CON-`, carried two stray `MEM-` and `TC-`
# rules, and two whole databases (`AgentElision` 30, `SwarmSafety` 15) had no
# file at all. So the directory was neither an input nor an export: a parallel
# corpus, free to drift, and drifted. `check-orphan-sources.sh` could not see
# it, because that finds `.rs` files no `mod` reaches, not **data** nothing
# loads.
#
# The decision was to generate rather than load or delete. Loading would have
# made 56 rules authoritative and thrown away 199 that actually run; deleting
# would lose a corpus an agent can read without running the compiler.
# Generating makes it true by construction — but only if something regenerates
# and compares, which is this.
#
# Same arrangement as `MAGE_ONTOLOGY.json`, and the same trap: **regenerate
# from a rebuilt binary.** A stale binary emits the old answer and this check
# passes on it.
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

BIN="${MG:-prototype/target/release/mage-parse}"
[ -x "$BIN" ] || BIN="$BIN.exe"
if [ ! -x "$BIN" ]; then
    echo "building the compiler first..." >&2
    cargo build --release --quiet --manifest-path prototype/Cargo.toml --bin mage-parse
    BIN=prototype/target/release/mage-parse
    [ -x "$BIN" ] || BIN="$BIN.exe"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

"$BIN" --emit-skb "$tmp" >/dev/null

# Compare file by file so the message names what drifted, rather than saying
# "the tree differs" about eight files at once.
drift=0
for f in "$tmp"/rules/*.json "$tmp"/manifest.json; do
    rel="skb/${f#"$tmp/"}"
    if [ ! -f "$rel" ]; then
        echo "  x  $rel is missing; regenerate with: mage-parse --emit-skb skb" >&2
        drift=$((drift + 1))
        continue
    fi
    # `diff -q` rather than a hash, so a real diff is one command away.
    if ! diff -q "$f" "$rel" >/dev/null 2>&1; then
        echo "  x  $rel differs from a fresh --emit-skb" >&2
        drift=$((drift + 1))
    fi
done

# The other direction: a file in the tree the generator does not produce. A
# database renamed in `skb.rs` would otherwise leave its old file behind,
# looking authoritative and read by nothing — which is the shape this whole
# item was about.
for rel in skb/rules/*.json; do
    gen="$tmp/rules/$(basename "$rel")"
    if [ ! -f "$gen" ]; then
        echo "  x  $rel is not produced by --emit-skb; it is an orphan" >&2
        drift=$((drift + 1))
    fi
done

if [ "$drift" -ne 0 ]; then
    echo >&2
    echo "     skb/ is generated from builtin_rules(). Edit prototype/src/skb.rs," >&2
    echo "     then run: mage-parse --emit-skb skb" >&2
    exit 1
fi

rules="$(python -c "
import json, io, glob
print(sum(len(json.load(io.open(f, encoding='utf-8'))) for f in glob.glob('skb/rules/*.json')))
")"
echo "  ok skb/ matches a fresh --emit-skb: $rules rules across 8 databases."

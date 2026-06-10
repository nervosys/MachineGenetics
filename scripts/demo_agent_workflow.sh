#!/usr/bin/env bash
# demo_agent_workflow.sh - end-to-end walk of the agent flow.
#
# Steps the script demonstrates:
#   1. Discover what's available    (read MECHGEN_ONTOLOGY.json)
#   2. Pick a template               (one of the framework's examples)
#   3. Compile to binary IR          (--target=abl-bytes -> .abl)
#   4. Decode for inspection         (--from=abl-bytes round-trip)
#   5. Dispatch on CpuBackend        (--run=abl-bytes)
#
# All without spawning the RAP server - shows the same surface the
# RAP methods (ontology/full, pipeline/recover-and-encode, abl/run)
# expose, but via CLI for easy human inspection.

set -uo pipefail

MGP="prototype/target/release/MechGen-parse.exe"
[ -x "$MGP" ] || MGP="prototype/target/release/MechGen-parse"

if [ ! -x "$MGP" ]; then
    echo "demo: building MechGen-parse..." >&2
    cargo build --release --manifest-path prototype/Cargo.toml --bin MechGen-parse \
        >/dev/null 2>&1
fi

ONTOLOGY="MECHGEN_ONTOLOGY.json"
DEMO_DIR=$(mktemp -d)
trap "rm -rf '$DEMO_DIR'" EXIT

separator() { printf '\n%s\n' "─────────────────────────────────────────────────────────────"; }

# ── Step 1: discover ─────────────────────────────────────────────────
separator
echo "STEP 1  Discover available templates"
echo "(reading $ONTOLOGY -- the same payload ontology/full returns)"
separator

if [ ! -f "$ONTOLOGY" ]; then
    "$MGP" --emit-ontology "$ONTOLOGY" >/dev/null
fi

# Show the examples-section entry count and the load-bearing fields.
python3 -c "
import json
with open('$ONTOLOGY') as f:
    o = json.load(f)
examples = o['sections']['examples']
print(f'  {len(examples)} parse-verified examples available')
for e in examples[:6]:
    print(f'    {e[\"name\"]:30s} {e[\"description\"]}')
print(f'    ... ({len(examples) - 6} more)')

print()
print(f'  Framework: {len(o[\"sections\"][\"framewerx_modules\"])} modules across categories:')
cats = {}
for e in o['sections']['framewerx_modules']:
    cats[e['category']] = cats.get(e['category'], 0) + 1
for cat, n in sorted(cats.items(), key=lambda kv: -kv[1])[:8]:
    print(f'    {cat:20s} {n}')
" 2>/dev/null || echo "  (python3 not available - skipping discover printout)"

# ── Step 2: pick a template ──────────────────────────────────────────
separator
echo "STEP 2  Pick a template"
separator

TEMPLATE="framework/framewerx/examples/flash_attention_block.mg"
echo "  Selected: $TEMPLATE"
echo
echo "  Source:"
sed 's/^/    /' "$TEMPLATE"

# ── Step 3: compile to Agentic Binary Language ──────────────────────────────────────────
separator
echo "STEP 3  Compile to binary IR (Agentic Binary Language container)"
echo "(equivalent to abl/encode over RAP)"
separator

Agentic Binary Language="$DEMO_DIR/flash_attention_block.abl"
"$MGP" --target=abl-bytes "$TEMPLATE" "$Agentic Binary Language"
if [ -f "$Agentic Binary Language" ]; then
    SIZE=$(wc -c < "$Agentic Binary Language")
    SRC_SIZE=$(wc -c < "$TEMPLATE")
    echo
    echo "  $TEMPLATE: $SRC_SIZE bytes (text)"
    echo "  $(basename "$Agentic Binary Language"): $SIZE bytes (binary IR)"
    echo "  First 32 bytes (hex):"
    xxd -l 32 -g 1 "$Agentic Binary Language" 2>/dev/null | sed 's/^/    /' || \
        od -An -tx1 -N 32 "$Agentic Binary Language" | sed 's/^/    /'
fi

# ── Step 4: decode round-trip ────────────────────────────────────────
separator
echo "STEP 4  Decode the Agentic Binary Language back to a MechGen view"
echo "(equivalent to abl/decode over RAP)"
separator

"$MGP" --from=abl-bytes "$Agentic Binary Language" 2>&1 | head -16 | sed 's/^/  /'

# ── Step 5: dispatch on CpuBackend ───────────────────────────────────
separator
echo "STEP 5  Dispatch the Agentic Binary Language on the CpuBackend"
echo "(equivalent to abl/run over RAP)"
separator

"$MGP" --run=abl-bytes "$Agentic Binary Language" 2>&1 | head -20 | sed 's/^/  /'

separator
echo "Done. The same five steps via RAP would be:"
echo "  1. POST ontology/full       (or ontology/section)"
echo "  2. agent picks a template and writes/adapts .mg source"
echo "  3. POST abl/encode         { source }"
echo "  4. POST abl/decode         { abl_hex }"
echo "  5. POST abl/run            { source }"
echo
echo "OR collapse 2-5 into one call:"
echo "     POST pipeline/recover-and-encode { source }"
separator

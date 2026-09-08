#!/usr/bin/env bash
# demo_agent_workflow.sh - end-to-end walk of the agent flow.
#
# Steps the script demonstrates:
#   1. Discover what's available    (read MAGE_ONTOLOGY.json)
#   2. Pick a template               (one of the framework's examples)
#   3. Compile to binary IR          (--target=abl-bytes -> .abl)
#   4. Decode for inspection         (--from=abl-bytes round-trip)
#   5. Dispatch on CpuBackend        (--run=abl-bytes)
#
# All without spawning the RAP server - shows the same surface the
# RAP methods (ontology/full, pipeline/recover-and-encode, abl/run)
# expose, but via CLI for easy human inspection.

set -uo pipefail

# Ask cargo where the binary is rather than guessing at
# `prototype/target/release/mage-parse.exe`.
#
# The guess resolves to a **leftover** on any machine whose cargo target
# directory is not the default. `~/.cargo/config.toml` sets a shared one for
# every Rust project here, so `prototype/target/` holds whatever was built
# before that change -- on 2026-09-08, a binary six days stale. A benchmark
# measuring with an old compiler is the "green result that is evidence about
# nothing" failure `scripts/find-mage-parse.sh` exists to end, and four of the
# figures below are pinned by `check-doc-counts.sh`.
#
# That file's own comment says "six checkers each carried their own copy": the
# sweep converted the six **checkers** and left the two benchmarks, the two
# demos and the agent wrapper still guessing. `MG`/`MGP` still override.
. "$(dirname "$0")/find-mage-parse.sh"
MGP="${MGP:-$(find_crate_bin prototype mage-parse release)}"

if [ ! -x "$MGP" ]; then
    echo "demo: building mage-parse..." >&2
    cargo build --release --manifest-path prototype/Cargo.toml --bin mage-parse \
        >/dev/null 2>&1
fi

ONTOLOGY="MAGE_ONTOLOGY.json"
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

ABL="$DEMO_DIR/flash_attention_block.abl"
"$MGP" --target=abl-bytes "$TEMPLATE" "$ABL"
if [ -f "$ABL" ]; then
    SIZE=$(wc -c < "$ABL")
    SRC_SIZE=$(wc -c < "$TEMPLATE")
    echo
    echo "  $TEMPLATE: $SRC_SIZE bytes (text)"
    echo "  $(basename "$ABL"): $SIZE bytes (binary IR)"
    echo "  First 32 bytes (hex):"
    xxd -l 32 -g 1 "$ABL" 2>/dev/null | sed 's/^/    /' || \
        od -An -tx1 -N 32 "$ABL" | sed 's/^/    /'
fi

# ── Step 4: decode round-trip ────────────────────────────────────────
separator
echo "STEP 4  Decode the Agentic Binary Language back to a MAGE view"
echo "(equivalent to abl/decode over RAP)"
separator

"$MGP" --from=abl-bytes "$ABL" 2>&1 | head -16 | sed 's/^/  /'

# ── Step 5: dispatch on CpuBackend ───────────────────────────────────
separator
echo "STEP 5  Dispatch the Agentic Binary Language on the CpuBackend"
echo "(equivalent to abl/run over RAP)"
separator

"$MGP" --run=abl-bytes "$ABL" 2>&1 | head -20 | sed 's/^/  /'

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

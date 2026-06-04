#!/usr/bin/env bash
# refine_oracle.sh — refine-protocol smoke-test wrapper.
#
# In propose mode: returns a deliberately broken MechGen source so the
# mechanical recovery pipeline (pattern-heal + structural) cannot save
# it — i.e. the bench is forced to invoke Stage-3 refine.
#
# In refine mode: returns the FIXED version of that source, so the
# bench records refine_succeeded=true and the wire-up is proven.
#
# Use this in CI to verify the refine-protocol plumbing without
# spending tokens on a real model.
#
# Usage from repo root (Windows: prefix `bash`):
#   cargo run --bin reliability-bench --manifest-path prototype/Cargo.toml \
#       -- --agent "subprocess:./scripts/agent_wrappers/refine_oracle.sh"

set -euo pipefail

# Drain stdin so the bench's writer doesn't see SIGPIPE.
PAYLOAD=$(cat)

if [ "${RDX_BENCH_MODE:-propose}" = "refine" ]; then
    # The broken source was `+f hi() -> s { "hi"` (no closing `}`,
    # and we deliberately corrupted it further with junk that the
    # mechanical recovery can't handle). Return the parsing fix.
    cat <<'MG'
+f hi() -> s { "hi" }
MG
else
    # Propose mode: return source that mechanical recovery can't fix.
    # Word-swap + corrupt token at start defeats both pattern-heal
    # and structural-balance / structural-completion.
    cat <<'MG'
%%% broken
MG
fi

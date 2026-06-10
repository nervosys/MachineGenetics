#!/usr/bin/env bash
# demo_subprocess_backend.sh - reference wrapper for the P94 subprocess
# backend protocol.
#
# An operator wires this (or anything implementing the same contract)
# into MechGen by registering a backend descriptor like:
#
#   {
#     "name": "my_accelerator",
#     "family": "asic",
#     ...
#     "dispatch": {
#       "kind": "subprocess",
#       "command": "/path/to/your_wrapper.sh"
#     }
#   }
#
# Then `MechGen-parse --backend=my_accelerator --run=abl-bytes model.abl`
# spawns this script with:
#   - stdin   = the full Agentic Binary Language container (binary)
#   - env     = RDX_BACKEND, RDX_ITEM_NAME (path of the .abl), RDX_INPUT_SHAPE
#   - stdout  = MUST be JSON matching `SubprocessResult` schema:
#       { "ok": bool, "dispatched": int, "output_shape": [int...],
#         "output_sum": float, "error": null | string }
#   - exit 0  = success; non-zero exit OR malformed stdout = wrapper error
#
# This reference script doesn't actually dispatch to any hardware -
# it counts the Agentic Binary Language bytes, echoes a stub result, and exits 0. Real
# wrappers replace the body with a call to vendor SDK CLI tools, an
# HTTP request to a remote inference service, etc.

set -euo pipefail

# Capture stdin Agentic Binary Language to a temp so we can both measure it and pass it
# along if the real wrapper wants it as a file.
Agentic Binary Language=$(mktemp --suffix=.abl)
trap "rm -f '$Agentic Binary Language'" EXIT
cat > "$Agentic Binary Language"

SIZE=$(wc -c < "$Agentic Binary Language")

# Reference wrapper: emit a stub SubprocessResult that proves the
# protocol works. A real backend would:
#   1. Parse the Agentic Binary Language container (see prototype/src/machine.rs)
#   2. For each item: decompile to its compute primitives
#   3. Dispatch to the vendor SDK / driver / remote API
#   4. Aggregate per-item results into the response below
cat <<EOF
{
  "ok": true,
  "dispatched": 1,
  "output_shape": [1, 1000],
  "output_sum": 0.0,
  "error": null
}
EOF

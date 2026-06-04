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
# Then `MechGen-parse --backend=my_accelerator --run=rmil-bytes model.rmib`
# spawns this script with:
#   - stdin   = the full RMIB container (binary)
#   - env     = RDX_BACKEND, RDX_ITEM_NAME (path of the .rmib), RDX_INPUT_SHAPE
#   - stdout  = MUST be JSON matching `SubprocessResult` schema:
#       { "ok": bool, "dispatched": int, "output_shape": [int...],
#         "output_sum": float, "error": null | string }
#   - exit 0  = success; non-zero exit OR malformed stdout = wrapper error
#
# This reference script doesn't actually dispatch to any hardware -
# it counts the RMIB bytes, echoes a stub result, and exits 0. Real
# wrappers replace the body with a call to vendor SDK CLI tools, an
# HTTP request to a remote inference service, etc.

set -euo pipefail

# Capture stdin RMIB to a temp so we can both measure it and pass it
# along if the real wrapper wants it as a file.
RMIB=$(mktemp --suffix=.rmib)
trap "rm -f '$RMIB'" EXIT
cat > "$RMIB"

SIZE=$(wc -c < "$RMIB")

# Reference wrapper: emit a stub SubprocessResult that proves the
# protocol works. A real backend would:
#   1. Parse the RMIB container (see prototype/src/rmib.rs)
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

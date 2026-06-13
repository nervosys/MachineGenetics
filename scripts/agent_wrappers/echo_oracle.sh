#!/usr/bin/env bash
# echo_oracle.sh — smoke-test wrapper for reliability-bench's
# --agent subprocess:<cmd> protocol.
#
# Reads a natural-language task description on stdin (ignored), writes
# a fixed minimal MAGE program on stdout. Always exits 0. Used to
# verify the bench plumbing works end-to-end without any LLM.
#
# Usage:
#   cargo run --bin reliability-bench --manifest-path prototype/Cargo.toml \
#       -- --agent "subprocess:./scripts/agent_wrappers/echo_oracle.sh"

# Drain stdin so the bench's writer doesn't get a SIGPIPE.
cat > /dev/null

# Emit a minimal MAGE function that parses cleanly today.
# (basic-001 reference, the shortest hello-world-shaped MG program.)
cat <<'MG'
+f hello() -> s { "Hello, World!" }
MG

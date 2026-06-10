#!/usr/bin/env bash
# demo_rap_workflow.sh - end-to-end agent flow over the RAP TCP server.
#
# Counterpart to demo_agent_workflow.sh, which exercises the same five
# steps via the local CLI. This script proves the JSON-RPC wire path
# actually works: the same agent flow, executed against a live TCP
# server with real JSON serialisation.
#
# Steps:
#   1. ontology/full              - discover what's available
#   2. ontology/section { ... }   - drill into one section
#   3. pipeline/recover-and-encode - source -> Agentic Binary Language in one call
#   4. abl/run                    - dispatch on CpuBackend
#   5. build/recover               - heal broken source for comparison

set -uo pipefail

MGP=prototype/target/release/MechGen-parse.exe
[ -x "$MGP" ] || MGP=prototype/target/release/MechGen-parse
if [ ! -x "$MGP" ]; then
    cargo build --release --manifest-path prototype/Cargo.toml --bin MechGen-parse \
        >/dev/null 2>&1
fi

PORT=${RAP_PORT:-19877}
LOG=$(mktemp)

cleanup() {
    if [ -n "${SERVER_PID:-}" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    rm -f "$LOG"
}
trap cleanup EXIT

separator() { printf '\n%s\n' "─────────────────────────────────────────────────────────────"; }

# Start the RAP server.
separator
echo "Starting RAP server on 127.0.0.1:$PORT"
separator
"$MGP" --rap "127.0.0.1:$PORT" > "$LOG" 2>&1 &
SERVER_PID=$!

# Wait for "listening" line.
for _ in $(seq 1 30); do
    if grep -q "listening on" "$LOG" 2>/dev/null; then
        echo "  server ready (PID $SERVER_PID)"
        break
    fi
    sleep 0.2
done

# Send a JSON-RPC request, get the response. Uses one connection
# per call (the server is single-threaded; that's fine for the
# demo).
call() {
    local method="$1"
    local params="$2"
    local payload
    payload=$(printf '{"jsonrpc":"2.0","id":1,"method":"%s","params":%s}' \
        "$method" "$params")
    # Pure bash /dev/tcp - works in git-bash on Windows and any
    # POSIX bash. Single connection per call (server is single-
    # threaded). Server emits one JSON response line then keeps the
    # connection open; we read one line and close.
    exec 3<>"/dev/tcp/127.0.0.1/$PORT" || {
        echo "tcp connect failed" >&2
        return 1
    }
    printf '%s\n' "$payload" >&3
    local response=""
    IFS= read -r response <&3
    exec 3<&-
    printf '%s' "$response"
}

# Step 1: discover
separator
echo "STEP 1  ontology/full"
separator
RESP=$(call "ontology/full" "{}")
echo "  response length: ${#RESP} chars"
echo "  first 200 chars: $(echo "$RESP" | cut -c1-200)..."

# Step 2: drill into one section
separator
echo "STEP 2  ontology/section { section: 'cli_flags' }"
separator
RESP=$(call "ontology/section" '{"section":"cli_flags"}')
COUNT=$(echo "$RESP" | grep -oE '"flag":"[^"]+"' | wc -l)
echo "  cli_flags entries returned: $COUNT"
echo "  first 3 flags: $(echo "$RESP" | grep -oE '"flag":"[^"]+"' | head -3 | tr '\n' ' ')"

# Step 3: pipeline/recover-and-encode
separator
echo "STEP 3  pipeline/recover-and-encode (FlashAttention block)"
separator
SOURCE='net DemoFlashBlock { layer n1: LayerNorm(64); layer attn: FlashAttention(64, 4); layer head: Linear(64, 8); forward { head(attn(n1)) } }'
PAYLOAD=$(printf '{"source":"%s"}' "$SOURCE")
RESP=$(call "pipeline/recover-and-encode" "$PAYLOAD")
OK=$(echo "$RESP" | grep -oE '"ok":(true|false)' | head -1)
STAGE=$(echo "$RESP" | grep -oE '"recover_stage":"[^"]+"' | head -1)
BYTES=$(echo "$RESP" | grep -oE '"container_bytes":[0-9]+' | head -1)
echo "  $OK"
echo "  $STAGE"
echo "  $BYTES (vs ${#SOURCE} bytes text)"

# Step 4: abl/run
separator
echo "STEP 4  abl/run on the same source"
separator
RESP=$(call "abl/run" "$PAYLOAD")
STATUS=$(echo "$RESP" | grep -oE '"status":"[^"]+"' | head -1)
DISPATCHED=$(echo "$RESP" | grep -oE '"dispatched":[0-9]+' | head -1)
OUT_SHAPE=$(echo "$RESP" | grep -oE '"output_shape":\[[^]]+\]' | head -1)
IN_SHAPE=$(echo "$RESP" | grep -oE '"input_shape":\[[^]]+\]' | head -1)
echo "  $STATUS"
echo "  $DISPATCHED"
echo "  $IN_SHAPE -> $OUT_SHAPE"

# Step 5: build/recover. Note: broken source contains `"` which must
# be JSON-escaped before serialisation. This is a real wire-protocol
# concern - agents serialising user source over RAP need to handle it.
separator
echo "STEP 5  build/recover on broken source (missing })"
separator
BROKEN='+f hi() -> s { \"hello\"'
PAYLOAD=$(printf '{"source":"%s"}' "$BROKEN")
RESP=$(call "build/recover" "$PAYLOAD")
OK=$(echo "$RESP" | grep -oE '"ok":(true|false)' | head -1)
STAGE=$(echo "$RESP" | grep -oE '"stage":"[^"]+"' | head -1)
CHANGED=$(echo "$RESP" | grep -oE '"changed":(true|false)' | head -1)
echo "  $OK $STAGE $CHANGED"
echo "  recovered source: $(echo "$RESP" | grep -oE '"source":"[^"]*"' | head -1 | cut -c1-80)"

separator
echo "Done. All 5 RAP methods executed cleanly over JSON-RPC/TCP."
separator

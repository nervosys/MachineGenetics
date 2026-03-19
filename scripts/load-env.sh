#!/usr/bin/env bash
# Load .env into the current shell session (service-scoped, not machine-global).
# Usage:  source scripts/load-env.sh
#         . scripts/load-env.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
ENV_FILE="${ROOT_DIR}/.env"

if [ ! -f "$ENV_FILE" ]; then
    echo "ERROR: ${ENV_FILE} not found." >&2
    echo "       Copy .env.example to .env and fill in your credentials:" >&2
    echo "         cp .env.example .env" >&2
    exit 1
fi

# Export each non-comment, non-empty line.
while IFS= read -r line || [ -n "$line" ]; do
    # Skip blanks and comments.
    [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue
    # Strip surrounding quotes from the value.
    export "${line?}"
done < "$ENV_FILE"

echo "Loaded OpenTelemetry environment from ${ENV_FILE}"

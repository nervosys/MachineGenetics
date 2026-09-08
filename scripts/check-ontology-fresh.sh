#!/usr/bin/env bash
# The committed `MAGE_ONTOLOGY.json` must match what the compiler generates now.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# The ontology is the machine-readable description of this language, and
# `DOCS.md` tells readers to *prefer* it over the prose. It was committed on
# 2026-06-12 and never regenerated, so it advertised ABL container version 2
# while the encoder had moved to 3 — and `decode` rejects a version mismatch, so
# anyone building to the published ontology produced artifacts this toolchain
# refuses. **A generated file that is committed but not checked is just a stale
# file with extra authority.**
#
# That check has existed since, as six lines inlined in
# `.github/workflows/ci.yml`. It is a script now for two reasons, neither of
# them tidiness:
#
#   1. **It could not be run before pushing.** Every other check here is
#      `bash scripts/check-*.sh`; this one was a YAML fragment, so the only way
#      to run it was to read the workflow and retype the commands.
#
#   2. **It was outside the guard that makes sure checkers are reached.**
#      `check-ci-paths.sh` audits "every checker is reached by a CI step" by
#      walking `scripts/`. A checker living only in the workflow is invisible to
#      it — so the one mechanism this repository built to catch unreached
#      checkers could not see this one.
#
# It also compared **bytes**. Git checks this file out CRLF on Windows and
# `--emit-ontology` writes LF, so anyone who did retype the commands on the
# platform this repository is developed on saw every line differ with the
# content identical. Same defect `check-ci-floors.sh` had, same fix
# `check-skb-tree.sh` already carried: `--strip-trailing-cr`.
#
# The comparison is exact in every way that matters — a changed figure, a new
# section, a removed field all fail. Only the line terminator is forgiven,
# because it is the one difference git itself introduces.
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

. scripts/find-mage-parse.sh
BIN="$(build_and_find_mage_parse)"

DOC=MAGE_ONTOLOGY.json
if [ ! -f "$DOC" ]; then
    echo "  x  $DOC is missing. Regenerate it:" >&2
    echo "     cargo run --release --manifest-path prototype/Cargo.toml \\" >&2
    echo "       --bin mage-parse -- --emit-ontology $DOC" >&2
    exit 1
fi

fresh="$(mktemp)"
trap 'rm -f "$fresh"' EXIT
"$BIN" --emit-ontology "$fresh" >/dev/null

if diff -q --strip-trailing-cr "$DOC" "$fresh" >/dev/null 2>&1; then
    echo "  ok $DOC matches a fresh generation."
    exit 0
fi

echo "  x  $DOC is stale — the compiler generates something different:" >&2
diff --strip-trailing-cr "$DOC" "$fresh" | head -40 | sed 's/^/     /' >&2
echo >&2
echo "     Regenerate it in the same commit as the change that moved it:" >&2
echo "     cargo run --release --manifest-path prototype/Cargo.toml \\" >&2
echo "       --bin mage-parse -- --emit-ontology $DOC" >&2
exit 1

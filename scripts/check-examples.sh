#!/usr/bin/env bash
# Pin which shipped examples typecheck, so the set cannot rot further in silence.
#
# ─────────────────────────────────────────────────────────────────────────────
# THE STATE THIS RECORDS
# ─────────────────────────────────────────────────────────────────────────────
#
# On 2026-08-05, **11 of 12 examples under `examples/` failed `--check`**, and
# nothing anywhere checked them: CI runs `cargo test`, and the examples are data,
# not tests. Ten shared one parse error (`use std::env;` — Rust's `::` where
# MAGE canonical wants `.`) and one failed type-checking.
#
# They are *not* fixed here. A mechanical `::` → `.` conversion was tried and
# does not work: every example then fails deeper in the file on some other
# construct. Repairing them means deciding what each was meant to demonstrate
# and rewriting it, across eleven files — real work with real judgement in it,
# and a different job from noticing.
#
# So this makes the debt explicit instead of silent. It fails when:
#
#   * an example that currently checks stops checking  — a regression, and the
#     thing most worth catching;
#   * an example listed as broken starts checking      — good news, but the list
#     must be updated so the count means something.
#
# The second case matters as much as the first. A known-bad list nobody prunes
# becomes a place where fixes go unrecorded, and then the list is just another
# stale document — which is the failure this repository spent 2026-08-05
# discovering it had, repeatedly.
#
# Usage: scripts/check-examples.sh [path-to-mage-parse]

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO"

MP="${1:-}"
if [ -z "$MP" ]; then
    for c in prototype/target/release/mage-parse prototype/target/release/mage-parse.exe \
             prototype/target/debug/mage-parse prototype/target/debug/mage-parse.exe; do
        [ -x "$c" ] && MP="$c" && break
    done
fi
if [ -z "$MP" ] || [ ! -x "$MP" ]; then
    echo "check-examples: no mage-parse binary; build it first" >&2
    exit 2
fi

# Examples known not to typecheck, with the first error each reports. Keep the
# reason: a bare list of names decays into "these are just broken" and stops
# being actionable.
declare -A KNOWN_BROKEN=(
    [agent-swarm]="parse: expected item, found Colon (use std::agent)"
    [autonomous-pipeline]="parse: expected item, found Colon"
    [cli-tool]="parse: expected item, found Colon (use std::env)"
    [cost-aware-optimizer]="parse: expected item, found Colon"
    [effects-showcase]="parse: expected item, found Colon"
    [hello-world]="type: expected a collection, found str"
    [http-client]="parse: expected item, found Colon (use std::io)"
    [live-compiler]="parse: expected item, found Colon"
    [multilang-bindings]="parse: expected item, found Colon"
    [safe-plugin-host]="parse: expected item, found Colon"
    [swarm-code-review]="parse: expected item, found Colon"
)

fail=0
checked=0
newly_ok=()
regressed=()

for dir in examples/*/; do
    src="$dir/src/main.mg"
    [ -f "$src" ] || continue
    name="$(basename "$dir")"
    checked=$((checked + 1))
    # Capture first, then match. `"$MP" --check … | grep -q` under
    # `set -o pipefail` reports failure even when grep *succeeds*: grep -q exits
    # at the first match, mage-parse takes SIGPIPE, and pipefail surfaces that
    # as the pipeline's status. Every broken example then read as passing.
    #
    # `scripts/purge-video-from-history.sh` carries a comment about this exact
    # trap, written after hitting it twice. Knowing about a footgun evidently
    # does not stop one standing on it.
    out="$("$MP" --check "$src" 2>&1)"
    if printf '%s' "$out" | grep -q "error:"; then
        ok=0
    else
        ok=1
    fi

    if [ -n "${KNOWN_BROKEN[$name]:-}" ]; then
        [ "$ok" = "1" ] && newly_ok+=("$name")
    else
        [ "$ok" = "0" ] && regressed+=("$name")
    fi
done

echo "Checked $checked examples; ${#KNOWN_BROKEN[@]} are known-broken."

if [ "${#regressed[@]}" -gt 0 ]; then
    echo "REGRESSION — these used to typecheck and no longer do:" >&2
    printf '  %s\n' "${regressed[@]}" >&2
    fail=1
fi

if [ "${#newly_ok[@]}" -gt 0 ]; then
    echo "These are listed as broken but now typecheck — remove them from" >&2
    echo "KNOWN_BROKEN in this script so the count stays meaningful:" >&2
    printf '  %s\n' "${newly_ok[@]}" >&2
    fail=1
fi

[ "$fail" -eq 0 ] && echo "OK — the example set matches its recorded state."
exit "$fail"

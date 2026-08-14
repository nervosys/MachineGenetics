#!/usr/bin/env bash
# Every `.mg` file in the repository either typechecks, or is listed here as a
# known sketch.
#
# Why this exists: `stdlib/` is 4,402 lines across 25 `.mg` files, and none of
# them are MAGE. They are Rust — `pub trait Read`, `&mut self`,
# `let total = 0usize;`, `pub mod agent;` — sitting behind a MAGE extension.
# Nothing read them: the compiler never opens `stdlib/`, no script checked them,
# and `u std.io` resolves without them because imports are nominal. So 25 files
# could sit there for months claiming to be the standard library of a language
# they were not written in.
#
# That is the same failure the twelve examples had before they were rewritten,
# and it is worse here, because an agent reading `stdlib/std/io.mg` to learn
# MAGE idiom learns Rust instead. A file with no consumer has no error message.
# This script is the consumer.
#
# The allowlist is deliberately unpleasant to add to. A `.mg` file that does not
# typecheck is either a bug to fix or a sketch to be honest about; the list
# exists so the count can only go down.
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

BIN=prototype/target/release/mage-parse
if [ ! -x "$BIN" ]; then
    echo "building the compiler first..."
    cargo build --release --manifest-path prototype/Cargo.toml --bin mage-parse
fi

# Known sketches: not MAGE, not checked, kept for reference only.
#
# `stdlib/` is Rust. Rewriting it into MAGE is real work and worth doing only
# alongside a consumer that keeps it honest — otherwise it rots exactly as it
# did. Until then it is listed, not silently skipped.
# Each entry needs a reason. "It fails" is not one.
SKETCHES=(
    # 25 files, 4,402 lines, all Rust. Rewriting them into MAGE is real work
    # and worth doing only alongside a consumer that keeps them honest —
    # otherwise they rot exactly as they did.
    "stdlib/"

    # A second, older example set, separate from the twelve in `examples/`
    # that `check-examples.sh` pins. Nothing checked these, and they drifted
    # the same way the shipped twelve had: `pub fn` with `?`-conditionals,
    # `U` used as a name, `fn` in expression position. Same rewrite job,
    # smaller. `hello.mg`, `structs.mg` and `effects.mg` are repaired and NOT
    # listed — rewriting them is what surfaced the `data`-sum and closure bugs.
    "prototype/examples/analysis.mg"
    "prototype/examples/pipeline_demo.mg"
    "prototype/examples/spec_synthesis.mg"

    # These reference `Tensor`, `Module` and `ParamStore` as bare type names.
    # Nothing defines them: they are in no module here, the ontology publishes
    # no such type, and the spec documents none. The files describe a tensor
    # framework the type system does not yet have.
    "framework/framewerx/src/loss.mg"
    "framework/framewerx/src/module.mg"
    "framework/framewerx/src/neurosymbolic.mg"
    "framework/framewerx/src/train.mg"

    # Parses and typechecks; fails the *shape* checker with a real-looking
    # complaint (a linear layer fed [1, 256, 20, 2] where it wants last dim
    # 256). Either the example or the shape rule is wrong, and telling which
    # needs someone who knows the intended architecture — it is not a
    # syntax-drift problem like the rest of this list.
    "framework/framewerx/examples/resnet_classifier.mg"

    # Untyped parameters (`f sumto(n){ … }`), which make `t += i` report
    # `unknown operator: +=` — the operator is fine, the operand type is
    # unknown. That diagnostic is its own bug; see HANDOFF.md.
    "benchmarks/cross_lang/tasks.mg"
)

is_sketch() {
    local f="$1"
    for s in "${SKETCHES[@]}"; do
        case "$f" in "$s"*) return 0 ;; esac
    done
    return 1
}

checked=0
skipped=0
failed=0
failures=""

while IFS= read -r f; do
    if is_sketch "$f"; then
        skipped=$((skipped + 1))
        continue
    fi
    checked=$((checked + 1))
    # Capture, then match. `cmd | grep -q` under `set -o pipefail` reports
    # failure when grep exits early and the writer takes SIGPIPE, so every
    # match reads as no-match. This has cost two bugs in this repo already.
    out="$("$BIN" --check "$f" 2>&1 || true)"
    if printf '%s' "$out" | grep -qE 'parse error|Errors: [1-9]'; then
        failed=$((failed + 1))
        first="$(printf '%s' "$out" | grep -E 'parse error|error:' | head -1)"
        failures="${failures}  ${f}
      ${first}
"
    fi
done < <(find . -name '*.mg' -not -path './prototype/target/*' | sed 's|^\./||' | sort)

echo "Checked $checked .mg files; $skipped skipped as known sketches."

if [ "$failed" -gt 0 ]; then
    echo
    echo "$failed file(s) do not typecheck:"
    printf '%s' "$failures"
    echo "Fix the file, or — if it is genuinely a sketch — add it to SKETCHES"
    echo "in this script and say why in the same commit."
    exit 1
fi

echo "OK — every .mg source outside the sketch list typechecks."

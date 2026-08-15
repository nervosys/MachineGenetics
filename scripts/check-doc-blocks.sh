#!/usr/bin/env bash
# Every ```mg / ```MAGE code block in the documentation must parse — or be
# counted in the baseline below, which may only shrink.
#
# Why this exists: 258 MAGE-tagged blocks live in the markdown, and 177 of them
# did not parse when anyone first checked. Nine are deliberate fragments
# (they contain `...`). The other 168 are Rust wearing a MAGE fence:
#
#     use std::llm::{LLM, Prompt, Response};      41 blocks, `::` paths
#     pub fn save(data: &str) -> Result<(), E>    10 blocks, Rust signatures
#     u std.agent.{Agent, Message}                18 blocks, brace imports
#
# This is the shipped examples' story again — they were "aspirational Rust that
# had never compiled" — but in the documents an agent *reads to learn the
# language*, and in `training/prompts/`, whose few-shot blocks teach a model
# what MAGE looks like. A model shown `I ~ Counter { … }` and
# `Counter @{ count: 0 }` learns two constructs that do not parse.
#
# Fixing 168 blocks is its own piece of work. This script makes the number a
# ratchet in the meantime: it can go down, never up. A new failing block, or a
# new file with failing blocks, fails the check.
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

BIN=prototype/target/release/mage-parse
if [ ! -x "$BIN" ]; then
    echo "building the compiler first..."
    cargo build --release --manifest-path prototype/Cargo.toml --bin mage-parse
fi

BASELINE=scripts/doc-blocks-baseline.txt

# Capture, then match. `cmd | grep -q` under `set -o pipefail` reports failure
# when grep exits early and the writer takes SIGPIPE, so every match reads as
# no-match. This has cost two bugs in this repo already.
measured="$(python - <<'PY'
import io, os, subprocess, collections

BIN = os.path.join('prototype', 'target', 'release', 'mage-parse')
TAGS = ('mg', 'mage')
ELLIPSIS = ('...', '…')
# A block introduced as broken is *supposed* not to parse — `few-shot-repair.md`
# pairs a broken input with its fix, and counting the input as a failure would
# put intentional errors in the baseline and muddy the ratchet.
BROKEN_MARKERS = ('broken', 'invalid', 'wrong', 'incorrect', 'do not', 'bad ')
probe = os.path.join('prototype', 'target', '.docblock.mg')

counts = collections.Counter()
for dirpath, dirnames, filenames in os.walk('.'):
    dirnames[:] = [d for d in dirnames if d not in ('.git', 'target', 'node_modules')]
    for fn in sorted(filenames):
        if not fn.endswith('.md'):
            continue
        path = os.path.join(dirpath, fn)
        rel = os.path.relpath(path, '.').replace(os.sep, '/')
        lines = io.open(path, encoding='utf-8', errors='replace').read().split('\n')
        i = 0
        while i < len(lines):
            st = lines[i].strip()
            if st.startswith('```') and st[3:].strip().lower() in TAGS:
                start, j = i + 1, i + 1
                while j < len(lines) and not lines[j].strip().startswith('```'):
                    j += 1
                body = '\n'.join(lines[start:j])
                i = j + 1
                # The nearest non-blank line above the fence labels the block.
                label = ''
                k = start - 2
                while k >= 0 and not lines[k].strip():
                    k -= 1
                if k >= 0:
                    label = lines[k].strip().lower()
                if not body.strip() or any(e in body for e in ELLIPSIS):
                    continue
                if any(m in label for m in BROKEN_MARKERS):
                    continue
                io.open(probe, 'w', encoding='utf-8').write(body + '\n')
                r = subprocess.run([BIN, '--check', probe], capture_output=True, text=True)
                if 'parse error' in (r.stdout or '') + (r.stderr or ''):
                    counts[rel] += 1
            else:
                i += 1
for rel in sorted(counts):
    print('%s %d' % (rel, counts[rel]))
PY
)"

if [ ! -f "$BASELINE" ]; then
    printf '%s\n' "$measured" > "$BASELINE"
    echo "baseline written to $BASELINE"
    exit 0
fi

fail=0
total_now=0
total_was=0

# `awk`, not a bash read-loop: a blank or short line made `$((total + was))`
# fail with "invalid arithmetic operator", and the totals came out zero while
# the script still reported success. A checker that cannot count is worse than
# no checker.
total_was=$(awk '{ n += $2 } END { print n+0 }' "$BASELINE")
total_now=$(printf '%s
' "$measured" | awk 'NF == 2 { n += $2 } END { print n+0 }')

while read -r file was; do
    [ -n "${file:-}" ] || continue
    [ -n "${was:-}" ] || continue
    now=$(printf '%s
' "$measured" | awk -v f="$file" '$1 == f { print $2 }')
    now=${now:-0}
    if [ "$now" -gt "$was" ]; then
        echo "  x  $file: baseline $was failing block(s), $now now"
        fail=1
    elif [ "$now" -lt "$was" ]; then
        echo "  +  $file: $was -> $now — fixed; refresh the baseline in this commit"
        fail=1
    fi
done < "$BASELINE"

while read -r file now; do
    [ -n "${file:-}" ] || continue
    [ -n "${now:-}" ] || continue
    if ! awk -v f="$file" '$1 == f { found = 1 } END { exit !found }' "$BASELINE"; then
        echo "  x  $file: $now failing block(s), not in the baseline"
        fail=1
    fi
done <<< "$measured"

echo "Doc blocks failing to parse: $total_now (baseline $total_was)."

if [ "$fail" -ne 0 ]; then
    echo
    echo "The count may only go down. If you fixed blocks, regenerate the"
    echo "baseline in the same commit:  rm $BASELINE && bash $0"
    exit 1
fi

echo "OK — no new unparseable documentation blocks."

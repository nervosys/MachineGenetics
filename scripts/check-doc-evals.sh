#!/usr/bin/env bash
# Every documentation block that defines an entry point must *run*.
#
# `check-doc-blocks.sh` proves the blocks typecheck. `--check` and `--eval` are
# independent oracles, though, and agreeing with one says nothing about the
# other — this repository has found four bugs of the shape "typechecks, then
# does not evaluate" and three of the inverse. The documentation had only ever
# been checked.
#
# Running it found the largest one yet: **thirteen of the seventeen builtin
# names `resolve` registers had no arm in the evaluator**. `assert` — the only
# assertion the language has, reached for by every `@test` in the docs — could
# only ever fail with `unknown function`. So could `panic`, `assert_eq`,
# `todo`, `vec` and `dbg`. It is the `println` bug again, one drawer over.
#
# What counts as a failure here is narrow on purpose: an error the *language*
# is responsible for. A capability error from the host — a file the example
# reads not existing — is not a defect in the block, and a block that reaches a
# capability the interpreter cannot perform (`net`, `agent`, `llm`) is skipped
# rather than reported.
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

BIN=prototype/target/release/mage-parse
if [ ! -x "$BIN" ]; then
    echo "building the compiler first..."
    cargo build --release --manifest-path prototype/Cargo.toml --bin mage-parse
fi

python - "$@" <<'PY'
import io
import os
import re
import subprocess
import sys

BIN = os.path.join('prototype', 'target', 'release', 'mage-parse')
TAGS = ('mg', 'mage')
ELLIPSIS = ('...', '…')
BROKEN = ('broken', 'invalid', 'wrong', 'incorrect', 'do not', 'bad ')
probe = os.path.join('prototype', 'target', '.docevals.mg')

# Capabilities the interpreter cannot perform, plus reads that would block.
UNRUNNABLE = ('net.', 'agent.', 'llm.', 'gpu.', 'process.', 'swarm.', 'json.',
              'kb.', 'io.read_line', 'io.read', 'fs.walk', 'time.sleep')

# Errors the language owns. An OS error reaching a real resource is the
# example's environment, not its correctness.
OURS = ('unknown function', 'parse error', 'type mismatch', 'assertion failed',
        'not callable', 'unresolved', 'wrong number of arguments',
        'no method', 'unknown effect', 'expects')

MAIN = re.compile(r'^\s*(?:\+f|f|pub\s+fn|fn)\s+main\s*\(', re.M)
TEST = re.compile(r'@test\s*\n\s*(?:\+f|f|pub\s+fn|fn)\s+(\w+)\s*\(', re.M)

ran = 0
failed = 0
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
                label = ''
                k = start - 2
                while k >= 0 and not lines[k].strip():
                    k -= 1
                if k >= 0:
                    label = lines[k].strip().lower()
                if not body.strip() or any(e in body for e in ELLIPSIS):
                    continue
                if any(m in label for m in BROKEN):
                    continue
                if any(ns in body for ns in UNRUNNABLE):
                    continue
                entries = []
                if MAIN.search(body):
                    entries.append('main')
                entries.extend(TEST.findall(body))
                if not entries:
                    continue
                io.open(probe, 'w', encoding='utf-8').write(body + '\n')
                for entry in entries:
                    try:
                        r = subprocess.run([BIN, '--eval', probe, entry],
                                           capture_output=True, text=True, timeout=20)
                    except subprocess.TimeoutExpired:
                        print('  x  %s:%d  %s() did not terminate' % (rel, start + 1, entry))
                        failed += 1
                        ran += 1
                        continue
                    out = ((r.stdout or '') + (r.stderr or '')).strip()
                    ran += 1
                    low = out.lower()
                    if any(m in low for m in OURS):
                        failed += 1
                        first = [l for l in out.split('\n') if 'error' in l.lower()]
                        print('  x  %s:%d  %s()  %s'
                              % (rel, start + 1, entry, (first[0] if first else out)[:100]))
            else:
                i += 1

print('doc_evals=%d' % ran)
print('Ran %d documentation entry point(s); %d failed.' % (ran, failed))
sys.exit(1 if failed else 0)
PY

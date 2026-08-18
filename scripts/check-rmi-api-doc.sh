#!/usr/bin/env bash
# Every item `RecursiveMachineIntelligence/docs/api.md` documents must exist in
# the crate.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# `api.md` is 1,463 lines of signatures and nothing compiled or resolved any of
# them. Found by accident on 2026-08-18 while checking something else:
#
#   * The FFI Bridge section documented `FfiValue` and `FfiFuncPtr`, which
#     appear in **no source file**, and marked `call_unchecked` `unsafe` when it
#     is safe — painting a raw-pointer FFI surface where the real registry
#     passes RMIL values to safe Rust closures.
#   * Every "Module:" line read `framewerx::…`. The crate is `rmi`. All twelve
#     paths were wrong and all twelve modules exist, so an agent following them
#     concluded the modules were missing.
#   * `CheckpointManager`'s entire method list was wrong — `save_checkpoint`
#     for `save`, `&str` ids for `Uuid`, a constructor that never took those
#     arguments. The *type* resolves, which is what made it hard to notice:
#     nothing fails until you call a method.
#
# A name-level check is deliberately weak — it cannot see a wrong signature on a
# function that exists, which is most of the `CheckpointManager` drift. It is
# what can be checked cheaply and without compiling the crate's docs, and it
# catches the whole-API-invented case, which is the one that wastes the most
# time. `cargo doc` remains the authority.
#
# The baseline works like `check-doc-blocks.sh`: a recorded count that can only
# shrink. Fixing entries lowers it; documenting something that does not exist
# raises it and fails.
#
# Usage: scripts/check-rmi-api-doc.sh
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

BASELINE_FILE=scripts/rmi-api-doc-baseline.txt

python - "$BASELINE_FILE" <<'PY'
import io
import os
import re
import sys

baseline_file = sys.argv[1]
doc_path = 'RecursiveMachineIntelligence/docs/api.md'
doc = io.open(doc_path, encoding='utf-8', errors='replace').read().replace('\r\n', '\n')

# Item names introduced in the documented signatures.
names = set()
for block in re.findall(r'```rust\n(.*?)```', doc, re.S):
    for kind in ('fn', 'struct', 'enum', 'trait', 'type'):
        names |= set(re.findall(r'\bpub ' + kind + r' (\w+)', block))

src = []
for root, _dirs, files in os.walk(os.path.join('RecursiveMachineIntelligence', 'src')):
    for f in files:
        if f.endswith('.rs'):
            src.append(io.open(os.path.join(root, f), encoding='utf-8', errors='replace').read())
src = '\n'.join(src)

# Word-boundary match: the name has to appear as an identifier, not as a
# substring of a longer one. `save` must not be satisfied by `save_with_tensors`.
missing = sorted(n for n in names if not re.search(r'\b' + re.escape(n) + r'\b', src))

print('documented items: %d' % len(names))
print('not found in src/: %d' % len(missing))
for m in missing:
    print('  x  %s' % m)

# Module paths, checked separately — these were all wrong at once, and a wrong
# crate prefix is invisible to the name check above.
bad_paths = []
for mod in re.findall(r'^\*\*Module:\*\* `([\w:]+)`', doc, re.M):
    parts = mod.split('::')
    if not parts or parts[0] != 'rmi':
        bad_paths.append('%s (crate is `rmi`)' % mod)
        continue
    rel = os.path.join('RecursiveMachineIntelligence', 'src', *parts[1:])
    if not (os.path.isdir(rel) or os.path.isfile(rel + '.rs')):
        bad_paths.append('%s (no such module)' % mod)
for b in bad_paths:
    print('  x  Module: %s' % b)

try:
    baseline = int(io.open(baseline_file, encoding='utf-8').read().strip())
except OSError:
    baseline = None

print()
if bad_paths:
    print('FAILED - %d documented module path(s) do not resolve.' % len(bad_paths))
    sys.exit(1)
if baseline is None:
    print('no baseline recorded; write %d to %s' % (len(missing), baseline_file))
    sys.exit(1)
if len(missing) > baseline:
    print('FAILED - %d documented items are missing from the crate; baseline is %d.'
          % (len(missing), baseline))
    print('Documenting an item that does not exist is how this file came to describe')
    print('an FFI API with no implementation. Fix the entry, or the source.')
    sys.exit(1)
if len(missing) < baseline:
    print('%d missing (baseline %d) — the ratchet moved. Update %s to %d in this commit.'
          % (len(missing), baseline, baseline_file, len(missing)))
    sys.exit(1)
print('OK - %d documented items, %d missing (at the baseline), all module paths resolve.'
      % (len(names), len(missing)))
PY

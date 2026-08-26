#!/usr/bin/env bash
# Every Rust item `internals/*.md` documents must exist in `prototype/src`.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# `README.md` calls `internals/` "Compiler-internals documentation". It is
# written in the present tense, as description of the shipped compiler — and
# measured on 2026-08-25, **15 of its 36 documented `pub` items do not exist**.
#
# The prose is further from the code than the signatures are. Chapter 1 opens:
#
#   "Each stage is a separate crate with a clean query-based interface."
#       `prototype` is ONE crate. 64 files, one `[package]`.
#   "The query engine (based on Salsa) tracks dependencies automatically."
#       `salsa` is not a dependency of any crate in this repository, and
#       nothing in `prototype/src` implements a query cache.
#   "The `CompileSession` holds all configuration for a compilation."
#       There is no `CompileSession`.
#
# `CompileSession`, `DefId`, `InferCtxt`, `TraitObligation`, `EffectChecker`
# are rustc/salsa-shaped names for a compiler that was designed and not built.
# That is a legitimate thing to have written; presenting it as documentation of
# what exists is not, and it is the same failure as `cookbook/` — the most
# detailed documentation in the repository, describing a system that was never
# there. An agent reading this to learn the codebase learns an architecture it
# will not find.
#
# The 12 blocks holding those 15 items now carry a `**Not implemented.**`
# label and are skipped, which is MAGE_SPEC.md's precedent rather than a way
# of getting to zero: the design text stays, the false impression goes, and the
# count of skipped blocks is printed on every run so the size of the gap stays
# visible. Each of the 15 was checked individually first -- none is a rename.
# The baseline is therefore 0, and it means "nothing here claims to exist
# without existing", not "there is nothing left to do here".
#
# ─────────────────────────────────────────────────────────────────────────────
# HOW IT DECIDES
# ─────────────────────────────────────────────────────────────────────────────
#
# A documented name must have a **definition** in `prototype/src` — `fn`,
# `struct`, `enum`, `trait`, `type`, `const`, `static`, `mod`. Not merely a
# mention: `check-rmi-api-doc.sh` used a bare-name search and 8 of its items
# turned out to be satisfied by English words in comments. This one starts with
# the stronger criterion rather than earning it later.
#
# The baseline can only shrink, like `doc-blocks-baseline.txt`. Fixing an entry
# lowers it; documenting something that does not exist raises it and fails; and
# an entry that has silently started existing also fails, because a baseline
# nobody is required to shrink is just a list.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHAT IT CANNOT SEE
# ─────────────────────────────────────────────────────────────────────────────
#
# **Prose.** Every one of the three claims quoted above is invisible here —
# they name no `pub` item. Nothing checks prose, which is why a false rule in a
# document outranks a broken example: a reader follows the rule where the
# examples do not reach. The three are labelled in `internals/01-architecture.md`
# instead, by hand, and that labelling has no mechanism behind it either.
#
# Signatures, too: a documented `fn` whose parameters are wrong passes here, as
# it does in the `rmi` checker. See HANDOFF.md rule 10.
#
# Usage:
#     bash scripts/check-internals-doc.sh

set -o nounset
set -o pipefail

cd "$(dirname "$0")/.." || exit 1

BASELINE_FILE=scripts/internals-doc-baseline.txt

python - "$BASELINE_FILE" <<'PY'
import io
import os
import re
import sys

baseline_file = sys.argv[1]

DOC_DIR = 'internals'
SRC = os.path.join('prototype', 'src')

if not os.path.isdir(DOC_DIR):
    print('  x  %s/ is missing; the documentation set moved' % DOC_DIR)
    sys.exit(1)

# A block whose label says it describes a design rather than the code is
# skipped, exactly as `check-doc-blocks.sh` skips a MAGE block labelled
# "Invalid MAGE today". `MAGE_SPEC.md` set that precedent for five constructs
# it documents and does not implement: **the design is worth keeping, the false
# impression is not.** The same applies here, and the label is the difference
# between the two.
#
# The label is the nearest non-blank line above the fence, lowercased — the
# same rule `check-doc-blocks.sh` uses, so there is one convention to learn.
DESIGN_MARKERS = ('not implemented', 'not built', 'designed and not',
                  'aspirational', 'does not exist', 'proposed', 'planned')

names = {}
skipped = 0
for fname in sorted(f for f in os.listdir(DOC_DIR) if f.endswith('.md')):
    lines = io.open(os.path.join(DOC_DIR, fname), encoding='utf-8',
                    errors='replace').read().splitlines()
    i = 0
    while i < len(lines):
        if lines[i].strip() == '```rust':
            start = j = i + 1
            while j < len(lines) and not lines[j].strip().startswith('```'):
                j += 1
            body = '\n'.join(lines[start:j])
            label = ''
            k = start - 2
            while k >= 0 and not lines[k].strip():
                k -= 1
            if k >= 0:
                label = lines[k].strip().lower()
            i = j + 1
            if any(m in label for m in DESIGN_MARKERS):
                skipped += 1
                continue
            for kind in ('fn', 'struct', 'enum', 'trait', 'type'):
                for n in re.findall(r'\bpub ' + kind + r' (\w+)', body):
                    names.setdefault(n, fname)
        else:
            i += 1

if not names:
    print('  x  no documented `pub` items found in %s/; the extraction broke'
          % DOC_DIR)
    sys.exit(1)

src = []
for root, _dirs, files in os.walk(SRC):
    for f in files:
        if f.endswith('.rs'):
            src.append(io.open(os.path.join(root, f), encoding='utf-8',
                               errors='replace').read())
src = '\n'.join(src)

DEFINITION = r'\b(?:fn|struct|enum|trait|type|const|static|mod|macro_rules!)\s+%s\b'
missing = sorted((n, f) for n, f in names.items()
                 if not re.search(DEFINITION % re.escape(n), src))

print('documented items: %d  (%d block(s) skipped: labelled as design)' % (len(names), skipped))
print('not defined in %s/: %d' % (SRC, len(missing)))
for n, f in missing:
    print('  x  %-24s %s/%s' % (n, DOC_DIR, f))

try:
    baseline = int(io.open(baseline_file, encoding='utf-8').read().strip())
except OSError:
    baseline = None

print()
if baseline is None:
    print('no baseline recorded; write %d to %s' % (len(missing), baseline_file))
    sys.exit(1)
if len(missing) > baseline:
    print('FAILED - %d documented items do not exist; baseline is %d.'
          % (len(missing), baseline))
    print('`internals/` is presented as documentation of the shipped compiler.')
    print('Fix the entry, or the source, or move the claim into a design')
    print('document that says it is one.')
    sys.exit(1)
if len(missing) < baseline:
    print('%d missing (baseline %d) - the ratchet moved. Update %s to %d in'
          % (len(missing), baseline, baseline_file, len(missing)))
    print('this commit; a baseline nobody is required to shrink is just a list.')
    sys.exit(1)
print('OK - %d documented items, %d missing (at the baseline).'
      % (len(names), len(missing)))
PY

#!/usr/bin/env bash
# Every word of the standard vocabulary (§8) must check *and* run, and produce
# the value its published signature says it produces.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# The 31 words in `resolve::VOCABULARY` are the language's whole standard
# library — there are no modules and no `stdlib/`, so an agent writing MAGE has
# these and nothing else. They are published in `MAGE_ONTOLOGY.json` with
# signatures, which is what an agent reads before writing any code.
#
# Three separate lists have to agree for a word to work: the resolver's (does
# the name exist), the checker's `infer_vocab_call` (does it have a type), and
# the evaluator's (does it run). Each was written at a different time, and
# nothing compared them end to end. `scan` and `group` were in the first and
# third and absent from the second, so `scan(1, 2, 3, 4, 5)` typechecked with
# zero errors for as long as `scan` has existed.
#
# The unit tests cover each list against its neighbour. This runs the *binary*,
# both oracles, one call per word, and compares the answer to the signature —
# which is the only thing that says the three lists agree about the same word.
#
# Usage: scripts/check-vocabulary.sh
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

BIN=prototype/target/release/mage-parse
if [ ! -x "$BIN" ]; then
    echo "building the compiler first..."
    cargo build --release --manifest-path prototype/Cargo.toml --bin mage-parse
fi

python - <<'PY'
import io
import json
import os
import subprocess
import sys
import tempfile

BIN = os.path.join('prototype', 'target', 'release', 'mage-parse')

# (name, call, expected `--eval` output). Each call is the smallest one that
# exercises the published signature. The expected value is written out rather
# than computed, so a change in what a word *means* fails here too — an
# `--eval` that agrees with a `--check` about the wrong answer is the failure
# shape this repository has found most often.
#
# Options print as `Some(x)`; strings print quoted. Both are the evaluator's
# rendering and are part of what is being pinned.
CASES = [
    ("map",      'len(map([1, 2, 3], fn(x) => x * 2))', "3"),
    ("filter",   'len(filter([1, 2, 3, 4], fn(x) => x > 2))', "2"),
    ("fold",     'fold([1, 2, 3], 0, fn(a, x) => a + x)', "6"),
    ("reduce",   'reduce([1, 2, 3], fn(a, b) => a + b)', "Some(6)"),
    ("sum",      'sum([1, 2, 3])', "6"),
    ("len",      'len([1, 2, 3])', "3"),
    ("count",    'count([1, 2, 3])', "3"),
    ("sort",     'first(sort([3, 1, 2]))', "Some(1)"),
    ("reverse",  'first(reverse([1, 2, 3]))', "Some(3)"),
    ("zip",      'len(zip([1, 2], [3, 4]))', "2"),
    ("freq",     'len(keys(freq([1, 1, 2])))', "2"),
    ("first",    'first([7, 8])', "Some(7)"),
    ("last",     'last([7, 8])', "Some(8)"),
    ("any",      'any([1, 2], fn(x) => x > 1)', "true"),
    ("all",      'all([1, 2], fn(x) => x > 0)', "true"),
    ("find",     'find([1, 2, 3], fn(x) => x > 1)', "Some(2)"),
    ("take",     'len(take([1, 2, 3], 2))', "2"),
    ("range",    'len(range(5))', "5"),
    ("keys",     'len(keys(freq([1, 2])))', "2"),
    ("values",   'len(values(freq([1, 2])))', "2"),
    ("flatten",  'len(flatten([[1, 2], [3]]))', "3"),
    # `group` by parity gives two keys; `scan` keeps the seed, so three inputs
    # give four results — both straight off the published signatures.
    ("group",    'len(keys(group([1, 2, 3, 4], fn(x) => x % 2)))', "2"),
    ("scan",     'len(scan([1, 2, 3], 0, fn(a, x) => a + x))', "4"),
    ("contains", 'contains([1, 2], 2)', "true"),
    ("split",    'len(split("a,b,c", ","))', "3"),
    ("join",     'join(["a", "b"], "-")', '"a-b"'),
    ("chars",    'len(chars("abc"))', "3"),
    ("words",    'len(words("a b c"))', "3"),
    ("lines",    'len(lines("a"))', "1"),
    ("upper",    'upper("ab")', '"AB"'),
    ("lower",    'lower("AB")', '"ab"'),
]


def run(args):
    p = subprocess.run([BIN] + args, capture_output=True, text=True, errors='replace')
    return (p.stdout or '') + (p.stderr or '')


published = [e['name'] for e in
             json.load(io.open('MAGE_ONTOLOGY.json', encoding='utf-8'))
             ['sections']['vocabulary']]
covered = [c[0] for c in CASES]

missing = [n for n in published if n not in covered]
extra = [n for n in covered if n not in published]
if missing or extra:
    # The whole point is coverage, so a gap in it is the first failure
    # reported, before any word is run.
    if missing:
        print('  x  published and not exercised here: %s' % ', '.join(missing))
    if extra:
        print('  x  exercised here and not published: %s' % ', '.join(extra))
    print('\nFAILED - the case list and the published vocabulary disagree.')
    sys.exit(1)

tmp = tempfile.mkdtemp()
bad = 0
for name, call, want in CASES:
    path = os.path.join(tmp, name + '.mg')
    io.open(path, 'w', encoding='utf-8').write('f probe() { %s }\n' % call)

    chk = run(['--check', path])
    if 'Errors: 0' not in chk:
        errs = [l.strip() for l in chk.splitlines() if 'error' in l.lower()]
        print('  x  %-9s --check rejects `%s`: %s' % (name, call, '; '.join(errs[:2])))
        bad += 1
        continue

    ev = run(['--eval', path, 'probe']).strip()
    got = ev.splitlines()[-1].strip() if ev else ''
    if got != want:
        print('  x  %-9s `%s` evaluated to %r, signature says %r'
              % (name, call, got, want))
        bad += 1

print()
if bad:
    print('FAILED - %d of %d vocabulary words disagree with a published signature.'
          % (bad, len(CASES)))
    sys.exit(1)
print('OK - all %d vocabulary words check, run, and return what §8 publishes.'
      % len(CASES))
PY

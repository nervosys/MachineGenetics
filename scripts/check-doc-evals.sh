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

# The compiler this checker runs, resolved through cargo rather than a
# hardcoded `prototype/target/release/mage-parse`. Six checkers each carried
# their own copy of that path, and on a machine with a shared cargo target
# directory every one of them ran whatever stale binary happened to be sitting
# there. See scripts/find-mage-parse.sh.
. scripts/find-mage-parse.sh
BIN="$(build_and_find_mage_parse)"

MAGE_PARSE_BIN="$BIN" python - "$@" <<'PY'
import io
import os
import re
import subprocess
import sys

# Passed in rather than rebuilt here. This block used to open with its own
# `BIN = os.path.join('prototype', 'target', 'release', 'mage-parse')`, a
# *second* copy of the path the shell above had already resolved — and a
# quoted heredoc does not expand `$BIN`, so the copy silently won. Pointing
# the shell variable at cargo's real target directory fixed nothing here:
# three checkers went on running whatever stale binary sat in
# `prototype/target/`. Found when a documentation block that typechecks
# was reported as failing, because the compiler being asked was three
# weeks old and had never heard of the construct in it.
BIN = os.environ['MAGE_PARSE_BIN']
TAGS = ('mg', 'mage')
ELLIPSIS = ('...', '…')
BROKEN = ('broken', 'invalid', 'wrong', 'incorrect', 'do not', 'bad ')
probe = os.path.join('prototype', 'target', '.docevals.mg')

# Capabilities the interpreter cannot perform, plus reads that would block.
UNRUNNABLE = ('net.', 'agent.', 'llm.', 'gpu.', 'process.', 'swarm.', 'json.',
              'kb.', 'io.read_line', 'io.read', 'fs.walk', 'time.sleep')

# Errors that are **not** the language's fault. Everything else is.
#
# This was the other way round until 2026-09-08: a denylist of ten error
# substrings, so an evaluator error phrased any other way **passed**.
# `migration-guide/08-case-studies.md` fails with `expected a collection`,
# which is not one of the ten, and would have gone on passing for as long as
# nobody added that string to the list.
#
# A denylist of failure modes is a promise to have thought of all of them.
# Every other checker here fails closed; this one now does too, so a *new*
# evaluator error is a failure the day it appears rather than the day
# somebody remembers to name it.
BENIGN = (
    # A capability the interpreter cannot perform. The block reached past
    # what `--eval` implements, which the UNRUNNABLE prefilter usually
    # catches first; this is the backstop for the ones it does not.
    'has no interpreter implementation',
    'no operation of',
    # A real resource the example names and this machine does not have.
    # The example's environment, not its correctness.
    'os error',
    'cannot find the file',
    'no such file',
)

# What counts as an entry point.
#
# `main` and `@test` only, until 2026-09-08 — and this script's own first
# line says "every documentation block that defines an entry point must
# run". `migration-guide/08-case-studies.md` defines `+f run(argv)`, twice,
# in the document that teaches porting a real crate, and **neither was ever
# evaluated**. The claim was broader than the regex.
#
# **Zero-argument only**, and both halves of that matter. `--eval <file>
# <entry>` invokes the entry with no arguments, so a function that takes
# one cannot be an entry point here. Widening the *name* set without
# widening it carefully produced three false failures on the first run:
# `migration-guide/08-case-studies.md`'s `+f run(argv: [s]~)` twice, which
# is a working function called wrongly, and `MAGE_SPEC.md`'s
# `f run(code: str) -> str;` — an **effect operation declared inside
# `effect Analyze { … }`**, which is not a function at all.
#
# Widening what a checker looks at invents findings exactly as readily as
# narrowing it hides them.
MAIN = re.compile(r'^\s*(?:\+f|f|pub\s+fn|fn)\s+(main|run|demo|example)\s*\(\s*\)', re.M)
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
                m = MAIN.search(body)
                if m:
                    entries.append(m.group(1))
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
                    # The evaluator's own marker, plus its exit status —
                    # **not** the word "error" anywhere in the output.
                    #
                    # My first version of this rule matched `'error' in
                    # output` and immediately reported two false positives:
                    # `agent-guide/examples/intermediate.md` legitimately
                    # prints `"JSON error: unexpected token"` and
                    # `cookbook/agents.md` prints `[LOG] error: disk full`.
                    # Both are the example working. A checker that reads a
                    # program's output as its own diagnostics invents
                    # findings, which is the failure this whole file exists
                    # to avoid on the other side.
                    is_error = r.returncode != 0 or 'eval error:' in low
                    if is_error and not any(b in low for b in BENIGN):
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

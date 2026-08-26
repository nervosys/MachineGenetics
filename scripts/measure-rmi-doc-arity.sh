#!/usr/bin/env bash
# MEASUREMENT, not a gate: how far `RecursiveMachineIntelligence/docs` agrees
# with the crate at *signature* level, which `check-rmi-api-doc.sh` does not
# check.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS, AND WHY IT DOES NOT FAIL
# ─────────────────────────────────────────────────────────────────────────────
#
# HANDOFF.md rule 10 deliberately did **not** build an arity checker: it would
# cover 41% of documented functions and had a false-positive class that tripped
# twice on the author's own placeholders. "A check that is right 41% of the
# time and cries wolf converts 'unknown' into 'passing'." That reasoning stands,
# and this script does not overturn it — it exits 0 whatever it finds.
#
# What it fixes is a smaller and more annoying problem. Rule 10 recorded the
# measurement "to be repeated by hand" — but recorded only the *numbers*, not
# the method. Repeating it on 2026-08-25 produced 210 functions and 119
# uniquely defined against the recorded 162 and 66, and **nothing could
# distinguish drift from a different counting rule.** A number without its
# method is not a measurement anyone else can take. This file is the method.
#
# Re-running it also produced a result worth having before anyone revisits
# rule 10's decision:
#
#   * **The false-positive class is a one-line filter.** All three false
#     positives were signatures written `pub fn name(/* … */);` — an elided
#     parameter list a naive parser reads as one argument. Skipping those
#     removes every false positive on the current corpus.
#   * **Coverage is 60%, not 41%** (117 of 196 documented functions are defined
#     exactly once and so comparable).
#   * At the time of writing, **0 of those 117 disagree.**
#
# That is the "new measurements" the roadmap asks for before revisiting a
# declined item. Wiring this into CI is now a decision someone can make with
# numbers in hand; it is deliberately not made here.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHAT IT CANNOT SEE
# ─────────────────────────────────────────────────────────────────────────────
#
# It matches **by bare name across the whole crate**, so a function defined
# exactly once is *assumed* to be the documented one. That assumption is why
# `backward_chain` was reported: the documented `InferenceEngine` method does
# not exist, and the only definition of that name is `#[test] fn
# backward_chain()` in `lang/grad.rs`. The report was correct and the reasoning
# was luck — had the test taken two arguments, it would have passed. Names
# defined more than once are skipped entirely rather than guessed at, which is
# where the missing 40% goes.
#
# It compares **arity and receiver**, not types. `add_node(&mut self, n) ->
# NodeId` and `add_node(&mut self, n) -> Uuid` are identical to this script,
# and the second is the true one.
#
# Usage:
#     bash scripts/measure-rmi-doc-arity.sh

set -o nounset
set -o pipefail

cd "$(dirname "$0")/.." || exit 1

python - <<'PY'
import io
import os
import re

DOCS = os.path.join('RecursiveMachineIntelligence', 'docs')
SRC = os.path.join('RecursiveMachineIntelligence', 'src')

# Same exemptions as check-rmi-api-doc.sh: sample code, not exports.
EXEMPT = {'encode_payload', 'decode_payload'}


def balanced(text, open_idx):
    """The substring inside the parens beginning at open_idx."""
    depth = 0
    for i in range(open_idx, len(text)):
        if text[i] == '(':
            depth += 1
        elif text[i] == ')':
            depth -= 1
            if depth == 0:
                return text[open_idx + 1:i]
    return None


def arity(params):
    """(count of non-self params, has a self receiver)."""
    params = params.strip()
    if not params:
        return 0, False
    depth, parts, cur = 0, [], ''
    for c in params:
        if c in '(<[':
            depth += 1
        elif c in ')>]':
            depth -= 1
        if c == ',' and depth == 0:
            parts.append(cur)
            cur = ''
        else:
            cur += c
    parts.append(cur)
    parts = [p.strip() for p in parts if p.strip()]
    has_self = bool(parts) and re.match(r'^(&\s*(mut\s+)?)?(self|mut self)\b', parts[0])
    if has_self:
        parts = parts[1:]
    return len(parts), bool(has_self)


placeholders = set()
documented = {}
for fname in sorted(os.listdir(DOCS)):
    if not fname.endswith('.md'):
        continue
    doc = io.open(os.path.join(DOCS, fname), encoding='utf-8', errors='replace').read()
    for block in re.findall(r'```rust\n(.*?)```', doc, re.S):
        for m in re.finditer(r'\bpub fn (\w+)\s*(?=\()', block):
            name = m.group(1)
            if name in EXEMPT:
                continue
            params = balanced(block, m.end())
            if params is None:
                continue
            # `pub fn name(/* … */);` — deliberately elided, not a claim about
            # arity. This single filter removed every false positive rule 10
            # was worried about.
            if '/*' in params:
                placeholders.add(name)
                continue
            documented.setdefault(name, []).append((arity(params), fname))

defined = {}
for root, _dirs, files in os.walk(SRC):
    for f in files:
        if not f.endswith('.rs'):
            continue
        path = os.path.join(root, f)
        text = io.open(path, encoding='utf-8', errors='replace').read()
        for m in re.finditer(r'\bfn (\w+)\s*(?:<[^>]*>)?\s*(?=\()', text):
            name = m.group(1)
            if name not in documented:
                continue
            params = balanced(text, m.end())
            if params is not None:
                defined.setdefault(name, []).append((arity(params), path))

unique, disagree = 0, []
for name in sorted(documented):
    defs = defined.get(name, [])
    if len(defs) != 1:
        continue
    unique += 1
    (d_a, d_s), path = defs[0]
    for (a, s), fname in documented[name]:
        if (a, s) != (d_a, d_s):
            disagree.append('%-28s doc %s: %d params%s   src %s: %d params%s'
                            % (name, fname, a, '+self' if s else '',
                               os.path.relpath(path, SRC), d_a, '+self' if d_s else ''))
            break
    if len({sig for sig, _ in documented[name]}) > 1:
        disagree.append('%-28s documented with more than one arity' % name)

total = len(documented)
print('documented functions (unique names):   %d' % total)
print('  placeholder signatures skipped:      %d' % len(placeholders))
print('  defined exactly once (comparable):   %d  (%d%%)'
      % (unique, round(100.0 * unique / max(1, total))))
print('  defined more than once (skipped):    %d'
      % sum(1 for n in documented if len(defined.get(n, [])) > 1))
print()
print('arity/receiver disagreements: %d' % len(disagree))
for d in disagree:
    print('  ?  %s' % d)
print()
print('This is a measurement and exits 0 regardless. See the header for why,')
print('and for what it cannot see.')
PY

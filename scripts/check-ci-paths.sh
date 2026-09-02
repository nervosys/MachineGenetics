#!/usr/bin/env bash
# Every file a checker reads must be a file CI is triggered by.
#
# Why this exists: `check-doc-counts.sh` pins 85 documented numbers across
# fourteen files, and **seven of those fourteen were outside the CI paths
# filter** — `HANDOFF.md`, `SECURITY_AUDIT.md`, `README.md`, `DOCS.md`,
# `UNIFICATION.md`, `DIRECT_CODEGEN_STRATEGY.md` and `benchmarks/STATUS.md`.
# A pull request touching only one of them moved a pinned count and got **no
# CI run at all**. Not a red build. No build.
#
# The checker was correct the whole time. It was simply never reached, which
# is the same failure as a ratchet whose baseline stops being measured: the
# green tick is evidence about what ran, not about what is true.
#
# Two directions, because one is not enough:
#
#   1. A file `check-doc-counts.sh` reads that no paths entry covers.
#      That is the gap this was written for.
#   2. A paths entry naming a **file** that does not exist. A filter entry
#      with a typo silently covers nothing, which looks identical to being
#      covered. Directory globs (`prototype/**`) are exempt — they are
#      prefixes, not paths, and a repository is allowed to not have one yet.
#
# Deliberately NOT checked: whether every *tracked* file is covered. Most of
# the repository is documentation no checker reads, and demanding coverage
# for it would make this fail forever and be switched off. The rule is
# narrower and enforceable: **if something checks it, CI must run when it
# changes.**
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

WORKFLOW=.github/workflows/ci.yml
COUNTS=scripts/check-doc-counts.sh

for f in "$WORKFLOW" "$COUNTS"; do
    if [ ! -f "$f" ]; then
        echo "  x  $f is missing; this check cannot mean anything" >&2
        exit 1
    fi
done

report="$(python - "$WORKFLOW" "$COUNTS" <<'PY'
import io, os, re, sys

workflow, counts = sys.argv[1], sys.argv[2]

# The files `check-doc-counts.sh` reads: the first TAB-separated field of each
# CHECKS row. Read out of the script rather than duplicated here, so adding a
# row cannot forget to add it in two places.
pinned = []
for line in io.open(counts, encoding='utf-8'):
    if line.startswith('#') or '\t' not in line:
        continue
    first = line.split('\t')[0].strip()
    if re.match(r'^[A-Za-z0-9_./-]+\.(md|sh|ps1|yml|json)$', first):
        pinned.append(first)
pinned = sorted(set(pinned))

wf = io.open(workflow, encoding='utf-8').read()

# Every `on:` trigger's paths list, taken separately: a path present in
# `pull_request` and missing from `push` still leaves a hole, in the other
# direction, and this repository merges through both.
triggers = {}
for name in ('push', 'pull_request'):
    m = re.search(r'^  %s:\n(.*?)(?=^  \w|^jobs:)' % name, wf, re.M | re.S)
    if not m:
        print('MISSING_TRIGGER\t%s' % name)
        continue
    triggers[name] = set(re.findall(r'^\s+- "([^"]+)"', m.group(1), re.M))

if not triggers:
    print('NO_TRIGGERS\t')
    raise SystemExit(0)


def covered(path, entries):
    for e in entries:
        if e == path:
            return True
        if e.endswith('/**') and path.startswith(e[:-2]):
            return True
    return False


# Direction 1 — a pinned file no filter entry reaches.
for name, entries in sorted(triggers.items()):
    for p in pinned:
        if not covered(p, entries):
            print('UNCOVERED\t%s\t%s' % (name, p))

# Direction 2 — a filter entry naming a file that is not there.
for name, entries in sorted(triggers.items()):
    for e in sorted(entries):
        if e.endswith('/**') or '*' in e:
            continue
        if not os.path.exists(e):
            print('NOSUCHFILE\t%s\t%s' % (name, e))

print('COUNTED\t%d\t%d' % (len(pinned), len(triggers)))
PY
)"

uncovered=0
missing=0
pinned_n=0
triggers_n=0

while IFS=$'\t' read -r kind a b; do
    case "$kind" in
        UNCOVERED)
            echo "  x  $b is read by $COUNTS and no '$a' paths entry covers it" >&2
            uncovered=$((uncovered + 1))
            ;;
        NOSUCHFILE)
            echo "  x  the '$a' filter names '$b', which does not exist" >&2
            missing=$((missing + 1))
            ;;
        MISSING_TRIGGER)
            echo "  x  $WORKFLOW has no '$a' trigger; this check assumed one" >&2
            uncovered=$((uncovered + 1))
            ;;
        COUNTED)
            pinned_n="$a"
            triggers_n="$b"
            ;;
    esac
done <<< "$report"

if [ "$uncovered" -ne 0 ] || [ "$missing" -ne 0 ]; then
    echo >&2
    echo "     A checker that CI never runs is not a guard. Add the path to" >&2
    echo "     both triggers in $WORKFLOW, or stop pinning the file." >&2
    exit 1
fi

echo "  ok all $pinned_n file(s) pinned by check-doc-counts.sh are covered by" \
     "each of the $triggers_n CI trigger(s), and every named path exists."

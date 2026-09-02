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
# **The first version of this script was itself too narrow, and stated
# something false while being it.** It said "most of the repository is
# documentation no checker reads". It is not. `check-doc-blocks.sh` walks the
# entire tree and compiles every MAGE block in every `.md`; `check-mg-sources.sh`
# and `check-examples.sh` do the same for every `.mg`. Every markdown and MAGE
# source here is read by a checker — and **76 of 101 `.md` files and 83 of 101
# `.mg` files were outside the filter**, `MAGE_SPEC.md` and the whole of
# `quick-start/`, `agent-guide/`, `cookbook/` and `training/prompts/` among them.
# Those are the documents an agent reads to learn the language, and a pull
# request adding a block to one of them that does not typecheck ran no CI.
# Direction 3 is the consequence.
#
# Still NOT checked: every tracked file of every kind. `LICENSE`, `media/` and
# `.gitignore` are read by nothing, and demanding coverage there would make this
# fail forever and be switched off. The rule has not changed — **if something
# checks it, CI must run when it changes** — what changed is the honest answer
# to "what does something check".
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


def _glob_re(entry):
    """GitHub's paths glob as a regex.

    `**` crosses directory separators, `*` and `?` do not — the filter syntax
    GitHub documents, not `fnmatch`, whose `*` would quietly match across `/`
    and make `prototype/*` look like `prototype/**`. Everything else is escaped,
    so a `.` in `HANDOFF.md` matches a dot and not any character.
    """
    out, i = [], 0
    while i < len(entry):
        if entry.startswith('**', i):
            out.append('.*')
            i += 2
        elif entry[i] == '*':
            out.append('[^/]*')
            i += 1
        elif entry[i] == '?':
            out.append('[^/]')
            i += 1
        else:
            out.append(re.escape(entry[i]))
            i += 1
    return re.compile('^' + ''.join(out) + '$')


_glob_cache = {}


def covered(path, entries):
    for e in entries:
        if e == path:
            return True
        if e not in _glob_cache:
            _glob_cache[e] = _glob_re(e)
        if _glob_cache[e].match(path):
            return True
    return False


# Direction 1 — a pinned file no filter entry reaches.
for name, entries in sorted(triggers.items()):
    for p in pinned:
        if not covered(p, entries):
            print('UNCOVERED\t%s\t%s' % (name, p))

# Direction 3 — every file a *content* checker compiles. `check-doc-blocks.sh`
# walks the tree for `.md` and `check-mg-sources.sh` for `.mg`, and both compile
# what they find, so any one of these files can turn CI red and every one of
# them must be able to turn CI on. Read from `git ls-files`, not a list here, so
# a new document is covered the day it is added rather than the day someone
# remembers this script exists.
import subprocess

tracked = subprocess.run(
    ['git', 'ls-files'], capture_output=True, text=True).stdout.splitlines()
compiled = [p for p in tracked if p.endswith('.md') or p.endswith('.mg')]
for name, entries in sorted(triggers.items()):
    for p in compiled:
        if not covered(p, entries):
            print('UNREAD\t%s\t%s' % (name, p))

# Direction 2 — a filter entry naming a file that is not there.
for name, entries in sorted(triggers.items()):
    for e in sorted(entries):
        if e.endswith('/**') or '*' in e:
            continue
        if not os.path.exists(e):
            print('NOSUCHFILE\t%s\t%s' % (name, e))

# Direction 4 — a checker CI never invokes. The first three directions ask
# whether CI *starts* when a checked file changes; this asks whether the
# checker runs once it has. A `check-*.sh` nobody wired up is the most likely
# next instance of "the checker was correct and simply never reached", and it
# is invisible: it passes locally, it is in the repository, and no run mentions
# it. Reachability is transitive — `check-doc-counts.sh` is invoked by
# `test-all.sh`, which CI invokes, and that counts — so this walks the closure
# rather than only the workflow's own `run:` lines.
import glob

# An *invocation*, not a mention. The first version of this walk matched any
# occurrence of `scripts/x.sh` in a script's text, and comments are full of
# them — this file names `check-doc-counts.sh` in its own header because it
# reads it. The consequence was not a missed report but a false clean one:
# cutting `check-doc-counts.sh` out of `test-all.sh` left it genuinely
# unreachable and this check still passed, certifying reachability that was
# not there. Found by breaking it, which is the only way that class shows up.
#
# So: comments stripped first, and the reference must be preceded by an
# interpreter, `./`, or the start of a command.
INVOKE = re.compile(
    r'(?:(?:^|[|&;(]|\bthen\b|\bdo\b|\belse\b)\s*|\b(?:bash|sh|zsh|pwsh|powershell|source|\.)\s+)'
    r'"?\$?\{?[A-Za-z_]*\}?/?(scripts/[A-Za-z0-9_.-]+\.(?:sh|ps1))')


def invoked_in(text, strip_comments):
    if strip_comments:
        text = '\n'.join(l for l in text.split('\n') if not l.lstrip().startswith('#'))
    return [m.group(1) for m in INVOKE.finditer(text)]


seen, frontier = set(), []
# The workflow's `run:` blocks are commands already; its YAML `#` comments are
# stripped for the same reason.
for m in re.finditer(r'^\s*run:\s*\|?(.*(?:\n\s{8,}.*)*)', wf, re.M):
    frontier += invoked_in(m.group(1), strip_comments=True)
while frontier:
    s = frontier.pop()
    if s in seen:
        continue
    seen.add(s)
    if not os.path.exists(s):
        continue
    body = io.open(s, encoding='utf-8', errors='replace').read()
    frontier += invoked_in(body, strip_comments=True)

checkers = sorted(glob.glob('scripts/check-*.sh'))
for c in checkers:
    if c.replace(os.sep, '/') not in seen:
        print('UNRUN\t-\t%s' % c.replace(os.sep, '/'))

print('COUNTED\t%d\t%d\t%d\t%d' % (len(pinned), len(triggers), len(compiled), len(checkers)))
PY
)"

uncovered=0
missing=0
pinned_n=0
triggers_n=0
compiled_n=0
checkers_n=0

while IFS=$'\t' read -r kind a b c d; do
    case "$kind" in
        UNCOVERED)
            echo "  x  $b is read by $COUNTS and no '$a' paths entry covers it" >&2
            uncovered=$((uncovered + 1))
            ;;
        UNREAD)
            echo "  x  $b is compiled by a doc/source checker and no '$a' paths entry covers it" >&2
            uncovered=$((uncovered + 1))
            ;;
        UNRUN)
            echo "  x  $b exists and no CI step reaches it, directly or through a script CI runs" >&2
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
            compiled_n="$c"
            checkers_n="$d"
            ;;
    esac
done <<< "$report"

if [ "$uncovered" -ne 0 ] || [ "$missing" -ne 0 ]; then
    echo >&2
    echo "     A checker that CI never runs is not a guard. Add the path to" >&2
    echo "     both triggers in $WORKFLOW, or stop pinning the file." >&2
    exit 1
fi

echo "  ok all $pinned_n pinned and $compiled_n compiled file(s) are covered by" \
     "each of the $triggers_n CI trigger(s), every named path exists, and all" \
     "$checkers_n checker(s) are reached by a CI step."

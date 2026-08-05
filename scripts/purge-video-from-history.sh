#!/usr/bin/env bash
# Remove the 37 MB rendered video from *git history*, shrinking the repository.
#
# ─────────────────────────────────────────────────────────────────────────────
# THIS REWRITES PUBLISHED HISTORY. READ BEFORE RUNNING.
# ─────────────────────────────────────────────────────────────────────────────
#
# `video/out/agentic-rain.mp4` is no longer tracked (it is a release asset and
# git-ignored), so the repository stops *growing*. But the blob is still in
# history, so a fresh clone still transfers ~37 MB and `.git` is still ~158 MB.
# Only rewriting history reclaims that, and rewriting history has consequences
# that are not reversible by another commit:
#
#   * Every commit SHA after the first touch of that file changes. The v0.2.0
#     tag moves; anyone who recorded a SHA now has a dangling reference.
#   * Every existing clone and fork diverges. Collaborators must re-clone or
#     hard-reset; a normal `git pull` produces a mess.
#   * It requires `git push --force` to a public repository.
#
# If nobody else has cloned nervosys/MachineGenetics, this is cheap and worth
# doing. If anyone has, coordinate first. That judgement is why this script
# exists rather than the rewrite having simply been done.
#
# Requires git-filter-repo (https://github.com/newren/git-filter-repo):
#     pip install git-filter-repo
#
# Usage:
#     scripts/purge-video-from-history.sh --dry-run   # report only, default
#     scripts/purge-video-from-history.sh --execute   # actually rewrite

set -euo pipefail

TARGET="video/out/agentic-rain.mp4"
MODE="${1:---dry-run}"

cd "$(dirname "${BASH_SOURCE[0]}")/.."

echo "Repository: $(pwd)"
echo "Target:     $TARGET"
echo "Current .git size: $(du -sh .git 2>/dev/null | cut -f1)"
echo

# Ask git directly, with NO pipe. Under `set -o pipefail`, piping a long git
# listing into anything that exits early (`grep -q`, `head -n1`) makes git take
# SIGPIPE and the pipeline report failure — which reads as "file not found" for
# a file that is plainly there, and kills the script under `set -e`. Both
# spellings of that bug were written here before this comment existed.
FOUND="$(git log --all --oneline -1 -- "$TARGET")"
if [ -z "$FOUND" ]; then
    echo "✓ $TARGET is not present in history — nothing to purge."
    exit 0
fi
echo "Present in history, first seen at: $FOUND"
echo

if [ "$MODE" != "--execute" ]; then
    cat <<'EOF'
DRY RUN. Nothing has been changed.

What --execute would do:

  1. Back up the current repo to ../MachineGenetics.backup-<timestamp>
  2. git filter-repo --invert-paths --path video/out/agentic-rain.mp4
  3. Report the new .git size

Afterwards, and only if you are certain:

  git remote add origin https://github.com/nervosys/MachineGenetics.git
  git push --force --all
  git push --force --tags

  (filter-repo removes the remote deliberately, so a force-push cannot happen
  by reflex. Re-adding it is the moment to stop and be sure.)

Before force-pushing, attach the video to the release so it stays available:

  gh release upload v0.2.0 video/out/agentic-rain.mp4
EOF
    exit 0
fi

if ! command -v git-filter-repo >/dev/null 2>&1; then
    echo "git-filter-repo is not installed. pip install git-filter-repo" >&2
    exit 1
fi

STAMP="$(git log -1 --format=%cd --date=format:%Y%m%d-%H%M%S)"
BACKUP="../MachineGenetics.backup-$STAMP.git"
# Mirror only the repository, not the working tree: node_modules and target/
# add ~7.6 GB of regenerable bytes and none of it is what needs preserving.
# A mirror clone holds every ref and object, which is exactly what a history
# rewrite puts at risk.
echo "Mirroring repository to $BACKUP …"
git clone --mirror . "$BACKUP" >/dev/null 2>&1
echo "✓ backup complete ($(du -sh "$BACKUP" 2>/dev/null | cut -f1))"
echo "  restore with: rm -rf .git && git clone --mirror $BACKUP .git-restored"

echo "Rewriting history …"
git filter-repo --invert-paths --path "$TARGET" --force

echo
echo "✓ done. New .git size: $(du -sh .git 2>/dev/null | cut -f1)"
echo
echo "The 'origin' remote was removed by filter-repo. Re-add and force-push"
echo "only when you are certain no one else has cloned this repository."

#!/usr/bin/env bash
# Copy corpus files out of the working tree once, by content hash, so arena
# runs on different branches measure against the same bytes.
#
# Usage:
#   scripts/freeze-corpora.sh --dir DIR FILE[,FILE...] [FILE...]
#
# Each FILE is copied to DIR/<first 16 hex digits of its SHA-256>/<basename>
# and made read-only. The frozen paths are printed on one line, comma-joined
# in argument order, ready for --heldout, --validation, --dev-text or
# --train-text:
#
#   HO=$(scripts/freeze-corpora.sh --dir E:/corpora README.md,germline/src/gate.rs)
#   arena succession --heldout "$HO" ...
#
# Running it again on unchanged files prints the same paths and copies
# nothing. An existing frozen copy whose bytes no longer match its hash is an
# error, not something to overwrite.
#
# Why: the corpora the ARENA.md runs use (README.md, germline/src/gate.rs,
# germline/src/runner.rs, GERMLINE.md) are live files that branches edit. A
# change to one moves every bits/byte figure, so numbers from run sets made on
# different trees cannot be compared (ARENA.md, "Several measurements per
# cycle"). A frozen copy also keeps the evaluator outside the files the work
# under test can change. DIR should be outside the repository.

set -euo pipefail

die() {
    echo "freeze-corpora: $*" >&2
    exit 2
}

sha() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

DIR=""
FILES=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dir) [[ $# -ge 2 ]] || die "--dir needs a directory"; DIR="$2"; shift 2 ;;
        -h|--help) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        -*) die "unknown option '$1'" ;;
        *)
            IFS=',' read -r -a parts <<< "$1"
            for p in "${parts[@]}"; do [[ -n "$p" ]] && FILES+=("$p"); done
            shift
            ;;
    esac
done

[[ -n "$DIR" ]] || die "--dir DIR is required"
[[ ${#FILES[@]} -ge 1 ]] || die "at least one FILE is required"

# Absolute path of a directory; on Git Bash/MSYS the Windows form (E:/...),
# which native binaries such as the arena read correctly inside a comma list.
absdir() {
    (cd "$1" && { pwd -W 2>/dev/null || pwd -P; })
}

created=""
[[ -d "$DIR" ]] || created=1
mkdir -p "$DIR"
DIR=$(absdir "$DIR")
if top=$(git rev-parse --show-toplevel 2>/dev/null); then
    top=$(absdir "$top")
    case "$DIR/" in
        "$top"/*)
            [[ -n "$created" ]] && rmdir "$DIR"
            die "--dir $DIR is inside the repository ($top); put frozen copies outside it"
            ;;
    esac
fi

out=()
for f in "${FILES[@]}"; do
    [[ -f "$f" ]] || die "$f is not a file"
    h=$(sha "$f")
    dest="$DIR/${h:0:16}/$(basename "$f")"
    if [[ -e "$dest" ]]; then
        [[ "$(sha "$dest")" == "$h" ]] || die "$dest exists but its contents no longer match hash ${h:0:16}"
    else
        mkdir -p "$(dirname "$dest")"
        cp "$f" "$dest.tmp.$$"
        mv "$dest.tmp.$$" "$dest"
        chmod a-w "$dest"
    fi
    out+=("$dest")
done

(IFS=','; echo "${out[*]}")

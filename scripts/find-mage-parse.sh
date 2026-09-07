#!/usr/bin/env bash
# Where is `mage-parse`? One answer, sourced by every script that needs it.
#
# Usage:  . scripts/find-mage-parse.sh
#         BIN="$(find_mage_parse)"            # release, or exit 1
#         BIN="$(find_mage_parse debug)"      # a specific profile
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# Six checkers had their own copy of the line `BIN=prototype/target/release/
# mage-parse`, and every copy was wrong in the same two ways:
#
# — and then **three of them had a second copy**, inside a `python - <<'PY'`
#   heredoc, which is quoted and so does not expand `$BIN`. Replacing the shell
#   variable with a resolved path fixed nothing in those three: they went on
#   running whatever binary sat in `prototype/target/`. That was found the way
#   everything here gets found — by running it, when a documentation block that
#   typechecks was reported as failing, because the compiler being asked was
#   three weeks old and had never heard of the construct in it. **A path in a
#   heredoc is a copy that does not look like one**, and finding six is a
#   reason to go looking for the seventh.
#
#   1. **The binary is not always there.** A `~/.cargo/config.toml` with a
#      shared `build.target-dir` redirects every Rust project on a machine to
#      one target directory — this repository's own HANDOFF.md records that as
#      the reason a build lock is global here — and then `cargo build
#      --manifest-path prototype/Cargo.toml` puts `mage-parse` somewhere
#      `prototype/target/` never hears about.
#
#   2. **Something else can be there instead.** A `prototype/target/` left over
#      from before that config was added still holds a binary, and it is found
#      first, forever. On 2026-09-07 the one sitting there was three weeks old.
#      A new script ran all 101 `.mg` sources through it and reported "101 of
#      101 could not be parsed" — a clean-looking count produced by a compiler
#      nobody had built. `check-mg-sources.sh` and four others would have said
#      "every .mg source typechecks" on the same evidence, which is worse,
#      because that one passes.
#
# CI never saw either failure: a fresh runner has no shared target directory
# and no leftovers, so the hardcoded path is correct there and only there. The
# checkers were right about what they checked and wrong about what they ran,
# which is the same shape as a checker that is never reached — a green result
# that is evidence about nothing.
#
# `cargo metadata` is the authority on where cargo puts things, so it is asked
# rather than guessed. There is deliberately **no fallback to a hardcoded
# path**: if the binary is not where cargo says it goes, it has not been built,
# and saying so beats running an older one.
#
# This does not make a binary *fresh* — only building does, and callers that
# care build first. It makes the path the one cargo would use.

# Print the path to `mage-parse` for a profile, or return 1 with a diagnostic.
find_mage_parse() {
    local profile="${1:-release}"
    local root target dir
    root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

    target="$(cargo metadata --format-version 1 --no-deps \
                --manifest-path "$root/prototype/Cargo.toml" 2>/dev/null \
              | sed 's/.*"target_directory":"\([^"]*\)".*/\1/')"
    if [ -z "$target" ]; then
        echo "find_mage_parse: cargo metadata failed; is cargo on PATH?" >&2
        return 1
    fi

    dir="$target/$profile"
    if [ -x "$dir/mage-parse" ]; then
        printf '%s' "$dir/mage-parse"
        return 0
    fi
    # Windows. Checked second so a Unix binary is never shadowed by a stale
    # `.exe` sitting beside it.
    if [ -x "$dir/mage-parse.exe" ]; then
        printf '%s' "$dir/mage-parse.exe"
        return 0
    fi

    echo "find_mage_parse: no mage-parse in $dir — build it:" >&2
    echo "    cargo build --$profile --manifest-path prototype/Cargo.toml --bin mage-parse" >&2
    return 1
}

# Build, then resolve. For callers that would otherwise run whatever is lying
# around: `cargo build` is a no-op when it is current, so this is cheap, and it
# is the only way to be sure the binary that runs is the one the source says.
build_and_find_mage_parse() {
    local profile="${1:-release}"
    local root
    root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    # `--debug` is not a cargo flag; the debug profile is the absence of one.
    if [ "$profile" = "release" ]; then
        cargo build --release --manifest-path "$root/prototype/Cargo.toml" \
            --bin mage-parse >&2 || return 1
    else
        cargo build --manifest-path "$root/prototype/Cargo.toml" \
            --bin mage-parse >&2 || return 1
    fi
    find_mage_parse "$profile"
}

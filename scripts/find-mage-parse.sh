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

# Print the path to any crate's binary, or return 1 with a diagnostic.
#
# `find_mage_parse` below is this with the arguments filled in. The general
# form exists because the 2026-09 sweep that introduced this file converted the
# six **checkers** and stopped there, while `benchmarks/capstone/run.sh`,
# `scripts/demo_agent_workflow.sh`, `scripts/demo_rap_workflow.sh` and
# `scripts/agent_wrappers/smart_fixer.sh` went on carrying
# `<crate>/target/release/<bin>.exe` by hand — and `forge` had no resolver at
# all, so its consumer could only guess.
#
# What that costs, measured on this machine 2026-09-08: `forge/target/` does
# not exist (cargo builds to `~/.cargo-target`), so `capstone/run.sh` exited 1
# with "missing binary: forge", which killed `emit-doc-counts.sh` mid-stream
# and produced `INCOMPLETE - 31 documented count(s) had nothing to compare
# against`. And `prototype/target/release/mage-parse.exe` **does** exist — a
# leftover dated six days earlier — so where the guess resolves at all it
# resolves to a stale compiler, which is the original failure this file was
# written about.
#
#   $1 crate directory (relative to the repo root, e.g. "forge")
#   $2 binary name (e.g. "forge")
#   $3 profile (default "release")
find_crate_bin() {
    local crate="$1" bin="$2" profile="${3:-release}"
    local root target dir
    root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

    target="$(cargo metadata --format-version 1 --no-deps \
                --manifest-path "$root/$crate/Cargo.toml" 2>/dev/null \
              | sed 's/.*"target_directory":"\([^"]*\)".*/\1/')"
    if [ -z "$target" ]; then
        echo "find_crate_bin: cargo metadata failed for $crate; is cargo on PATH?" >&2
        return 1
    fi

    dir="$target/$profile"
    # `.exe` **first**, which is the opposite of `find_mage_parse` below, and
    # the difference is deliberate.
    #
    # Under MSYS/Git Bash, `[ -x "$dir/mage-parse" ]` is true when only
    # `mage-parse.exe` exists — the runtime resolves the extensionless name for
    # you — so the extensionless path works for anything bash executes, and is
    # *not a file* to anything else. `capstone/run.sh` hands its resolved
    # compiler to `forge` as `FORGE_MG`, and forge is a native Windows program
    # that opens it: it answered `error: FORGE_MG points at
    # C:/…/release/mage-parse, which is not a file`, and the benchmark reported
    # "forge check did not pass" for a compiler that was sitting right there.
    #
    # `find_mage_parse` orders these the other way and says why: so a stale
    # `.exe` cannot shadow a real Unix binary beside it. That concern is a
    # cross-build artifact — on Linux `$bin.exe` does not exist, so this order
    # falls straight through to the same answer — and it is worth less here
    # than handing a consumer a path it cannot open.
    if [ -x "$dir/$bin.exe" ]; then
        printf '%s' "$dir/$bin.exe"
        return 0
    fi
    if [ -x "$dir/$bin" ]; then
        printf '%s' "$dir/$bin"
        return 0
    fi

    echo "find_crate_bin: no $bin in $dir — build it:" >&2
    echo "    cargo build --$profile --manifest-path $crate/Cargo.toml --bin $bin" >&2
    return 1
}

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

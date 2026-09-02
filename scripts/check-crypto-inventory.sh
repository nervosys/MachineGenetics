#!/usr/bin/env bash
# Every cryptographic dependency in the repository must appear in
# `SECURITY_AUDIT.md` §2's inventory, and every crate §2 names must still be
# one.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# §2 concluded, for months, that there was **"no secret keying, no signatures,
# no KDF"** and that the FIPS gap therefore affected "compliance posture, not
# present-day confidentiality". By 2026-08-25 the repository contained
# HMAC-SHA256 under a fleet secret key (three call sites), Ed25519 signatures
# over build provenance with per-worker private seeds and a revocation store,
# constant-time comparison, and a TLS 1.3 worker transport. None of it was in
# the inventory.
#
# The code was fine — the HMAC is checked against RFC 4231, comparison is
# constant-time, the trust model is stated at the call sites. The *document*
# was wrong, and it was wrong for a structural reason worth naming:
#
#   `ribosome` and `germline` were extracted from `forge` after §2 was
#   written. §1 was widened to audit five lockfiles. §2's scope was not. All
#   the cryptography arrived with the crates that were never added to it.
#
# **An absence claim cannot fail loudly.** "There is no X" stays green by
# doing nothing, and every scope change silently expires it. That is what this
# check exists to convert into a failure.
#
# ─────────────────────────────────────────────────────────────────────────────
# HOW IT DECIDES
# ─────────────────────────────────────────────────────────────────────────────
#
# Two directions, both read out of the document rather than hardcoded here:
#
#   a crypto dependency in any Cargo.toml that §2 does not name  -> FAIL
#   a crate §2 names that is no longer a dependency anywhere     -> FAIL
#
# The second matters as much as the first: a row for a crate that has been
# removed describes a cryptographic posture the code no longer has, and reads
# exactly like a row someone checked.
#
# Hand-rolled primitives are not dependencies, so they are matched separately
# by the function names that define them (`ribosome/src/mac.rs`). A hand-rolled
# MAC is the single most important thing an inventory like this can fail to
# mention, and it is invisible to any manifest scan.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHAT THIS DOES NOT CHECK
# ─────────────────────────────────────────────────────────────────────────────
#
# It matches **names**, not use. It cannot tell that `sha2` is used for
# content-addressing in one crate and keyed as HMAC in another — §2's "Use"
# column is prose and stays a human claim. It also only knows the algorithm
# families in `CRYPTO_CRATES` below: a dependency on something not in that list
# is invisible, so the list is the honest boundary of this instrument and is
# deliberately generous. Add to it rather than trusting it to be complete.
#
# It reads **direct** dependencies from `Cargo.toml`, not the resolved graph.
# A transitive crypto crate is not reported; `cargo audit` covers the graph for
# vulnerabilities, and an inventory of what this repository *chose* is the more
# useful thing for a posture section.
#
# Usage:
#     bash scripts/check-crypto-inventory.sh

set -o nounset
set -o pipefail

cd "$(dirname "$0")/.." || exit 1

DOC=SECURITY_AUDIT.md

# Algorithm families worth inventorying. Generous on purpose — a false positive
# here costs one sentence in §2, a false negative costs what this check exists
# to prevent.
CRYPTO_CRATES="sha2 sha1 sha3 md-5 md5 blake2 blake3 hmac ring rustls native-tls openssl
aes chacha20 chacha20poly1305 argon2 pbkdf2 scrypt bcrypt ed25519-dalek ed25519 rsa
p256 p384 k256 x25519-dalek curve25519-dalek webpki rustls-webpki x509-parser subtle
zeroize digest signature aws-lc-rs boring"

# Hand-rolled primitives, keyed by the function that defines them. These have no
# manifest entry and are the ones an inventory most needs to name.
#   <defining regex><TAB><file><TAB><what §2 must mention>
HANDROLLED="pub fn hmac_sha256	ribosome/src/mac.rs	HMAC
pub fn ct_eq	ribosome/src/mac.rs	onstant-time"

fail=0

if [ ! -f "$DOC" ]; then
    echo "  x  $DOC is missing; there is no inventory to check" >&2
    exit 1
fi

# §2 only: from the `## 2.` heading to the next `##`.
section="$(awk '/^## 2\./ { inside = 1; next } /^## / { inside = 0 } inside' "$DOC")"

if [ -z "$section" ]; then
    echo "  x  $DOC has no \`## 2.\` section; the inventory moved or was renamed" >&2
    exit 1
fi

# ── Direction 1: a crypto dependency §2 does not name ────────────────────────

declared=""
for manifest in */Cargo.toml; do
    [ -f "$manifest" ] || continue
    crate_dir="${manifest%/Cargo.toml}"
    for name in $CRYPTO_CRATES; do
        # A dependency line: `name = ...` or `name.workspace = ...`, at the
        # start of a line so a mention inside a comment or a feature list does
        # not count as a declaration.
        if grep -qE "^${name}[[:space:]]*(=|\.)" "$manifest"; then
            declared="$declared$name $crate_dir"$'\n'
        fi
    done
done

if [ -z "$declared" ]; then
    echo "  x  no crypto dependency found in any Cargo.toml." >&2
    echo "       This repository has several; finding none means the manifest" >&2
    echo "       glob or the crate list stopped matching, not that the" >&2
    echo "       dependencies went away." >&2
    exit 1
fi

checked=0
while read -r name crate_dir; do
    [ -n "$name" ] || continue
    checked=$((checked + 1))
    if ! printf '%s\n' "$section" | grep -qF -- "$name"; then
        echo "  x  \`$name\` is a dependency of $crate_dir and $DOC §2 does not name it" >&2
        echo "       §2 is the cryptographic inventory, and its findings are" >&2
        echo "       absence claims — \"no secret keying, no signatures\" — which" >&2
        echo "       stay green by doing nothing. Add a row, even to say the" >&2
        echo "       dependency performs no security function." >&2
        fail=1
    fi
done <<< "$declared"

# ── Direction 2: an inventory row naming a crate that is not in the build ────
#
# Scoped to the **Crate column of §2's table**, and checked against the
# lockfiles rather than the manifests. Both narrowings are load-bearing, and
# the first draft of this check had neither — it compared every crypto name
# appearing anywhere in §2 against the *direct* dependencies, and reported four
# failures, all of them wrong:
#
#   `ring`       a real package, reached through `rustls`'s feature rather
#                than declared — invisible to a manifest scan, present in
#                `ribosome/Cargo.lock`
#   `ed25519`    a real package, pulled in by `ed25519-dalek`
#   `signature`  a real package, *and* an ordinary English word that appears
#                throughout a section about signatures
#   `aws-lc-rs`  named as the FIPS migration target §2 recommends, which is
#                the opposite of a stale row: it is a dependency the
#                repository deliberately does not have yet
#
# A check that fires four times on a correct document teaches its reader to
# ignore it, which is rule 10 exactly: an instrument that cries wolf converts
# "unknown" into "passing". The Crate column is where the document *claims* a
# dependency; prose is where it discusses one.

table_crates="$(printf '%s\n' "$section" | awk -F'|' '/^\|/ { print $3 }')"

for name in $CRYPTO_CRATES; do
    printf '%s\n' "$table_crates" | grep -qF -- "$name" || continue
    if ! grep -qs "^name = \"$name\"$" -- */Cargo.lock; then
        echo "  x  $DOC §2's inventory table names \`$name\`, which no lockfile contains" >&2
        echo "       A row describing cryptography that is not in the build reads" >&2
        echo "       exactly like a row somebody re-checked. Remove it, or move it" >&2
        echo "       into prose saying when and why it went." >&2
        fail=1
    fi
done

# ── Hand-rolled primitives ───────────────────────────────────────────────────

while IFS=$'\t' read -r defn file mention; do
    [ -n "${defn:-}" ] || continue
    [ -f "$file" ] || continue
    grep -qF -- "$defn" "$file" || continue
    checked=$((checked + 1))
    if ! printf '%s\n' "$section" | grep -qF -- "$mention"; then
        echo "  x  $file defines \`$defn\` and $DOC §2 never mentions \"$mention\"" >&2
        echo "       A hand-rolled primitive has no manifest entry, so it is" >&2
        echo "       invisible to every dependency scan — which makes naming it" >&2
        echo "       in the inventory the only way anyone finds out it is there." >&2
        fail=1
    fi
done <<< "$HANDROLLED"

if [ "$fail" -ne 0 ]; then
    exit 1
fi

echo "  ok $DOC §2 names every crypto dependency and hand-rolled primitive found ($checked checked)."

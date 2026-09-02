# Security Audit — MAGE (Machine Genetics) + RecursiveMachineIntelligence (rmi)

**Org:** NERVOSYS · **Date:** 2026-06-04, §1 and §2 re-run 2026-08-25 ·
**Scope:** `RecursiveMachineIntelligence/` (crate `rmi`), `prototype/`
(compiler + RAP server), `ribosome/` (build engine — **the crate that holds
every cryptographic mechanism in this repository**), `germline/` (RSI control
plane), `forge/` (registry), `video/` (npm), and `agentic-eval` (separate
AetherShell repo). Frameworks applied: **CVE/RustSec**, **NIST FIPS 140-3**,
**MITRE ATT&CK**, **CMMC 2.0**.

> **This line named three surfaces until 2026-08-25**, and that is not
> cosmetic: `ribosome` and `germline` were extracted from `forge` after this
> document was written, §1 was widened to audit five lockfiles, and §2 was not.
> Everything those two crates brought with them — HMAC, Ed25519, TLS — sat
> outside the stated scope of the section that inventories cryptography, so
> §2's central finding ("no secret keying, no signatures") was true of the
> old scope and false of the repository. **When the scope moves, every
> "there is no X" claim in the document silently expires.**

> Posture summary: this is **pre-release research software** (a compiler + an
> embedded AI framework), not a deployed networked service. The realistic threat
> model is *supply-chain* (dependencies) and *local agent execution* (running
> code an LLM just wrote), not remote multi-tenant attack. Findings below are
> triaged against that model; one CVE was fixed, the rest are documented with
> deployment guidance.

---

## 1. CVE / RustSec (`cargo audit`)

Run on all **five** Cargo.lock surfaces against the RustSec advisory-db —
three when this was written, plus `ribosome/` and `germline/` since they were
extracted from `forge` (steps 148–149). Both are clean, as is `forge`.

> **Re-run 2026-08-05, and one surface was not clean.** `cargo audit` found
> **RUSTSEC-2026-0204** (`crossbeam-epoch 0.9.18`, invalid pointer dereference in
> the `fmt::Pointer` impl) in `prototype`'s committed `Cargo.lock`. Fixed by
> `cargo update -p crossbeam-epoch` → 0.9.20. All five surfaces now report zero
> vulnerabilities.
>
> **`rmi` was never affected, and the first draft of this note said it was.**
> Its `Cargo.lock` is git-ignored on purpose (`RecursiveMachineIntelligence/.gitignore`)
> so the vendored crate does not inherit this repo's pins — which means there is
> no committed pin to *be* stale, and a fresh resolve picks 0.9.20 unaided
> (verified by deleting the lockfile and regenerating: 0.9.20, zero
> vulnerabilities). The 0.9.18 that `cargo audit` saw was a stale artifact in one
> developer's working copy. A local finding is not a repository finding, and the
> difference is the whole value of checking where a lockfile comes from.
>
> Worth recording *how* the real one was missed: **GitHub Dependabot reported
> zero Rust alerts** the same day, while reporting 16 npm ones. "Dependabot is
> quiet" was taken as evidence the Rust side was clean; it was not evidence at
> all. That is why §4's recommendation is now implemented rather than repeated.

| ID | Crate | Sev | Status |
|---|---|---|---|
| **RUSTSEC-2026-0041** | `lz4_flex 0.11.5` | **8.2 High** — decompress of invalid data can leak uninitialized/reused buffer memory | **FIXED** — pinned `>=0.11.6` via `cargo update -p lz4_flex --precise 0.11.6`. rmi uses lz4 for protocol compression, so this was in-path. |
| RUSTSEC-2024-0436 | `paste 1.0.15` | unmaintained (warning) | **Accepted** — transitive via `wgpu→metal`, only under the non-default `gpu` feature; no code path in default builds. Tracked for when wgpu updates. |
| ~~RUSTSEC-2026-0097~~ | ~~`rand 0.8/0.9`~~ | unsound (warning) | **No longer applies.** The rationale here was "not applicable — the unsoundness needs a reentrant logger". That was true, and it is now moot: `prototype`'s lockfile carries `rand 0.8.6`, which the advisory lists as *patched* (`< 0.9.0, >= 0.8.6`). `cargo audit` has stopped reporting it. Kept struck through rather than deleted, because an accepted-risk row that silently vanishes is indistinguishable from one nobody re-checked. |
| **RUSTSEC-2026-0190** | `anyhow 1.0.102` | unsound (warning) — borrow-rule violation in `Error::downcast_mut()` after `Error::context` | **FIXED 2026-08-18** — `cargo update -p anyhow` → 1.0.104 (patched at ≥ 1.0.103). Transitive via the `wit-bindgen`/`wit-component` chain; nothing in this repository calls `downcast_mut`, so it was not in-path, but a patched version existed and accepting a fixable finding is not triage. **This advisory is dated 2026-06-25 and was not in this table** — the register had drifted in both directions at once: listing a warning that had stopped firing, and missing one that had started. |
| (yanked) | `lz4_flex 0.11.5` | yanked | resolved by the 0.11.6 pin above. |

**Result (re-run 2026-08-25, unchanged from 2026-08-18):** 0 open
vulnerabilities. The four *committed* lockfiles — `prototype`, `forge`,
`ribosome`, `germline` — report **zero findings of any kind**, warnings
included. The single remaining warning is `paste` (unmaintained) on
`RecursiveMachineIntelligence/Cargo.lock`, which is git-ignored.

**And that warning is not merely local, which this row used to imply.** It said
the `paste` finding was "a property of a local resolve rather than of this
repository" — the phrasing that was exactly right for `crossbeam-epoch 0.9.18`,
a stale artifact in one working copy that a fresh resolve did not reproduce.
`paste` is not that. CI generates `rmi`'s lockfile from scratch and
`check-security-register.sh` reports the identical single advisory there
(2026-08-25, run 32902128162), so a fresh consumer resolve gets it too: it is a
property of `rmi`'s dependency graph via `wgpu→metal`, and it is git-ignored
rather than local. The two cases look the same in a report and are not the
same, and only running the fresh resolve tells them apart.

agentic-eval's own dependency surface is 2 optional crates (`tiktoken-rs`,
`serde`) — no findings.

**Four or five?** Both numbers are right and they count different things. There
are **five** lockfile surfaces and CI audits all five; only **four** are
*committed*, and the fifth — `RecursiveMachineIntelligence/Cargo.lock` — is
git-ignored so that the vendored crate does not inherit this repo's pins. CI
generates it fresh, which audits what a consumer would actually resolve rather
than one developer's working copy; that distinction is what made a
`crossbeam-epoch` hit there a false alarm while the identical hit in
`prototype` was real. So a finding on the four is a property of this
repository, and a finding on the fifth may not be.
`check-security-register.sh` audits all five and labels the fifth
`(git-ignored)` wherever it reports one.

> **An accepted-risk register decays like any other measured claim, and worse.**
> Both drifts above are invisible to a reader: the document names two accepted
> warnings, and someone checking whether the accepted set is complete would have
> found it neither complete nor current. `cargo audit` in CI catches new
> *vulnerabilities*; for a long time nothing compared this table against the
> warnings actually reported.
>
> **Something does now, as of 2026-08-25: `scripts/check-security-register.sh`**,
> run by CI's `audit` job. It reads the rows *out of this table* rather than
> keeping its own copy, and fails in both directions — an advisory reported by
> any of the five surfaces that has no row here, **and** a row marked
> **Accepted** that no surface reports any more. A row whose Status column it
> cannot classify as FIXED, Accepted or "No longer applies" is also a failure,
> because a row this check cannot read must not read as a row that passed.
> Verified by breaking each case and watching it fail. Run it by hand with
> `bash scripts/check-security-register.sh`, or the surfaces directly with
> `for d in prototype forge ribosome germline; do cargo audit --file $d/Cargo.lock; done`
>
> What it does **not** check is the *rationale*: it compares advisory
> identities, not the reasoning in the Status column. "Only under the
> non-default `gpu` feature" is a claim about a dependency graph, and nothing
> re-derives it. Re-read those by hand when touching this file.
>
> **And the npm surface, which nothing here had ever named.** `HANDOFF.md`
> claimed "0 npm"; `video/` had **one high-severity** advisory
> (GHSA-2v37-7h3g-55p8, `nanoid < 3.3.18`, transitive via `postcss`), fixed
> 2026-08-18 with `npm audit fix --package-lock-only` → 3.3.18, and now 0
> vulnerabilities (of 294 packages then, 293 on 2026-08-25 — the package count
> is not a security figure and is recorded only so the two runs can be told
> apart). Both `video/package.json` and `video/package-lock.json` are tracked,
> so this was a repository finding rather than a local one. Check it with
> `cd video && npm audit`. CI's `audit` job now covers it twice: an
> `npm audit --audit-level=high` step that gates, and
> `check-security-register.sh`, which reads the *total* across every severity —
> because the claim this document makes about that surface is **zero**, and a
> moderate finding is still a finding nobody wrote down. That sentence used to
> end "CI's `audit` job covers the five Cargo lockfiles and not this one",
> which had been false since the npm step was added the same day.

**Recommendation (CMMC SI / supply chain): ✅ implemented 2026-08-05.** CI now
has an `audit` job running `cargo audit` over each of the five lockfiles
separately — separately because each workspace resolves its own dependency graph
and a clean result in one says nothing about the others.

It deliberately does **not** pass `-D warnings`. The remaining findings are
`unmaintained` and `unsound` advisories in transitive dependencies with no
patched version to move to; failing on those would make the job permanently red
and therefore ignored, which is worse than not having it. Vulnerabilities fail
the build; warnings are printed and read. `cargo deny` with a `deny.toml`
allowlist remains the stricter option if a release gate is ever wanted.

---

## 2. NIST FIPS 140-3 (cryptographic posture)

**Cryptography inventory:**

> **Corrected 2026-08-25, and this section was the most wrong in the
> document.** The inventory below listed three primitives, none of them keyed,
> and concluded from that "no secret keying, no signatures, no KDF". The
> repository contains **HMAC-SHA256 with fleet secret keys, Ed25519 signatures
> with per-worker private seeds and a revocation store, constant-time
> comparison, and a challenge-response worker authentication handshake** —
> every one of them in `ribosome`/`germline`, and none of them listed.
>
> *How it happened is the same mechanism as everywhere else in this document.*
> The **Scope** line at the top names three surfaces — `rmi`, `prototype`,
> `agentic-eval`. `ribosome` and `germline` were extracted from `forge` later
> (§1, steps 148–149). §1 was updated to audit **five** lockfiles; §2's scope
> was never widened, so the crypto that arrived with those crates was never
> inventoried. **A scope that grows silently makes every "there is no X"
> finding in the document expire without notice**, and an absence claim is
> exactly the kind that cannot fail loudly.
>
> **The code is not the problem.** The hand-rolled HMAC is validated against
> RFC 4231 cases 1–3 including the long-key case, comparison is constant-time,
> and the trust model is written down at the call site rather than assumed. The
> defect is entirely in the section whose job was to record that it exists.

| Primitive | Crate | Use | FIPS-approved algorithm? |
|---|---|---|---|
| SHA-256 | `sha2 0.10` | content-addressing (ontology/protocol/storage IDs, ParamStore weight keys) | **Yes** (FIPS 180-4) — but RustCrypto `sha2` is **not a FIPS 140-3 *validated module*** |
| **HMAC-SHA256** | **hand-rolled**, `ribosome/src/mac.rs` (RFC 2104, over `sha2`) | **Authentication with a shared secret key**, in three places: `ribosome::provenance` (build-cache claims), `germline::attest` (verdicts), and `ribosome::remote` (worker challenge-response). Symmetric, so any verifier can also mint — stated at the call sites | **Yes** (FIPS 198-1) — implemented directly rather than via a crate, and **tested against RFC 4231 cases 1–3**, long-key case included. Not a validated module |
| **Ed25519** | **`ed25519-dalek 3.0`**, `ribosome/src/provenance.rs` | **Digital signatures over build provenance**, with per-worker 32-byte private seeds, a `TrustStore` of public keys, and revocation. This is the asymmetric scheme the HMAC path's own doc comment says it is waiting for | **Yes** (FIPS 186-5, since 2023). Not a validated module |
| Constant-time compare | hand-rolled `ct_eq`, `ribosome/src/mac.rs` | MAC and signature comparison without a timing oracle | N/A — a side-channel control, not an algorithm, and its presence is what makes the two rows above defensible |
| **TLS 1.3** | **`rustls 0.23` + `ring`**, optional (`ribosome`'s `tls` feature) | Encrypted worker transport, `ribosome/src/tls.rs`. **Not** used by RAP | `ring` is not a FIPS-validated module; `rustls` can be built on one (`aws-lc-rs` FIPS), which is the concrete migration if a regulated deployment ever needs it |
| xxHash (xxh3/xxh64) | `xxhash-rust` | non-cryptographic hashing (caches, dedup) | N/A — non-security use, correctly chosen |
| LCG (internal) | rmi | deterministic weight init / fix-seed | N/A — explicitly not cryptographic |

**Findings:**
- **No FIPS-validated cryptographic module is in use.** SHA-256 via RustCrypto is the correct *algorithm* but the crate carries no CMVP certificate. For any deployment with a FIPS 140-3 requirement (federal/CMMC L2+), the SHA-256 calls must route through a validated module (e.g. AWS-LC-FIPS / OpenSSL 3 FIPS provider).
- **~~All SHA-256 usage is integrity/addressing, not confidentiality or authentication. No secret keying, no signatures, no KDF.~~** **False, and the conclusion drawn from it does not hold.** SHA-256 is *also* keyed, as HMAC, to authenticate build-cache claims, germline verdicts and worker handshakes; Ed25519 signs provenance with per-worker private keys. **There is secret key material in this system** — a fleet HMAC key and per-worker Ed25519 seeds — and its handling is a real security property, not a compliance abstraction. Still no KDF: keys are supplied, not derived, which is worth stating because it means **key provisioning is entirely the operator's problem and nothing here helps with it**. The FIPS gap is therefore *not* purely non-cryptographic-assurance: for a regulated deployment, the HMAC and Ed25519 paths are in scope for validated-module requirements, where the old finding said nothing was.
- **No transport encryption on RAP**, and this row used to say "no transport encryption" flat. The RAP server (`--rap`) is **plaintext JSON-RPC over TCP** — verified 2026-08-25 — so no cipher-suite FIPS question arises *there*; see ATT&CK §3. But `ribosome` ships a **TLS 1.3 worker transport** behind `--features tls` (`rustls` + `ring`), so the repository does have transport encryption and does raise a cipher-suite question, just not on the surface this row was looking at. Open item 3 records the trust posture as deliberately the operator's.
**Key management, measured 2026-08-25.** §2 previously said nothing about keys
because it believed there were none. What the code actually does, and what it
leaves to whoever embeds it:

- **There is no production key source.** Every `Signer::new`, every
  `AsymmetricSigner::from_seed`, and every `auth_key` call site in this
  repository is **in a test**. No environment variable, config field or CLI
  flag provisions a key. That is a defensible library posture — the fleet key
  is the embedder's — but it means there is no reference path, and none of the
  guidance below exists anywhere else.
- **Keys are not validated, and an empty one is accepted.** Measured, not
  inferred: `Signer::new("w", Vec::new())` signs, and the record verifies; so
  does a one-byte key. **An empty `Vec` is what an unset environment variable
  or a missing config field naturally becomes**, so a fleet whose key was never
  provisioned would authenticate every claim and report success while providing
  nothing — and no verifier can tell, because a MAC over an empty key is a
  well-formed MAC. RFC 2104 recommends at least the hash output length, 32
  bytes here. **Left as an owner decision rather than changed**: refusing a
  weak key means `Signer::new` returns a `Result`, which is a breaking change
  to a public API, and this document is not the place to make that call. The
  constructor's rustdoc now warns; the recommendation is below.
- **The two schemes have opposite rotation stories, and only one is written
  down.** The Ed25519 path rotates properly: keys are per-worker, `TrustStore`
  holds public keys, `revoke` records a revocation rather than deleting the key
  so a rejoining node cannot re-trust itself, and `verify` fails closed on
  revoked, unknown, substituted-key, wrong-subject and bad-signature — checked
  by reading every branch. The **HMAC path has no rotation at all**: one key per
  `Signer`, one `auth_key` per `WorkerServer`, no key id and no overlapping
  acceptance window, so changing the fleet key requires every worker and every
  verifier to change simultaneously. For a shared secret that is the hardest
  operation to perform safely, and it is the one with no support.
- **The auth nonce is deliberately predictable**, and says so:
  `ribosome::remote::next_nonce` is `HMAC(b"ribosome-nonce", clock ‖ counter)`
  under a **constant, public** key, with a comment arguing that "the
  requirement is uniqueness per connection, not unpredictability, because the
  secret is the key". That holds against replay. It is weaker against a
  **pre-play** attacker who can predict which nonce the server will issue and
  induce a legitimate client to answer it first — which needs a rogue endpoint
  and a guess at the issuing nanosecond, and is off the table entirely when the
  `tls` transport is used. **Recorded as a stated assumption for the owner to
  confirm, not as a defect**: whether it matters is a threat-model question,
  and the reasoning in the code is explicit rather than accidental.

- **Action (documented, not yet implemented):** (a) gate SHA-256 **and the HMAC and Ed25519 paths** behind a `fips` feature that swaps to a validated provider for regulated deployments — the original said SHA-256 only, because it did not know the other two existed; (b) if RAP is ever exposed beyond loopback, require rustls with a FIPS-validated backend, and build `ribosome`'s existing `tls` feature on `aws-lc-rs` FIPS rather than `ring` for the same reason; (c) **the three key-management items above**, in priority order: refuse a weak or empty HMAC key (an owner's API call, and the highest-value of the three because the failure mode is silent success); give the HMAC path a key id and an overlapping acceptance window so the fleet secret can be rotated without a simultaneous fleet-wide change; and confirm or revise the stated assumption that the auth nonce need not be unpredictable.

---

## 3. MITRE ATT&CK (threat model of the live surfaces)

Mapped to ATT&CK techniques for the realistic adversary: untrusted input to the
compiler/server, and agent-generated code executed locally.

| Surface | Technique | Assessment / Mitigation |
|---|---|---|
| **RAP server** `--rap` (TcpListener) | T1071 (App-layer C2), T1190 (exploit public-facing) | **Binds `127.0.0.1:9876` by default** (loopback). No auth/authz on the socket, so it **must not** be bound to `0.0.0.0` without a reverse proxy doing authN/Z + TLS. **The refusal is implemented** (`rap::is_non_loopback`): a non-loopback or wildcard bind exits 2 with an explanatory message unless the operator sets `MAGE_RAP_ALLOW_REMOTE=1`, and warns even then. Verified by running it: `--rap 0.0.0.0:9911` → `rap: REFUSING to bind non-loopback address`, rc=2. **Re-verified 2026-08-25**, including the override path: `MAGE_RAP_ALLOW_REMOTE=1` warns (`WARNING binding non-loopback … with no auth/TLS`) and then binds, so both halves of the row are measured rather than one. This row said "proposed, not yet enforced" — **an under-claim in a security document is still a defect**: a reader planning a deployment would build a guard that already exists, or avoid the tool for a gap it does not have. |
| **Subprocess backends** (`--backends-file`, `Command::new(prog).spawn()`) | T1059 (command/scripting), T1106 (native API) | Runs an operator-supplied wrapper program. **Already classified `exec`** in the CLI manifest and RMI safety effect-map. Only reachable via an explicit local flag — operator-controlled, not attacker-reachable. Fail-safe: no shell interpolation (args passed as argv, not `sh -c`). |
| **Deserialization** (Agentic Binary Language containers, MessagePack protocol, checkpoints) | T1565 (data manipulation) | Agentic Binary Language decode is **length-checked, bounds-validated** (`take()` guards every field) — in `run_decode_abl_bytes` and `run_dispatch_abl_bytes`, which hand-roll the same decoder. **Verified by running it 2026-08-25**, not by reading it: eight malformed containers — truncated header, bad magic, bad version, four-billion item count, and `name_len`/`expr_len` at the `u32` ceiling — each produced a diagnostic and no panic, abort or hang, through both entry points. Pinned by `prototype/tests/abl_container_exit_status.rs`. (The `*pos + n` bound is an unchecked add, but `n` comes from a `u32` field and `pos` from the buffer, so it cannot overflow a 64-bit `usize`; on a 32-bit target it would wrap to a panicking slice range, not an out-of-bounds read.) **No `pickle`-class arbitrary-code-execution path** — formats are data-only (contrast PyTorch `torch.load`, flagged in agentic-eval). Malformed input yields a diagnostic and a non-zero exit from these two, and a typed `RmiError` from `rmi`'s own decoder — not memory unsafety. **Until 2026-08-25 both of these exited 0 on every corrupt container**, while the same match arm exited 1 for a file it could not read; the guards had always worked and nothing surfaced that they had fired. |
| **Agent-generated code execution** | T1059, T1027 | The whole point of the compiler is to process untrusted (LLM-written) source. Front-end is **memory-safe Rust** — `prototype/src` contains **no `unsafe` outside `cuda_backend.rs`** (3 blocks, IronAccelerator FFI, behind the non-default `cuda` feature); the remaining hits in `lexer.rs`/`elision.rs`/`token_budget.rs` are the *MAGE keyword* `unsafe`, not Rust code. Parse/check/lower cannot escape the process.
    **Corrected 2026-08-18.** This row said "rmi has 1 audited `unsafe` in lib.rs, 3 in the CUDA FFI shim — all reviewed, FFI-boundary only". All three parts were wrong. `lib.rs` has none. The real surface is **13 `unsafe` items in `RecursiveMachineIntelligence/src/runtime/memory_pool.rs`** — 9 blocks in an arena allocator doing `alloc_zeroed`, pointer `add` and `offset_from`, plus **4 `unsafe impl Send`/`Sync`** on `Slab` and `TensorBuffer` — which is **not an FFI boundary** and is **compiled unconditionally** (`pub mod runtime` has no feature gate). This row said "9 blocks" for a while, counting only the blocks; the four impls are the higher-risk half, because a block's soundness is local while an `unsafe impl` is an unchecked global assertion — and they are where the fifth defect actually was (§5). Re-derive both with `grep -nE '\bunsafe\s*(\{|fn |impl )'`, which reports 13 here, 16 in the never-compiled `compute/cuda_full.rs`, and nothing anywhere else in `rmi`, `forge`, `ribosome` or `germline`. A memory allocator is the highest-risk category of `unsafe` in an assessment like this, and the document asserted no such code existed. Separately, `compute/cuda_full.rs` holds 16 more, but **no `mod` declaration references it**, so it is never compiled — the file is a dead cudarc-0.10 port that `compute/mod.rs` describes in a comment as "unused". Counting it would overstate the surface as badly as the old row understated it. *Running* compiled output is the operator's risk surface → see CMMC sandboxing note. |
| **Self-modification** (`evolution::self_modification`) | T1565.001, T1027 | Applies code patches through `SandboxLimits` + `ResourceUsage` checks. Effect-mapped **exec-equivalent**; documented in the manifest as "gate behind approval in agent deployments." |
| **Supply chain** | T1195.001 (compromised dep) | Covered by §1; the lz4_flex fix closes the one in-path high-sev item. |

**No credential, token, or secret material is handled anywhere in the codebase** (confirmed by §0 leak scan) — so credential-access tactics (T1552 etc.) have no target.

---

## 4. CMMC 2.0 (practice-level gaps)

Assessed against CMMC L1/L2 practices relevant to a source release (not a CUI-handling deployment — most CMMC practices are organizational/operational and out of scope for a repo, so this lists only what the *codebase* can satisfy or block).

| Domain / Practice | Status |
|---|---|
| **AC** (Access Control) | RAP has no authN — **gap for any networked deployment.** Loopback-default mitigates for local use. Documented. |
| **AU** (Audit) | `RmiError::category()` + structured diagnostics give machine-parseable audit events; no centralized audit log (app-level concern). |
| **CM** (Config Mgmt) | Deterministic ontology/manifest + `Cargo.lock` committed → reproducible builds. ✅ for the **four** workspaces `git ls-files '*Cargo.lock'` returns — `forge`, `germline`, `prototype`, `ribosome`. `RecursiveMachineIntelligence/Cargo.lock` is git-ignored **by design**, so the vendored crate does not inherit this repo's pins; its build is reproducible only to the extent a fresh resolve is. That is the same four-versus-five distinction as §1, and this row used to state the ✅ unqualified. |
| **IA** (Identification & Auth) | None in-app (see AC). |
| **RA** (Risk Assessment) | This document + `cargo audit` in CI over five lockfiles (done 2026-08-05), plus `scripts/check-security-register.sh`, which compares §1's accepted-risk register against what those runs report (2026-08-25). **This row read "Recommend wiring `cargo audit`/`cargo deny` into CI as a gate (the one concrete CI action item)" for twenty days after the gate was wired** — the recommendation and the "Open recommendations" section below it had already disagreed. `cargo deny` remains genuinely open. |
| **SC** (System & Comms Protection) | No TLS on RAP (SC gap if exposed); memory-safe core (SC partial via Rust). |
| **SI** (System & Info Integrity) | lz4_flex CVE remediated; bounds-checked deserialization — **now measured rather than asserted**: eight malformed containers, including length fields at the `u32` ceiling and a four-billion item count, each produce a diagnostic and no panic, abort or hang through both `--from=abl-bytes` and `--run=abl-bytes` (2026-08-25). Every one of them also exited **0** until that run, which is fixed and pinned by `prototype/tests/abl_container_exit_status.rs`. Typed errors. ✅ for the fixed items. |

**CMMC bottom line:** the codebase is consistent with L1 self-assessment for a local research tool. L2 (CUI) would require, at minimum: FIPS-validated crypto module (§2), RAP authN+TLS or removal of network exposure (§3), and the CI audit gate (§1). None are blockers for an **open-source research release**; all are documented prerequisites for a *regulated* deployment.

---

## Actions taken in this audit
1. **Fixed RUSTSEC-2026-0041** (lz4_flex high-severity) — pinned 0.11.6, re-audited clean, 1226 tests passing at the time (2,904 across five crates now).
1b. **Fixed RUSTSEC-2026-0190** (anyhow unsound, 2026-08-18) — `cargo update -p anyhow` → 1.0.104; the four committed lockfiles now report zero findings of any kind.
2. Inventoried crypto (FIPS gap documented), deserialization (bounds-checked, no pickle-class RCE), and network/exec surfaces (loopback default, effect-mapped).
3. Confirmed **zero secret/credential material** in the codebase (leak scan).
   **Re-run 2026-08-25 and still zero, but read what it covered**: 559 tracked
   files matched against high-signal issuer patterns only — AWS access key ids,
   `ghp_`/`sk-`/`xox*` tokens, and PEM private-key headers. That is a
   *tool-less* scan: no `gitleaks` or `trufflehog` is installed here, so there
   is no entropy analysis, no history scan, and nothing that would catch a
   credential without a recognisable prefix. It is also a scan of the working
   tree, not of `git log`. **The original line named no tool, no corpus and no
   date**, which is the shape of claim this document keeps discovering to be
   unfalsifiable; this one is narrow enough to be worth repeating and honest
   about what it misses. A real secret-scanning gate remains open, alongside
   `cargo deny`.

## Open recommendations (non-blocking for OSS release)
- `cargo audit` is wired into CI (done 2026-08-05, five lockfiles separately).
  `cargo deny` with a `deny.toml` remains open; the allowlist would now hold
  **one** entry (`paste`, unmaintained, rmi's uncommitted lockfile only) rather
  than the two this line used to name — see §1's re-run note for why that count
  moved in both directions.
- ~~Add a loud warning/refusal when `--rap` binds a non-loopback address.~~
  **Implemented** — refusal with an `MAGE_RAP_ALLOW_REMOTE=1` override, plus a
  warning on the override path. §3's row said "proposed, not yet enforced"
  until 2026-08-18; it was enforced well before that and nobody had re-read the
  code.
- ~~**Audit the `unsafe` in `rmi`'s memory pool.**~~ **Done 2026-08-18**, and it
  found two defects in `Slab::new`, both reachable in one call from public API
  because `PoolConfig`'s fields are public and `MemoryPool::with_config` passes
  `initial_capacity` straight through with no guard:
  - **Zero capacity → undefined behaviour.** `PoolConfig { initial_capacity: 0,
    .. }` computed a zero-sized `Layout` and called `alloc::alloc_zeroed` on it,
    which the standard library documents as UB — at *construction*, before any
    allocation was requested. `growth_factor: 0.0` reaches the same place
    through `alloc()`, since a float-to-int cast saturates to 0. Now refused.
  - **Unchecked `size * capacity`.** Wrapping made the slab tiny while
    `capacity` kept the large value. Measured consequence: the process
    **aborts** (`memory allocation of 2305843009213693960 bytes failed`, exit
    `0xc0000409`) — the `free_list` `Vec` wants one index per claimed block, so
    it fails first. A crash from a library constructor, not an out-of-bounds
    access; the two deserve different words and this was checked rather than
    assumed. Now refused with a named error.

  And two more in `TensorBuffer`, the zero-copy view type in the same file,
  both reachable from safe public API with ordinary values:
  - **`Drop` freed a layout the allocator never handed out.** `from_vec`
    forgot the `Vec`'s capacity and `Drop` rebuilt it as
    `from_raw_parts(ptr, len, len)`. Capacity differs from length in the
    ordinary case — `Vec::with_capacity(64)` with three elements pushed, or any
    growth pattern — so the global allocator was asked to deallocate a layout
    it never allocated. Undefined behaviour **on the normal path**, not an edge
    case, and the SAFETY comment asserted "the pointer, len, and capacity match
    what was passed to `mem::forget`", which was false whenever the two
    differed. The capacity is now carried.
  - **`slice(offset, len)` added the two without checking.**
    `slice(usize::MAX, 1)` wraps to 0, passes the `> self.len` bound, and
    offsets the pointer by `usize::MAX` — a buffer at a wild address that
    `as_bytes()` will read. Two plain integers through a safe public method.
    Now `checked_add`.

  All four have regression tests, each verified by removing its guard. The
  remaining blocks are sound as used: `Slab::free`'s `offset_from` needs a
  same-allocation pointer and every caller goes through `contains` first, which
  is integer comparison and safe on a foreign pointer — though the SAFETY
  comment stated that guarantee backwards and has been corrected.

  **The four `unsafe impl Send`/`Sync` were the fifth defect, and this passage
  used to understate it.** It said they were "defensible under the refcount
  discipline" but that reading the counter `Relaxed` while gating mutation was
  "a synchronisation decision rather than a counter — worth a second opinion
  from someone who owns this code." That was the wrong verdict, and hedging is
  how it survived: written down, the interleaving is a data race on the
  ordinary sharing path. `as_bytes_mut` is `Arc::get_mut` by hand. Thread A
  reads through `as_bytes`, then drops its handle (`fetch_sub`, `Release`);
  thread B's `Relaxed` load observes `1` **without** synchronizing-with that
  `Release`, so B's writes through the returned `&mut [u8]` are unordered
  against A's reads of the same bytes. **Fixed 2026-08-19**: the load is now
  `Acquire` (`memory_pool.rs:516`), which is the ordering `Arc::get_mut` uses
  and for exactly this reason, and the four `unsafe impl` are sound under it.
  `refcount()` stays `Relaxed` and is documented as advisory, since a statistic
  confers nothing.

  No test here verifies that and none can: the fix is invisible to a
  single-threaded suite, and on x86 — where loads are acquire in hardware — the
  broken version would not have manifested either. `loom` would verify it and
  is not a dependency this vendored crate should gain unilaterally. Recorded
  rather than instrumented.

- **Dead `unsafe` in `compute/cuda_full.rs`.** 16 blocks in a file no `mod`
  declaration references, so it never compiles. It reads as reviewed code and
  is not covered by anything; anyone adding a `mod cuda_full;` activates it
  silently. Delete it or gate it explicitly — left alone here because the crate
  is vendored and that is its owner's call.
- Add a `fips` feature flag routing SHA-256 through a validated module, for regulated downstreams.
- If RAP is ever productionized: rustls (FIPS backend) + token auth.

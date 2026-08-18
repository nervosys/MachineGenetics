# Security Audit — MAGE (Machine Genetics) + RecursiveMachineIntelligence (rmi)

**Org:** NERVOSYS · **Date:** 2026-06-04, §1 re-run 2026-08-18 · **Scope:** `RecursiveMachineIntelligence/`
(crate `rmi`), `prototype/` (compiler + RAP server), `agentic-eval` (separate
AetherShell repo). Frameworks applied: **CVE/RustSec**, **NIST FIPS 140-3**,
**MITRE ATT&CK**, **CMMC 2.0**.

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

**Result (re-run 2026-08-18):** 0 open vulnerabilities. The four *committed*
lockfiles — `prototype`, `forge`, `ribosome`, `germline` — report **zero
findings of any kind**, warnings included. The single remaining warning is
`paste` (unmaintained) on `RecursiveMachineIntelligence/Cargo.lock`, which is
git-ignored, so it is a property of a local resolve rather than of this
repository. agentic-eval's own dependency surface is 2 optional crates
(`tiktoken-rs`, `serde`) — no findings.

> **An accepted-risk register decays like any other measured claim, and worse.**
> Both drifts above are invisible to a reader: the document names two accepted
> warnings, and someone checking whether the accepted set is complete would have
> found it neither complete nor current. `cargo audit` in CI catches new
> *vulnerabilities*; nothing compares this table against the warnings actually
> reported. Re-run the five surfaces when touching this file:
> `for d in prototype forge ribosome germline; do cargo audit --file $d/Cargo.lock; done`
>
> **And the npm surface, which nothing here had ever named.** `HANDOFF.md`
> claimed "0 npm"; `video/` had **one high-severity** advisory
> (GHSA-2v37-7h3g-55p8, `nanoid < 3.3.18`, transitive via `postcss`), fixed
> 2026-08-18 with `npm audit fix --package-lock-only` → 3.3.18, and now 0 of
> 294 packages. Both `video/package.json` and `video/package-lock.json` are
> tracked, so this was a repository finding rather than a local one. Check it
> with `cd video && npm audit`; CI's `audit` job covers the five Cargo
> lockfiles and not this one.

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

| Primitive | Crate | Use | FIPS-approved algorithm? |
|---|---|---|---|
| SHA-256 | `sha2 0.10` | content-addressing (ontology/protocol/storage IDs, ParamStore weight keys) | **Yes** (FIPS 180-4) — but RustCrypto `sha2` is **not a FIPS 140-3 *validated module*** |
| xxHash (xxh3/xxh64) | `xxhash-rust` | non-cryptographic hashing (caches, dedup) | N/A — non-security use, correctly chosen |
| LCG (internal) | rmi | deterministic weight init / fix-seed | N/A — explicitly not cryptographic |

**Findings:**
- **No FIPS-validated cryptographic module is in use.** SHA-256 via RustCrypto is the correct *algorithm* but the crate carries no CMVP certificate. For any deployment with a FIPS 140-3 requirement (federal/CMMC L2+), the SHA-256 calls must route through a validated module (e.g. AWS-LC-FIPS / OpenSSL 3 FIPS provider).
- **All SHA-256 usage is integrity/addressing, not confidentiality or authentication.** No secret keying, no signatures, no KDF. So the FIPS gap is *non-cryptographic-assurance* — it affects compliance posture, not present-day confidentiality.
- **No transport encryption.** The RAP server (`--rap`) is **plaintext JSON-RPC over TCP**. There is no TLS, so no cipher-suite FIPS question arises — but see ATT&CK §3.
- **Action (documented, not yet implemented):** (a) gate SHA-256 behind a `fips` feature that swaps to a validated provider for regulated deployments; (b) if RAP is ever exposed beyond loopback, require rustls with a FIPS-validated backend.

---

## 3. MITRE ATT&CK (threat model of the live surfaces)

Mapped to ATT&CK techniques for the realistic adversary: untrusted input to the
compiler/server, and agent-generated code executed locally.

| Surface | Technique | Assessment / Mitigation |
|---|---|---|
| **RAP server** `--rap` (TcpListener) | T1071 (App-layer C2), T1190 (exploit public-facing) | **Binds `127.0.0.1:9876` by default** (loopback) — not network-exposed unless the operator passes a routable addr. No auth/authz on the socket. **Mitigation:** documented as `network` effect in the CLI manifest; **must not** be bound to `0.0.0.0` without a reverse proxy doing authN/Z + TLS. Recommend an explicit refusal or loud warning on non-loopback bind (proposed, not yet enforced). |
| **Subprocess backends** (`--backends-file`, `Command::new(prog).spawn()`) | T1059 (command/scripting), T1106 (native API) | Runs an operator-supplied wrapper program. **Already classified `exec`** in the CLI manifest and RMI safety effect-map. Only reachable via an explicit local flag — operator-controlled, not attacker-reachable. Fail-safe: no shell interpolation (args passed as argv, not `sh -c`). |
| **Deserialization** (Agentic Binary Language containers, MessagePack protocol, checkpoints) | T1565 (data manipulation) | Agentic Binary Language decode is **length-checked, bounds-validated** (`take()` guards every field) — verified in `run_dispatch_abl_bytes`. **No `pickle`-class arbitrary-code-execution path** — formats are data-only (contrast PyTorch `torch.load`, flagged in agentic-eval). Malformed input yields a typed `RmiError`, not memory unsafety. |
| **Agent-generated code execution** | T1059, T1027 | The whole point of the compiler is to process untrusted (LLM-written) source. Front-end is **memory-safe Rust** (`#![forbid(unsafe)]` in agentic-eval; rmi has 1 audited `unsafe` in lib.rs, 3 in the CUDA FFI shim — all reviewed, FFI-boundary only). Parse/check/lower cannot escape the process. *Running* compiled output is the operator's risk surface → see CMMC sandboxing note. |
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
| **CM** (Config Mgmt) | Deterministic ontology/manifest + `Cargo.lock` committed → reproducible builds. ✅ |
| **IA** (Identification & Auth) | None in-app (see AC). |
| **RA** (Risk Assessment) | This document + `cargo audit`. **Recommend wiring `cargo audit`/`cargo deny` into CI as a gate** (the one concrete CI action item). |
| **SC** (System & Comms Protection) | No TLS on RAP (SC gap if exposed); memory-safe core (SC partial via Rust). |
| **SI** (System & Info Integrity) | lz4_flex CVE remediated; bounds-checked deserialization; typed errors. ✅ for the fixed items. |

**CMMC bottom line:** the codebase is consistent with L1 self-assessment for a local research tool. L2 (CUI) would require, at minimum: FIPS-validated crypto module (§2), RAP authN+TLS or removal of network exposure (§3), and the CI audit gate (§1). None are blockers for an **open-source research release**; all are documented prerequisites for a *regulated* deployment.

---

## Actions taken in this audit
1. **Fixed RUSTSEC-2026-0041** (lz4_flex high-severity) — pinned 0.11.6, re-audited clean, 1226 tests passing at the time (2,904 across five crates now).
1b. **Fixed RUSTSEC-2026-0190** (anyhow unsound, 2026-08-18) — `cargo update -p anyhow` → 1.0.104; the four committed lockfiles now report zero findings of any kind.
2. Inventoried crypto (FIPS gap documented), deserialization (bounds-checked, no pickle-class RCE), and network/exec surfaces (loopback default, effect-mapped).
3. Confirmed **zero secret/credential material** in the codebase (leak scan).

## Open recommendations (non-blocking for OSS release)
- `cargo audit` is wired into CI (done 2026-08-05, five lockfiles separately).
  `cargo deny` with a `deny.toml` remains open; the allowlist would now hold
  **one** entry (`paste`, unmaintained, rmi's uncommitted lockfile only) rather
  than the two this line used to name — see §1's re-run note for why that count
  moved in both directions.
- Add a loud warning/refusal when `--rap` binds a non-loopback address.
- Add a `fips` feature flag routing SHA-256 through a validated module, for regulated downstreams.
- If RAP is ever productionized: rustls (FIPS backend) + token auth.

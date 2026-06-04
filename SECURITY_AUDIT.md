# Security Audit — MachineGenetics (MechGen) + RecursiveMachineIntelligence (rmi)

**Org:** NERVOSYS · **Date:** 2026-06-04 · **Scope:** `RecursiveMachineIntelligence/`
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

Run on all three Cargo.lock surfaces against RustSec advisory-db (1116 advisories).

| ID | Crate | Sev | Status |
|---|---|---|---|
| **RUSTSEC-2026-0041** | `lz4_flex 0.11.5` | **8.2 High** — decompress of invalid data can leak uninitialized/reused buffer memory | **FIXED** — pinned `>=0.11.6` via `cargo update -p lz4_flex --precise 0.11.6`. rmi uses lz4 for protocol compression, so this was in-path. |
| RUSTSEC-2024-0436 | `paste 1.0.15` | unmaintained (warning) | **Accepted** — transitive via `wgpu→metal`, only under the non-default `gpu` feature; no code path in default builds. Tracked for when wgpu updates. |
| RUSTSEC-2026-0097 | `rand 0.8/0.9` | unsound (warning) | **Not applicable** — the unsoundness requires a custom global logger calling `rand::rng()` reentrantly; rmi uses `rand` only for weight init / sampling, never from a logger. No remediation needed; documented. |
| (yanked) | `lz4_flex 0.11.5` | yanked | resolved by the 0.11.6 pin above. |

**Result:** 0 open vulnerabilities after the lz4_flex fix; 2 informational warnings accepted with rationale. agentic-eval's own dependency surface is 2 optional crates (`tiktoken-rs`, `serde`) — no findings.

**Recommendation (CMMC SI / supply chain):** add `cargo audit` and `cargo deny` to CI as a release gate (a `deny.toml` allowlisting the two accepted advisories). Not yet wired — see §4.

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
| **Deserialization** (RMIB containers, MessagePack protocol, checkpoints) | T1565 (data manipulation) | RMIB decode is **length-checked, bounds-validated** (`take()` guards every field) — verified in `run_dispatch_rmil_bytes`. **No `pickle`-class arbitrary-code-execution path** — formats are data-only (contrast PyTorch `torch.load`, flagged in agentic-eval). Malformed input yields a typed `RmiError`, not memory unsafety. |
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
1. **Fixed RUSTSEC-2026-0041** (lz4_flex high-severity) — pinned 0.11.6, re-audited clean, 1226 tests still pass.
2. Inventoried crypto (FIPS gap documented), deserialization (bounds-checked, no pickle-class RCE), and network/exec surfaces (loopback default, effect-mapped).
3. Confirmed **zero secret/credential material** in the codebase (leak scan).

## Open recommendations (non-blocking for OSS release)
- Wire `cargo audit` + `cargo deny` into CI with a `deny.toml` (accept the 2 informational advisories explicitly).
- Add a loud warning/refusal when `--rap` binds a non-loopback address.
- Add a `fips` feature flag routing SHA-256 through a validated module, for regulated downstreams.
- If RAP is ever productionized: rustls (FIPS backend) + token auth.

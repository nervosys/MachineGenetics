# Ribosome — the distributed, agent-operated build engine

> **Status: implemented, including distribution.**
> `ribosome/` — its own crate, 162 tests. Every claim marked ✅ below is executed
> by a test; every ◻ is a design with no code behind it yet. This split is
> load-bearing: see [DOCS.md](DOCS.md) for why this repository marks unbuilt
> designs rather than describing them in the present tense.

A ribosome reads a genetic sequence and synthesizes the protein it encodes. This
one reads source and synthesizes artifacts — and like its namesake it is *many,
identical, and concurrent*: any number of them work the same tape and produce the
same product.

It lives in its own workspace and **depends on nothing else in this repository**.
It started inside `forge`, the package registry, which was a convenient place to
put it and the wrong place to leave it: a build system that ships inside a
package registry can only ever be that registry's build system, and §2.1's claim
that no language is privileged below the planner is not credible from a crate
that depends on one language's compiler. Its whole dependency list is `serde`,
`serde_json`, `sha2`, `ed25519-dalek`. CI checks that with `cargo tree` rather
than trusting this paragraph. `forge` depends on *it*, and `germline` drives it
through the same public API any other caller would use.

---

## 1. The one question a build system answers

**When may a previous result be reused?**

Everything else — parallelism, remote execution, dashboards — is optimization
around that question. Get it wrong and you have a fast liar.

Bazel answers it with *hermeticity by convention*: declare your inputs, promise
not to read anything else, and it hashes what you declared. The promise is
unenforced, so the failure mode is silent — an action reads the clock or an
ambient file, the key does not change, and a stale artifact is served. Every large
Bazel deployment grows folklore to cope: `--nocache_test_results`, repro-only CI
lanes, "try a clean build."

MAGE can answer it **structurally**, because two properties already hold and are
measured (`MEASUREMENTS.md` §2 "Determinism"):

1. **ABL artifacts are byte-stable.** The same spec builds to byte-identical
   bytes across runs and machines. A content hash is exact identity, not a proxy.
2. **Construction is tool-mediated.** An agent submits a *spec*; the compiler
   constructs the artifact. There is no ambient-state escape hatch to forget to
   declare, because the agent never hands over a command line — it hands over data.

So a cache key here is a statement about identity rather than a bet on
discipline. That is the entire design. Everything below follows from taking it
seriously.

### What this is not

This is not Bazel-equivalent, and claiming so would be the kind of thing
[DOCS.md](DOCS.md) exists to catch. Bazel's value is 15 years of language
rulesets, a hardened remote-execution protocol, and operation at a scale this has
never seen. What Ribosome has is a **better foundation for the correctness
property** and a design shaped around agents rather than humans. Those are
different claims, and only the first is demonstrated.

---

## 2. Architecture

```
        ┌──────────────────────────────────────────────┐
agents  │  plan() · to_json() · BuildReport · fitness() │   data in, data out
        └───────────────────────┬──────────────────────┘
                                │
        ┌───────────────────────▼──────────────────────┐
        │  sched   cache → dispatch → heal → record     │
        └───┬───────────────┬──────────────┬───────────┘
            │               │              │
     ┌──────▼─────┐  ┌──────▼──────┐  ┌────▼─────┐
     │   graph    │  │     cas     │  │   heal   │
     │ DAG, waves │  │ CAS +action │  │ remedies │
     │ crit. path │  │    cache    │  │          │
     └──────┬─────┘  └─────────────┘  └──────────┘
            │
     ┌──────▼──────────────────────────────────────────┐
     │  exec :: Executor  — local · pool · remote (TCP)    │
     └─────────────────────────────────────────────────┘
```

Above `graph` sits `lang`, which is the only module that knows any language
exists — see §2.1.

| Module | Role | Status |
|---|---|:--:|
| `lang` | languages, toolchains, hermeticity tiers | ✅ |
| `graph` | action DAG; edges **derived** from inputs, not declared | ✅ |
| `key` | SHA-256 over a canonical, length-prefixed encoding | ✅ |
| `cas` | content-addressed store + action cache, self-verifying | ✅ |
| `exec` | the `Executor` seam; `LocalExecutor`, `PoolExecutor` | ✅ |
| `heal` | failure classification → remedy | ✅ |
| `sched` | cache/dispatch/heal/record, `plan()`, `BuildReport` | ✅ |
| `manifest` | a build file: targets, toolchains, pinning | ✅ |
| `remote` | transport over any `Read + Write`, `RemoteExecutor`, worker registry | ✅ |
| `provenance` | signed action-cache claims: HMAC + Ed25519 with revocation | ✅ |
| `subprocess` | sandboxed execution of foreign tools | ✅ |

### Derived edges

A caller states which action produces which output. Dependency edges are then
*computed* from what each action consumes. Hand-declared edges are how build
graphs go subtly wrong — an edge you forgot is a race, an edge you added need-
lessly is lost parallelism, and neither surfaces until the graph is large.
Deriving them makes both unrepresentable. ✅

### Keys

The key covers tool identity *including version*, arguments, input **digests**,
allowlisted environment, requested outputs, and platform. It excludes the action's
name and its cost hint — renaming a target must not rebuild it.

Two details that are easy to get wrong and expensive to discover late:

- **Length-prefixed fields.** Without it, tool `ab` + arg `c` keys identically to
  tool `a` + arg `bc`. A silent wrong hit. ✅ (tested)
- **Sorted inputs, unsorted args.** Two agents declaring the same inputs in
  different orders must share a cache entry; `-o a b` and `-o b a` must not. ✅

### Two stores, deliberately separate

- **CAS** `digest → bytes`: immutable, self-verifying, shareable with anyone who
  trusts SHA-256. Never needs invalidation.
- **Action cache** `key → result`: a *claim* that this action produces these
  outputs. Invalidatable, and reasonably distrusted.

That separation is what makes a fleet safe to share: blobs can come from an
untrusted mirror and be verified on arrival, while action-cache entries are the
thing to sign, audit, or refuse. `Cas::get` rehashes on every read — disks rot,
and a corrupted artifact flowing into a build puts the symptom arbitrarily far
from the cause. ✅

### 2.1 Arbitrary languages, without pretending

§1's answer — reuse is safe because ABL is byte-stable and construction is
tool-mediated — is a claim about *MAGE's* pipeline. It does not survive contact
with `gcc`, and a build engine that only builds its own language is a toy.

The problem is specific. `Action::tool` is a string, and `"gcc@13.2.0"` on two
workers is not evidence that the same compiler ran: one machine's is
distribution-patched, another's embeds `__DATE__`, a third resolves different
libc headers. The keys agree, the outputs do not, and a shared cache serves one
machine's artifact to another — the worst failure a build system can have,
because it is silent and gets *more* likely as your hit rate improves.

So the strength of the claim is an explicit, per-toolchain, **keyed** value:

| `Hermeticity` | Means | Remote cache |
|---|---|:--:|
| `Structural` | output byte-stable by construction — *measured* (MAGE/ABL) | yes |
| `Pinned` | the toolchain binary is identified by content digest | yes |
| `Declared` | the toolchain is named, not verified | **no** |

Two consequences, both enforced rather than documented and hoped for:

- A pinned toolchain's digest goes **into the tool id and therefore into the
  key** (`clang@18.1.0+sha256-1f3a9c2b7d40`), so two different `gcc-13.2.0`
  binaries cannot collide. ✅ (tested)
- A declared toolchain is marked `+unpinned`, and `Store::open_shared` **refuses
  to publish its claims**. The action still runs and still caches locally, where
  "same host, same binary" is reasonable; it is never offered to a fleet, where
  it is not. Enforcement is per action, so one unpinned C file does not cost the
  MAGE artifact beside it its shareability. ✅ (tested)

Upgrading is one call: `Registry::pin("c", digest)` measures the compiler and the
same build becomes remote-cacheable. Every key changes, which is correct — the
old keys were claims about a tool nobody had checked. ✅

**Granularity is declared, not assumed.** C compiles per translation unit and
links; `rustc` and `go` consume a whole crate or package at once. Modelling both
as one-action-per-file would produce a graph that misrepresents both their
parallelism and their rebuilds, so `Granularity::{PerSource, WholeTarget}` is a
language property and the planner emits the shape the toolchain really has. ✅

Builtins ship for MAGE, C, C++, Rust, Go, Python, and TypeScript — all `Declared`
except MAGE, because shipping a digest that depends on the operator's machine
would be a lie. Adding a language is data, not code: a name, extensions, a
toolchain, a granularity, and argument templates. ✅ (tested end to end with a
language the engine had never heard of)

What this is **not**: a package manager. A dependency here is an artifact
produced by another target in the same graph. Fetching third-party code is a
separate trust problem, and conflating the two is how build systems become
unauditable. ◻

### 2.2 A command line, and what running it found

The engine was a library with no entry point — drivable from Rust and from its
own tests, and not from a terminal. `ribosome/src/main.rs` fixes that:

```sh
ribosome plan  build.json          # what would be built, without building
ribosome build build.json --out .  # build it
ribosome languages                 # the registry, as JSON
```

Everything answers in JSON, because the premise is that agents drive it and an
answer a program must scrape is one it will eventually scrape wrong. Arguments
are parsed by hand; a build engine that pulls in an argument parser to print JSON
has started down the road that put a registry server in the same crate as a build
system. ✅

A manifest is **data** — no conditionals, functions, includes, or globs. Every
build system that grew a configuration language regrets it, and here it would be
worse than regrettable: a manifest that could compute is a second door for
ambient state, which is what the action key exists to shut. Globs specifically
are refused because they make the source list depend on the filesystem at build
time, so two workers with slightly different checkouts agree on a key and
disagree on the answer — §2.1's failure, reintroduced one convenience above it.
Generate the JSON with whatever you like; then the generation is visible and the
build is still reproducible from it. ✅

A manifest cannot *state* a toolchain digest, because a digest is a property of
this machine's binary and a checked-in one would be a claim about someone else's.
It says `"pin": true` and resolution hashes the executable where it is. ✅

**Running it against a real compiler immediately found three defects that the
test suite could not.** Recorded because they are the argument for building the
entry point, not an embarrassment to be tidied away:

| Found | Was | Why no test caught it |
|---|---|---|
| `rust` builtin | passed **every** source to `rustc`, which errors on more than one input filename — a two-file crate could not build | tests asserted what the planner *emits*, and the emitted args were never handed to a compiler |
| `python` builtin | declared output `{stem}.pyc`, which `-m py_compile` never writes (PEP 3147 puts it in `__pycache__/` under a version-stamped name) | same |
| `cache_hit_ratio` | `1 - work_done/work_total`; a failed action increments neither, so a build that failed **everything** reported **1.0 — perfect reuse** | every test used graphs that succeeded |

The third is the serious one. `Fitness::reuse` is a selection signal for the RSI
loop, and the old form paid a candidate its maximum reuse score for breaking the
build. The correctness gate in `composite()` contained the damage; it should not
have had to. Now tracked as `work_cached` directly, with a regression test. ✅

Verified end to end with real `rustc` 1.97.1: cold build → artifact; rebuild →
cache hit; edit a **non-root** source → rebuilds (proving siblings are keyed even
though they are not arguments); unchanged → cache hit again. And the §2.1 policy
under a real toolchain: unpinned + `--shared` rebuilds every time, unpinned +
local caches on the second run. ✅

The builtins' argument templates are otherwise **unverified against real
toolchains** — `c`, `cpp`, `go`, `typescript`, and the corrected `python` have not
met their tools here. They are marked as such in the source. The engine is what
is tested; a builtin that has not met its compiler is a hypothesis. ◻

---

## 3. Hardware-agnostic

Actions declare a platform *requirement*; executors advertise what they *satisfy*.
Accelerators are strings from the same open registry the compiler already uses
(`prototype/src/backends.rs` — 8 builtins, runtime-extensible), so adding a device
class to the fleet is a registration, not a code change. ✅

The subtle part is cache partitioning. An accelerator participates in the key
**only when the action declares one**:

- A source→ABL lowering is device-independent. One cache entry, shared by every
  worker in a heterogeneous fleet. ✅
- A kernel autotune is not. Its output encodes device-specific choices, so it gets
  its own cache line and can never be served to a request that asked for something
  else. ✅

Without that distinction a mixed CPU/GPU fleet either shares nothing (and caches
nothing useful) or shares everything (and is wrong).

---

## 4. Self-healing

"Self-healing" usually means "retries". Retrying is one of four strategies here,
and the least interesting. A failure carries information about *which* assumption
broke, and each broken assumption has its own repair:

| Failure | Broken assumption | Remedy | Status |
|---|---|---|:--:|
| `Transient` | infrastructure held still | retry, bounded | ✅ |
| `Corrupt` / `Missing` blob | stored bytes stayed those bytes | evict, rebuild | ✅ |
| `NoCapablePlatform` | the fleet has this device | relax the pin *if opted in* | ✅ |
| `Deterministic` | the action was correct | escalate to an agent | ✅ |
| source-level defect | — | agent-authored fix via the compiler's 17 repair patterns | ◻ |

Only the last two need an agent, and that is the point: **a build system operated
by agents should consume agent attention only for failures that are about the
program.** Everything else is infrastructure noise the system absorbs — and
*records*, because a build that quietly heals the same action every run is a
defect wearing a disguise.

**Relaxing a platform pin is the dangerous one.** Falling back from CUDA to CPU is
sound only when both produce identical bytes — true for compilation, false by
definition for an autotune. So fallback is opt-in per action, and taking it
*changes the action and therefore its key*: the CPU result caches under the CPU
key and can never be served to something that asked for CUDA. A build system that
silently satisfied a GPU request from a CPU entry would return wrong answers
quickly, which is worse than being slow. ✅

### Failure does not abort the build

A failed action fails its dependents transitively, with a **named cause**, and the
scheduler carries on with everything unaffected. Abort-on-first-error suits a
human who will fix one thing and re-run; an agent wants the complete problem set
in one round trip so it can plan a single repair. ✅

---

## 5. Agent-operated, one to many

Every surface is data in, data out: `ActionGraph::to_json`, `BuildReport::json`,
and `Scheduler::plan` — which answers *"what would you do?"* without doing it,
the same no-exec discipline as `--describe=abl`. An agent can inspect, predict,
and audit a build without running one. ✅

Many agents drive one build safely because **the unit of coordination is the
action key, not a lock**. Two agents that submit the same action submit the same
key; the second is a cache hit. No negotiation, no leader.

Genuinely conflicting work — two agents rewriting the same target — is a different
problem, and the compiler already has the machinery for it: the semantic lease
manager (`prototype/src/lease.rs`), the 5-phase consensus engine
(`consensus.rs`), and the CRDT merge layer (`crdt.rs`). Ribosome is written to be
*driven by* those rather than to reimplement them. Wiring them together is ◻.

---

## 6. Distribution

✅ **The seam.** `Executor` is `Send + Sync`, takes materialized input bytes, and
returns output bytes. An executor never opens a file it was not handed — which is
both the enforcement half of hermeticity and exactly the interface a remote worker
needs, since inputs must be shipped anyway.

✅ **`PoolExecutor`.** Capability-first routing with round-robin among capable
workers.

✅ **Real network transport** (`remote.rs`). `WorkerServer` serves actions over
TCP; `RemoteExecutor` implements `Executor`, so the scheduler cannot tell a remote
worker from a local one. Tested against live loopback workers, not mocks.

Design notes worth stating:

- **Thread-per-connection, no async runtime.** Build actions are coarse —
  milliseconds to minutes — so a fleet is hundreds of concurrent connections, not
  hundreds of thousands. `std::net` is entirely adequate at that scale and keeps
  this crate free of an async dependency tree. If the fleet outgrows it, the file
  to replace is `remote.rs`; nothing above `Executor` changes.
- **Capabilities are queried, not configured.** A worker advertises its own
  platform. A hand-maintained roster drifts, and a drifted roster routes GPU work
  to a machine without one.
- **Failure classification survives the wire.** A remote `Deterministic` stays
  deterministic and a `Transient` stays transient. Losing that distinction would
  make the healer retry compile errors and give up on network blips.
- **Frame size is capped.** A length prefix read from a socket is
  attacker-controlled; without a cap, `0xFFFFFFFF` is a 4 GB allocation from one
  packet.

✅ **Worker registry** with heartbeat, eviction after repeated failures (not one —
a single missed beat is a hiccup and evicting on it makes the fleet flap),
recovery, and restart-as-recovery rather than duplication.

✅ **Signed provenance** (`provenance.rs`). Action-cache entries are *claims*, and
a shared cache without authenticated claims is a channel by which any participant
hands every other an arbitrary build result. Now every result carries an HMAC over
`(action key, output digests, worker)`.

✅ **Connection authentication.** HMAC challenge-response, gating *every* frame
rather than just `Execute` — a worker that answers `Describe` to an
unauthenticated peer has already disclosed its capabilities and tool set. The
nonce is server-generated per connection, so a captured proof cannot be replayed
against a later one. Opt-in, because a single-host fleet on loopback has nobody
to authenticate against and mandatory ceremony that everyone disables is worse
than an honest opt-in.

✅ **Sandboxed subprocess execution** (`subprocess.rs`) for foreign tools: a fresh
working directory per action, a **cleared environment** (only declared vars plus a
minimal survival set), an executable allowlist so a build graph cannot name an
arbitrary binary, path-traversal rejection, and a wall-clock timeout that becomes
a retryable `Transient` rather than a hang. This is *containment*, not isolation —
no namespaces, cgroups or job objects — and it removes the accidental
non-hermeticity that makes caches wrong, which is the failure that actually
happens.

✅ **Per-worker asymmetric provenance** (Ed25519). The symmetric path remains for
intra-domain use; across a trust boundary a shared secret makes every holder able
to *mint* claims, so one compromised worker forges for the whole fleet with no way
to tell which or to exclude it. Per-worker keys make a compromise attributable,
containable, and **revocable** — `TrustStore::revoke` excludes one worker without
disturbing the rest. Verifiers hold only public keys, so a mirror or auditor can
check provenance without being able to produce it.

✅ **A transport seam, not a socket.** The protocol is length-prefixed JSON over
an ordered byte stream; it was *written* against `TcpStream` concretely, and that
spelling — not cryptography — was what blocked encrypting it. Both ends are now
generic over `Read + Write` with one wrapper point each: `serve_with` and
`connect_over`. Tested end to end with a byte-transforming wrapper, **including
the negative case** where the two ends disagree and the connection fails —
without that, a wrapper that quietly did nothing would pass. The generalization
also let the protocol be driven over a scripted in-memory stream, so
"authentication gates every frame, including `Describe`" is now asserted
deterministically instead of against a live socket with sleeps.

◻ **Remaining, stated rather than implied: still no encryption.** Frames are
plaintext, so anyone on the path reads the source, the artifacts, and the action
graph; this belongs on a trusted segment. What is left is no longer plumbing — a
TLS session plugs into the two wrapper points and nothing else changes. It is a
**deployment decision deliberately not made here**: self-signed certificates
pinned per worker, mutual TLS against an internal CA, and public PKI are three
different operational stories with three different failure modes, and a build
system that quietly picked one would be making a security decision on its
operator's behalf.

*This paragraph previously also listed per-worker asymmetric keys and the
subprocess sandbox as unbuilt. Both landed in steps 145–146 and the line was not
updated — corrected 2026-08-04.*

---

## 7. The RSI loop — mechanism, and an honest assessment

The goal stated for this component is populations of models evolving higher
fitness and overwriting themselves. Here is the mechanism it implies and what is
actually in place.

**What a build system contributes to RSI** is the *evaluation harness*. Self-
improvement needs a loop of propose → build → measure → select → replace, and the
hard, unglamorous parts are the middle three: building a variant reproducibly,
measuring it without contamination from the last variant, and being able to undo
it. That is what this is.

✅ **The measurement.** `BuildReport::fitness()` reduces a build to four
normalized axes — correctness, reuse, parallelism, stability — plus a scalar
`composite()` for when selection needs a total order.

One design decision there is worth stating, because it is exactly the kind of
thing that decides whether such a loop is safe or merely fast:

> **Correctness is a gate, not a weight.** A weighted sum cannot express
> "correctness dominates" — with any fixed weights, a build that fails a third of
> its actions but caches perfectly ties or beats a fully-correct cold build. A
> selection loop run on that signal will evolve toward a fast build system that
> does not work. So the range is split: all-succeeded scores in `[0.5, 1.0]`,
> anything-failed scores strictly below `0.5`, whatever its other axes. ✅ (tested)
>
> **A fitness function with a loophole is a specification of what the population
> will exploit.** This is the general case of reward hacking, and it is not a
> hypothetical concern in a system designed to optimize against its own metric.

◻ **Not built:** population management, selection, mutation, crossover, and the
self-overwrite step. The compiler already has the genetic operators
(`evolve_gen.rs`: selection/crossover/mutation strategies behind the `evolve`
keyword) and an operation-log VCS with semantic branching and rollback
(`semantic_vcs.rs`) which is the natural substrate for "overwrite yourself, and be
able to not have." Composing them into a closed loop is designed, unwritten.

### The honest part

Recursive self-improvement via evolutionary search over build and toolchain
configurations is a **research bet, not an engineering certainty**. What is
defensible today: this makes variants reproducibly buildable, comparably
measurable, and cleanly revertible. Those are prerequisites, and most of the
difficulty in such loops turns out to live there rather than in the search.

What is *not* established, and should not be asserted in a README: that iterating
this loop yields compounding capability gains. Evolutionary search over a
well-specified fitness landscape reliably produces local optimization; whether
that composes into open-ended self-improvement is precisely the open question, and
a measured system should say so rather than assume it.

Three control points are worth building *before* the loop closes, not after:

1. **The fitness gate** above — the loop optimizes exactly what it measures.
2. **Provenance.** Every artifact traceable to the action, worker, and inputs that
   produced it (`certs.rs` is the existing substrate). A self-modifying system
   without an audit trail cannot be debugged, only restarted.
3. **Reversibility.** Self-overwrite must be a commit in a log that can be walked
   backwards. `semantic_vcs.rs` already models this.

---

## 8. Reproducing the claims

```powershell
cargo test --manifest-path ribosome/Cargo.toml                  # 162 tests
cargo test --manifest-path ribosome/Cargo.toml --test build     # 10 end-to-end scenarios
cargo test --manifest-path ribosome/Cargo.toml --test multilang # 8 multi-language scenarios
```

The end-to-end tests are the interesting ones, because each asserts a property a
build system is actually judged on:

| Test | Property |
|---|---|
| `a_mixed_language_program_builds_in_dependency_order` | C, Rust, and MAGE in one graph |
| `a_shared_store_refuses_to_publish_results_from_an_unverified_toolchain` | hermeticity is enforced, not labelled |
| `pinning_the_toolchain_earns_the_shared_cache` | and the upgrade path works |
| `one_unpinned_language_does_not_block_the_pinned_ones_beside_it` | enforcement is per action |
| `a_language_nobody_anticipated_builds_with_no_change_to_the_engine` | a language is data |
| `a_cold_build_runs_everything_and_produces_the_right_bytes` | it builds, and the bytes are right |
| `rebuilding_unchanged_sources_does_no_work_at_all` | a no-op rebuild runs **zero** tools |
| `editing_one_source_rebuilds_exactly_it_and_its_dependents` | minimal correct incrementality |
| `a_failing_action_skips_only_its_dependents` | failure is contained and named |
| `a_transient_failure_heals_and_the_build_succeeds` | healing works and is recorded |
| `cache_corruption_is_detected_and_repaired` | rot is caught on read, not propagated |
| `gpu_work_routes_to_a_gpu_worker_and_cpu_work_runs_anywhere` | heterogeneous routing |
| `a_missing_accelerator_falls_back_only_when_opted_in` | no silent wrong-device results |
| `plan_predicts_the_build_without_running_it` | no-exec introspection |
| `fitness_is_ordered_by_correctness_first` | the selection signal has no loophole |

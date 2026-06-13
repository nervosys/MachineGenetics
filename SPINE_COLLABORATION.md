# MAGE agent collaboration over SPINE

**Goal.** Let MAGE ABL agents *collaborate* — discover each other, exchange
artifacts, run each other's models, and reach swarm consensus — over **SPINE**
(`nervosys/SPINE`, the agentic-first wire protocol that tops agentic-eval's
web-stack benchmark at 0.90).

**Why SPINE and not RAP.** MAGE's own RAP control plane scores low for
collaboration (loopback-only, unauthenticated, and it ships ABL artifacts as
**hex inside JSON-RPC**, so the binary compaction evaporates on the wire). SPINE
fixes exactly those gaps:

| Need | RAP today | SPINE |
|---|---|---|
| Wire transport of an ABL artifact | `abl_hex` (hex-in-JSON, ~2× inflation) | **`SpineBinary` / `EncodedFrame`** — raw CBOR byte string, content-typed |
| Agent-to-agent messaging | none (single client→server) | `AgentMessage` / `MessageContent`, multiplexed |
| Discovery | `ontology/full` dump (local) | `CapabilityQuery` → `CapabilityAdvertisement` (networked, semantic) |
| Multi-agent coordination | none | `Swarm` / `SwarmCoordinator` |
| Per-message security | none (loopback gate only) | **Ed25519 `signed_frame`** + W3C TraceContext |

ABL's **no-exec, content-hashed, self-describing** artifact is the *payload*;
SPINE is the *transport + coordination plane*. They compose cleanly.

---

## 1. Identity: an ABL agent → a SPINE `AgentProfile`

The bridge maps an ABL `agent` spec (built by `mage-parse --build=abl`) to a
`spine_agentic::AgentProfile`:

```rust
// abl agent spec: {"agent":"Builder","capabilities":["read_source","run_tests"],
//                  "requires_approval":["write_files"]}
fn abl_agent_to_profile(spec: &AblAgentSpec, pubkey: Vec<u8>) -> AgentProfile {
    AgentProfile::new(&spec.agent)
        .with_capabilities(map_caps(&spec.capabilities))   // → Vec<AgentCapability>
        .with_trust(TrustLevel::Verified)
        .with_ontology(abl_ontology())                     // from --emit-ontology
        // public_key carries the Ed25519 verifier for signed_frame
        ;
}

// MAGE capability identifiers → SPINE AgentCapability
fn map_caps(ids: &[String]) -> Vec<AgentCapability> {
    let mut v = vec![AgentCapability::AgentCommunication, AgentCapability::SwarmParticipation];
    for id in ids {
        v.push(match id.as_str() {
            "run_tests" | "execute" | "run"            => AgentCapability::CodeExecution,
            "read_source" | "describe"                 => AgentCapability::ContentExtraction,
            "query_kb" | "store_kb"                    => AgentCapability::KnowledgeManagement,
            other                                       => AgentCapability::Custom(other.into()),
        });
    }
    v
}
```

`requires_approval` is **not** dropped — it becomes the agent's approval policy,
enforced before any incoming `ActionRequest` is honored (see §4), reusing the
exact `abl_bridge::eval_agent_policy` evaluator already shipped.

Each profile is registered with the runtime:
`AgenticWebRuntime::register_agent(profile)`.

---

## 2. Transport: ABL artifacts as SPINE binary frames

An ABL container (`magic ABL1`, byte-stable, content-hashable) travels as a
`SpineBinary` — **not** hex, not re-JSON'd:

```rust
// produce the artifact with the tested pipeline, then hand the raw bytes to SPINE
let bytes: Vec<u8> = build_abl(&spec)?;            // mage-parse --build=abl (in-proc)
let bin = SpineBinary::from_bytes(bytes);          // rides as a CBOR byte string
client.execute_binary(bin).await?;                 // spine-agent AgentClient
```

The receiver loads it as **pure data** (`abl::decode_container` /
`decode_symbols`) — bounds-checked, no code execution — then introspects
(`--describe=abl`) or runs (`--run=abl`: kb fixpoint / agent policy / swarm
consensus). The content hash doubles as the cache key and the signed-frame digest.

---

## 3. Messages: the collaboration verbs

MAGE collaboration maps onto `spine_agentic::MessageContent` 1:1 — no new
protocol needed:

| Collaboration intent | `MessageContent` | Payload |
|---|---|---|
| "here is a model/kb I built" | `KnowledgeShare { topic, knowledge }` or `ActionRequest` | ABL bytes (binary frame) + content hash |
| "run this artifact for me" | `ActionRequest(Action)` | artifact ref + `--run=abl` inputs (`ops` / `proposals`) |
| "result of the run" | `ActionComplete { success, result }` / `Response { data, confidence }` | derived facts / decisions / forward-pass output |
| "find me an agent that can X" | `Query(SemanticQuery)` | capability + latent embedding |
| "join this swarm" | `SwarmInvite(SwarmTask)` | from an ABL `swarm` spec |
| accept/decline | `SwarmResponse { accepted, reason }` | capability match result |

Sending is one call: `runtime.send_message(to, content).await?`.

---

## 4. Capability gating across the boundary

When agent B receives `ActionRequest(action)` from agent A, B applies its **own**
ABL policy before acting — the same evaluator `--run=abl` uses:

```rust
let decisions = abl_bridge::eval_agent_policy(&b.capabilities, &b.requires_approval, &[action.op()]);
match decisions[0].1 {
    OpDecision::Allowed          => b.execute(action).await,          // run it
    OpDecision::RequiresApproval => b.request_human_approval(action), // gate
    OpDecision::Denied           => b.reply(MessageContent::Error{..}),
}
```

So capability gating is **end-to-end**: declared in the ABL spec, carried in the
profile, and enforced at the receiver — not just advisory metadata.

---

## 5. Swarms: an ABL `swarm` spec → a SPINE `Swarm`

An ABL `swarm` spec drives `SwarmCoordinator`:

```rust
// abl: {"swarm":"Reviewers","agent":"Reviewer","size":5,"topology":"ring","consensus":"quorum"}
let task = SwarmTask {
    description: spec.swarm.clone(),
    required_capabilities: map_caps(&[spec.agent.clone()]),
    min_members: spec.size, max_members: spec.size, ..
};
let swarm_id = coordinator.create_swarm(task, leader).await;
for (i, agent) in members.enumerate() {
    coordinator.agent_joined(swarm_id, agent, role_for(i, &spec.topology)); // Worker/Validator/…
}
```

- **topology** → the member graph and `rounds_to_converge` (mesh/star/broadcast=1,
  ring=n−1, tree=⌈log₂n⌉) — the propagation model already in `abl_bridge`.
- **consensus** → SPINE's `Swarm.consensus_threshold` *and* the decision rule:
  members emit proposals as `Response`/`ActionComplete`, the coordinator runs
  `abl_bridge::eval_swarm_consensus(size, topology, consensus, &proposals)` to
  decide (majority/quorum/unanimous, deterministic tiebreak). One evaluator,
  used both for `--run=abl` locally and for live swarm decisions over SPINE.

---

## 6. Security: defense in depth

Collaboration crosses a trust boundary, so three independent guarantees stack:

1. **Transport** — SPINE bearer auth + TLS (secure-by-default since SPINE v1.3).
2. **Per-message** — `spine_agentic::signed_frame` wraps each ABL frame in an
   Ed25519 signature over the exact wire bytes (the artifact's content hash),
   verified **before** decode → integrity + authenticity + non-repudiation. The
   profile's `public_key` is the verifier. (This is the message-level guarantee
   RAP entirely lacks.)
3. **Payload** — the ABL artifact is **no-exec**: loading is bounds-checked pure
   data decode, and capability gating (§4) refuses out-of-policy actions before
   execution. A malicious frame cannot run code; an out-of-policy request is denied.

---

## 7. Worked scenario: a build→review swarm

1. **Builder** agent runs `--build=abl` on a `{"net":..}` spec → 200 B artifact;
   sends `KnowledgeShare` to a `Reviewers` swarm, ABL bytes as a signed binary frame.
2. Each **Reviewer** verifies the signature, `--describe=abl` (no-exec) to check
   structure, optionally `--run=abl` (forward pass / shape check), and emits a
   proposal (`1`=accept / `0`=reject) via `Response`.
3. **Coordinator** runs `eval_swarm_consensus(5, "ring", "quorum", &proposals)`;
   on a quorum it broadcasts `ActionComplete{success:true}` and the artifact is
   admitted (content hash recorded). No artifact ever executed code to get reviewed.

The whole exchange is binary on the wire, signed per message, capability-gated at
each receiver, and decided by the same consensus evaluator MAGE already ships.

---

## 8. Where the code lives (the MAGE side is BUILT)

- **MAGE side — implemented** in `prototype/src/spine_bridge.rs` + CLI:
  - `agent_profile(&AgentSpec)` → SPINE `AgentProfile`-shaped JSON
    (`mage-parse --spine=profile agent.json`)
  - `swarm_task(&SwarmSpec)` / `swarm_decide(..)` → `SwarmTask` JSON + the
    coordinator decision (`--spine=swarm swarm.json`), reusing `eval_swarm_consensus`
  - `artifact_frame(&[u8])` → the binary collaboration frame: byte length,
    FNV-1a `content_digest` (what `signed_frame` covers), `exec:false`
    (`--spine=frame model.abl`)
  - `gate(&AgentSpec, op)` → receiver-side capability gate, reusing
    `eval_agent_policy` (the same evaluator `--run=abl` uses)
  - `map_capability(id)` → SPINE `AgentCapability` (unit variant or `Custom`)

  Unit-tested (5 tests): capability map, profile shape + baseline caps, gate
  decisions, swarm task/decision, deterministic no-exec frame digest.

- **Type-link boundary — now BUILT** as the `spine-mage` crate *inside* the
  SPINE workspace (`nervosys/SPINE`, `src/spine-mage`). It deserializes the
  JSON above into the real `spine_agentic` types, so the join is **compile-checked**:
  - `AblAgentEnvelope::into_profile()` → `AgentProfile` (capabilities
    deserialize straight into `Vec<AgentCapability>` — the proof the vocabularies
    match)
  - `AblSwarmEnvelope::into_task()` → `SwarmTask`
  - `AblArtifactFrame::into_artifact()` → `swe::SweArtifact`, after **FNV-digest
    cross-validation** of the decoded bytes
  5 tests parse MAGE's exact `--spine=*` output.

- **SPINE side** — consumed as-is: `spine-agentic` (`AgentProfile`, `AgentMessage`,
  `Swarm`, `SwarmCoordinator`, `AgenticWebRuntime`), `spine-agent` (`AgentClient`,
  `execute_binary`), `spine-crypto` (`signed_frame`). No SPINE changes required.

---

## 9. Does SPINE need new code for collaborative agentic SWE?

**Assessment (evidence-grounded, against the Hyperlight tree): essentially no new
protocol or substrate — SPINE already has what collaborative agentic SWE needs.**
The remaining items are narrow wiring/conventions built on existing primitives.

### Already present (no work needed)

| Collaboration need | SPINE has it |
|---|---|
| Agent identity + discovery | `AgentProfile`, **capability marketplace** — registry, discovery, **bidding, contracts, reputation, audit log** (ROADMAP §106) |
| Messaging (extensible verbs) | `AgentMessage` / `MessageContent` (Query/Response/ActionRequest/ActionComplete/KnowledgeShare/SwarmInvite/Custom) |
| Swarm coordination | `SwarmCoordinator` (candidate selection + graphical-model choice), `SwarmRole`, `SwarmTask` |
| **Consensus** | `spine-agentic/consensus.rs` — weighted `vote`/`tally`/`finalize`/`check_quorum` (tested); cluster-level **Raft** (`spine-cluster/raft.rs`: leader election, log replication, snapshots) |
| **Shared knowledge** | `spine-knowledge` CRDTs (`VectorClock`, `LwwRegister`, `GSet`), episodic/semantic/collective memory, **content-hashed** entries |
| Binary artifact transport | `SpineBinary` / `EncodedFrame` (raw CBOR bytes, content-typed) |
| Sandboxed execution | `AgentClient::execute_binary` → `spine-compiler` + `spine-wasm` (`WasmExecutionResult`) |
| Per-message security | `spine-crypto` `signed_frame` (Ed25519), W3C TraceContext, bearer+TLS |
| Planning | `Plan`/`PlanStep`/`Action`/`Goal`/`Intention`, `ExecutionEngine::execute_plan` (parallel steps) |

SWE-specific message *content* (code review, test results, build artifacts, diffs)
needs **no new wire types** — it rides the existing extensible `MessageContent`
(`KnowledgeShare` / `ActionRequest` / `ActionComplete` / `Custom`).

### Genuine gaps (small, and built on existing primitives)

1. **Wire `ExecutionEngine::execute_action` to real execution (the one true code
   gap).** `Action::Execute { command, args }` currently returns a *stub*
   `{"status":"executed"}` (`spine-agentic/src/lib.rs`) — it runs nothing. To let
   an agent actually *do* SWE work, route it to the sandboxed path SPINE already
   has (`execute_binary` → `spine-wasm`). This is **wiring existing capability**,
   not new infrastructure. Small, real, needed.

2. **A first-class SWE-artifact convention (additive, optional).** There is no
   typed "versioned build/code artifact with provenance + lineage." The CRDT
   knowledge layer (content-hash + `GSet`/`LwwRegister` + episodic memory) can
   model it, but a thin `SweArtifact { content_hash, producer: AgentId,
   supersedes: Option<hash>, signature }` would make build→review→version flows
   first-class. A convention on top of existing layers — not a protocol change.

3. **A dependency-aware work-DAG (enhancement, mostly covered).** Decompose →
   assign is largely handled by `Plan` + `SwarmTask` + the capability marketplace
   (bidding/contracts). A shared subtask graph with claim/complete/blocked-on
   would strengthen large multi-agent SWE decomposition, but no missing primitive
   blocks it today.

4. **The ABL↔SPINE type-safe join — MAGE/glue side, not SPINE.** The
   ~dozen-line crate inside the Hyperlight workspace (see §8) that
   `serde_json::from_value`s the bridge envelopes into the real `spine_agentic`
   types. Glue, already scoped.

### Bottom line

Collaborative agentic SWE is **achievable on SPINE today** with the existing
substrate plus the MAGE bridge (§1–8). The only must-do SPINE change is one
wiring fix (#1); #2–#3 are additive ergonomics, not blockers. SPINE was built as
a general agent-native collaboration plane, and SWE is just one workload over it.

### Update — all four items now implemented (in `nervosys/SPINE`)

1. ✅ **Real execution** — `Action::Execute` now compiles + runs the command in
   the fuel-metered `spine-wasm` sandbox (default-on `wasm-exec` feature); honest
   error when disabled, never a fake "executed". (`spine-agentic`)
2. ✅ **SWE artifacts** — `spine-agentic::swe::SweArtifact` (content-addressed,
   `supersedes` lineage, producer, Ed25519 signature) + `SweArtifactStore`.
3. ✅ **Work DAG** — `spine-agentic::swe::WorkGraph` (deps, capabilities,
   Ready/Claimed/Done, claim/complete unblocking, topological-order/cycle check).
4. ✅ **Type-safe join** — the `spine-mage` crate (§8).

spine-agentic 285 tests, spine-mage 5 tests, dependents build clean.

The bridge is intentionally small because the two systems already meet in the
middle: ABL supplies a self-describing, no-exec, content-hashed **artifact**, and
SPINE supplies the agent-native **transport, discovery, messaging, swarm, and
signing**. MAGE's `eval_agent_policy` / `eval_swarm_consensus` are the shared
decision logic used identically for local `--run=abl` and networked collaboration.

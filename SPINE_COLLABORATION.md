# MechGen agent collaboration over SPINE

**Goal.** Let MechGen ABL agents *collaborate* — discover each other, exchange
artifacts, run each other's models, and reach swarm consensus — over **SPINE**
(`nervosys/SPINE`, the agentic-first wire protocol that tops agentic-eval's
web-stack benchmark at 0.90).

**Why SPINE and not RAP.** MechGen's own RAP control plane scores low for
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

The bridge maps an ABL `agent` spec (built by `MechGen-parse --build=abl`) to a
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

// MechGen capability identifiers → SPINE AgentCapability
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
let bytes: Vec<u8> = build_abl(&spec)?;            // MechGen-parse --build=abl (in-proc)
let bin = SpineBinary::from_bytes(bytes);          // rides as a CBOR byte string
client.execute_binary(bin).await?;                 // spine-agent AgentClient
```

The receiver loads it as **pure data** (`abl::decode_container` /
`decode_symbols`) — bounds-checked, no code execution — then introspects
(`--describe=abl`) or runs (`--run=abl`: kb fixpoint / agent policy / swarm
consensus). The content hash doubles as the cache key and the signed-frame digest.

---

## 3. Messages: the collaboration verbs

MechGen collaboration maps onto `spine_agentic::MessageContent` 1:1 — no new
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
each receiver, and decided by the same consensus evaluator MechGen already ships.

---

## 8. Where the code lives (the MechGen side is BUILT)

- **MechGen side — implemented** in `prototype/src/spine_bridge.rs` + CLI:
  - `agent_profile(&AgentSpec)` → SPINE `AgentProfile`-shaped JSON
    (`MechGen-parse --spine=profile agent.json`)
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

- **Type-link boundary (honest):** the JSON above targets the `spine_agentic`
  serde shapes by field name; it is **not** compile-checked against the crate,
  because `spine-agentic` inherits Hyperlight-workspace deps and cannot be a
  plain path-dependency from here. To make the join type-safe, add a tiny crate
  *inside* the Hyperlight workspace that depends on both `spine-agentic` and this
  bridge's JSON contract and `serde_json::from_value`s the envelopes into the
  real types — a few dozen lines, no changes to either side.

- **SPINE side** — consumed as-is: `spine-agentic` (`AgentProfile`, `AgentMessage`,
  `Swarm`, `SwarmCoordinator`, `AgenticWebRuntime`), `spine-agent` (`AgentClient`,
  `execute_binary`), `spine-crypto` (`signed_frame`). No SPINE changes required.

The bridge is intentionally small because the two systems already meet in the
middle: ABL supplies a self-describing, no-exec, content-hashed **artifact**, and
SPINE supplies the agent-native **transport, discovery, messaging, swarm, and
signing**. MechGen's `eval_agent_policy` / `eval_swarm_consensus` are the shared
decision logic used identically for local `--run=abl` and networked collaboration.

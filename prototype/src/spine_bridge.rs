//! SPINE collaboration bridge — MechGen ABL agents → SPINE messages.
//!
//! The MechGen side of the design in `SPINE_COLLABORATION.md`. It reuses the
//! verified [`crate::abl_bridge`] decision logic (capability gating + swarm
//! consensus) and emits **SPINE-protocol-shaped JSON** whose field names target
//! the `spine_agentic` serde types (`AgentProfile`, `SwarmTask`,
//! `MessageContent`).
//!
//! Why JSON and not a direct type link: `spine-agentic` is a member of the
//! Hyperlight workspace and inherits its deps (`serde.workspace = true`, plus
//! `spine-neural`/`spine-crypto`/`spine-protocol`/…), so it cannot be a plain
//! path-dependency from this crate. The type-level join therefore lives at the
//! integration boundary (link `spine-agentic` from inside that workspace);
//! everything in THIS module — the capability map, the gate, the consensus
//! decision, and the content digest a `signed_frame` covers — is MechGen-side,
//! dependency-free, and unit-tested.

use crate::abl_bridge::{eval_agent_policy, eval_swarm_consensus, OpDecision};
use crate::builder::{AgentSpec, SwarmSpec};

/// FNV-1a 64 over the artifact bytes — the content digest a SPINE `signed_frame`
/// signs and SPINE uses as a cache key. Deterministic, dependency-free.
pub fn content_digest(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Map a MechGen capability identifier to a SPINE `AgentCapability` JSON value
/// (a unit-variant string, or `{"Custom": "<id>"}` for project-specific ones).
pub fn map_capability(id: &str) -> serde_json::Value {
    match id {
        "run_tests" | "execute" | "run" => serde_json::json!("CodeExecution"),
        "read_source" | "describe" => serde_json::json!("ContentExtraction"),
        "query_kb" | "store_kb" => serde_json::json!("KnowledgeManagement"),
        other => serde_json::json!({ "Custom": other }),
    }
}

/// Build a SPINE `AgentProfile`-shaped JSON from an ABL agent spec. Always
/// advertises AgentCommunication + SwarmParticipation (the collaboration
/// baseline); `requires_approval` is preserved as policy metadata so the
/// receiver can gate (see [`gate`]).
pub fn agent_profile(spec: &AgentSpec) -> serde_json::Value {
    let mut caps = vec![
        serde_json::json!("AgentCommunication"),
        serde_json::json!("SwarmParticipation"),
    ];
    caps.extend(spec.capabilities.iter().map(|c| map_capability(c)));
    serde_json::json!({
        "name": spec.agent,
        "capabilities": caps,
        "trust_level": "Verified",
        "miras_variant": "Titans",
        "requires_approval": spec.requires_approval,
    })
}

/// Receiver-side capability gate for an incoming `ActionRequest`. Reuses the
/// exact evaluator `--run=abl` uses, so a request is honored / approval-gated /
/// refused identically whether local or networked.
pub fn gate(spec: &AgentSpec, op: &str) -> OpDecision {
    let ops = [op.to_string()];
    eval_agent_policy(&spec.capabilities, &spec.requires_approval, &ops)[0].1
}

/// A SPINE `SwarmTask`-shaped JSON from an ABL swarm spec.
pub fn swarm_task(spec: &SwarmSpec) -> serde_json::Value {
    let size = spec.size.unwrap_or(1).max(1) as usize;
    serde_json::json!({
        "description": spec.swarm,
        "required_capabilities": [ map_capability(&spec.agent) ],
        "min_members": size,
        "max_members": size,
        // MechGen coordination metadata that drives eval_swarm_consensus:
        "topology": spec.topology,
        "consensus": spec.consensus,
    })
}

/// Coordinator decision over member proposals (reuses [`eval_swarm_consensus`]).
pub fn swarm_decide(spec: &SwarmSpec, proposals: &[i64]) -> serde_json::Value {
    let topo = spec.topology.clone().unwrap_or_else(|| "mesh".into());
    let cons = spec.consensus.clone().unwrap_or_else(|| "majority".into());
    let r = eval_swarm_consensus(spec.size.unwrap_or(1), &topo, &cons, proposals);
    serde_json::json!({
        "size": r.size,
        "topology": r.topology,
        "consensus": r.consensus,
        "rounds_to_converge": r.rounds_to_converge,
        "decided": r.decided,
        "reason": r.reason,
    })
}

/// A binary collaboration frame. The raw ABL bytes ride as a SPINE `SpineBinary`
/// (a CBOR byte string on the wire — NOT hex, unlike RAP); this envelope adds
/// the content digest a `signed_frame` covers. `payload_hex` is an inspection
/// view only — the actual wire carries the raw bytes.
pub fn artifact_frame(abl_bytes: &[u8]) -> serde_json::Value {
    serde_json::json!({
        "kind": "abl-artifact",
        "byte_len": abl_bytes.len(),
        "content_digest": format!("{:016x}", content_digest(abl_bytes)),
        "exec": false,                                // ABL load never executes code
        "signed": false,                              // set true by spine_crypto::signed_frame
        "payload_hex": crate::abl::to_hex(abl_bytes), // inspection view; wire = raw bytes
    })
}

/// A `MessageContent::ActionRequest`-shaped message: "run this artifact for me".
pub fn action_request(artifact_digest: &str, op: &str) -> serde_json::Value {
    serde_json::json!({ "ActionRequest": { "artifact": artifact_digest, "op": op } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{AgentSpec, SwarmSpec};

    fn agent(json: &str) -> AgentSpec {
        serde_json::from_str(json).unwrap()
    }
    fn swarm(json: &str) -> SwarmSpec {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn capabilities_map_to_spine_variants() {
        assert_eq!(map_capability("run_tests"), serde_json::json!("CodeExecution"));
        assert_eq!(map_capability("read_source"), serde_json::json!("ContentExtraction"));
        assert_eq!(map_capability("deploy"), serde_json::json!({ "Custom": "deploy" }));
    }

    #[test]
    fn agent_profile_has_spine_shape_and_baseline_caps() {
        let p = agent_profile(&agent(
            r#"{"agent":"Builder","capabilities":["run_tests"],"requires_approval":["write_files"]}"#,
        ));
        assert_eq!(p["name"], "Builder");
        assert_eq!(p["trust_level"], "Verified");
        let caps = p["capabilities"].as_array().unwrap();
        assert!(caps.contains(&serde_json::json!("AgentCommunication")));
        assert!(caps.contains(&serde_json::json!("SwarmParticipation")));
        assert!(caps.contains(&serde_json::json!("CodeExecution")));
        assert_eq!(p["requires_approval"][0], "write_files");
    }

    #[test]
    fn gate_matches_capability_policy() {
        let a = agent(r#"{"agent":"B","capabilities":["read_source","write_files"],"requires_approval":["write_files"]}"#);
        assert_eq!(gate(&a, "read_source"), OpDecision::Allowed);
        assert_eq!(gate(&a, "write_files"), OpDecision::RequiresApproval);
        assert_eq!(gate(&a, "deploy"), OpDecision::Denied);
    }

    #[test]
    fn swarm_task_and_decision_use_shared_consensus() {
        let s = swarm(r#"{"swarm":"Reviewers","agent":"Reviewer","size":5,"topology":"ring","consensus":"quorum"}"#);
        let t = swarm_task(&s);
        assert_eq!(t["description"], "Reviewers");
        assert_eq!(t["min_members"], 5);
        let d = swarm_decide(&s, &[1, 1, 1, 0, 1]);
        assert_eq!(d["decided"], 1, "4/5 quorum");
        assert_eq!(d["rounds_to_converge"], 4, "ring diameter = n-1");
    }

    #[test]
    fn artifact_frame_is_no_exec_and_digest_is_deterministic() {
        let bytes = b"ABL1\x02\x00demo";
        let f = artifact_frame(bytes);
        assert_eq!(f["exec"], false);
        assert_eq!(f["byte_len"], bytes.len());
        assert_eq!(content_digest(bytes), content_digest(bytes), "digest deterministic");
        assert_eq!(f["content_digest"], format!("{:016x}", content_digest(bytes)));
    }
}

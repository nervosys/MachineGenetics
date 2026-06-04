//! # MechGen ↔ RMI Runtime Adapter
//!
//! Bridges MechGen's [`crate::agent_runtime::AgentRuntime`] with RMI's
//! [`rmi::core::collaboration::AgentRuntime`]. The two runtimes have
//! overlapping but distinct strengths:
//!
//! | Capability                  | MechGen | RMI |
//! |-----------------------------|:-------:|:---:|
//! | Semantic leases             |    ✓    |     |
//! | CRDT merge                  |    ✓    |     |
//! | Consensus (5-phase)         |    ✓    |     |
//! | Semantic VCS                |    ✓    |     |
//! | NL → code synthesis         |    ✓    |     |
//! | SharedWorkspace blackboard  |         |  ✓  |
//! | Task delegation             |         |  ✓  |
//! | Federated learning          |         |  ✓  |
//! | Distributed Raft/BFT        |         |  ✓  |
//! | Model registry              |         |  ✓  |
//!
//! The adapter exposes RMI's capabilities through a MechGen-shaped façade so
//! that swarm code targeting MechGen's surface can opt into RMI's distributed
//! and ML-ops machinery without leaving the MechGen module graph.
//!
//! ## Topology
//!
//! ```text
//!    ┌───────────────────────────────────────────────┐
//!    │            MechGen agent_runtime              │
//!    │  (leases, CRDT, consensus, VCS, NL-codegen)   │
//!    └───────────────┬───────────────────────────────┘
//!                    │ delegates via RmiAdapter
//!                    ▼
//!    ┌───────────────────────────────────────────────┐
//!    │   rmi::core::collaboration::AgentRuntime      │
//!    │ (SharedWorkspace, TaskDelegator, registries)  │
//!    │                +                              │
//!    │   rmi::distributed (transport, Raft, BFT)     │
//!    └───────────────────────────────────────────────┘
//! ```
//!
//! Both runtimes share the [`rmi::core::agent::Agent`] identity type, so an
//! agent registered in MechGen is addressable in RMI's workspace under the
//! same id.

use std::sync::Arc;

use rmi::core::collaboration::{
    AgentRuntime as RmiRuntime, RuntimeConfig as RmiRuntimeConfig, SharedWorkspace,
    TaskDelegator,
};
use uuid::Uuid;

/// Adapter that owns an RMI runtime and exposes a MechGen-shaped interface.
pub struct RmiAdapter {
    runtime: RmiRuntime,
}

impl Default for RmiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RmiAdapter {
    /// Construct an adapter with RMI's default runtime configuration.
    pub fn new() -> Self {
        Self {
            runtime: RmiRuntime::new(RmiRuntimeConfig::default()),
        }
    }

    /// Borrow the underlying RMI runtime for advanced use.
    pub fn runtime(&self) -> &RmiRuntime {
        &self.runtime
    }

    /// Mutably borrow the underlying RMI runtime.
    pub fn runtime_mut(&mut self) -> &mut RmiRuntime {
        &mut self.runtime
    }

    /// Get the shared blackboard workspace handle.
    pub fn workspace(&self) -> Arc<SharedWorkspace> {
        self.runtime.workspace()
    }

    /// Get the task delegator (capability-based routing) handle.
    pub fn delegator(&self) -> Arc<TaskDelegator> {
        self.runtime.delegator()
    }

    /// Convenience: post a string note to the shared workspace under `tag`.
    /// Returns the new version number assigned by the workspace.
    pub fn post_note(&self, tag: &str, content: &str, author: Uuid) -> u64 {
        self.workspace().put(tag, content.as_bytes().to_vec(), author)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_constructs_and_exposes_workspace() {
        let adapter = RmiAdapter::new();
        let author = Uuid::new_v4();
        let v = adapter.post_note("hello", "from MechGen", author);
        assert!(v >= 1);
        // Smoke-test: workspace + delegator handles are live.
        let _ = adapter.workspace();
        let _ = adapter.delegator();
    }
}

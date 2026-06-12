// ── Example 2: Capability-Sandboxed Agent ───────────────────────────
//
// Demonstrates running an untrusted agent within a capability sandbox:
// 1. Create a sandbox with strict resource limits
// 2. Grant minimal capabilities (read-only filesystem)
// 3. Execute agent code within the sandbox
// 4. Audit all actions the agent took

use mechgen.sandbox.{SandboxManager, CapabilityToken, ResourceLimits};

// ── Configuration ──────────────────────────────────────────────────

// Define strict resource limits for the untrusted agent.
//
// val limits = ResourceLimits {
//     max_memory: 512 * 1024,      // 512 KB — very constrained
//     max_cpu_ms: 2000,             // 2 seconds
//     max_syscalls: 50,
//     max_file_ops: 5,              // can read at most 5 files
//     max_network_ops: 0,           // NO network access
// };

// ── Sandbox Setup ──────────────────────────────────────────────────

// f setup_sandbox(agent_id: &s) -> SandboxManager
//     @fx mem
// {
//     val mgr = SandboxManager.new();
//
//     // Create the sandbox
//     mgr.create(agent_id, limits);
//
//     // Grant minimal capabilities
//     mgr.grant(agent_id, CapabilityToken.read_only("config"));
//     mgr.grant(agent_id, CapabilityToken.restricted("fs.read"));
//
//     // Verify: agent can read but not write
//     assert!(mgr.check_access(agent_id, "fs.read"));
//     assert!(!mgr.check_access(agent_id, "fs.write"));
//     assert!(!mgr.check_access(agent_id, "net"));
//
//     mgr
// }

// ── Agent Task ─────────────────────────────────────────────────────

// The untrusted agent: reads configuration, processes data,
// returns a result. All within its sandbox.
//
// f agent_task(mgr: &SandboxManager, agent_id: &s) -> s!SandboxError
//     @fx io, fs
// {
//     // Each file operation consumes from the sandbox budget
//     mgr.consume_resource(agent_id, "file_ops", 1)?;
//     val config = read_config("project.toml")?;
//
//     mgr.consume_resource(agent_id, "file_ops", 1)?;
//     val data = read_data("input.json")?;
//
//     // Process data (pure computation, no resource consumption)
//     val result = process(config, data);
//
//     Ok(result)
// }

// ── Execution and Audit ────────────────────────────────────────────

// +f run_sandboxed(agent_id: &s) -> s!SandboxError
//     @fx io, fs, mem
// {
//     val mgr = setup_sandbox(agent_id);
//
//     // Execute the agent
//     val result = agent_task(&mgr, agent_id)?;
//
//     // Review the audit log
//     val log = mgr.audit_log(agent_id);
//     p"Agent {agent_id} actions:";
//     @ entry in log.entries() {
//         p"  [{entry.timestamp}] {entry.kind}: {entry.details}";
//     }
//
//     // Destroy sandbox and release all resources
//     mgr.destroy(agent_id);
//
//     Ok(result)
// }

// ── Capability Attenuation Example ─────────────────────────────────

// Demonstrates how capabilities can only be narrowed, never widened.
//
// f demonstrate_attenuation() {
//     // Start with full filesystem capability
//     val full_fs = CapabilityToken.full("fs");
//     assert!(full_fs.allows("fs.read"));
//     assert!(full_fs.allows("fs.write"));
//
//     // Attenuate to read-only
//     val read_only = full_fs.attenuate("fs.read");
//     assert!(read_only.allows("fs.read"));
//     assert!(!read_only.allows("fs.write"));
//
//     // Cannot widen back
//     // val widened = read_only.attenuate("fs");  // ERROR at compile time
// }

// ── Multi-Agent Delegation ─────────────────────────────────────────

// A trusted agent can create sub-agents with attenuated capabilities.
//
// f delegate_to_sub_agent(mgr: &SandboxManager, parent_id: &s, child_id: &s)
//     @req mgr.check_access(parent_id, "fs.read")
//     @fx mem
// {
//     // Parent has fs.read — child gets a subset
//     val child_limits = ResourceLimits {
//         max_memory: 256 * 1024,   // half of parent
//         max_cpu_ms: 1000,
//         max_syscalls: 20,
//         max_file_ops: 2,
//         max_network_ops: 0,
//     };
//
//     mgr.create(child_id, child_limits);
//     mgr.grant(child_id, CapabilityToken.restricted("fs.read"));
//
//     // Child inherits a narrower view of parent's capabilities
// }

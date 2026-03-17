/// Cost Oracle — per-construct cost queries per target (P19).
///
/// Every language construct exposes its exact cost — cycles, memory, allocations,
/// latency, token count — as query-time constants. Agents query costs *before*
/// emitting code, not after profiling.
///
/// The cost oracle integrates with MLIR's per-target cost modeling:
///   cost/query  →  { construct, target, optimization_level }  →  CostEstimate
use serde::{Deserialize, Serialize};

/// A cost estimate for a language construct on a specific target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    /// The construct being costed (e.g., "Vec::push", "[T]~.push").
    pub construct: String,
    /// Target architecture (e.g., "x86_64", "aarch64", "wasm32").
    pub target: String,
    /// Optimization level used for the estimate.
    pub opt_level: OptLevel,
    /// Estimated CPU cycles (amortized).
    pub cycles: u64,
    /// Memory footprint in bytes.
    pub memory_bytes: u64,
    /// Number of heap allocations.
    pub allocations: u32,
    /// Estimated latency in nanoseconds.
    pub latency_ns: u64,
    /// Token count to express this construct in Redox syntax.
    pub token_count: u32,
    /// Whether this is an exact cost or a statistical estimate.
    pub is_exact: bool,
    /// Confidence in the estimate: 0.0 (unknown) to 1.0 (measured).
    pub confidence: f64,
}

/// Optimization level for cost estimation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptLevel {
    Debug,
    Release,
    ReleaseLto,
}

/// A cost comparison between two alternative constructs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostComparison {
    pub option_a: CostEstimate,
    pub option_b: CostEstimate,
    /// Which option is recommended and why.
    pub recommendation: String,
}

// ── Built-in Cost Database ───────────────────────────────────────────

/// Cost entries for common Redox constructs.
///
/// These are estimated costs for x86_64 at Release optimization.
/// A real implementation would compute from MLIR cost models.
fn builtin_costs() -> Vec<CostEstimate> {
    let x86 = "x86_64";
    let rel = OptLevel::Release;

    vec![
        // Collection types
        cost(x86, rel, "[T]~.push (Vec::push)", 5, 0, 0, 3, 3, 0.9),
        cost(x86, rel, "[T]~.push (amortized realloc)", 50, 2048, 1, 40, 3, 0.8),
        cost(x86, rel, "[T; N] (stack array)", 1, 0, 0, 1, 3, 0.95),
        cost(x86, rel, "{K: V}.insert (HashMap)", 20, 0, 0, 15, 4, 0.85),
        cost(x86, rel, "{K}.insert (HashSet)", 18, 0, 0, 13, 3, 0.85),
        // Smart pointers
        cost(x86, rel, "^T (Box alloc)", 30, 8, 1, 25, 2, 0.9),
        cost(x86, rel, "^T (Box deref)", 1, 0, 0, 1, 1, 0.95),
        cost(x86, rel, "$T (Rc clone)", 3, 0, 0, 3, 2, 0.9),
        cost(x86, rel, "@T (Arc clone)", 8, 0, 0, 8, 2, 0.9),
        // String operations
        cost(x86, rel, "s.new (String alloc)", 30, 24, 1, 25, 2, 0.9),
        cost(x86, rel, "&s (str borrow)", 0, 0, 0, 0, 2, 1.0),
        cost(x86, rel, "f\"...\" (format!)", 40, 64, 1, 35, 3, 0.8),
        cost(x86, rel, "p\"...\" (println!)", 200, 64, 1, 500, 3, 0.7),
        // Control flow
        cost(x86, rel, "? (match, 2 arms)", 2, 0, 0, 2, 1, 0.95),
        cost(x86, rel, "? (if/else)", 1, 0, 0, 1, 1, 0.95),
        cost(x86, rel, "@(range) (for loop setup)", 3, 0, 0, 3, 3, 0.9),
        // Async
        cost(x86, rel, "af (async fn, no await)", 5, 64, 1, 5, 2, 0.8),
        cost(x86, rel, ".await (context switch)", 50, 0, 0, 100, 1, 0.7),
        // Concurrency
        cost(x86, rel, "std.sync.Mutex.lock", 15, 0, 0, 15, 3, 0.85),
        cost(x86, rel, "std.sync.RwLock.read", 10, 0, 0, 10, 3, 0.85),
        cost(x86, rel, "std.sync.Channel.send", 20, 0, 0, 18, 3, 0.8),
        // Agent primitives (Redox-unique)
        cost(x86, rel, "Swarm.broadcast", 100, 0, 0, 200, 3, 0.6),
        cost(x86, rel, "Bus.publish", 50, 128, 0, 80, 4, 0.6),
        cost(x86, rel, "Lease.acquire", 30, 64, 1, 40, 3, 0.5),
        cost(x86, rel, "Memory.persist", 500, 0, 0, 1000, 3, 0.5),
    ]
}

fn cost(target: &str, opt: OptLevel, construct: &str, cycles: u64, mem: u64, allocs: u32, lat: u64, tokens: u32, conf: f64) -> CostEstimate {
    CostEstimate {
        construct: construct.into(),
        target: target.into(),
        opt_level: opt,
        cycles,
        memory_bytes: mem,
        allocations: allocs,
        latency_ns: lat,
        token_count: tokens,
        is_exact: conf >= 0.95,
        confidence: conf,
    }
}

// ── Query Interface ──────────────────────────────────────────────────

/// Query the cost of a construct on a given target.
pub fn query_cost(construct: &str, target: &str, opt: OptLevel) -> Option<CostEstimate> {
    let costs = builtin_costs();
    costs.into_iter().find(|c| {
        c.construct.contains(construct) && c.target == target && c.opt_level == opt
    })
}

/// List all available cost entries for a target.
pub fn list_costs(target: &str) -> Vec<CostEstimate> {
    builtin_costs().into_iter().filter(|c| c.target == target).collect()
}

/// Compare costs of two constructs on the same target.
pub fn compare(construct_a: &str, construct_b: &str, target: &str, opt: OptLevel) -> Option<CostComparison> {
    let a = query_cost(construct_a, target, opt)?;
    let b = query_cost(construct_b, target, opt)?;

    let recommendation = if a.cycles <= b.cycles && a.memory_bytes <= b.memory_bytes {
        format!("`{}` is cheaper ({} cycles, {} bytes vs {} cycles, {} bytes)",
            a.construct, a.cycles, a.memory_bytes, b.cycles, b.memory_bytes)
    } else if b.cycles <= a.cycles && b.memory_bytes <= a.memory_bytes {
        format!("`{}` is cheaper ({} cycles, {} bytes vs {} cycles, {} bytes)",
            b.construct, b.cycles, b.memory_bytes, a.cycles, a.memory_bytes)
    } else {
        format!("Trade-off: `{}` ({} cycles, {} bytes) vs `{}` ({} cycles, {} bytes)",
            a.construct, a.cycles, a.memory_bytes, b.construct, b.cycles, b.memory_bytes)
    };

    Some(CostComparison { option_a: a, option_b: b, recommendation })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_vec_push() {
        let cost = query_cost("Vec::push", "x86_64", OptLevel::Release).unwrap();
        assert!(cost.cycles > 0);
        assert!(cost.confidence > 0.5);
    }

    #[test]
    fn query_stack_array() {
        let cost = query_cost("stack array", "x86_64", OptLevel::Release).unwrap();
        assert_eq!(cost.allocations, 0);
        assert!(cost.is_exact);
    }

    #[test]
    fn compare_box_vs_rc() {
        let cmp = compare("Box alloc", "Rc clone", "x86_64", OptLevel::Release).unwrap();
        // Rc clone should be cheaper in cycles
        assert!(cmp.option_b.cycles < cmp.option_a.cycles);
    }

    #[test]
    fn list_all_x86() {
        let costs = list_costs("x86_64");
        assert!(costs.len() >= 20);
    }

    #[test]
    fn agent_primitives_have_costs() {
        assert!(query_cost("Swarm.broadcast", "x86_64", OptLevel::Release).is_some());
        assert!(query_cost("Bus.publish", "x86_64", OptLevel::Release).is_some());
        assert!(query_cost("Lease.acquire", "x86_64", OptLevel::Release).is_some());
    }

    #[test]
    fn unknown_construct_returns_none() {
        assert!(query_cost("nonexistent_construct", "x86_64", OptLevel::Release).is_none());
    }
}

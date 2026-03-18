// ── CRDT Merge Engine ──────────────────────────────────────────────
//
// Semantic CRDTs for concurrent AST/HIR modifications by multiple
// agents.  Each operation is a deterministic, commutative, idempotent
// merge unit.
//
// Supported operations:
//   InsertItem       – add a new top-level item
//   RemoveItem       – remove an item by name
//   ModifyBody       – replace a function body
//   ModifySignature  – replace a function signature (params/return)
//   AddImpl          – add an impl block or item to an existing impl
//   Rename           – rename a symbol throughout the module
//
// The merge algorithm:
//   1. Each op carries a Lamport timestamp + agent-id (total order).
//   2. Concurrent non-conflicting ops merge trivially.
//   3. Conflicting ops (same target, different payloads) resolve by
//      timestamp > agent-id lexicographic order (last-writer-wins, but
//      deterministic).

use std::collections::BTreeMap;
use std::fmt;

// ── Lamport Clock ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LamportClock {
    pub time: u64,
    /// Tie-breaker: lexicographic agent id encoded as a u64 hash.
    pub agent_hash: u64,
}

impl LamportClock {
    pub fn new(time: u64, agent: &str) -> Self {
        // Simple hash for deterministic ordering.
        let agent_hash =
            agent.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        Self { time, agent_hash }
    }

    pub fn tick(&self) -> Self {
        Self { time: self.time + 1, agent_hash: self.agent_hash }
    }

    pub fn merge(a: LamportClock, b: LamportClock) -> LamportClock {
        LamportClock { time: a.time.max(b.time) + 1, agent_hash: a.agent_hash }
    }
}

// ── Operations ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrdtOp {
    InsertItem {
        name: String,
        /// Textual source of the item (Redox syntax).
        source: String,
    },
    RemoveItem {
        name: String,
    },
    ModifyBody {
        function_name: String,
        new_body: String,
    },
    ModifySignature {
        function_name: String,
        new_params: String,
        new_return_type: Option<String>,
    },
    AddImpl {
        target_type: String,
        impl_source: String,
    },
    Rename {
        old_name: String,
        new_name: String,
    },
}

impl CrdtOp {
    /// The "target" key for conflict detection.
    pub fn target_key(&self) -> String {
        match self {
            CrdtOp::InsertItem { name, .. } => format!("insert:{name}"),
            CrdtOp::RemoveItem { name } => format!("remove:{name}"),
            CrdtOp::ModifyBody { function_name, .. } => format!("body:{function_name}"),
            CrdtOp::ModifySignature { function_name, .. } => format!("sig:{function_name}"),
            CrdtOp::AddImpl { target_type, .. } => format!("impl:{target_type}"),
            CrdtOp::Rename { old_name, .. } => format!("rename:{old_name}"),
        }
    }
}

impl fmt::Display for CrdtOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CrdtOp::InsertItem { name, .. } => write!(f, "InsertItem({name})"),
            CrdtOp::RemoveItem { name } => write!(f, "RemoveItem({name})"),
            CrdtOp::ModifyBody { function_name, .. } => write!(f, "ModifyBody({function_name})"),
            CrdtOp::ModifySignature { function_name, .. } => {
                write!(f, "ModifySignature({function_name})")
            }
            CrdtOp::AddImpl { target_type, .. } => write!(f, "AddImpl({target_type})"),
            CrdtOp::Rename { old_name, new_name } => write!(f, "Rename({old_name} → {new_name})"),
        }
    }
}

// ── Stamped Operation ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct StampedOp {
    pub clock: LamportClock,
    pub agent: String,
    pub op: CrdtOp,
}

impl StampedOp {
    pub fn new(agent: impl Into<String>, time: u64, op: CrdtOp) -> Self {
        let agent = agent.into();
        let clock = LamportClock::new(time, &agent);
        Self { clock, agent, op }
    }
}

// ── Merge Result ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// Applied cleanly, no conflict.
    Clean,
    /// Conflict resolved by last-writer-wins.
    ResolvedLWW { winner: String, loser: String },
    /// Insert of duplicate name — later op wins.
    DuplicateInsert { name: String, winner: String },
}

// ── Merge Log ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MergeLog {
    pub entries: Vec<MergeLogEntry>,
}

#[derive(Debug, Clone)]
pub struct MergeLogEntry {
    pub op: String,
    pub outcome: MergeOutcome,
}

impl MergeLog {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn push(&mut self, op: &CrdtOp, outcome: MergeOutcome) {
        self.entries.push(MergeLogEntry { op: op.to_string(), outcome });
    }

    pub fn conflicts(&self) -> Vec<&MergeLogEntry> {
        self.entries.iter().filter(|e| !matches!(e.outcome, MergeOutcome::Clean)).collect()
    }
}

// ── CRDT State ─────────────────────────────────────────────────────

/// Represents the merged state of a module being edited by multiple agents.
pub struct CrdtState {
    /// Resolved op log, keyed by target, keeping the winning op.
    resolved: BTreeMap<String, StampedOp>,
    /// Full operation history (append-only).
    history: Vec<StampedOp>,
    /// Merge log.
    pub log: MergeLog,
}

impl CrdtState {
    pub fn new() -> Self {
        Self { resolved: BTreeMap::new(), history: Vec::new(), log: MergeLog::new() }
    }

    /// Apply an operation. Returns the merge outcome.
    pub fn apply(&mut self, stamped: StampedOp) -> MergeOutcome {
        let key = stamped.op.target_key();
        let outcome = if let Some(existing) = self.resolved.get(&key) {
            // Conflict: same target.  LWW by Lamport clock.
            if stamped.clock > existing.clock {
                let loser = existing.agent.clone();
                let winner = stamped.agent.clone();
                let outcome = match &stamped.op {
                    CrdtOp::InsertItem { name, .. } => {
                        MergeOutcome::DuplicateInsert { name: name.clone(), winner: winner.clone() }
                    }
                    _ => MergeOutcome::ResolvedLWW { winner: winner.clone(), loser },
                };
                self.resolved.insert(key, stamped.clone());
                outcome
            } else {
                let winner = existing.agent.clone();
                let loser = stamped.agent.clone();
                match &stamped.op {
                    CrdtOp::InsertItem { name, .. } => {
                        MergeOutcome::DuplicateInsert { name: name.clone(), winner }
                    }
                    _ => MergeOutcome::ResolvedLWW { winner, loser },
                }
            }
        } else {
            self.resolved.insert(key, stamped.clone());
            MergeOutcome::Clean
        };

        self.log.push(&stamped.op, outcome.clone());
        self.history.push(stamped);
        outcome
    }

    /// Apply a batch of operations, sorting by Lamport clock first.
    pub fn apply_batch(&mut self, mut ops: Vec<StampedOp>) -> Vec<MergeOutcome> {
        ops.sort_by_key(|o| o.clock);
        ops.into_iter().map(|o| self.apply(o)).collect()
    }

    /// Get the resolved (winning) operations.
    pub fn resolved_ops(&self) -> Vec<&StampedOp> {
        self.resolved.values().collect()
    }

    /// Full history length.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Produce a JSON snapshot of the resolved state.
    pub fn to_json(&self) -> String {
        let mut entries = Vec::new();
        for (key, sop) in &self.resolved {
            entries.push(format!(
                "{{\"key\":\"{key}\",\"agent\":\"{}\",\"time\":{},\"op\":\"{}\"}}",
                sop.agent, sop.clock.time, sop.op
            ));
        }
        format!("[{}]", entries.join(","))
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lamport_ordering() {
        let a = LamportClock::new(1, "alice");
        let b = LamportClock::new(2, "bob");
        assert!(b > a);
    }

    #[test]
    fn lamport_tiebreak_by_agent() {
        let a = LamportClock::new(5, "alice");
        let b = LamportClock::new(5, "bob");
        assert_ne!(a, b); // different agent hashes
    }

    #[test]
    fn lamport_merge() {
        let a = LamportClock::new(3, "a");
        let b = LamportClock::new(7, "b");
        let merged = LamportClock::merge(a, b);
        assert_eq!(merged.time, 8); // max(3,7) + 1
    }

    // ── Clean merges ──────────────────────────────────────────────

    #[test]
    fn non_conflicting_ops_merge_clean() {
        let mut state = CrdtState::new();
        let o1 = StampedOp::new(
            "a",
            1,
            CrdtOp::InsertItem { name: "Foo".into(), source: "S Foo {}".into() },
        );
        let o2 = StampedOp::new(
            "b",
            1,
            CrdtOp::InsertItem { name: "Bar".into(), source: "S Bar {}".into() },
        );
        assert_eq!(state.apply(o1), MergeOutcome::Clean);
        assert_eq!(state.apply(o2), MergeOutcome::Clean);
        assert_eq!(state.resolved_ops().len(), 2);
    }

    #[test]
    fn modify_body_clean() {
        let mut state = CrdtState::new();
        let op = StampedOp::new(
            "a",
            1,
            CrdtOp::ModifyBody { function_name: "foo".into(), new_body: "{ 42 }".into() },
        );
        assert_eq!(state.apply(op), MergeOutcome::Clean);
    }

    // ── LWW conflict resolution ───────────────────────────────────

    #[test]
    fn conflicting_modify_body_lww() {
        let mut state = CrdtState::new();
        let op1 = StampedOp::new(
            "alice",
            1,
            CrdtOp::ModifyBody { function_name: "foo".into(), new_body: "{ 1 }".into() },
        );
        let op2 = StampedOp::new(
            "bob",
            2,
            CrdtOp::ModifyBody { function_name: "foo".into(), new_body: "{ 2 }".into() },
        );
        state.apply(op1);
        let outcome = state.apply(op2);
        assert!(matches!(outcome, MergeOutcome::ResolvedLWW { winner, .. } if winner == "bob"));
    }

    #[test]
    fn earlier_timestamp_loses() {
        let mut state = CrdtState::new();
        let op1 = StampedOp::new(
            "bob",
            5,
            CrdtOp::ModifyBody { function_name: "foo".into(), new_body: "{ 5 }".into() },
        );
        let op2 = StampedOp::new(
            "alice",
            2,
            CrdtOp::ModifyBody { function_name: "foo".into(), new_body: "{ 2 }".into() },
        );
        state.apply(op1);
        let outcome = state.apply(op2);
        assert!(matches!(outcome, MergeOutcome::ResolvedLWW { winner, .. } if winner == "bob"));
    }

    // ── Duplicate insert ──────────────────────────────────────────

    #[test]
    fn duplicate_insert_resolved() {
        let mut state = CrdtState::new();
        let o1 = StampedOp::new(
            "a",
            1,
            CrdtOp::InsertItem { name: "Foo".into(), source: "S Foo { x: i32 }".into() },
        );
        let o2 = StampedOp::new(
            "b",
            2,
            CrdtOp::InsertItem { name: "Foo".into(), source: "S Foo { y: f64 }".into() },
        );
        state.apply(o1);
        let outcome = state.apply(o2);
        assert!(
            matches!(outcome, MergeOutcome::DuplicateInsert { name, winner } if name == "Foo" && winner == "b")
        );
    }

    // ── Batch apply ───────────────────────────────────────────────

    #[test]
    fn batch_sorts_by_clock() {
        let mut state = CrdtState::new();
        // Out-of-order batch.
        let ops = vec![
            StampedOp::new(
                "b",
                3,
                CrdtOp::InsertItem { name: "Z".into(), source: "S Z {}".into() },
            ),
            StampedOp::new(
                "a",
                1,
                CrdtOp::InsertItem { name: "A".into(), source: "S A {}".into() },
            ),
            StampedOp::new(
                "c",
                2,
                CrdtOp::InsertItem { name: "M".into(), source: "S M {}".into() },
            ),
        ];
        let outcomes = state.apply_batch(ops);
        assert!(outcomes.iter().all(|o| *o == MergeOutcome::Clean));
        assert_eq!(state.history_len(), 3);
    }

    // ── Rename ────────────────────────────────────────────────────

    #[test]
    fn rename_clean() {
        let mut state = CrdtState::new();
        let op = StampedOp::new(
            "a",
            1,
            CrdtOp::Rename { old_name: "Foo".into(), new_name: "Bar".into() },
        );
        assert_eq!(state.apply(op), MergeOutcome::Clean);
    }

    #[test]
    fn conflicting_rename_lww() {
        let mut state = CrdtState::new();
        let o1 = StampedOp::new(
            "a",
            1,
            CrdtOp::Rename { old_name: "Foo".into(), new_name: "Bar".into() },
        );
        let o2 = StampedOp::new(
            "b",
            2,
            CrdtOp::Rename { old_name: "Foo".into(), new_name: "Baz".into() },
        );
        state.apply(o1);
        let outcome = state.apply(o2);
        assert!(matches!(outcome, MergeOutcome::ResolvedLWW { winner, .. } if winner == "b"));
    }

    // ── AddImpl ───────────────────────────────────────────────────

    #[test]
    fn add_impl_clean() {
        let mut state = CrdtState::new();
        let op = StampedOp::new(
            "a",
            1,
            CrdtOp::AddImpl {
                target_type: "Vec".into(),
                impl_source: "impl Vec { f push(&m self, item: T) {} }".into(),
            },
        );
        assert_eq!(state.apply(op), MergeOutcome::Clean);
    }

    // ── ModifySignature ───────────────────────────────────────────

    #[test]
    fn modify_signature_clean() {
        let mut state = CrdtState::new();
        let op = StampedOp::new(
            "a",
            1,
            CrdtOp::ModifySignature {
                function_name: "process".into(),
                new_params: "(data: &[u8], len: usize)".into(),
                new_return_type: Some("Result".into()),
            },
        );
        assert_eq!(state.apply(op), MergeOutcome::Clean);
    }

    // ── Remove ────────────────────────────────────────────────────

    #[test]
    fn remove_item_clean() {
        let mut state = CrdtState::new();
        let op = StampedOp::new("a", 1, CrdtOp::RemoveItem { name: "OldStruct".into() });
        assert_eq!(state.apply(op), MergeOutcome::Clean);
    }

    // ── Merge log ─────────────────────────────────────────────────

    #[test]
    fn merge_log_tracks_conflicts() {
        let mut state = CrdtState::new();
        let o1 = StampedOp::new(
            "a",
            1,
            CrdtOp::ModifyBody { function_name: "f".into(), new_body: "{}".into() },
        );
        let o2 = StampedOp::new(
            "b",
            2,
            CrdtOp::ModifyBody { function_name: "f".into(), new_body: "{ 1 }".into() },
        );
        state.apply(o1);
        state.apply(o2);
        assert_eq!(state.log.conflicts().len(), 1);
    }

    // ── JSON output ───────────────────────────────────────────────

    #[test]
    fn to_json_contains_ops() {
        let mut state = CrdtState::new();
        state.apply(StampedOp::new(
            "a",
            1,
            CrdtOp::InsertItem { name: "Foo".into(), source: "S Foo {}".into() },
        ));
        let json = state.to_json();
        assert!(json.contains("InsertItem(Foo)"));
        assert!(json.contains("\"agent\":\"a\""));
    }

    // ── Target key uniqueness ─────────────────────────────────────

    #[test]
    fn different_op_types_different_keys() {
        let insert = CrdtOp::InsertItem { name: "Foo".into(), source: "".into() };
        let remove = CrdtOp::RemoveItem { name: "Foo".into() };
        assert_ne!(insert.target_key(), remove.target_key());
    }
}

//! # Redox Semantic VCS
//!
//! Operation-log-based version control for agent swarms.
//! Replaces git-level merges with semantic-operation-level merges.
//!
//! Core concepts:
//! - **SemanticOp**: A semantic operation on the codebase (not a text diff)
//! - **OpLog**: Append-only log of operations with causal ordering
//! - **Snapshot**: A named point-in-time view of the operation history
//! - **Branch**: A named reference to a snapshot
//! - **SemanticVCS**: The main VCS with commit, branch, merge, and query

use std::collections::BTreeMap;

// ── Operations ──────────────────────────────────────────────────────────────

/// Lamport timestamp for causal ordering.
pub type Timestamp = u64;

/// Unique identifier for an operation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpId(pub u64);

impl std::fmt::Display for OpId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "op-{}", self.0)
    }
}

/// The kind of semantic operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpKind {
    /// Add a new definition (function, type, module, etc.).
    AddDefinition { name: String, region: String },
    /// Remove a definition.
    RemoveDefinition { name: String, region: String },
    /// Rename a symbol across all usage sites.
    RenameSymbol { old_name: String, new_name: String, region: String },
    /// Modify the body/implementation of a definition.
    ModifyBody { name: String, region: String },
    /// Modify a type signature or trait interface.
    ModifyInterface { name: String, region: String },
    /// Move a definition between regions/modules.
    MoveDefinition { name: String, from_region: String, to_region: String },
    /// Add or remove a dependency between modules.
    ModifyDependency { from_region: String, to_region: String, added: bool },
}

impl OpKind {
    /// Get the primary region affected by this operation.
    pub fn primary_region(&self) -> &str {
        match self {
            OpKind::AddDefinition { region, .. } => region,
            OpKind::RemoveDefinition { region, .. } => region,
            OpKind::RenameSymbol { region, .. } => region,
            OpKind::ModifyBody { region, .. } => region,
            OpKind::ModifyInterface { region, .. } => region,
            OpKind::MoveDefinition { from_region, .. } => from_region,
            OpKind::ModifyDependency { from_region, .. } => from_region,
        }
    }

    /// Check if two operations can conflict (touch the same name in the same region).
    pub fn conflicts_with(&self, other: &OpKind) -> bool {
        match (self, other) {
            (
                OpKind::ModifyBody { name: n1, region: r1 },
                OpKind::ModifyBody { name: n2, region: r2 },
            ) => n1 == n2 && r1 == r2,
            (
                OpKind::ModifyBody { name: n1, region: r1 },
                OpKind::RemoveDefinition { name: n2, region: r2 },
            )
            | (
                OpKind::RemoveDefinition { name: n2, region: r2 },
                OpKind::ModifyBody { name: n1, region: r1 },
            ) => n1 == n2 && r1 == r2,
            (
                OpKind::RenameSymbol { old_name: on1, region: r1, .. },
                OpKind::RenameSymbol { old_name: on2, region: r2, .. },
            ) => on1 == on2 && r1 == r2,
            (
                OpKind::ModifyInterface { name: n1, region: r1 },
                OpKind::ModifyInterface { name: n2, region: r2 },
            ) => n1 == n2 && r1 == r2,
            (
                OpKind::AddDefinition { name: n1, region: r1 },
                OpKind::AddDefinition { name: n2, region: r2 },
            ) => n1 == n2 && r1 == r2,
            _ => false,
        }
    }
}

/// A semantic operation in the log.
#[derive(Debug, Clone)]
pub struct SemanticOp {
    pub id: OpId,
    pub timestamp: Timestamp,
    pub agent: String,
    pub branch: String,
    pub kind: OpKind,
    pub rationale: Option<String>,
}

impl SemanticOp {
    pub fn new(id: u64, timestamp: Timestamp, agent: &str, branch: &str, kind: OpKind) -> Self {
        Self {
            id: OpId(id),
            timestamp,
            agent: agent.to_string(),
            branch: branch.to_string(),
            kind,
            rationale: None,
        }
    }

    pub fn with_rationale(mut self, rationale: &str) -> Self {
        self.rationale = Some(rationale.to_string());
        self
    }
}

// ── Operation Log ───────────────────────────────────────────────────────────

/// Append-only operation log with causal ordering.
pub struct OpLog {
    ops: Vec<SemanticOp>,
    next_id: u64,
    next_timestamp: Timestamp,
}

impl OpLog {
    pub fn new() -> Self {
        Self { ops: Vec::new(), next_id: 1, next_timestamp: 1 }
    }

    /// Append an operation to the log.
    pub fn append(&mut self, agent: &str, branch: &str, kind: OpKind) -> OpId {
        let id = self.next_id;
        let ts = self.next_timestamp;
        self.next_id += 1;
        self.next_timestamp += 1;
        let op = SemanticOp::new(id, ts, agent, branch, kind);
        let op_id = op.id.clone();
        self.ops.push(op);
        op_id
    }

    /// Append with a rationale.
    pub fn append_with_rationale(
        &mut self,
        agent: &str,
        branch: &str,
        kind: OpKind,
        rationale: &str,
    ) -> OpId {
        let id = self.next_id;
        let ts = self.next_timestamp;
        self.next_id += 1;
        self.next_timestamp += 1;
        let op = SemanticOp::new(id, ts, agent, branch, kind).with_rationale(rationale);
        let op_id = op.id.clone();
        self.ops.push(op);
        op_id
    }

    /// Get all operations.
    pub fn ops(&self) -> &[SemanticOp] {
        &self.ops
    }

    /// Get operations on a specific branch since a given timestamp (exclusive).
    pub fn ops_on_branch_since(
        &self,
        branch: &str,
        since_timestamp: Timestamp,
    ) -> Vec<&SemanticOp> {
        self.ops.iter().filter(|op| op.branch == branch && op.timestamp > since_timestamp).collect()
    }

    /// Get operations by a specific agent.
    pub fn ops_by_agent(&self, agent: &str) -> Vec<&SemanticOp> {
        self.ops.iter().filter(|op| op.agent == agent).collect()
    }

    /// Get operations affecting a specific region.
    pub fn ops_in_region(&self, region: &str) -> Vec<&SemanticOp> {
        self.ops.iter().filter(|op| op.kind.primary_region() == region).collect()
    }

    /// Number of operations in the log.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Current timestamp (last assigned).
    pub fn current_timestamp(&self) -> Timestamp {
        if self.next_timestamp > 1 { self.next_timestamp - 1 } else { 0 }
    }
}

impl Default for OpLog {
    fn default() -> Self {
        Self::new()
    }
}

// ── Snapshots and Branches ──────────────────────────────────────────────────

/// A snapshot: a point-in-time reference into the operation log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub name: String,
    pub timestamp: Timestamp,
    pub op_count: usize,
}

/// A branch: a named, movable reference to a snapshot.
#[derive(Debug, Clone)]
pub struct Branch {
    pub name: String,
    pub head_timestamp: Timestamp,
    pub head_op_count: usize,
    pub parent_branch: Option<String>,
    pub fork_timestamp: Timestamp,
}

// ── Merge ───────────────────────────────────────────────────────────────────

/// A conflict detected during semantic merge.
#[derive(Debug, Clone)]
pub struct MergeConflict {
    pub op_a: OpId,
    pub op_b: OpId,
    pub description: String,
}

/// The result of a semantic merge.
#[derive(Debug)]
pub struct MergeResult {
    /// Operations that merged cleanly.
    pub merged_ops: Vec<SemanticOp>,
    /// Conflicts that need resolution.
    pub conflicts: Vec<MergeConflict>,
}

impl MergeResult {
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }

    pub fn merged_count(&self) -> usize {
        self.merged_ops.len()
    }
}

// ── VCS Errors ──────────────────────────────────────────────────────────────

/// Errors from the semantic VCS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VcsError {
    BranchNotFound(String),
    BranchAlreadyExists(String),
    SnapshotNotFound(String),
    CannotMergeSameBranch,
    NoCommonAncestor(String, String),
}

impl std::fmt::Display for VcsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VcsError::BranchNotFound(name) => write!(f, "Branch not found: {}", name),
            VcsError::BranchAlreadyExists(name) => write!(f, "Branch already exists: {}", name),
            VcsError::SnapshotNotFound(name) => write!(f, "Snapshot not found: {}", name),
            VcsError::CannotMergeSameBranch => write!(f, "Cannot merge a branch with itself"),
            VcsError::NoCommonAncestor(a, b) => {
                write!(f, "No common ancestor between '{}' and '{}'", a, b)
            }
        }
    }
}

// ── Semantic VCS ────────────────────────────────────────────────────────────

/// The main semantic version control system.
///
/// Operations are committed to the current branch's op log.
/// Branches can be created, switched, and merged.
/// Merges operate at the semantic level, not the text level.
pub struct SemanticVCS {
    log: OpLog,
    branches: BTreeMap<String, Branch>,
    snapshots: BTreeMap<String, Snapshot>,
    current_branch: String,
}

impl SemanticVCS {
    /// Create a new VCS with a "main" branch.
    pub fn new() -> Self {
        let mut branches = BTreeMap::new();
        branches.insert(
            "main".to_string(),
            Branch {
                name: "main".to_string(),
                head_timestamp: 0,
                head_op_count: 0,
                parent_branch: None,
                fork_timestamp: 0,
            },
        );
        Self {
            log: OpLog::new(),
            branches,
            snapshots: BTreeMap::new(),
            current_branch: "main".to_string(),
        }
    }

    /// Commit an operation to the current branch.
    pub fn commit(&mut self, agent: &str, kind: OpKind) -> OpId {
        let branch = self.current_branch.clone();
        let op_id = self.log.append(agent, &branch, kind);
        self.advance_branch_head();
        op_id
    }

    /// Commit an operation with rationale.
    pub fn commit_with_rationale(&mut self, agent: &str, kind: OpKind, rationale: &str) -> OpId {
        let branch = self.current_branch.clone();
        let op_id = self.log.append_with_rationale(agent, &branch, kind, rationale);
        self.advance_branch_head();
        op_id
    }

    fn advance_branch_head(&mut self) {
        if let Some(branch) = self.branches.get_mut(&self.current_branch) {
            branch.head_timestamp = self.log.current_timestamp();
            branch.head_op_count = self.log.len();
        }
    }

    /// Create a new branch from the current branch at the current position.
    pub fn create_branch(&mut self, name: &str) -> Result<(), VcsError> {
        if self.branches.contains_key(name) {
            return Err(VcsError::BranchAlreadyExists(name.to_string()));
        }
        let current = &self.branches[&self.current_branch];
        let branch = Branch {
            name: name.to_string(),
            head_timestamp: current.head_timestamp,
            head_op_count: current.head_op_count,
            parent_branch: Some(self.current_branch.clone()),
            fork_timestamp: current.head_timestamp,
        };
        self.branches.insert(name.to_string(), branch);
        Ok(())
    }

    /// Switch to an existing branch.
    pub fn switch_branch(&mut self, name: &str) -> Result<(), VcsError> {
        if !self.branches.contains_key(name) {
            return Err(VcsError::BranchNotFound(name.to_string()));
        }
        self.current_branch = name.to_string();
        Ok(())
    }

    /// Get the current branch name.
    pub fn current_branch(&self) -> &str {
        &self.current_branch
    }

    /// List all branch names.
    pub fn branches(&self) -> Vec<&str> {
        self.branches.keys().map(|s| s.as_str()).collect()
    }

    /// Create a named snapshot of the current state.
    pub fn create_snapshot(&mut self, name: &str) -> Result<Snapshot, VcsError> {
        if self.snapshots.contains_key(name) {
            return Err(VcsError::SnapshotNotFound(name.to_string()));
        }
        let snapshot = Snapshot {
            name: name.to_string(),
            timestamp: self.log.current_timestamp(),
            op_count: self.log.len(),
        };
        self.snapshots.insert(name.to_string(), snapshot.clone());
        Ok(snapshot)
    }

    /// Get a snapshot by name.
    pub fn get_snapshot(&self, name: &str) -> Option<&Snapshot> {
        self.snapshots.get(name)
    }

    /// Merge another branch into the current branch.
    /// Returns a MergeResult with cleanly merged ops and any conflicts.
    pub fn merge(&self, other_branch: &str) -> Result<MergeResult, VcsError> {
        if other_branch == self.current_branch {
            return Err(VcsError::CannotMergeSameBranch);
        }

        let other = self
            .branches
            .get(other_branch)
            .ok_or_else(|| VcsError::BranchNotFound(other_branch.to_string()))?;

        // Find the common ancestor (fork point)
        let fork_ts = other.fork_timestamp;

        // Get ops on each branch since the fork point, filtered by branch name
        let ops_current: Vec<&SemanticOp> =
            self.log.ops_on_branch_since(&self.current_branch, fork_ts);
        let ops_other: Vec<&SemanticOp> = self.log.ops_on_branch_since(other_branch, fork_ts);

        // Find conflicts between the two sets
        let mut conflicts = Vec::new();
        let mut conflicted_current = std::collections::BTreeSet::new();
        let mut conflicted_other = std::collections::BTreeSet::new();

        for op_a in &ops_current {
            for op_b in &ops_other {
                if op_a.kind.conflicts_with(&op_b.kind) {
                    conflicts.push(MergeConflict {
                        op_a: op_a.id.clone(),
                        op_b: op_b.id.clone(),
                        description: format!(
                            "Conflicting operations: {} and {} in region '{}'",
                            op_a.id,
                            op_b.id,
                            op_a.kind.primary_region()
                        ),
                    });
                    conflicted_current.insert(op_a.id.clone());
                    conflicted_other.insert(op_b.id.clone());
                }
            }
        }

        // Collect non-conflicting ops from both sides
        let mut merged_ops: Vec<SemanticOp> = Vec::new();
        for op in &ops_current {
            if !conflicted_current.contains(&op.id) {
                merged_ops.push((*op).clone());
            }
        }
        for op in &ops_other {
            if !conflicted_other.contains(&op.id) {
                merged_ops.push((*op).clone());
            }
        }

        // Sort by timestamp
        merged_ops.sort_by_key(|op| op.timestamp);

        Ok(MergeResult { merged_ops, conflicts })
    }

    /// Query the operation history.
    pub fn history(&self) -> &[SemanticOp] {
        self.log.ops()
    }

    /// Query operations by agent.
    pub fn history_by_agent(&self, agent: &str) -> Vec<&SemanticOp> {
        self.log.ops_by_agent(agent)
    }

    /// Query operations in a region.
    pub fn history_in_region(&self, region: &str) -> Vec<&SemanticOp> {
        self.log.ops_in_region(region)
    }

    /// Query operations by intent (search rationale).
    pub fn query_by_intent(&self, query: &str) -> Vec<&SemanticOp> {
        let query_lower = query.to_lowercase();
        self.log
            .ops()
            .iter()
            .filter(|op| {
                op.rationale
                    .as_ref()
                    .map(|r| r.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Total operations in the log.
    pub fn op_count(&self) -> usize {
        self.log.len()
    }
}

impl Default for SemanticVCS {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── OpKind tests ──

    #[test]
    fn op_kind_primary_region() {
        let op = OpKind::AddDefinition { name: "Foo".to_string(), region: "core".to_string() };
        assert_eq!(op.primary_region(), "core");

        let op = OpKind::MoveDefinition {
            name: "Bar".to_string(),
            from_region: "a".to_string(),
            to_region: "b".to_string(),
        };
        assert_eq!(op.primary_region(), "a");
    }

    #[test]
    fn op_kind_conflicts_same_modify() {
        let a = OpKind::ModifyBody { name: "foo".to_string(), region: "core".to_string() };
        let b = OpKind::ModifyBody { name: "foo".to_string(), region: "core".to_string() };
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn op_kind_no_conflict_different_region() {
        let a = OpKind::ModifyBody { name: "foo".to_string(), region: "core".to_string() };
        let b = OpKind::ModifyBody { name: "foo".to_string(), region: "utils".to_string() };
        assert!(!a.conflicts_with(&b));
    }

    #[test]
    fn op_kind_no_conflict_different_name() {
        let a = OpKind::ModifyBody { name: "foo".to_string(), region: "core".to_string() };
        let b = OpKind::ModifyBody { name: "bar".to_string(), region: "core".to_string() };
        assert!(!a.conflicts_with(&b));
    }

    #[test]
    fn op_kind_conflict_modify_remove() {
        let a = OpKind::ModifyBody { name: "foo".to_string(), region: "core".to_string() };
        let b = OpKind::RemoveDefinition { name: "foo".to_string(), region: "core".to_string() };
        assert!(a.conflicts_with(&b));
        assert!(b.conflicts_with(&a));
    }

    #[test]
    fn op_kind_conflict_rename_same_symbol() {
        let a = OpKind::RenameSymbol {
            old_name: "foo".to_string(),
            new_name: "bar".to_string(),
            region: "core".to_string(),
        };
        let b = OpKind::RenameSymbol {
            old_name: "foo".to_string(),
            new_name: "baz".to_string(),
            region: "core".to_string(),
        };
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn op_kind_no_conflict_different_ops() {
        let a = OpKind::AddDefinition { name: "foo".to_string(), region: "core".to_string() };
        let b = OpKind::ModifyBody { name: "bar".to_string(), region: "core".to_string() };
        assert!(!a.conflicts_with(&b));
    }

    // ── OpLog tests ──

    #[test]
    fn oplog_append_and_query() {
        let mut log = OpLog::new();
        assert!(log.is_empty());

        log.append(
            "agent1",
            "main",
            OpKind::AddDefinition { name: "Foo".to_string(), region: "core".to_string() },
        );
        log.append(
            "agent2",
            "main",
            OpKind::ModifyBody { name: "Bar".to_string(), region: "utils".to_string() },
        );

        assert_eq!(log.len(), 2);
        assert_eq!(log.current_timestamp(), 2);
    }

    #[test]
    fn oplog_query_by_agent() {
        let mut log = OpLog::new();
        log.append(
            "agent1",
            "main",
            OpKind::AddDefinition { name: "A".to_string(), region: "core".to_string() },
        );
        log.append(
            "agent2",
            "main",
            OpKind::AddDefinition { name: "B".to_string(), region: "core".to_string() },
        );
        log.append(
            "agent1",
            "main",
            OpKind::ModifyBody { name: "A".to_string(), region: "core".to_string() },
        );

        assert_eq!(log.ops_by_agent("agent1").len(), 2);
        assert_eq!(log.ops_by_agent("agent2").len(), 1);
    }

    #[test]
    fn oplog_query_by_region() {
        let mut log = OpLog::new();
        log.append(
            "a1",
            "main",
            OpKind::AddDefinition { name: "X".to_string(), region: "core".to_string() },
        );
        log.append(
            "a1",
            "main",
            OpKind::AddDefinition { name: "Y".to_string(), region: "utils".to_string() },
        );

        assert_eq!(log.ops_in_region("core").len(), 1);
        assert_eq!(log.ops_in_region("utils").len(), 1);
        assert_eq!(log.ops_in_region("other").len(), 0);
    }

    #[test]
    fn oplog_ops_on_branch_since() {
        let mut log = OpLog::new();
        log.append(
            "a1",
            "main",
            OpKind::AddDefinition { name: "A".to_string(), region: "r".to_string() },
        );
        log.append(
            "a1",
            "feature",
            OpKind::AddDefinition { name: "B".to_string(), region: "r".to_string() },
        );
        log.append(
            "a1",
            "main",
            OpKind::AddDefinition { name: "C".to_string(), region: "r".to_string() },
        );

        assert_eq!(log.ops_on_branch_since("main", 0).len(), 2);
        assert_eq!(log.ops_on_branch_since("feature", 0).len(), 1);
        assert_eq!(log.ops_on_branch_since("main", 2).len(), 1); // only C (ts=3)
    }

    #[test]
    fn oplog_with_rationale() {
        let mut log = OpLog::new();
        let id = log.append_with_rationale(
            "agent1",
            "main",
            OpKind::AddDefinition { name: "Foo".to_string(), region: "core".to_string() },
            "Adding error handling",
        );
        assert_eq!(id, OpId(1));
        assert_eq!(log.ops()[0].rationale.as_deref(), Some("Adding error handling"));
    }

    // ── SemanticVCS tests ──

    #[test]
    fn vcs_new_has_main_branch() {
        let vcs = SemanticVCS::new();
        assert_eq!(vcs.current_branch(), "main");
        assert_eq!(vcs.branches(), vec!["main"]);
        assert_eq!(vcs.op_count(), 0);
    }

    #[test]
    fn vcs_commit() {
        let mut vcs = SemanticVCS::new();
        let id = vcs.commit(
            "agent1",
            OpKind::AddDefinition { name: "Foo".to_string(), region: "core".to_string() },
        );
        assert_eq!(id, OpId(1));
        assert_eq!(vcs.op_count(), 1);
    }

    #[test]
    fn vcs_commit_with_rationale() {
        let mut vcs = SemanticVCS::new();
        vcs.commit_with_rationale(
            "agent1",
            OpKind::AddDefinition { name: "Foo".to_string(), region: "core".to_string() },
            "Initial setup",
        );
        assert_eq!(vcs.history()[0].rationale.as_deref(), Some("Initial setup"));
    }

    #[test]
    fn vcs_branch_create_and_switch() {
        let mut vcs = SemanticVCS::new();
        vcs.create_branch("feature").unwrap();
        assert_eq!(vcs.branches().len(), 2);

        vcs.switch_branch("feature").unwrap();
        assert_eq!(vcs.current_branch(), "feature");
    }

    #[test]
    fn vcs_branch_duplicate_error() {
        let mut vcs = SemanticVCS::new();
        vcs.create_branch("feature").unwrap();
        let err = vcs.create_branch("feature").unwrap_err();
        assert_eq!(err, VcsError::BranchAlreadyExists("feature".to_string()));
    }

    #[test]
    fn vcs_branch_not_found_error() {
        let mut vcs = SemanticVCS::new();
        let err = vcs.switch_branch("nonexistent").unwrap_err();
        assert_eq!(err, VcsError::BranchNotFound("nonexistent".to_string()));
    }

    #[test]
    fn vcs_snapshot() {
        let mut vcs = SemanticVCS::new();
        vcs.commit("a1", OpKind::AddDefinition { name: "X".to_string(), region: "r".to_string() });
        let snap = vcs.create_snapshot("v1").unwrap();
        assert_eq!(snap.name, "v1");
        assert_eq!(snap.op_count, 1);

        let retrieved = vcs.get_snapshot("v1").unwrap();
        assert_eq!(retrieved.name, "v1");
    }

    #[test]
    fn vcs_merge_clean() {
        let mut vcs = SemanticVCS::new();

        // Commit on main
        vcs.commit(
            "a1",
            OpKind::AddDefinition { name: "Base".to_string(), region: "core".to_string() },
        );

        // Create feature branch and switch to it
        vcs.create_branch("feature").unwrap();
        vcs.switch_branch("feature").unwrap();

        // Commit on feature (different region — no conflict)
        vcs.commit(
            "a2",
            OpKind::AddDefinition { name: "Feature".to_string(), region: "utils".to_string() },
        );

        // Switch back to main, commit something non-conflicting
        vcs.switch_branch("main").unwrap();
        vcs.commit(
            "a1",
            OpKind::ModifyBody { name: "Base".to_string(), region: "core".to_string() },
        );

        // Merge feature into main — should be clean
        let result = vcs.merge("feature").unwrap();
        assert!(
            result.is_clean(),
            "Expected clean merge, got {} conflicts",
            result.conflict_count()
        );
        assert!(result.merged_count() > 0);
    }

    #[test]
    fn vcs_merge_conflict() {
        let mut vcs = SemanticVCS::new();

        // Base commit on main
        vcs.commit(
            "a1",
            OpKind::AddDefinition { name: "Shared".to_string(), region: "core".to_string() },
        );

        // Create feature branch and switch
        vcs.create_branch("feature").unwrap();
        vcs.switch_branch("feature").unwrap();

        // Modify same function on feature branch
        vcs.commit(
            "a2",
            OpKind::ModifyBody { name: "Shared".to_string(), region: "core".to_string() },
        );

        // Switch to main and modify same function
        vcs.switch_branch("main").unwrap();
        vcs.commit(
            "a1",
            OpKind::ModifyBody { name: "Shared".to_string(), region: "core".to_string() },
        );

        // Merge: should detect conflict
        let result = vcs.merge("feature").unwrap();
        assert!(!result.is_clean());
        assert_eq!(result.conflict_count(), 1);
    }

    #[test]
    fn vcs_merge_same_branch_error() {
        let vcs = SemanticVCS::new();
        let err = vcs.merge("main").unwrap_err();
        assert_eq!(err, VcsError::CannotMergeSameBranch);
    }

    #[test]
    fn vcs_merge_nonexistent_branch_error() {
        let vcs = SemanticVCS::new();
        let err = vcs.merge("nonexistent").unwrap_err();
        assert_eq!(err, VcsError::BranchNotFound("nonexistent".to_string()));
    }

    #[test]
    fn vcs_history_by_agent() {
        let mut vcs = SemanticVCS::new();
        vcs.commit(
            "agent_a",
            OpKind::AddDefinition { name: "X".to_string(), region: "r".to_string() },
        );
        vcs.commit(
            "agent_b",
            OpKind::AddDefinition { name: "Y".to_string(), region: "r".to_string() },
        );

        assert_eq!(vcs.history_by_agent("agent_a").len(), 1);
        assert_eq!(vcs.history_by_agent("agent_b").len(), 1);
    }

    #[test]
    fn vcs_history_in_region() {
        let mut vcs = SemanticVCS::new();
        vcs.commit(
            "a",
            OpKind::AddDefinition { name: "X".to_string(), region: "core".to_string() },
        );
        vcs.commit(
            "a",
            OpKind::AddDefinition { name: "Y".to_string(), region: "utils".to_string() },
        );

        assert_eq!(vcs.history_in_region("core").len(), 1);
        assert_eq!(vcs.history_in_region("utils").len(), 1);
    }

    #[test]
    fn vcs_query_by_intent() {
        let mut vcs = SemanticVCS::new();
        vcs.commit_with_rationale(
            "a",
            OpKind::AddDefinition { name: "Handler".to_string(), region: "core".to_string() },
            "Adding error handling logic",
        );
        vcs.commit_with_rationale(
            "a",
            OpKind::AddDefinition { name: "Parser".to_string(), region: "parse".to_string() },
            "Implementing token parser",
        );

        let results = vcs.query_by_intent("error handling");
        assert_eq!(results.len(), 1);

        let results = vcs.query_by_intent("parser");
        assert_eq!(results.len(), 1);

        let results = vcs.query_by_intent("nonexistent");
        assert_eq!(results.len(), 0);
    }

    // ── Error Display tests ──

    #[test]
    fn vcs_error_display() {
        let err = VcsError::BranchNotFound("feature".to_string());
        assert!(format!("{}", err).contains("Branch not found"));

        let err = VcsError::CannotMergeSameBranch;
        assert!(format!("{}", err).contains("Cannot merge"));
    }

    #[test]
    fn opid_display() {
        let id = OpId(42);
        assert_eq!(format!("{}", id), "op-42");
    }

    // ── Conflict detection edge cases ──

    #[test]
    fn conflict_add_add_same_name() {
        let a = OpKind::AddDefinition { name: "Foo".to_string(), region: "core".to_string() };
        let b = OpKind::AddDefinition { name: "Foo".to_string(), region: "core".to_string() };
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn conflict_interface_interface() {
        let a = OpKind::ModifyInterface { name: "Trait1".to_string(), region: "core".to_string() };
        let b = OpKind::ModifyInterface { name: "Trait1".to_string(), region: "core".to_string() };
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn no_conflict_move_vs_add() {
        let a = OpKind::MoveDefinition {
            name: "X".to_string(),
            from_region: "a".to_string(),
            to_region: "b".to_string(),
        };
        let b = OpKind::AddDefinition { name: "Y".to_string(), region: "c".to_string() };
        assert!(!a.conflicts_with(&b));
    }
}

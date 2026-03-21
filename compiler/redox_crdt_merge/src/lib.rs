// CRDT-based semantic merge engine for concurrent AST/HIR modifications.
// (REDOX_PROPOSAL.md §7.3)
//
// Semantic CRDTs operate on the AST/HIR level, not on raw text.
// Operations: InsertNode, DeleteNode, MoveNode, RenameSymbol, ChangeType, AddImport.
// Merge resolution: conflict-free when operations target disjoint regions,
// arbitration-required when they overlap on the same node.

use std::collections::{BTreeMap, BTreeSet};

// ── Identifiers ────────────────────────────────────────────────────────────

/// Globally unique node identifier (lamport-style: agent + sequence).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId {
    pub agent: String,
    pub seq: u64,
}

impl NodeId {
    pub fn new(agent: &str, seq: u64) -> Self {
        NodeId { agent: agent.to_string(), seq }
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.agent, self.seq)
    }
}

/// A definition ID for symbols in the semantic tree.
pub type DefId = u64;

/// A module ID.
pub type ModuleId = u64;

/// A content hash for optimistic concurrency.
pub type Hash = u64;

/// Lamport timestamp for causal ordering.
pub type Timestamp = u64;

// ── CRDT Operations ────────────────────────────────────────────────────────

/// A semantic CRDT operation on the codebase AST/HIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrdtOp {
    /// Unique ID of this operation.
    pub id: NodeId,
    /// Lamport timestamp for causal ordering.
    pub timestamp: Timestamp,
    /// The actual operation.
    pub kind: OpKind,
}

/// The kinds of semantic operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpKind {
    /// Insert a new AST node (function, struct, impl, etc.) under a parent.
    InsertNode {
        parent: ModuleId,
        node_type: NodeType,
        name: String,
        position: InsertPosition,
    },
    /// Delete an existing AST node.
    DeleteNode {
        target: DefId,
        old_hash: Hash,
    },
    /// Move a node from one parent to another.
    MoveNode {
        target: DefId,
        from_parent: ModuleId,
        to_parent: ModuleId,
        position: InsertPosition,
    },
    /// Rename a symbol across all usage sites.
    RenameSymbol {
        target: DefId,
        old_name: String,
        new_name: String,
    },
    /// Change the type of a definition.
    ChangeType {
        target: DefId,
        old_type: String,
        new_type: String,
    },
    /// Add an import statement to a module.
    AddImport {
        module: ModuleId,
        path: String,
    },
}

/// The type of AST node being inserted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeType {
    Function,
    Struct,
    Enum,
    Impl,
    Trait,
    Const,
    Static,
    TypeAlias,
    Module,
}

/// Ordering hint for where to insert a new node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertPosition {
    /// At the end of the parent's children.
    Append,
    /// Before a specific sibling.
    Before(DefId),
    /// After a specific sibling.
    After(DefId),
    /// At a specific index.
    AtIndex(usize),
}

// ── Merge Results ──────────────────────────────────────────────────────────

/// Result of merging two concurrent operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeResult {
    /// Both operations can be applied without conflict.
    BothApply,
    /// Operations must be sequenced (first before second).
    Sequence,
    /// Conflict that requires arbitration.
    Conflict(ConflictKind),
}

/// The kind of conflict between operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictKind {
    /// Two agents modified the same node.
    SameNodeModified { target: DefId },
    /// Move conflicts (same node moved to different targets).
    DivergentMove { target: DefId },
    /// Rename conflicts (same symbol renamed differently).
    DivergentRename { target: DefId, name_a: String, name_b: String },
    /// Type change conflicts (same def changed to different types).
    DivergentType { target: DefId, type_a: String, type_b: String },
    /// Delete vs modify conflict.
    DeleteModify { target: DefId },
    /// Duplicate import.
    DuplicateImport { module: ModuleId, path: String },
}

impl std::fmt::Display for ConflictKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConflictKind::SameNodeModified { target } =>
                write!(f, "same node {target} modified concurrently"),
            ConflictKind::DivergentMove { target } =>
                write!(f, "node {target} moved to different locations"),
            ConflictKind::DivergentRename { target, name_a, name_b } =>
                write!(f, "node {target} renamed to both '{name_a}' and '{name_b}'"),
            ConflictKind::DivergentType { target, type_a, type_b } =>
                write!(f, "node {target} typed as both '{type_a}' and '{type_b}'"),
            ConflictKind::DeleteModify { target } =>
                write!(f, "node {target} deleted and modified concurrently"),
            ConflictKind::DuplicateImport { module, path } =>
                write!(f, "duplicate import '{path}' in module {module}"),
        }
    }
}

// ── Merge Engine ───────────────────────────────────────────────────────────

/// The semantic CRDT merge engine.
pub struct SemanticCRDT {
    /// All applied operations, ordered by timestamp.
    operations: Vec<CrdtOp>,
    /// Set of deleted node DefIds (tombstones).
    tombstones: BTreeSet<DefId>,
    /// Current imports per module (for dedup).
    imports: BTreeMap<ModuleId, BTreeSet<String>>,
    /// Current names per DefId (for rename tracking).
    names: BTreeMap<DefId, String>,
    /// Current types per DefId (for type change tracking).
    types: BTreeMap<DefId, String>,
    /// Current parent per DefId (for move tracking).
    parents: BTreeMap<DefId, ModuleId>,
}

impl SemanticCRDT {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
            tombstones: BTreeSet::new(),
            imports: BTreeMap::new(),
            names: BTreeMap::new(),
            types: BTreeMap::new(),
            parents: BTreeMap::new(),
        }
    }

    /// Apply a single operation, returning Ok or a conflict.
    pub fn apply(&mut self, op: CrdtOp) -> Result<(), ConflictKind> {
        match &op.kind {
            OpKind::InsertNode { parent, name, .. } => {
                // Inserts are always conflict-free (unique IDs).
                self.names.insert(op.id.seq, name.clone());
                self.parents.insert(op.id.seq, *parent);
            }
            OpKind::DeleteNode { target, .. } => {
                if self.tombstones.contains(target) {
                    // Already deleted — idempotent.
                    return Ok(());
                }
                self.tombstones.insert(*target);
            }
            OpKind::MoveNode { target, to_parent, .. } => {
                if self.tombstones.contains(target) {
                    return Err(ConflictKind::DeleteModify { target: *target });
                }
                self.parents.insert(*target, *to_parent);
            }
            OpKind::RenameSymbol { target, new_name, .. } => {
                if self.tombstones.contains(target) {
                    return Err(ConflictKind::DeleteModify { target: *target });
                }
                self.names.insert(*target, new_name.clone());
            }
            OpKind::ChangeType { target, new_type, .. } => {
                if self.tombstones.contains(target) {
                    return Err(ConflictKind::DeleteModify { target: *target });
                }
                self.types.insert(*target, new_type.clone());
            }
            OpKind::AddImport { module, path } => {
                let set = self.imports.entry(*module).or_default();
                if set.contains(path) {
                    return Err(ConflictKind::DuplicateImport {
                        module: *module,
                        path: path.clone(),
                    });
                }
                set.insert(path.clone());
            }
        }
        self.operations.push(op);
        Ok(())
    }

    /// Merge two sets of operations, returning conflicts.
    pub fn merge(ops_a: &[CrdtOp], ops_b: &[CrdtOp]) -> MergeOutcome {
        let mut conflicts = Vec::new();
        let mut applied = Vec::new();

        // Group operations by their target for conflict detection
        for a in ops_a {
            for b in ops_b {
                if let Some(conflict) = Self::check_conflict(a, b) {
                    conflicts.push(MergeConflict {
                        op_a: a.clone(),
                        op_b: b.clone(),
                        kind: conflict,
                    });
                }
            }
        }

        if conflicts.is_empty() {
            // No conflicts: merge by timestamp ordering
            let mut all: Vec<CrdtOp> = ops_a.iter().chain(ops_b.iter()).cloned().collect();
            all.sort_by_key(|op| (op.timestamp, op.id.clone()));
            applied = all;
        }

        MergeOutcome { applied, conflicts }
    }

    /// Merge all operations from multiple agents.
    pub fn merge_all(op_sets: &[&[CrdtOp]]) -> MergeOutcome {
        if op_sets.is_empty() {
            return MergeOutcome { applied: vec![], conflicts: vec![] };
        }
        if op_sets.len() == 1 {
            return MergeOutcome {
                applied: op_sets[0].to_vec(),
                conflicts: vec![],
            };
        }

        let mut result = Self::merge(op_sets[0], op_sets[1]);
        for ops in &op_sets[2..] {
            let next = Self::merge(&result.applied, ops);
            result.conflicts.extend(next.conflicts);
            result.applied = next.applied;
        }
        result
    }

    /// Check if two operations conflict.
    pub fn check_conflict(a: &CrdtOp, b: &CrdtOp) -> Option<ConflictKind> {
        match (&a.kind, &b.kind) {
            // Insert + Insert: always conflict-free (different IDs)
            (OpKind::InsertNode { .. }, OpKind::InsertNode { .. }) => None,

            // Insert + anything else: conflict-free
            (OpKind::InsertNode { .. }, _) | (_, OpKind::InsertNode { .. }) => None,

            // AddImport + AddImport: conflict only if same module + same path
            (
                OpKind::AddImport { module: m1, path: p1 },
                OpKind::AddImport { module: m2, path: p2 },
            ) => {
                if m1 == m2 && p1 == p2 {
                    Some(ConflictKind::DuplicateImport { module: *m1, path: p1.clone() })
                } else {
                    None
                }
            }

            // AddImport + something else: no conflict
            (OpKind::AddImport { .. }, _) | (_, OpKind::AddImport { .. }) => None,

            // Delete + Delete same target: idempotent, no conflict
            (OpKind::DeleteNode { target: t1, .. }, OpKind::DeleteNode { target: t2, .. }) => {
                if t1 == t2 { None } else { None }
            }

            // Delete + Modify/Move/Rename/ChangeType same target: conflict
            (OpKind::DeleteNode { target: t1, .. }, other) => {
                if Self::targets_node(other, *t1) {
                    Some(ConflictKind::DeleteModify { target: *t1 })
                } else {
                    None
                }
            }
            (other, OpKind::DeleteNode { target: t1, .. }) => {
                if Self::targets_node(other, *t1) {
                    Some(ConflictKind::DeleteModify { target: *t1 })
                } else {
                    None
                }
            }

            // Move + Move same target to different parents
            (
                OpKind::MoveNode { target: t1, to_parent: p1, .. },
                OpKind::MoveNode { target: t2, to_parent: p2, .. },
            ) => {
                if t1 == t2 && p1 != p2 {
                    Some(ConflictKind::DivergentMove { target: *t1 })
                } else {
                    None
                }
            }

            // Rename + Rename same target to different names
            (
                OpKind::RenameSymbol { target: t1, new_name: n1, .. },
                OpKind::RenameSymbol { target: t2, new_name: n2, .. },
            ) => {
                if t1 == t2 && n1 != n2 {
                    Some(ConflictKind::DivergentRename {
                        target: *t1,
                        name_a: n1.clone(),
                        name_b: n2.clone(),
                    })
                } else {
                    None
                }
            }

            // ChangeType + ChangeType same target to different types
            (
                OpKind::ChangeType { target: t1, new_type: ty1, .. },
                OpKind::ChangeType { target: t2, new_type: ty2, .. },
            ) => {
                if t1 == t2 && ty1 != ty2 {
                    Some(ConflictKind::DivergentType {
                        target: *t1,
                        type_a: ty1.clone(),
                        type_b: ty2.clone(),
                    })
                } else {
                    None
                }
            }

            // Move + Rename on same target: sequenceable, not conflicting
            (OpKind::MoveNode { target: t1, .. }, OpKind::RenameSymbol { target: t2, .. })
            | (OpKind::RenameSymbol { target: t1, .. }, OpKind::MoveNode { target: t2, .. }) => {
                if t1 == t2 {
                    None // Can be sequenced: rename then move or vice-versa
                } else {
                    None
                }
            }

            // Move + ChangeType on same target: sequenceable
            (OpKind::MoveNode { target: t1, .. }, OpKind::ChangeType { target: t2, .. })
            | (OpKind::ChangeType { target: t1, .. }, OpKind::MoveNode { target: t2, .. }) => {
                if t1 == t2 {
                    None // Can be sequenced
                } else {
                    None
                }
            }

            // Rename + ChangeType on same target: sequenceable
            (OpKind::RenameSymbol { target: t1, .. }, OpKind::ChangeType { target: t2, .. })
            | (OpKind::ChangeType { target: t1, .. }, OpKind::RenameSymbol { target: t2, .. }) => {
                if t1 == t2 {
                    None // Can be sequenced
                } else {
                    None
                }
            }
        }
    }

    /// Check if an operation targets a specific DefId.
    fn targets_node(kind: &OpKind, def_id: DefId) -> bool {
        match kind {
            OpKind::MoveNode { target, .. } => *target == def_id,
            OpKind::RenameSymbol { target, .. } => *target == def_id,
            OpKind::ChangeType { target, .. } => *target == def_id,
            OpKind::DeleteNode { target, .. } => *target == def_id,
            OpKind::InsertNode { .. } | OpKind::AddImport { .. } => false,
        }
    }

    /// Determine the merge result for two operations (proposal §7.3 API).
    pub fn merge_pair(a: &CrdtOp, b: &CrdtOp) -> MergeResult {
        if let Some(conflict) = Self::check_conflict(a, b) {
            MergeResult::Conflict(conflict)
        } else {
            // Check if they need sequencing (e.g. type change before body mod)
            let a_target = Self::op_target(&a.kind);
            let b_target = Self::op_target(&b.kind);
            if a_target.is_some() && a_target == b_target {
                MergeResult::Sequence
            } else {
                MergeResult::BothApply
            }
        }
    }

    fn op_target(kind: &OpKind) -> Option<DefId> {
        match kind {
            OpKind::DeleteNode { target, .. }
            | OpKind::MoveNode { target, .. }
            | OpKind::RenameSymbol { target, .. }
            | OpKind::ChangeType { target, .. } => Some(*target),
            OpKind::InsertNode { .. } | OpKind::AddImport { .. } => None,
        }
    }

    // ── Query methods ──

    /// Get all applied operations.
    pub fn operations(&self) -> &[CrdtOp] {
        &self.operations
    }

    /// Check if a node has been deleted.
    pub fn is_deleted(&self, def_id: DefId) -> bool {
        self.tombstones.contains(&def_id)
    }

    /// Get the current name for a DefId, if tracked.
    pub fn current_name(&self, def_id: DefId) -> Option<&str> {
        self.names.get(&def_id).map(|s| s.as_str())
    }

    /// Get the current type for a DefId, if tracked.
    pub fn current_type(&self, def_id: DefId) -> Option<&str> {
        self.types.get(&def_id).map(|s| s.as_str())
    }

    /// Get imports for a module.
    pub fn imports_for(&self, module: ModuleId) -> Vec<&str> {
        self.imports.get(&module)
            .map(|set| set.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get parent module for a node.
    pub fn parent_of(&self, def_id: DefId) -> Option<ModuleId> {
        self.parents.get(&def_id).copied()
    }
}

impl Default for SemanticCRDT {
    fn default() -> Self {
        Self::new()
    }
}

// ── Merge Outcome ──────────────────────────────────────────────────────────

/// The result of merging operation sets.
#[derive(Debug, Clone)]
pub struct MergeOutcome {
    /// Operations that can be applied (sorted by timestamp).
    pub applied: Vec<CrdtOp>,
    /// Conflicts that need arbitration.
    pub conflicts: Vec<MergeConflict>,
}

impl MergeOutcome {
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }

    /// Format as human-readable text.
    pub fn format_text(&self) -> String {
        let mut out = String::new();
        if self.is_clean() {
            out.push_str(&format!("Clean merge: {} operations applied\n", self.applied.len()));
        } else {
            out.push_str(&format!(
                "Merge with {} conflicts ({} operations applied)\n",
                self.conflicts.len(),
                self.applied.len()
            ));
            for (i, conflict) in self.conflicts.iter().enumerate() {
                out.push_str(&format!(
                    "  Conflict {}: {} vs {} — {}\n",
                    i + 1,
                    conflict.op_a.id,
                    conflict.op_b.id,
                    conflict.kind,
                ));
            }
        }
        out
    }
}

/// A specific merge conflict between two operations.
#[derive(Debug, Clone)]
pub struct MergeConflict {
    pub op_a: CrdtOp,
    pub op_b: CrdtOp,
    pub kind: ConflictKind,
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn op(agent: &str, seq: u64, ts: Timestamp, kind: OpKind) -> CrdtOp {
        CrdtOp { id: NodeId::new(agent, seq), timestamp: ts, kind }
    }

    fn insert(agent: &str, seq: u64, ts: u64, parent: ModuleId, name: &str) -> CrdtOp {
        op(agent, seq, ts, OpKind::InsertNode {
            parent,
            node_type: NodeType::Function,
            name: name.to_string(),
            position: InsertPosition::Append,
        })
    }

    fn delete(agent: &str, seq: u64, ts: u64, target: DefId) -> CrdtOp {
        op(agent, seq, ts, OpKind::DeleteNode { target, old_hash: 0 })
    }

    fn move_node(agent: &str, seq: u64, ts: u64, target: DefId, from: ModuleId, to: ModuleId) -> CrdtOp {
        op(agent, seq, ts, OpKind::MoveNode {
            target, from_parent: from, to_parent: to,
            position: InsertPosition::Append,
        })
    }

    fn rename(agent: &str, seq: u64, ts: u64, target: DefId, old: &str, new_name: &str) -> CrdtOp {
        op(agent, seq, ts, OpKind::RenameSymbol {
            target, old_name: old.to_string(), new_name: new_name.to_string(),
        })
    }

    fn change_type(agent: &str, seq: u64, ts: u64, target: DefId, old: &str, new_type: &str) -> CrdtOp {
        op(agent, seq, ts, OpKind::ChangeType {
            target, old_type: old.to_string(), new_type: new_type.to_string(),
        })
    }

    fn add_import(agent: &str, seq: u64, ts: u64, module: ModuleId, path: &str) -> CrdtOp {
        op(agent, seq, ts, OpKind::AddImport { module, path: path.to_string() })
    }

    // ── Insert tests ──

    #[test]
    fn test_insert_always_conflict_free() {
        let a = insert("alice", 1, 1, 0, "foo");
        let b = insert("bob", 1, 2, 0, "bar");
        assert_eq!(SemanticCRDT::check_conflict(&a, &b), None);
    }

    #[test]
    fn test_insert_merge_both_apply() {
        let a = insert("alice", 1, 1, 0, "foo");
        let b = insert("bob", 1, 2, 0, "bar");
        assert_eq!(SemanticCRDT::merge_pair(&a, &b), MergeResult::BothApply);
    }

    // ── Delete tests ──

    #[test]
    fn test_delete_same_node_no_conflict() {
        let a = delete("alice", 1, 1, 42);
        let b = delete("bob", 2, 2, 42);
        // Deleting same node is idempotent
        assert_eq!(SemanticCRDT::check_conflict(&a, &b), None);
    }

    #[test]
    fn test_delete_vs_rename_conflict() {
        let a = delete("alice", 1, 1, 42);
        let b = rename("bob", 2, 2, 42, "old", "new");
        let conflict = SemanticCRDT::check_conflict(&a, &b).unwrap();
        assert!(matches!(conflict, ConflictKind::DeleteModify { target: 42 }));
    }

    #[test]
    fn test_delete_vs_change_type_conflict() {
        let a = delete("alice", 1, 1, 42);
        let b = change_type("bob", 2, 2, 42, "i32", "i64");
        let conflict = SemanticCRDT::check_conflict(&a, &b).unwrap();
        assert!(matches!(conflict, ConflictKind::DeleteModify { target: 42 }));
    }

    #[test]
    fn test_delete_vs_move_conflict() {
        let a = delete("alice", 1, 1, 42);
        let b = move_node("bob", 2, 2, 42, 0, 1);
        let conflict = SemanticCRDT::check_conflict(&a, &b).unwrap();
        assert!(matches!(conflict, ConflictKind::DeleteModify { target: 42 }));
    }

    // ── Rename tests ──

    #[test]
    fn test_rename_same_target_same_name_no_conflict() {
        let a = rename("alice", 1, 1, 42, "old", "new");
        let b = rename("bob", 2, 2, 42, "old", "new");
        assert_eq!(SemanticCRDT::check_conflict(&a, &b), None);
    }

    #[test]
    fn test_rename_same_target_different_names_conflict() {
        let a = rename("alice", 1, 1, 42, "old", "alpha");
        let b = rename("bob", 2, 2, 42, "old", "beta");
        let conflict = SemanticCRDT::check_conflict(&a, &b).unwrap();
        assert!(matches!(conflict, ConflictKind::DivergentRename { target: 42, .. }));
    }

    #[test]
    fn test_rename_different_targets_no_conflict() {
        let a = rename("alice", 1, 1, 42, "old", "alpha");
        let b = rename("bob", 2, 2, 99, "old", "beta");
        assert_eq!(SemanticCRDT::check_conflict(&a, &b), None);
    }

    // ── Move tests ──

    #[test]
    fn test_move_same_target_same_destination_no_conflict() {
        let a = move_node("alice", 1, 1, 42, 0, 1);
        let b = move_node("bob", 2, 2, 42, 0, 1);
        assert_eq!(SemanticCRDT::check_conflict(&a, &b), None);
    }

    #[test]
    fn test_move_same_target_different_destinations_conflict() {
        let a = move_node("alice", 1, 1, 42, 0, 1);
        let b = move_node("bob", 2, 2, 42, 0, 2);
        let conflict = SemanticCRDT::check_conflict(&a, &b).unwrap();
        assert!(matches!(conflict, ConflictKind::DivergentMove { target: 42 }));
    }

    // ── ChangeType tests ──

    #[test]
    fn test_change_type_same_new_type_no_conflict() {
        let a = change_type("alice", 1, 1, 42, "i32", "i64");
        let b = change_type("bob", 2, 2, 42, "i32", "i64");
        assert_eq!(SemanticCRDT::check_conflict(&a, &b), None);
    }

    #[test]
    fn test_change_type_different_new_types_conflict() {
        let a = change_type("alice", 1, 1, 42, "i32", "i64");
        let b = change_type("bob", 2, 2, 42, "i32", "u32");
        let conflict = SemanticCRDT::check_conflict(&a, &b).unwrap();
        assert!(matches!(conflict, ConflictKind::DivergentType { target: 42, .. }));
    }

    // ── AddImport tests ──

    #[test]
    fn test_import_different_paths_no_conflict() {
        let a = add_import("alice", 1, 1, 0, "std::io");
        let b = add_import("bob", 2, 2, 0, "std::fs");
        assert_eq!(SemanticCRDT::check_conflict(&a, &b), None);
    }

    #[test]
    fn test_import_same_path_same_module_conflict() {
        let a = add_import("alice", 1, 1, 0, "std::io");
        let b = add_import("bob", 2, 2, 0, "std::io");
        let conflict = SemanticCRDT::check_conflict(&a, &b).unwrap();
        assert!(matches!(conflict, ConflictKind::DuplicateImport { .. }));
    }

    #[test]
    fn test_import_same_path_different_modules_no_conflict() {
        let a = add_import("alice", 1, 1, 0, "std::io");
        let b = add_import("bob", 2, 2, 1, "std::io");
        assert_eq!(SemanticCRDT::check_conflict(&a, &b), None);
    }

    // ── merge() tests ──

    #[test]
    fn test_merge_clean() {
        let ops_a = vec![insert("alice", 1, 1, 0, "foo")];
        let ops_b = vec![insert("bob", 1, 2, 0, "bar")];
        let outcome = SemanticCRDT::merge(&ops_a, &ops_b);
        assert!(outcome.is_clean());
        assert_eq!(outcome.applied.len(), 2);
        // Sorted by timestamp
        assert_eq!(outcome.applied[0].timestamp, 1);
        assert_eq!(outcome.applied[1].timestamp, 2);
    }

    #[test]
    fn test_merge_with_conflict() {
        let ops_a = vec![rename("alice", 1, 1, 42, "old", "alpha")];
        let ops_b = vec![rename("bob", 2, 2, 42, "old", "beta")];
        let outcome = SemanticCRDT::merge(&ops_a, &ops_b);
        assert!(!outcome.is_clean());
        assert_eq!(outcome.conflict_count(), 1);
    }

    #[test]
    fn test_merge_all_three_agents() {
        let a = vec![insert("a", 1, 1, 0, "f1")];
        let b = vec![insert("b", 1, 2, 0, "f2")];
        let c = vec![insert("c", 1, 3, 0, "f3")];
        let outcome = SemanticCRDT::merge_all(&[&a, &b, &c]);
        assert!(outcome.is_clean());
        assert_eq!(outcome.applied.len(), 3);
    }

    #[test]
    fn test_merge_all_empty() {
        let outcome = SemanticCRDT::merge_all(&[]);
        assert!(outcome.is_clean());
        assert_eq!(outcome.applied.len(), 0);
    }

    #[test]
    fn test_merge_all_single() {
        let a = vec![insert("a", 1, 1, 0, "f1")];
        let outcome = SemanticCRDT::merge_all(&[&a]);
        assert!(outcome.is_clean());
        assert_eq!(outcome.applied.len(), 1);
    }

    // ── apply() tests ──

    #[test]
    fn test_apply_insert() {
        let mut crdt = SemanticCRDT::new();
        crdt.apply(insert("a", 1, 1, 0, "my_fn")).unwrap();
        assert_eq!(crdt.operations().len(), 1);
        assert_eq!(crdt.current_name(1), Some("my_fn"));
        assert_eq!(crdt.parent_of(1), Some(0));
    }

    #[test]
    fn test_apply_delete() {
        let mut crdt = SemanticCRDT::new();
        crdt.apply(delete("a", 1, 1, 42)).unwrap();
        assert!(crdt.is_deleted(42));
    }

    #[test]
    fn test_apply_delete_idempotent() {
        let mut crdt = SemanticCRDT::new();
        crdt.apply(delete("a", 1, 1, 42)).unwrap();
        crdt.apply(delete("b", 2, 2, 42)).unwrap(); // idempotent
        assert!(crdt.is_deleted(42));
    }

    #[test]
    fn test_apply_rename() {
        let mut crdt = SemanticCRDT::new();
        crdt.apply(rename("a", 1, 1, 42, "old", "new")).unwrap();
        assert_eq!(crdt.current_name(42), Some("new"));
    }

    #[test]
    fn test_apply_rename_deleted_node() {
        let mut crdt = SemanticCRDT::new();
        crdt.apply(delete("a", 1, 1, 42)).unwrap();
        let err = crdt.apply(rename("b", 2, 2, 42, "old", "new")).unwrap_err();
        assert!(matches!(err, ConflictKind::DeleteModify { target: 42 }));
    }

    #[test]
    fn test_apply_change_type() {
        let mut crdt = SemanticCRDT::new();
        crdt.apply(change_type("a", 1, 1, 42, "i32", "i64")).unwrap();
        assert_eq!(crdt.current_type(42), Some("i64"));
    }

    #[test]
    fn test_apply_move() {
        let mut crdt = SemanticCRDT::new();
        crdt.apply(move_node("a", 1, 1, 42, 0, 1)).unwrap();
        assert_eq!(crdt.parent_of(42), Some(1));
    }

    #[test]
    fn test_apply_import() {
        let mut crdt = SemanticCRDT::new();
        crdt.apply(add_import("a", 1, 1, 0, "std::io")).unwrap();
        assert_eq!(crdt.imports_for(0), vec!["std::io"]);
    }

    #[test]
    fn test_apply_duplicate_import_error() {
        let mut crdt = SemanticCRDT::new();
        crdt.apply(add_import("a", 1, 1, 0, "std::io")).unwrap();
        let err = crdt.apply(add_import("b", 2, 2, 0, "std::io")).unwrap_err();
        assert!(matches!(err, ConflictKind::DuplicateImport { .. }));
    }

    // ── merge_pair tests ──

    #[test]
    fn test_merge_pair_both_apply() {
        let a = insert("a", 1, 1, 0, "foo");
        let b = insert("b", 1, 2, 0, "bar");
        assert_eq!(SemanticCRDT::merge_pair(&a, &b), MergeResult::BothApply);
    }

    #[test]
    fn test_merge_pair_sequence() {
        // Rename and change type on same target: sequenceable
        let a = rename("a", 1, 1, 42, "old", "new");
        let b = change_type("b", 2, 2, 42, "i32", "i64");
        assert_eq!(SemanticCRDT::merge_pair(&a, &b), MergeResult::Sequence);
    }

    #[test]
    fn test_merge_pair_conflict() {
        let a = rename("a", 1, 1, 42, "old", "alpha");
        let b = rename("b", 2, 2, 42, "old", "beta");
        assert!(matches!(SemanticCRDT::merge_pair(&a, &b), MergeResult::Conflict(_)));
    }

    // ── Format tests ──

    #[test]
    fn test_merge_outcome_format_clean() {
        let outcome = MergeOutcome { applied: vec![insert("a", 1, 1, 0, "f")], conflicts: vec![] };
        let text = outcome.format_text();
        assert!(text.contains("Clean merge"));
        assert!(text.contains("1 operations"));
    }

    #[test]
    fn test_conflict_kind_display() {
        let c = ConflictKind::DivergentRename { target: 42, name_a: "a".to_string(), name_b: "b".to_string() };
        assert!(c.to_string().contains("'a'"));
        assert!(c.to_string().contains("'b'"));
    }

    #[test]
    fn test_node_id_display() {
        let id = NodeId::new("agent1", 42);
        assert_eq!(id.to_string(), "agent1:42");
    }

    // ── Cross-operation interaction tests ──

    #[test]
    fn test_rename_and_move_different_targets_no_conflict() {
        let a = rename("a", 1, 1, 42, "old", "new");
        let b = move_node("b", 2, 2, 99, 0, 1);
        assert_eq!(SemanticCRDT::check_conflict(&a, &b), None);
    }

    #[test]
    fn test_rename_and_move_same_target_no_conflict() {
        // Rename + move same target: can be sequenced, not conflicting
        let a = rename("a", 1, 1, 42, "old", "new");
        let b = move_node("b", 2, 2, 42, 0, 1);
        assert_eq!(SemanticCRDT::check_conflict(&a, &b), None);
    }

    #[test]
    fn test_default_crdt() {
        let crdt = SemanticCRDT::default();
        assert_eq!(crdt.operations().len(), 0);
    }
}

// ── Semantic VCS ───────────────────────────────────────────────────
//
// Operation-log-based version control for MAGE programs.
//
// Instead of text diffs, history is recorded as structured semantic
// operations (add function, rename field, change contract, etc.).
//
// Components:
//   - `SemanticOp` — fine-grained structured operations
//   - `OpLog`      — append-only log of operations with branch support
//   - `Branch`     — named branch pointer into the log
//   - `SemanticMerge` — three-way merge at operation level
//   - `HistoryQuery`  — intent-based queries over the log

use std::collections::{BTreeMap, BTreeSet};

// ── Semantic operations ────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticOp {
    AddFunction { name: String, signature: String },
    RemoveFunction { name: String },
    RenameFunction { old_name: String, new_name: String },
    ModifyBody { name: String, new_body: String },
    ModifySignature { name: String, new_sig: String },
    AddField { struct_name: String, field_name: String, field_type: String },
    RemoveField { struct_name: String, field_name: String },
    RenameField { struct_name: String, old_name: String, new_name: String },
    AddContract { target: String, contract: String },
    RemoveContract { target: String, contract: String },
    AddImport { path: String },
    RemoveImport { path: String },
    AddStruct { name: String },
    RemoveStruct { name: String },
    ChangeVisibility { target: String, new_vis: String },
    AddEffect { target: String, effect: String },
    RemoveEffect { target: String, effect: String },
}

impl SemanticOp {
    pub fn target_name(&self) -> &str {
        match self {
            SemanticOp::AddFunction { name, .. }
            | SemanticOp::RemoveFunction { name }
            | SemanticOp::ModifyBody { name, .. }
            | SemanticOp::ModifySignature { name, .. } => name,
            SemanticOp::RenameFunction { old_name, .. } => old_name,
            SemanticOp::AddField { struct_name, .. }
            | SemanticOp::RemoveField { struct_name, .. }
            | SemanticOp::RenameField { struct_name, .. } => struct_name,
            SemanticOp::AddContract { target, .. }
            | SemanticOp::RemoveContract { target, .. }
            | SemanticOp::ChangeVisibility { target, .. }
            | SemanticOp::AddEffect { target, .. }
            | SemanticOp::RemoveEffect { target, .. } => target,
            SemanticOp::AddImport { path } | SemanticOp::RemoveImport { path } => path,
            SemanticOp::AddStruct { name } | SemanticOp::RemoveStruct { name } => name,
        }
    }

    /// Human-readable summary of the operation.
    pub fn description(&self) -> String {
        match self {
            SemanticOp::AddFunction { name, .. } => format!("add function `{name}`"),
            SemanticOp::RemoveFunction { name } => format!("remove function `{name}`"),
            SemanticOp::RenameFunction { old_name, new_name } => {
                format!("rename `{old_name}` → `{new_name}`")
            }
            SemanticOp::ModifyBody { name, .. } => format!("modify body of `{name}`"),
            SemanticOp::ModifySignature { name, .. } => format!("modify signature of `{name}`"),
            SemanticOp::AddField { struct_name, field_name, .. } => {
                format!("add field `{field_name}` to `{struct_name}`")
            }
            SemanticOp::RemoveField { struct_name, field_name } => {
                format!("remove field `{field_name}` from `{struct_name}`")
            }
            SemanticOp::RenameField { struct_name, old_name, new_name } => {
                format!("rename field `{old_name}` → `{new_name}` in `{struct_name}`")
            }
            SemanticOp::AddContract { target, contract } => {
                format!("add contract `{contract}` to `{target}`")
            }
            SemanticOp::RemoveContract { target, contract } => {
                format!("remove contract `{contract}` from `{target}`")
            }
            SemanticOp::AddImport { path } => format!("add import `{path}`"),
            SemanticOp::RemoveImport { path } => format!("remove import `{path}`"),
            SemanticOp::AddStruct { name } => format!("add struct `{name}`"),
            SemanticOp::RemoveStruct { name } => format!("remove struct `{name}`"),
            SemanticOp::ChangeVisibility { target, new_vis } => {
                format!("change visibility of `{target}` to `{new_vis}`")
            }
            SemanticOp::AddEffect { target, effect } => {
                format!("add effect `{effect}` to `{target}`")
            }
            SemanticOp::RemoveEffect { target, effect } => {
                format!("remove effect `{effect}` from `{target}`")
            }
        }
    }
}

// ── Commit ─────────────────────────────────────────────────────────

pub type CommitId = u64;

#[derive(Debug, Clone)]
pub struct Commit {
    pub id: CommitId,
    pub parent: Option<CommitId>,
    pub author: String,
    pub message: String,
    pub ops: Vec<SemanticOp>,
    pub timestamp: u64,
}

// ── Branch ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Branch {
    pub name: String,
    pub head: CommitId,
}

// ── OpLog ──────────────────────────────────────────────────────────

pub struct OpLog {
    commits: BTreeMap<CommitId, Commit>,
    branches: BTreeMap<String, Branch>,
    next_id: CommitId,
    default_branch: String,
}

impl OpLog {
    pub fn new() -> Self {
        let mut log = Self {
            commits: BTreeMap::new(),
            branches: BTreeMap::new(),
            next_id: 1,
            default_branch: "main".into(),
        };
        // Create initial empty commit on main.
        let root = Commit {
            id: 0,
            parent: None,
            author: "system".into(),
            message: "initial".into(),
            ops: vec![],
            timestamp: 0,
        };
        log.commits.insert(0, root);
        log.branches.insert("main".into(), Branch { name: "main".into(), head: 0 });
        log
    }

    /// Commit a set of operations to a branch.
    pub fn commit(
        &mut self,
        branch_name: &str,
        author: &str,
        message: &str,
        ops: Vec<SemanticOp>,
        timestamp: u64,
    ) -> Result<CommitId, String> {
        let branch = self.branches.get(branch_name)
            .ok_or_else(|| format!("branch `{branch_name}` not found"))?;
        let parent = branch.head;
        let id = self.next_id;
        self.next_id += 1;
        let commit = Commit {
            id,
            parent: Some(parent),
            author: author.into(),
            message: message.into(),
            ops,
            timestamp,
        };
        self.commits.insert(id, commit);
        self.branches.get_mut(branch_name).unwrap().head = id;
        Ok(id)
    }

    /// Create a new branch from an existing branch's head.
    pub fn create_branch(&mut self, name: &str, from: &str) -> Result<(), String> {
        if self.branches.contains_key(name) {
            return Err(format!("branch `{name}` already exists"));
        }
        let head = self.branches.get(from)
            .ok_or_else(|| format!("source branch `{from}` not found"))?
            .head;
        self.branches.insert(name.into(), Branch { name: name.into(), head });
        Ok(())
    }

    /// List branch names.
    pub fn branch_names(&self) -> Vec<&str> {
        self.branches.keys().map(|s| s.as_str()).collect()
    }

    /// Get a commit by id.
    pub fn get_commit(&self, id: CommitId) -> Option<&Commit> {
        self.commits.get(&id)
    }

    /// Walk the history of a branch (newest first).
    pub fn history(&self, branch_name: &str) -> Vec<&Commit> {
        let mut result = Vec::new();
        let Some(branch) = self.branches.get(branch_name) else { return result };
        let mut cur = Some(branch.head);
        while let Some(id) = cur {
            if let Some(c) = self.commits.get(&id) {
                result.push(c);
                cur = c.parent;
            } else {
                break;
            }
        }
        result
    }

    /// Find the common ancestor of two branches.
    pub fn common_ancestor(&self, branch_a: &str, branch_b: &str) -> Option<CommitId> {
        let ancestors_a: BTreeSet<CommitId> = self.history(branch_a).iter().map(|c| c.id).collect();
        for commit in self.history(branch_b) {
            if ancestors_a.contains(&commit.id) {
                return Some(commit.id);
            }
        }
        None
    }

    /// Collect all ops from `after` (exclusive) to `head` (inclusive) of a branch.
    fn ops_since(&self, branch_name: &str, after: CommitId) -> Vec<SemanticOp> {
        let mut ops = Vec::new();
        for commit in self.history(branch_name) {
            if commit.id == after {
                break;
            }
            // Prepend so ops are in chronological order.
            let mut these = commit.ops.clone();
            these.extend(ops);
            ops = these;
        }
        ops
    }

    /// Three-way semantic merge of `source` into `target`.
    pub fn merge(
        &mut self,
        target: &str,
        source: &str,
        author: &str,
        timestamp: u64,
    ) -> Result<MergeResult, String> {
        let ancestor = self.common_ancestor(target, source)
            .ok_or("no common ancestor")?;

        let target_ops = self.ops_since(target, ancestor);
        let source_ops = self.ops_since(source, ancestor);

        let (merged, conflicts) = semantic_merge(&target_ops, &source_ops);

        let id = self.commit(
            target,
            author,
            &format!("merge {source} into {target}"),
            merged.clone(),
            timestamp,
        )?;

        Ok(MergeResult {
            commit_id: id,
            merged_ops: merged,
            conflicts,
        })
    }

    /// Query history by intent.
    pub fn query(&self, branch_name: &str, query: &HistoryQuery) -> Vec<&Commit> {
        self.history(branch_name)
            .into_iter()
            .filter(|c| query.matches(c))
            .collect()
    }

    /// Total number of commits.
    pub fn commit_count(&self) -> usize {
        self.commits.len()
    }
}

// ── Merge Result ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MergeResult {
    pub commit_id: CommitId,
    pub merged_ops: Vec<SemanticOp>,
    pub conflicts: Vec<MergeConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    pub target_name: String,
    pub ours: SemanticOp,
    pub theirs: SemanticOp,
    pub resolution: ConflictResolution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictResolution {
    TakeOurs,
    TakeTheirs,
    Divergent,
}

// ── Semantic Merge Logic ───────────────────────────────────────────

fn semantic_merge(
    ours: &[SemanticOp],
    theirs: &[SemanticOp],
) -> (Vec<SemanticOp>, Vec<MergeConflict>) {
    let mut merged = Vec::new();
    let mut conflicts = Vec::new();
    let mut theirs_used: BTreeSet<usize> = BTreeSet::new();

    for our_op in ours {
        let mut conflicting = false;
        for (i, their_op) in theirs.iter().enumerate() {
            if theirs_used.contains(&i) {
                continue;
            }
            if our_op.target_name() == their_op.target_name() && our_op != their_op {
                // Conflict: both sides modified same target differently.
                conflicts.push(MergeConflict {
                    target_name: our_op.target_name().to_string(),
                    ours: our_op.clone(),
                    theirs: their_op.clone(),
                    resolution: ConflictResolution::TakeOurs,
                });
                merged.push(our_op.clone()); // default: take ours
                theirs_used.insert(i);
                conflicting = true;
                break;
            } else if our_op == their_op {
                // Duplicate: same op on both sides — keep one.
                theirs_used.insert(i);
                break;
            }
        }
        if !conflicting {
            merged.push(our_op.clone());
        }
    }

    // Add remaining theirs ops not yet used.
    for (i, their_op) in theirs.iter().enumerate() {
        if !theirs_used.contains(&i) {
            merged.push(their_op.clone());
        }
    }

    (merged, conflicts)
}

// ── History Query ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HistoryQuery {
    pub author: Option<String>,
    pub target_name: Option<String>,
    pub op_kind: Option<String>,
    pub message_contains: Option<String>,
}

impl HistoryQuery {
    pub fn new() -> Self {
        Self { author: None, target_name: None, op_kind: None, message_contains: None }
    }

    pub fn by_author(mut self, author: &str) -> Self {
        self.author = Some(author.into());
        self
    }

    pub fn by_target(mut self, target: &str) -> Self {
        self.target_name = Some(target.into());
        self
    }

    pub fn by_message(mut self, pat: &str) -> Self {
        self.message_contains = Some(pat.into());
        self
    }

    fn matches(&self, commit: &Commit) -> bool {
        if let Some(ref a) = self.author {
            if &commit.author != a { return false; }
        }
        if let Some(ref t) = self.target_name {
            if !commit.ops.iter().any(|op| op.target_name() == t) { return false; }
        }
        if let Some(ref m) = self.message_contains {
            if !commit.message.contains(m.as_str()) { return false; }
        }
        true
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> OpLog {
        OpLog::new()
    }

    // ── Basic commit ──────────────────────────────────────────────

    #[test]
    fn commit_on_main() {
        let mut l = log();
        let id = l.commit("main", "alice", "add foo", vec![
            SemanticOp::AddFunction { name: "foo".into(), signature: "fn foo()".into() },
        ], 1).unwrap();
        assert_eq!(id, 1);
        assert_eq!(l.commit_count(), 2); // root + 1
    }

    #[test]
    fn commit_nonexistent_branch() {
        let mut l = log();
        assert!(l.commit("nope", "alice", "x", vec![], 1).is_err());
    }

    // ── History ───────────────────────────────────────────────────

    #[test]
    fn history_order() {
        let mut l = log();
        l.commit("main", "alice", "c1", vec![], 1).unwrap();
        l.commit("main", "alice", "c2", vec![], 2).unwrap();
        let h = l.history("main");
        assert_eq!(h.len(), 3); // root + c1 + c2
        assert_eq!(h[0].message, "c2"); // newest first
        assert_eq!(h[2].message, "initial");
    }

    // ── Branches ──────────────────────────────────────────────────

    #[test]
    fn create_branch() {
        let mut l = log();
        l.commit("main", "a", "c1", vec![], 1).unwrap();
        l.create_branch("feature", "main").unwrap();
        assert!(l.branch_names().contains(&"feature"));
    }

    #[test]
    fn duplicate_branch() {
        let mut l = log();
        assert!(l.create_branch("main", "main").is_err());
    }

    #[test]
    fn branch_independent_history() {
        let mut l = log();
        l.commit("main", "a", "m1", vec![], 1).unwrap();
        l.create_branch("feat", "main").unwrap();
        l.commit("feat", "b", "f1", vec![], 2).unwrap();
        l.commit("main", "a", "m2", vec![], 3).unwrap();
        assert_eq!(l.history("feat").len(), 3); // root + m1 + f1
        assert_eq!(l.history("main").len(), 3); // root + m1 + m2
    }

    // ── Common ancestor ───────────────────────────────────────────

    #[test]
    fn common_ancestor() {
        let mut l = log();
        l.commit("main", "a", "shared", vec![], 1).unwrap();
        l.create_branch("feat", "main").unwrap();
        l.commit("feat", "b", "f1", vec![], 2).unwrap();
        l.commit("main", "a", "m2", vec![], 3).unwrap();
        assert_eq!(l.common_ancestor("main", "feat"), Some(1));
    }

    // ── Merge — clean ─────────────────────────────────────────────

    #[test]
    fn clean_merge() {
        let mut l = log();
        l.commit("main", "a", "shared", vec![], 1).unwrap();
        l.create_branch("feat", "main").unwrap();
        l.commit("feat", "b", "add bar", vec![
            SemanticOp::AddFunction { name: "bar".into(), signature: "fn bar()".into() },
        ], 2).unwrap();
        l.commit("main", "a", "add baz", vec![
            SemanticOp::AddFunction { name: "baz".into(), signature: "fn baz()".into() },
        ], 3).unwrap();
        let result = l.merge("main", "feat", "a", 4).unwrap();
        assert!(result.conflicts.is_empty());
        assert_eq!(result.merged_ops.len(), 2); // baz + bar
    }

    // ── Merge — conflict ──────────────────────────────────────────

    #[test]
    fn conflicting_merge() {
        let mut l = log();
        l.commit("main", "a", "shared", vec![], 1).unwrap();
        l.create_branch("feat", "main").unwrap();
        l.commit("feat", "b", "change foo body (feat)", vec![
            SemanticOp::ModifyBody { name: "foo".into(), new_body: "body_feat".into() },
        ], 2).unwrap();
        l.commit("main", "a", "change foo body (main)", vec![
            SemanticOp::ModifyBody { name: "foo".into(), new_body: "body_main".into() },
        ], 3).unwrap();
        let result = l.merge("main", "feat", "a", 4).unwrap();
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].target_name, "foo");
        assert_eq!(result.conflicts[0].resolution, ConflictResolution::TakeOurs);
    }

    // ── Merge — duplicate ops ─────────────────────────────────────

    #[test]
    fn duplicate_ops_merged_once() {
        let mut l = log();
        l.commit("main", "a", "shared", vec![], 1).unwrap();
        l.create_branch("feat", "main").unwrap();
        let op = SemanticOp::AddImport { path: "std::io".into() };
        l.commit("feat", "b", "import", vec![op.clone()], 2).unwrap();
        l.commit("main", "a", "import", vec![op.clone()], 3).unwrap();
        let result = l.merge("main", "feat", "a", 4).unwrap();
        assert!(result.conflicts.is_empty());
        assert_eq!(result.merged_ops.len(), 1); // deduped
    }

    // ── History query — by author ─────────────────────────────────

    #[test]
    fn query_by_author() {
        let mut l = log();
        l.commit("main", "alice", "c1", vec![], 1).unwrap();
        l.commit("main", "bob", "c2", vec![], 2).unwrap();
        l.commit("main", "alice", "c3", vec![], 3).unwrap();
        let q = HistoryQuery::new().by_author("alice");
        assert_eq!(l.query("main", &q).len(), 2);
    }

    // ── History query — by target ─────────────────────────────────

    #[test]
    fn query_by_target() {
        let mut l = log();
        l.commit("main", "a", "add foo", vec![
            SemanticOp::AddFunction { name: "foo".into(), signature: "fn foo()".into() },
        ], 1).unwrap();
        l.commit("main", "a", "add bar", vec![
            SemanticOp::AddFunction { name: "bar".into(), signature: "fn bar()".into() },
        ], 2).unwrap();
        let q = HistoryQuery::new().by_target("foo");
        let results = l.query("main", &q);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message, "add foo");
    }

    // ── History query — by message ────────────────────────────────

    #[test]
    fn query_by_message() {
        let mut l = log();
        l.commit("main", "a", "fix: resolve crash in parser", vec![], 1).unwrap();
        l.commit("main", "a", "feat: add new lint", vec![], 2).unwrap();
        let q = HistoryQuery::new().by_message("fix:");
        assert_eq!(l.query("main", &q).len(), 1);
    }

    // ── Op descriptions ───────────────────────────────────────────

    #[test]
    fn op_descriptions() {
        let op = SemanticOp::RenameFunction { old_name: "a".into(), new_name: "b".into() };
        assert!(op.description().contains("rename"));
        let op2 = SemanticOp::AddField { struct_name: "S".into(), field_name: "x".into(), field_type: "i32".into() };
        assert!(op2.description().contains("add field"));
    }

    // ── Target name extraction ────────────────────────────────────

    #[test]
    fn target_names() {
        assert_eq!(SemanticOp::AddStruct { name: "Foo".into() }.target_name(), "Foo");
        assert_eq!(SemanticOp::RemoveImport { path: "std::io".into() }.target_name(), "std::io");
        assert_eq!(SemanticOp::ChangeVisibility { target: "bar".into(), new_vis: "pub".into() }.target_name(), "bar");
    }

    // ── Multi-op commit ───────────────────────────────────────────

    #[test]
    fn multi_op_commit() {
        let mut l = log();
        let ops = vec![
            SemanticOp::AddStruct { name: "Config".into() },
            SemanticOp::AddField { struct_name: "Config".into(), field_name: "verbose".into(), field_type: "bool".into() },
            SemanticOp::AddFunction { name: "parse_config".into(), signature: "fn parse_config() -> Config".into() },
        ];
        let id = l.commit("main", "a", "add Config", ops, 1).unwrap();
        let c = l.get_commit(id).unwrap();
        assert_eq!(c.ops.len(), 3);
    }

    // ── Effect ops ────────────────────────────────────────────────

    #[test]
    fn effect_ops() {
        let op = SemanticOp::AddEffect { target: "read_file".into(), effect: "IO".into() };
        assert_eq!(op.target_name(), "read_file");
        assert!(op.description().contains("IO"));
    }
}

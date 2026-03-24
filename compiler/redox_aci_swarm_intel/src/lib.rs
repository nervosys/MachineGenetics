//! # ACI Swarm Coordination Intelligence
//!
//! Learns from swarm history to predict conflicts and optimize task decomposition.
//!
//! Two core capabilities:
//! 1. **Conflict Prediction** — Predicts merge conflicts and semantic interference
//!    between concurrent swarm agents based on historical patterns (file overlap,
//!    symbol dependency, change velocity).
//! 2. **Decomposition Learning** — Learns optimal task decomposition strategies
//!    from past swarm sessions (granularity, dependency ordering, agent assignment).
//!
//! Pipeline:
//! ```text
//! Swarm History ──▶ Feature Extraction ──▶ Conflict Predictor
//!                                      ──▶ Decomposition Scorer
//! ```
//!
//! (ROADMAP Step 65)

use std::collections::{HashMap, HashSet};

// ═══════════════════════════════════════════════════════════════════════════
// Swarm History Types
// ═══════════════════════════════════════════════════════════════════════════

/// A recorded swarm session.
#[derive(Debug, Clone)]
pub struct SwarmSession {
    pub session_id: String,
    pub tasks: Vec<SwarmTask>,
    pub conflicts: Vec<ConflictRecord>,
    pub total_duration_secs: u64,
    pub success: bool,
}

/// A task executed by a swarm agent.
#[derive(Debug, Clone)]
pub struct SwarmTask {
    pub task_id: String,
    pub agent_id: String,
    pub description: String,
    pub files_touched: Vec<String>,
    pub symbols_modified: Vec<String>,
    pub duration_secs: u64,
    pub success: bool,
    /// Tasks this one depended on.
    pub dependencies: Vec<String>,
}

/// A conflict that occurred during a swarm session.
#[derive(Debug, Clone)]
pub struct ConflictRecord {
    pub task_a: String,
    pub task_b: String,
    pub kind: ConflictKind,
    pub file: String,
    pub resolution_secs: u64,
}

/// Types of conflicts between concurrent agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConflictKind {
    /// Both agents modified the same file region.
    MergeConflict,
    /// Semantic interference (e.g., one changed an API the other relies on).
    SemanticInterference,
    /// Resource contention (e.g., both need exclusive lock on a build artifact).
    ResourceContention,
    /// Ordering violation (task ran before its dependency completed).
    OrderingViolation,
}

// ═══════════════════════════════════════════════════════════════════════════
// Conflict Prediction
// ═══════════════════════════════════════════════════════════════════════════

/// Features extracted for a pair of tasks for conflict prediction.
#[derive(Debug, Clone)]
pub struct PairFeatures {
    pub task_a: String,
    pub task_b: String,
    /// Number of files both tasks touch.
    pub file_overlap: usize,
    /// Number of symbols both tasks modify.
    pub symbol_overlap: usize,
    /// Historical conflict rate for this file set.
    pub historical_conflict_rate: f64,
    /// Whether the tasks have a dependency relationship.
    pub has_dependency: bool,
}

/// Extract features for every pair of tasks.
pub fn extract_pair_features(tasks: &[SwarmTask], history: &[SwarmSession]) -> Vec<PairFeatures> {
    let historical_rates = compute_historical_rates(history);
    let mut features = Vec::new();

    for i in 0..tasks.len() {
        for j in (i + 1)..tasks.len() {
            let a = &tasks[i];
            let b = &tasks[j];

            let a_files: HashSet<&str> = a.files_touched.iter().map(|s| s.as_str()).collect();
            let b_files: HashSet<&str> = b.files_touched.iter().map(|s| s.as_str()).collect();
            let file_overlap = a_files.intersection(&b_files).count();

            let a_syms: HashSet<&str> = a.symbols_modified.iter().map(|s| s.as_str()).collect();
            let b_syms: HashSet<&str> = b.symbols_modified.iter().map(|s| s.as_str()).collect();
            let symbol_overlap = a_syms.intersection(&b_syms).count();

            let mut rate_sum = 0.0;
            let mut rate_count = 0;
            for file in a_files.intersection(&b_files) {
                if let Some(r) = historical_rates.get(*file) {
                    rate_sum += r;
                    rate_count += 1;
                }
            }
            let historical_conflict_rate =
                if rate_count > 0 { rate_sum / rate_count as f64 } else { 0.0 };

            let has_dependency =
                a.dependencies.contains(&b.task_id) || b.dependencies.contains(&a.task_id);

            features.push(PairFeatures {
                task_a: a.task_id.clone(),
                task_b: b.task_id.clone(),
                file_overlap,
                symbol_overlap,
                historical_conflict_rate,
                has_dependency,
            });
        }
    }
    features
}

/// Compute per-file historical conflict rate from swarm history.
fn compute_historical_rates(history: &[SwarmSession]) -> HashMap<String, f64> {
    let mut file_touches: HashMap<String, u64> = HashMap::new();
    let mut file_conflicts: HashMap<String, u64> = HashMap::new();

    for session in history {
        for task in &session.tasks {
            for file in &task.files_touched {
                *file_touches.entry(file.clone()).or_insert(0) += 1;
            }
        }
        for conflict in &session.conflicts {
            *file_conflicts.entry(conflict.file.clone()).or_insert(0) += 1;
        }
    }

    let mut rates = HashMap::new();
    for (file, touches) in &file_touches {
        let conflicts = file_conflicts.get(file).copied().unwrap_or(0);
        if *touches > 0 {
            rates.insert(file.clone(), conflicts as f64 / *touches as f64);
        }
    }
    rates
}

/// Predicted conflict between two tasks.
#[derive(Debug, Clone)]
pub struct ConflictPrediction {
    pub task_a: String,
    pub task_b: String,
    pub probability: f64,
    pub likely_kind: ConflictKind,
    pub risk_factors: Vec<String>,
}

/// Predict conflicts from pair features using a weighted heuristic model.
pub fn predict_conflicts(features: &[PairFeatures]) -> Vec<ConflictPrediction> {
    let mut predictions = Vec::new();

    for pair in features {
        let mut prob = 0.0;
        let mut factors = Vec::new();

        // File overlap is the strongest conflict predictor.
        if pair.file_overlap > 0 {
            let file_score = (pair.file_overlap as f64 * 0.15).min(0.6);
            prob += file_score;
            factors.push(format!("{} overlapping files", pair.file_overlap));
        }

        // Symbol overlap: very likely semantic conflict.
        if pair.symbol_overlap > 0 {
            let sym_score = (pair.symbol_overlap as f64 * 0.25).min(0.5);
            prob += sym_score;
            factors.push(format!("{} overlapping symbols", pair.symbol_overlap));
        }

        // Historical rate factor.
        if pair.historical_conflict_rate > 0.0 {
            prob += pair.historical_conflict_rate * 0.3;
            factors.push(format!(
                "historical conflict rate {:.0}%",
                pair.historical_conflict_rate * 100.0
            ));
        }

        // Dependency relationship reduces merge conflict risk (ordered) but
        // increases ordering violation risk if not respected.
        if pair.has_dependency {
            prob *= 0.5;
            factors.push("dependency relationship (mitigating)".to_string());
        }

        prob = prob.min(1.0);

        if prob > 0.05 {
            let likely_kind = if pair.symbol_overlap > 0 {
                ConflictKind::SemanticInterference
            } else if pair.file_overlap > 0 {
                ConflictKind::MergeConflict
            } else {
                ConflictKind::ResourceContention
            };

            predictions.push(ConflictPrediction {
                task_a: pair.task_a.clone(),
                task_b: pair.task_b.clone(),
                probability: prob,
                likely_kind,
                risk_factors: factors,
            });
        }
    }

    predictions.sort_by(|a, b| {
        b.probability.partial_cmp(&a.probability).unwrap_or(std::cmp::Ordering::Equal)
    });
    predictions
}

// ═══════════════════════════════════════════════════════════════════════════
// Decomposition Learning
// ═══════════════════════════════════════════════════════════════════════════

/// Strategy for decomposing work.
#[derive(Debug, Clone)]
pub struct DecompositionStrategy {
    pub task_groups: Vec<TaskGroup>,
    pub estimated_duration_secs: u64,
    pub estimated_conflict_prob: f64,
    pub parallelism_factor: f64,
    pub score: f64,
}

/// A group of tasks that should run together / sequentially.
#[derive(Debug, Clone)]
pub struct TaskGroup {
    pub group_id: usize,
    pub task_ids: Vec<String>,
    /// Groups that must complete before this one.
    pub depends_on: Vec<usize>,
}

/// Score a decomposition strategy based on historical outcomes.
pub fn score_decomposition(
    tasks: &[SwarmTask],
    groups: &[TaskGroup],
    history: &[SwarmSession],
) -> DecompositionStrategy {
    let avg_historical_duration = if history.is_empty() {
        600 // default 10 minutes
    } else {
        history.iter().map(|s| s.total_duration_secs).sum::<u64>() / history.len() as u64
    };

    // Parallelism factor: how many groups can run concurrently
    let max_depth = compute_group_depth(groups);
    let parallelism_factor =
        if max_depth > 0 { groups.len() as f64 / max_depth as f64 } else { 1.0 };

    // Estimated duration: sequential time / parallelism
    let total_task_time: u64 = tasks.iter().map(|t| t.duration_secs).sum();
    let estimated_duration = if parallelism_factor > 0.0 {
        (total_task_time as f64 / parallelism_factor) as u64
    } else {
        total_task_time
    };

    // Estimate conflict probability from pair analysis
    let features = extract_pair_features(tasks, history);
    let predictions = predict_conflicts(&features);
    let max_conflict_prob = predictions.iter().map(|p| p.probability).fold(0.0_f64, f64::max);

    // Score: reward parallelism, penalize conflicts and duration
    let time_ratio = if avg_historical_duration > 0 {
        estimated_duration as f64 / avg_historical_duration as f64
    } else {
        1.0
    };
    let score = parallelism_factor * (1.0 - max_conflict_prob) / (1.0 + time_ratio);

    DecompositionStrategy {
        task_groups: groups.to_vec(),
        estimated_duration_secs: estimated_duration,
        estimated_conflict_prob: max_conflict_prob,
        parallelism_factor,
        score,
    }
}

/// Compute the critical-path depth of a group DAG.
fn compute_group_depth(groups: &[TaskGroup]) -> usize {
    let mut depths: HashMap<usize, usize> = HashMap::new();

    fn depth_of(gid: usize, groups: &[TaskGroup], cache: &mut HashMap<usize, usize>) -> usize {
        if let Some(d) = cache.get(&gid) {
            return *d;
        }
        let group = match groups.iter().find(|g| g.group_id == gid) {
            Some(g) => g,
            None => return 1,
        };
        let max_dep =
            group.depends_on.iter().map(|dep| depth_of(*dep, groups, cache)).max().unwrap_or(0);
        let d = max_dep + 1;
        cache.insert(gid, d);
        d
    }

    for group in groups {
        depth_of(group.group_id, groups, &mut depths);
    }
    depths.values().copied().max().unwrap_or(0)
}

/// Suggest an improved decomposition from history.
pub fn suggest_decomposition(
    tasks: &[SwarmTask],
    history: &[SwarmSession],
) -> DecompositionStrategy {
    // Build file-affinity clusters: tasks touching the same files go together
    let mut file_to_tasks: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, task) in tasks.iter().enumerate() {
        for file in &task.files_touched {
            file_to_tasks.entry(file.clone()).or_default().push(i);
        }
    }

    // Union-find style clustering
    let n = tasks.len();
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut [usize], x: usize) -> usize {
        let mut r = x;
        while parent[r] != r {
            r = parent[r];
        }
        // Path compression
        let mut c = x;
        while c != r {
            let next = parent[c];
            parent[c] = r;
            c = next;
        }
        r
    }

    for indices in file_to_tasks.values() {
        if indices.len() > 1 {
            let root = find(&mut parent, indices[0]);
            for idx in &indices[1..] {
                let r = find(&mut parent, *idx);
                if r != root {
                    parent[r] = root;
                }
            }
        }
    }

    // Build groups from clusters
    let mut cluster_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        cluster_map.entry(root).or_default().push(i);
    }

    let mut groups: Vec<TaskGroup> = Vec::new();
    let mut task_to_group: HashMap<String, usize> = HashMap::new();

    for (gid, (_root, members)) in cluster_map.into_iter().enumerate() {
        let task_ids: Vec<String> = members.iter().map(|i| tasks[*i].task_id.clone()).collect();
        for tid in &task_ids {
            task_to_group.insert(tid.clone(), gid);
        }
        groups.push(TaskGroup { group_id: gid, task_ids, depends_on: Vec::new() });
    }

    // Add inter-group dependencies
    for task in tasks {
        if let Some(my_group) = task_to_group.get(&task.task_id) {
            for dep_id in &task.dependencies {
                if let Some(dep_group) = task_to_group.get(dep_id) {
                    if dep_group != my_group && !groups[*my_group].depends_on.contains(dep_group) {
                        groups[*my_group].depends_on.push(*dep_group);
                    }
                }
            }
        }
    }

    score_decomposition(tasks, &groups, history)
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(id: &str, files: &[&str], symbols: &[&str], deps: &[&str]) -> SwarmTask {
        SwarmTask {
            task_id: id.to_string(),
            agent_id: format!("agent-{id}"),
            description: format!("Task {id}"),
            files_touched: files.iter().map(|f| f.to_string()).collect(),
            symbols_modified: symbols.iter().map(|s| s.to_string()).collect(),
            duration_secs: 60,
            success: true,
            dependencies: deps.iter().map(|d| d.to_string()).collect(),
        }
    }

    fn make_history() -> Vec<SwarmSession> {
        vec![SwarmSession {
            session_id: "s1".to_string(),
            tasks: vec![
                make_task("h1", &["main.mg", "util.mg"], &["parse"], &[]),
                make_task("h2", &["main.mg"], &["compile"], &[]),
            ],
            conflicts: vec![ConflictRecord {
                task_a: "h1".to_string(),
                task_b: "h2".to_string(),
                kind: ConflictKind::MergeConflict,
                file: "main.mg".to_string(),
                resolution_secs: 30,
            }],
            total_duration_secs: 300,
            success: true,
        }]
    }

    // ── Historical Rates ─────────────────────────────────────────────────

    #[test]
    fn historical_rates() {
        let history = make_history();
        let rates = compute_historical_rates(&history);
        assert!(rates["main.mg"] > 0.0);
        assert_eq!(rates.get("util.mg").copied().unwrap_or(0.0), 0.0);
    }

    // ── Feature Extraction ───────────────────────────────────────────────

    #[test]
    fn pair_features_file_overlap() {
        let tasks = vec![
            make_task("a", &["f1.mg", "f2.mg"], &[], &[]),
            make_task("b", &["f2.mg", "f3.mg"], &[], &[]),
        ];
        let features = extract_pair_features(&tasks, &[]);
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].file_overlap, 1);
    }

    #[test]
    fn pair_features_symbol_overlap() {
        let tasks = vec![
            make_task("a", &["f1.mg"], &["foo", "bar"], &[]),
            make_task("b", &["f2.mg"], &["bar", "baz"], &[]),
        ];
        let features = extract_pair_features(&tasks, &[]);
        assert_eq!(features[0].symbol_overlap, 1);
    }

    #[test]
    fn pair_features_no_overlap() {
        let tasks = vec![
            make_task("a", &["f1.mg"], &["x"], &[]),
            make_task("b", &["f2.mg"], &["y"], &[]),
        ];
        let features = extract_pair_features(&tasks, &[]);
        assert_eq!(features[0].file_overlap, 0);
        assert_eq!(features[0].symbol_overlap, 0);
    }

    #[test]
    fn pair_features_with_history() {
        let tasks =
            vec![make_task("a", &["main.mg"], &[], &[]), make_task("b", &["main.mg"], &[], &[])];
        let history = make_history();
        let features = extract_pair_features(&tasks, &history);
        assert!(features[0].historical_conflict_rate > 0.0);
    }

    #[test]
    fn pair_features_dependency() {
        let tasks =
            vec![make_task("a", &["f1.mg"], &[], &[]), make_task("b", &["f1.mg"], &[], &["a"])];
        let features = extract_pair_features(&tasks, &[]);
        assert!(features[0].has_dependency);
    }

    // ── Conflict Prediction ──────────────────────────────────────────────

    #[test]
    fn predict_file_conflict() {
        let tasks = vec![
            make_task("a", &["f1.mg", "f2.mg"], &[], &[]),
            make_task("b", &["f2.mg", "f3.mg"], &[], &[]),
        ];
        let features = extract_pair_features(&tasks, &[]);
        let preds = predict_conflicts(&features);
        assert_eq!(preds.len(), 1);
        assert_eq!(preds[0].likely_kind, ConflictKind::MergeConflict);
        assert!(preds[0].probability > 0.0);
    }

    #[test]
    fn predict_symbol_conflict() {
        let tasks = vec![
            make_task("a", &["f1.mg"], &["foo"], &[]),
            make_task("b", &["f1.mg"], &["foo"], &[]),
        ];
        let features = extract_pair_features(&tasks, &[]);
        let preds = predict_conflicts(&features);
        assert_eq!(preds[0].likely_kind, ConflictKind::SemanticInterference);
    }

    #[test]
    fn predict_no_conflict() {
        let tasks = vec![
            make_task("a", &["f1.mg"], &["x"], &[]),
            make_task("b", &["f2.mg"], &["y"], &[]),
        ];
        let features = extract_pair_features(&tasks, &[]);
        let preds = predict_conflicts(&features);
        assert!(preds.is_empty());
    }

    #[test]
    fn dependency_reduces_probability() {
        let tasks_no_dep = vec![
            make_task("a", &["f1.mg"], &["foo"], &[]),
            make_task("b", &["f1.mg"], &["foo"], &[]),
        ];
        let tasks_dep = vec![
            make_task("a", &["f1.mg"], &["foo"], &[]),
            make_task("b", &["f1.mg"], &["foo"], &["a"]),
        ];
        let f1 = extract_pair_features(&tasks_no_dep, &[]);
        let f2 = extract_pair_features(&tasks_dep, &[]);
        let p1 = predict_conflicts(&f1);
        let p2 = predict_conflicts(&f2);
        assert!(p2[0].probability < p1[0].probability);
    }

    #[test]
    fn predictions_sorted_by_probability() {
        let tasks = vec![
            make_task("a", &["f1.mg", "f2.mg"], &["x", "y"], &[]),
            make_task("b", &["f1.mg"], &[], &[]),
            make_task("c", &["f1.mg", "f2.mg"], &["x", "y", "z"], &[]),
        ];
        let features = extract_pair_features(&tasks, &[]);
        let preds = predict_conflicts(&features);
        for w in preds.windows(2) {
            assert!(w[0].probability >= w[1].probability);
        }
    }

    // ── Group Depth ──────────────────────────────────────────────────────

    #[test]
    fn group_depth_single() {
        let groups =
            vec![TaskGroup { group_id: 0, task_ids: vec!["a".to_string()], depends_on: vec![] }];
        assert_eq!(compute_group_depth(&groups), 1);
    }

    #[test]
    fn group_depth_chain() {
        let groups = vec![
            TaskGroup { group_id: 0, task_ids: vec!["a".to_string()], depends_on: vec![] },
            TaskGroup { group_id: 1, task_ids: vec!["b".to_string()], depends_on: vec![0] },
            TaskGroup { group_id: 2, task_ids: vec!["c".to_string()], depends_on: vec![1] },
        ];
        assert_eq!(compute_group_depth(&groups), 3);
    }

    #[test]
    fn group_depth_parallel() {
        let groups = vec![
            TaskGroup { group_id: 0, task_ids: vec!["a".to_string()], depends_on: vec![] },
            TaskGroup { group_id: 1, task_ids: vec!["b".to_string()], depends_on: vec![] },
            TaskGroup { group_id: 2, task_ids: vec!["c".to_string()], depends_on: vec![0, 1] },
        ];
        assert_eq!(compute_group_depth(&groups), 2);
    }

    // ── Decomposition Scoring ────────────────────────────────────────────

    #[test]
    fn score_parallel_better_than_serial() {
        let tasks =
            vec![make_task("a", &["f1.mg"], &[], &[]), make_task("b", &["f2.mg"], &[], &[])];
        let parallel = vec![
            TaskGroup { group_id: 0, task_ids: vec!["a".to_string()], depends_on: vec![] },
            TaskGroup { group_id: 1, task_ids: vec!["b".to_string()], depends_on: vec![] },
        ];
        let serial = vec![
            TaskGroup { group_id: 0, task_ids: vec!["a".to_string()], depends_on: vec![] },
            TaskGroup { group_id: 1, task_ids: vec!["b".to_string()], depends_on: vec![0] },
        ];
        let sp = score_decomposition(&tasks, &parallel, &[]);
        let ss = score_decomposition(&tasks, &serial, &[]);
        assert!(sp.score > ss.score, "parallel decomposition should score higher");
    }

    #[test]
    fn score_with_history() {
        let tasks =
            vec![make_task("a", &["main.mg"], &[], &[]), make_task("b", &["main.mg"], &[], &[])];
        let groups = vec![TaskGroup {
            group_id: 0,
            task_ids: vec!["a".to_string(), "b".to_string()],
            depends_on: vec![],
        }];
        let history = make_history();
        let strategy = score_decomposition(&tasks, &groups, &history);
        assert!(strategy.estimated_conflict_prob > 0.0);
    }

    // ── Suggest Decomposition ────────────────────────────────────────────

    #[test]
    fn suggest_clusters_overlapping_files() {
        let tasks = vec![
            make_task("a", &["f1.mg", "f2.mg"], &[], &[]),
            make_task("b", &["f2.mg", "f3.mg"], &[], &[]),
            make_task("c", &["f4.mg"], &[], &[]),
        ];
        let strategy = suggest_decomposition(&tasks, &[]);
        // a and b should be in the same group (share f2.mg), c separate
        assert!(strategy.task_groups.len() >= 2);
        let c_group =
            strategy.task_groups.iter().find(|g| g.task_ids.contains(&"c".to_string())).unwrap();
        assert!(!c_group.task_ids.contains(&"a".to_string()));
    }

    #[test]
    fn suggest_all_independent() {
        let tasks = vec![
            make_task("a", &["f1.mg"], &[], &[]),
            make_task("b", &["f2.mg"], &[], &[]),
            make_task("c", &["f3.mg"], &[], &[]),
        ];
        let strategy = suggest_decomposition(&tasks, &[]);
        assert_eq!(strategy.task_groups.len(), 3);
        assert!(strategy.parallelism_factor > 1.0);
    }

    #[test]
    fn suggest_all_overlapping() {
        let tasks = vec![
            make_task("a", &["f1.mg"], &[], &[]),
            make_task("b", &["f1.mg"], &[], &[]),
            make_task("c", &["f1.mg"], &[], &[]),
        ];
        let strategy = suggest_decomposition(&tasks, &[]);
        assert_eq!(strategy.task_groups.len(), 1);
    }

    #[test]
    fn suggest_with_dependencies() {
        let tasks =
            vec![make_task("a", &["f1.mg"], &[], &[]), make_task("b", &["f2.mg"], &[], &["a"])];
        let strategy = suggest_decomposition(&tasks, &[]);
        // Different files → different groups, but b depends on a's group
        if strategy.task_groups.len() > 1 {
            let b_group = strategy
                .task_groups
                .iter()
                .find(|g| g.task_ids.contains(&"b".to_string()))
                .unwrap();
            assert!(!b_group.depends_on.is_empty());
        }
    }

    #[test]
    fn suggest_empty_tasks() {
        let strategy = suggest_decomposition(&[], &[]);
        assert!(strategy.task_groups.is_empty());
    }

    // ── Conflict Kind ────────────────────────────────────────────────────

    #[test]
    fn conflict_kind_eq() {
        assert_eq!(ConflictKind::MergeConflict, ConflictKind::MergeConflict);
        assert_ne!(ConflictKind::MergeConflict, ConflictKind::SemanticInterference);
    }
}

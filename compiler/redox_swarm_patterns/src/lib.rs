// redox_swarm_patterns: Swarm orchestration pattern library —
// map-reduce, pipeline, scatter-gather, saga.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Pattern kind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatternKind {
    MapReduce,
    Pipeline,
    ScatterGather,
    Saga,
}

impl PatternKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::MapReduce => "map-reduce",
            Self::Pipeline => "pipeline",
            Self::ScatterGather => "scatter-gather",
            Self::Saga => "saga",
        }
    }

    pub fn all() -> &'static [PatternKind] {
        &[Self::MapReduce, Self::Pipeline, Self::ScatterGather, Self::Saga]
    }
}

// ---------------------------------------------------------------------------
// Stage / Step
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    pub name: String,
    pub agent_role: String,
    pub timeout_ms: u64,
}

// ---------------------------------------------------------------------------
// Pattern definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternDef {
    pub name: String,
    pub kind: PatternKind,
    pub stages: Vec<Stage>,
    pub description: String,
    pub tags: Vec<String>,
}

impl PatternDef {
    pub fn new(name: &str, kind: PatternKind) -> Self {
        Self {
            name: name.to_string(),
            kind,
            stages: Vec::new(),
            description: String::new(),
            tags: Vec::new(),
        }
    }

    pub fn add_stage(&mut self, name: &str, agent_role: &str, timeout_ms: u64) {
        self.stages.push(Stage {
            name: name.to_string(),
            agent_role: agent_role.to_string(),
            timeout_ms,
        });
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn total_timeout(&self) -> u64 {
        self.stages.iter().map(|s| s.timeout_ms).sum()
    }
}

// ---------------------------------------------------------------------------
// Execution status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Compensating,
    Compensated,
}

impl StageStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Compensating => "compensating",
            Self::Compensated => "compensated",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Compensated)
    }
}

// ---------------------------------------------------------------------------
// Execution record
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageExec {
    pub stage_name: String,
    pub status: StageStatus,
    pub duration_ms: u64,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternExec {
    pub pattern_name: String,
    pub kind: PatternKind,
    pub stages: Vec<StageExec>,
}

impl PatternExec {
    pub fn new(pattern: &PatternDef) -> Self {
        let stages = pattern.stages.iter().map(|s| StageExec {
            stage_name: s.name.clone(),
            status: StageStatus::Pending,
            duration_ms: 0,
            output: String::new(),
        }).collect();
        Self { pattern_name: pattern.name.clone(), kind: pattern.kind, stages }
    }

    pub fn complete_stage(&mut self, index: usize, duration_ms: u64, output: &str) {
        if let Some(s) = self.stages.get_mut(index) {
            s.status = StageStatus::Completed;
            s.duration_ms = duration_ms;
            s.output = output.to_string();
        }
    }

    pub fn fail_stage(&mut self, index: usize, duration_ms: u64, output: &str) {
        if let Some(s) = self.stages.get_mut(index) {
            s.status = StageStatus::Failed;
            s.duration_ms = duration_ms;
            s.output = output.to_string();
        }
    }

    pub fn is_complete(&self) -> bool {
        self.stages.iter().all(|s| s.status == StageStatus::Completed)
    }

    pub fn has_failure(&self) -> bool {
        self.stages.iter().any(|s| s.status == StageStatus::Failed)
    }

    pub fn total_duration(&self) -> u64 {
        self.stages.iter().map(|s| s.duration_ms).sum()
    }

    pub fn completed_count(&self) -> usize {
        self.stages.iter().filter(|s| s.status == StageStatus::Completed).count()
    }
}

// ---------------------------------------------------------------------------
// Saga compensation
// ---------------------------------------------------------------------------

pub fn compensate_saga(exec: &mut PatternExec) {
    if exec.kind != PatternKind::Saga { return; }

    // Walk backwards from the failure point, marking completed stages as compensating/compensated
    let fail_idx = exec.stages.iter().position(|s| s.status == StageStatus::Failed);
    if let Some(fi) = fail_idx {
        for i in (0..fi).rev() {
            if exec.stages[i].status == StageStatus::Completed {
                exec.stages[i].status = StageStatus::Compensated;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pattern library
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PatternLibrary {
    patterns: Vec<PatternDef>,
}

impl PatternLibrary {
    pub fn new() -> Self {
        Self { patterns: Vec::new() }
    }

    pub fn add(&mut self, pattern: PatternDef) {
        self.patterns.push(pattern);
    }

    pub fn by_kind(&self, kind: PatternKind) -> Vec<&PatternDef> {
        self.patterns.iter().filter(|p| p.kind == kind).collect()
    }

    pub fn by_name(&self, name: &str) -> Option<&PatternDef> {
        self.patterns.iter().find(|p| p.name == name)
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&PatternDef> {
        self.patterns.iter().filter(|p| p.tags.iter().any(|t| t == tag)).collect()
    }

    pub fn all(&self) -> &[PatternDef] {
        &self.patterns
    }

    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn stats(&self) -> LibraryStats {
        let mut by_kind: HashMap<PatternKind, usize> = HashMap::new();
        for p in &self.patterns {
            *by_kind.entry(p.kind).or_insert(0) += 1;
        }
        LibraryStats {
            total: self.patterns.len(),
            by_kind,
        }
    }
}

impl Default for PatternLibrary {
    fn default() -> Self { Self::new() }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryStats {
    pub total: usize,
    pub by_kind: HashMap<PatternKind, usize>,
}

// ---------------------------------------------------------------------------
// Pre-built patterns
// ---------------------------------------------------------------------------

pub fn map_reduce_pattern() -> PatternDef {
    let mut p = PatternDef::new("Standard Map-Reduce", PatternKind::MapReduce);
    p.description = "Split work across agents, then merge results.".to_string();
    p.tags = vec!["parallel".into(), "data".into()];
    p.add_stage("split", "coordinator", 500);
    p.add_stage("map", "worker", 5000);
    p.add_stage("reduce", "aggregator", 2000);
    p
}

pub fn pipeline_pattern() -> PatternDef {
    let mut p = PatternDef::new("Standard Pipeline", PatternKind::Pipeline);
    p.description = "Sequential stages with hand-off.".to_string();
    p.tags = vec!["sequential".into(), "transform".into()];
    p.add_stage("parse", "parser", 1000);
    p.add_stage("analyse", "analyser", 3000);
    p.add_stage("codegen", "generator", 2000);
    p.add_stage("verify", "checker", 1500);
    p
}

pub fn scatter_gather_pattern() -> PatternDef {
    let mut p = PatternDef::new("Standard Scatter-Gather", PatternKind::ScatterGather);
    p.description = "Fan-out queries to multiple agents, gather best result.".to_string();
    p.tags = vec!["parallel".into(), "consensus".into()];
    p.add_stage("scatter", "coordinator", 500);
    p.add_stage("query", "specialist", 3000);
    p.add_stage("gather", "coordinator", 1000);
    p
}

pub fn saga_pattern() -> PatternDef {
    let mut p = PatternDef::new("Standard Saga", PatternKind::Saga);
    p.description = "Multi-step transaction with compensating actions on failure.".to_string();
    p.tags = vec!["transaction".into(), "reliable".into()];
    p.add_stage("reserve", "allocator", 1000);
    p.add_stage("execute", "worker", 5000);
    p.add_stage("commit", "coordinator", 1000);
    p
}

pub fn build_standard_library() -> PatternLibrary {
    let mut lib = PatternLibrary::new();
    lib.add(map_reduce_pattern());
    lib.add(pipeline_pattern());
    lib.add(scatter_gather_pattern());
    lib.add(saga_pattern());
    lib
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- PatternKind --
    #[test]
    fn test_pattern_kind_labels() {
        assert_eq!(PatternKind::MapReduce.label(), "map-reduce");
        assert_eq!(PatternKind::Saga.label(), "saga");
    }

    #[test]
    fn test_pattern_kind_all() {
        assert_eq!(PatternKind::all().len(), 4);
    }

    // -- PatternDef --
    #[test]
    fn test_pattern_def_stages() {
        let p = pipeline_pattern();
        assert_eq!(p.stage_count(), 4);
    }

    #[test]
    fn test_total_timeout() {
        let p = pipeline_pattern();
        assert_eq!(p.total_timeout(), 7500);
    }

    // -- StageStatus --
    #[test]
    fn test_stage_status_terminal() {
        assert!(StageStatus::Completed.is_terminal());
        assert!(StageStatus::Failed.is_terminal());
        assert!(!StageStatus::Running.is_terminal());
    }

    #[test]
    fn test_stage_status_label() {
        assert_eq!(StageStatus::Pending.label(), "pending");
    }

    // -- PatternExec --
    #[test]
    fn test_pattern_exec_new() {
        let p = map_reduce_pattern();
        let exec = PatternExec::new(&p);
        assert_eq!(exec.stages.len(), 3);
        assert!(exec.stages.iter().all(|s| s.status == StageStatus::Pending));
    }

    #[test]
    fn test_complete_stage() {
        let p = map_reduce_pattern();
        let mut exec = PatternExec::new(&p);
        exec.complete_stage(0, 100, "done");
        assert_eq!(exec.stages[0].status, StageStatus::Completed);
        assert_eq!(exec.stages[0].duration_ms, 100);
    }

    #[test]
    fn test_fail_stage() {
        let p = map_reduce_pattern();
        let mut exec = PatternExec::new(&p);
        exec.fail_stage(1, 50, "error");
        assert!(exec.has_failure());
    }

    #[test]
    fn test_is_complete() {
        let p = map_reduce_pattern();
        let mut exec = PatternExec::new(&p);
        for i in 0..3 { exec.complete_stage(i, 100, "ok"); }
        assert!(exec.is_complete());
    }

    #[test]
    fn test_total_duration() {
        let p = map_reduce_pattern();
        let mut exec = PatternExec::new(&p);
        exec.complete_stage(0, 100, "ok");
        exec.complete_stage(1, 200, "ok");
        assert_eq!(exec.total_duration(), 300);
    }

    #[test]
    fn test_completed_count() {
        let p = map_reduce_pattern();
        let mut exec = PatternExec::new(&p);
        exec.complete_stage(0, 100, "ok");
        assert_eq!(exec.completed_count(), 1);
    }

    // -- Saga compensation --
    #[test]
    fn test_saga_compensation() {
        let p = saga_pattern();
        let mut exec = PatternExec::new(&p);
        exec.complete_stage(0, 100, "reserved");
        exec.fail_stage(1, 50, "execution failed");
        compensate_saga(&mut exec);
        assert_eq!(exec.stages[0].status, StageStatus::Compensated);
        assert_eq!(exec.stages[1].status, StageStatus::Failed);
    }

    #[test]
    fn test_saga_compensation_no_failure() {
        let p = saga_pattern();
        let mut exec = PatternExec::new(&p);
        exec.complete_stage(0, 100, "ok");
        exec.complete_stage(1, 100, "ok");
        compensate_saga(&mut exec);
        // no changes since no failure
        assert_eq!(exec.stages[0].status, StageStatus::Completed);
    }

    #[test]
    fn test_compensate_non_saga() {
        let p = map_reduce_pattern();
        let mut exec = PatternExec::new(&p);
        exec.fail_stage(0, 50, "err");
        compensate_saga(&mut exec);
        // no-op for non-saga
        assert_eq!(exec.stages[0].status, StageStatus::Failed);
    }

    // -- PatternLibrary --
    #[test]
    fn test_library_empty() {
        let lib = PatternLibrary::new();
        assert!(lib.is_empty());
    }

    #[test]
    fn test_standard_library() {
        let lib = build_standard_library();
        assert_eq!(lib.len(), 4);
    }

    #[test]
    fn test_by_kind() {
        let lib = build_standard_library();
        assert_eq!(lib.by_kind(PatternKind::Saga).len(), 1);
    }

    #[test]
    fn test_by_name() {
        let lib = build_standard_library();
        assert!(lib.by_name("Standard Pipeline").is_some());
        assert!(lib.by_name("nonexistent").is_none());
    }

    #[test]
    fn test_by_tag() {
        let lib = build_standard_library();
        let parallel = lib.by_tag("parallel");
        assert_eq!(parallel.len(), 2); // map-reduce + scatter-gather
    }

    #[test]
    fn test_library_stats() {
        let lib = build_standard_library();
        let s = lib.stats();
        assert_eq!(s.total, 4);
        assert_eq!(s.by_kind.len(), 4);
    }

    #[test]
    fn test_default_library() {
        let lib = PatternLibrary::default();
        assert!(lib.is_empty());
    }
}

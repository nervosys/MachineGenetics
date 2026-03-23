// redox_self_heal: Self-healing compiler — auto-repair pipeline
// with confidence ranking.
//
// Detects compilation errors, proposes ranked repair candidates,
// applies the best fix, and validates the result.

// ---------------------------------------------------------------------------
// Error kind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    TypeMismatch,
    UndefinedVariable,
    MissingImport,
    SyntaxError,
    BorrowCheck,
    LifetimeError,
    UnusedVariable,
    AmbiguousType,
}

impl ErrorKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::TypeMismatch => "type-mismatch",
            Self::UndefinedVariable => "undefined-variable",
            Self::MissingImport => "missing-import",
            Self::SyntaxError => "syntax-error",
            Self::BorrowCheck => "borrow-check",
            Self::LifetimeError => "lifetime-error",
            Self::UnusedVariable => "unused-variable",
            Self::AmbiguousType => "ambiguous-type",
        }
    }

    pub fn severity(self) -> Severity {
        match self {
            Self::SyntaxError | Self::TypeMismatch | Self::BorrowCheck | Self::LifetimeError
                => Severity::Error,
            Self::UndefinedVariable | Self::MissingImport | Self::AmbiguousType
                => Severity::Error,
            Self::UnusedVariable => Severity::Warning,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Warning,
    Error,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

// ---------------------------------------------------------------------------
// Compiler diagnostic
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub kind: ErrorKind,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Repair candidate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct RepairCandidate {
    pub description: String,
    pub patch: String,
    pub confidence: f64, // 0.0 .. 1.0
    pub kind: ErrorKind,
}

// ---------------------------------------------------------------------------
// Repair result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairOutcome {
    Fixed,
    PartialFix,
    NoFix,
    Regressed,
}

impl RepairOutcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::PartialFix => "partial-fix",
            Self::NoFix => "no-fix",
            Self::Regressed => "regressed",
        }
    }

    pub fn is_success(self) -> bool {
        matches!(self, Self::Fixed | Self::PartialFix)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RepairResult {
    pub diagnostic: Diagnostic,
    pub candidate: RepairCandidate,
    pub outcome: RepairOutcome,
    pub remaining_errors: u32,
}

// ---------------------------------------------------------------------------
// Repair strategy
// ---------------------------------------------------------------------------

pub type StrategyFn = fn(&Diagnostic) -> Vec<RepairCandidate>;

pub fn strategy_type_mismatch(diag: &Diagnostic) -> Vec<RepairCandidate> {
    vec![
        RepairCandidate {
            description: format!("Insert type cast at {}:{}", diag.file, diag.line),
            patch: format!("/* cast at line {} */", diag.line),
            confidence: 0.85,
            kind: ErrorKind::TypeMismatch,
        },
        RepairCandidate {
            description: format!("Change variable type at {}:{}", diag.file, diag.line),
            patch: format!("/* retype at line {} */", diag.line),
            confidence: 0.65,
            kind: ErrorKind::TypeMismatch,
        },
    ]
}

pub fn strategy_missing_import(diag: &Diagnostic) -> Vec<RepairCandidate> {
    vec![RepairCandidate {
        description: format!("Add missing import for {}", diag.message),
        patch: format!("use {};", diag.message),
        confidence: 0.95,
        kind: ErrorKind::MissingImport,
    }]
}

pub fn strategy_undefined_variable(diag: &Diagnostic) -> Vec<RepairCandidate> {
    vec![
        RepairCandidate {
            description: "Declare variable".to_string(),
            patch: format!("let {} = Default::default();", diag.message),
            confidence: 0.50,
            kind: ErrorKind::UndefinedVariable,
        },
    ]
}

pub fn strategy_unused_variable(diag: &Diagnostic) -> Vec<RepairCandidate> {
    vec![RepairCandidate {
        description: format!("Prefix with underscore: _{}", diag.message),
        patch: format!("_{}", diag.message),
        confidence: 0.99,
        kind: ErrorKind::UnusedVariable,
    }]
}

pub fn default_strategies() -> Vec<(ErrorKind, StrategyFn)> {
    vec![
        (ErrorKind::TypeMismatch, strategy_type_mismatch),
        (ErrorKind::MissingImport, strategy_missing_import),
        (ErrorKind::UndefinedVariable, strategy_undefined_variable),
        (ErrorKind::UnusedVariable, strategy_unused_variable),
    ]
}

// ---------------------------------------------------------------------------
// Self-healing pipeline
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct HealingPipeline {
    strategies: Vec<(ErrorKind, StrategyFn)>,
    history: Vec<RepairResult>,
    max_iterations: u32,
}

impl HealingPipeline {
    pub fn new() -> Self {
        Self {
            strategies: default_strategies(),
            history: Vec::new(),
            max_iterations: 10,
        }
    }

    pub fn with_max_iterations(mut self, n: u32) -> Self {
        self.max_iterations = n;
        self
    }

    pub fn add_strategy(&mut self, kind: ErrorKind, f: StrategyFn) {
        self.strategies.push((kind, f));
    }

    /// Propose repair candidates for a diagnostic, sorted by confidence (highest first).
    pub fn propose(&self, diag: &Diagnostic) -> Vec<RepairCandidate> {
        let mut candidates = Vec::new();
        for (kind, strategy) in &self.strategies {
            if *kind == diag.kind {
                candidates.extend(strategy(diag));
            }
        }
        candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        candidates
    }

    /// Simulate applying the top candidate and record the result.
    pub fn apply_top(&mut self, diag: &Diagnostic) -> Option<RepairResult> {
        let candidates = self.propose(diag);
        let candidate = candidates.into_iter().next()?;

        let outcome = if candidate.confidence >= 0.8 {
            RepairOutcome::Fixed
        } else if candidate.confidence >= 0.5 {
            RepairOutcome::PartialFix
        } else {
            RepairOutcome::NoFix
        };

        let result = RepairResult {
            diagnostic: diag.clone(),
            candidate,
            outcome,
            remaining_errors: if outcome == RepairOutcome::Fixed { 0 } else { 1 },
        };

        self.history.push(result.clone());
        Some(result)
    }

    /// Run the pipeline over a list of diagnostics.
    pub fn heal_all(&mut self, diagnostics: &[Diagnostic]) -> Vec<RepairResult> {
        let mut results = Vec::new();
        for (i, diag) in diagnostics.iter().enumerate() {
            if i as u32 >= self.max_iterations { break; }
            if let Some(r) = self.apply_top(diag) {
                results.push(r);
            }
        }
        results
    }

    pub fn history(&self) -> &[RepairResult] {
        &self.history
    }

    pub fn success_count(&self) -> usize {
        self.history.iter().filter(|r| r.outcome.is_success()).count()
    }

    pub fn success_rate(&self) -> f64 {
        if self.history.is_empty() { return 0.0; }
        self.success_count() as f64 / self.history.len() as f64 * 100.0
    }

    pub fn summary(&self) -> PipelineSummary {
        PipelineSummary {
            total_repairs: self.history.len(),
            fixed: self.history.iter().filter(|r| r.outcome == RepairOutcome::Fixed).count(),
            partial: self.history.iter().filter(|r| r.outcome == RepairOutcome::PartialFix).count(),
            no_fix: self.history.iter().filter(|r| r.outcome == RepairOutcome::NoFix).count(),
            regressed: self.history.iter().filter(|r| r.outcome == RepairOutcome::Regressed).count(),
        }
    }
}

impl Default for HealingPipeline {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineSummary {
    pub total_repairs: usize,
    pub fixed: usize,
    pub partial: usize,
    pub no_fix: usize,
    pub regressed: usize,
}

pub fn format_summary(s: &PipelineSummary) -> String {
    format!(
        "repairs={} fixed={} partial={} no_fix={} regressed={}",
        s.total_repairs, s.fixed, s.partial, s.no_fix, s.regressed,
    )
}

// ---------------------------------------------------------------------------
// Pre-built example
// ---------------------------------------------------------------------------

pub fn sample_diagnostics() -> Vec<Diagnostic> {
    vec![
        Diagnostic { file: "main.rs".into(), line: 10, column: 5, kind: ErrorKind::TypeMismatch, message: "expected i32, found &str".into() },
        Diagnostic { file: "lib.rs".into(), line: 1, column: 1, kind: ErrorKind::MissingImport, message: "std::collections::HashMap".into() },
        Diagnostic { file: "lib.rs".into(), line: 25, column: 9, kind: ErrorKind::UnusedVariable, message: "x".into() },
        Diagnostic { file: "util.rs".into(), line: 42, column: 12, kind: ErrorKind::UndefinedVariable, message: "result".into() },
    ]
}

pub fn build_sample_pipeline() -> HealingPipeline {
    HealingPipeline::new()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- ErrorKind --
    #[test]
    fn test_error_kind_labels() {
        assert_eq!(ErrorKind::TypeMismatch.label(), "type-mismatch");
        assert_eq!(ErrorKind::BorrowCheck.label(), "borrow-check");
    }

    #[test]
    fn test_error_severity() {
        assert_eq!(ErrorKind::SyntaxError.severity(), Severity::Error);
        assert_eq!(ErrorKind::UnusedVariable.severity(), Severity::Warning);
    }

    #[test]
    fn test_severity_label() {
        assert_eq!(Severity::Error.label(), "error");
        assert_eq!(Severity::Warning.label(), "warning");
    }

    // -- RepairOutcome --
    #[test]
    fn test_outcome_label() {
        assert_eq!(RepairOutcome::Fixed.label(), "fixed");
        assert_eq!(RepairOutcome::Regressed.label(), "regressed");
    }

    #[test]
    fn test_outcome_is_success() {
        assert!(RepairOutcome::Fixed.is_success());
        assert!(RepairOutcome::PartialFix.is_success());
        assert!(!RepairOutcome::NoFix.is_success());
        assert!(!RepairOutcome::Regressed.is_success());
    }

    // -- Strategies --
    #[test]
    fn test_strategy_type_mismatch() {
        let diag = Diagnostic { file: "a.rs".into(), line: 1, column: 1, kind: ErrorKind::TypeMismatch, message: "".into() };
        let candidates = strategy_type_mismatch(&diag);
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn test_strategy_missing_import() {
        let diag = Diagnostic { file: "a.rs".into(), line: 1, column: 1, kind: ErrorKind::MissingImport, message: "HashMap".into() };
        let candidates = strategy_missing_import(&diag);
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].confidence > 0.9);
    }

    #[test]
    fn test_strategy_unused() {
        let diag = Diagnostic { file: "a.rs".into(), line: 1, column: 1, kind: ErrorKind::UnusedVariable, message: "x".into() };
        let candidates = strategy_unused_variable(&diag);
        assert!(candidates[0].patch.contains("_x"));
    }

    // -- HealingPipeline --
    #[test]
    fn test_propose_sorted_by_confidence() {
        let pipeline = HealingPipeline::new();
        let diag = Diagnostic { file: "a.rs".into(), line: 1, column: 1, kind: ErrorKind::TypeMismatch, message: "".into() };
        let candidates = pipeline.propose(&diag);
        assert!(candidates.len() >= 2);
        assert!(candidates[0].confidence >= candidates[1].confidence);
    }

    #[test]
    fn test_propose_no_match() {
        let pipeline = HealingPipeline::new();
        let diag = Diagnostic { file: "a.rs".into(), line: 1, column: 1, kind: ErrorKind::BorrowCheck, message: "".into() };
        let candidates = pipeline.propose(&diag);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_apply_top() {
        let mut pipeline = HealingPipeline::new();
        let diag = Diagnostic { file: "a.rs".into(), line: 1, column: 1, kind: ErrorKind::MissingImport, message: "HashMap".into() };
        let result = pipeline.apply_top(&diag).unwrap();
        assert_eq!(result.outcome, RepairOutcome::Fixed);
    }

    #[test]
    fn test_apply_top_no_strategy() {
        let mut pipeline = HealingPipeline::new();
        let diag = Diagnostic { file: "a.rs".into(), line: 1, column: 1, kind: ErrorKind::LifetimeError, message: "".into() };
        assert!(pipeline.apply_top(&diag).is_none());
    }

    #[test]
    fn test_heal_all() {
        let mut pipeline = HealingPipeline::new();
        let diags = sample_diagnostics();
        let results = pipeline.heal_all(&diags);
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_heal_all_max_iterations() {
        let mut pipeline = HealingPipeline::new().with_max_iterations(2);
        let diags = sample_diagnostics();
        let results = pipeline.heal_all(&diags);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_history() {
        let mut pipeline = HealingPipeline::new();
        let diag = Diagnostic { file: "a.rs".into(), line: 1, column: 1, kind: ErrorKind::UnusedVariable, message: "x".into() };
        pipeline.apply_top(&diag);
        assert_eq!(pipeline.history().len(), 1);
    }

    #[test]
    fn test_success_count() {
        let mut pipeline = HealingPipeline::new();
        let diags = sample_diagnostics();
        pipeline.heal_all(&diags);
        assert!(pipeline.success_count() > 0);
    }

    #[test]
    fn test_success_rate() {
        let mut pipeline = HealingPipeline::new();
        let diags = sample_diagnostics();
        pipeline.heal_all(&diags);
        assert!(pipeline.success_rate() > 0.0);
    }

    #[test]
    fn test_summary() {
        let mut pipeline = HealingPipeline::new();
        let diags = sample_diagnostics();
        pipeline.heal_all(&diags);
        let s = pipeline.summary();
        assert_eq!(s.total_repairs, 4);
    }

    #[test]
    fn test_format_summary() {
        let s = PipelineSummary { total_repairs: 4, fixed: 2, partial: 1, no_fix: 1, regressed: 0 };
        let text = format_summary(&s);
        assert!(text.contains("repairs=4"));
    }

    #[test]
    fn test_default_strategies_len() {
        let strats = default_strategies();
        assert_eq!(strats.len(), 4);
    }

    #[test]
    fn test_default_pipeline() {
        let pipeline = HealingPipeline::default();
        assert_eq!(pipeline.history().len(), 0);
    }

    #[test]
    fn test_sample_diagnostics() {
        let diags = sample_diagnostics();
        assert_eq!(diags.len(), 4);
    }
}

//! # ACI Dynamic Warning Engine
//!
//! ML-based warning generation that learns from a project's bug history and
//! swarm session outcomes. Unlike static lints, these warnings adapt to the
//! specific patterns that have caused bugs *in this project*.
//!
//! Pipeline:
//! ```text
//! Bug History ──┐
//!               ├──▶ Pattern Extractor ──▶ Warning Rules ──▶ Code Analyzer ──▶ Warnings
//! Swarm History ┘        ▲                                        │
//!                        └── Feedback (false-positive suppression)─┘
//! ```
//!
//! Reference: REDOX_PROPOSAL.md — ACI Dynamic Warning Engine
//!   "learns from project bug history + swarm sessions"
//!
//! (ROADMAP Step 62)

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// Bug Patterns
// ═══════════════════════════════════════════════════════════════════════════

/// Category of bug pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BugCategory {
    /// Off-by-one errors in loops / indexing.
    OffByOne,
    /// Null / None dereference.
    NullDeref,
    /// Race condition or data race.
    RaceCondition,
    /// Resource leak (file, memory, connection).
    ResourceLeak,
    /// Integer overflow / underflow.
    IntegerOverflow,
    /// Type confusion or incorrect cast.
    TypeConfusion,
    /// Logic error (wrong condition, inverted check).
    LogicError,
    /// Concurrency deadlock.
    Deadlock,
    /// API misuse (wrong argument order, missing init).
    ApiMisuse,
    /// Performance regression.
    PerfRegression,
}

impl fmt::Display for BugCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BugCategory::OffByOne => write!(f, "off-by-one"),
            BugCategory::NullDeref => write!(f, "null-deref"),
            BugCategory::RaceCondition => write!(f, "race-condition"),
            BugCategory::ResourceLeak => write!(f, "resource-leak"),
            BugCategory::IntegerOverflow => write!(f, "integer-overflow"),
            BugCategory::TypeConfusion => write!(f, "type-confusion"),
            BugCategory::LogicError => write!(f, "logic-error"),
            BugCategory::Deadlock => write!(f, "deadlock"),
            BugCategory::ApiMisuse => write!(f, "api-misuse"),
            BugCategory::PerfRegression => write!(f, "perf-regression"),
        }
    }
}

/// A historical bug record used for pattern learning.
#[derive(Debug, Clone)]
pub struct BugRecord {
    pub id: u64,
    pub category: BugCategory,
    /// Code pattern that caused the bug (snippet or AST signature).
    pub trigger_pattern: String,
    /// Description of the bug.
    pub description: String,
    /// How the bug was fixed (for suggestion generation).
    pub fix_description: String,
    /// Severity: 1 (low) to 5 (critical).
    pub severity: u8,
    /// Number of times this pattern has recurred.
    pub occurrences: u32,
}

impl BugRecord {
    pub fn new(id: u64, category: BugCategory, trigger: &str, description: &str) -> Self {
        BugRecord {
            id,
            category,
            trigger_pattern: trigger.to_string(),
            description: description.to_string(),
            fix_description: String::new(),
            severity: 3,
            occurrences: 1,
        }
    }

    pub fn with_fix(mut self, fix: &str) -> Self {
        self.fix_description = fix.to_string();
        self
    }

    pub fn with_severity(mut self, severity: u8) -> Self {
        self.severity = severity.clamp(1, 5);
        self
    }

    pub fn with_occurrences(mut self, n: u32) -> Self {
        self.occurrences = n;
        self
    }
}

/// Swarm session outcome used for warning calibration.
#[derive(Debug, Clone)]
pub struct SwarmSessionRecord {
    pub session_id: String,
    /// Warnings that were generated during this session.
    pub warnings_generated: Vec<String>,
    /// Warnings that led to actual bugs being found.
    pub warnings_that_caught_bugs: Vec<String>,
    /// Warnings that were false positives.
    pub false_positives: Vec<String>,
}

impl SwarmSessionRecord {
    pub fn precision(&self) -> f64 {
        if self.warnings_generated.is_empty() {
            return 1.0;
        }
        self.warnings_that_caught_bugs.len() as f64 / self.warnings_generated.len() as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Warning Rule Engine
// ═══════════════════════════════════════════════════════════════════════════

/// A learned warning rule derived from bug patterns.
#[derive(Debug, Clone)]
pub struct WarningRule {
    pub id: String,
    pub category: BugCategory,
    /// Pattern to match in code (keyword-based for simulation).
    pub trigger_keywords: Vec<String>,
    /// Human-readable warning message.
    pub message: String,
    /// Suggested fix.
    pub suggestion: String,
    /// Confidence that this rule is accurate (0.0–1.0).
    pub confidence: f64,
    /// Base severity from bug history.
    pub severity: u8,
    /// Number of historical bugs this rule covers.
    pub evidence_count: u32,
    /// False positive rate from swarm feedback.
    pub false_positive_rate: f64,
}

impl WarningRule {
    /// Effective priority: higher = more important.
    pub fn priority(&self) -> f64 {
        let sev = self.severity as f64;
        let conf = self.confidence;
        let fp_penalty = 1.0 - self.false_positive_rate;
        let evidence = (self.evidence_count as f64).ln().max(1.0);
        sev * conf * fp_penalty * evidence
    }

    /// Whether this rule should be suppressed due to high false-positive rate.
    pub fn is_suppressed(&self) -> bool {
        self.false_positive_rate > 0.7
    }
}

impl fmt::Display for WarningRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} (confidence: {:.0}%, severity: {})",
            self.category,
            self.message,
            self.confidence * 100.0,
            self.severity
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Pattern Extractor
// ═══════════════════════════════════════════════════════════════════════════

/// Extract warning rules from bug history.
pub fn extract_warning_rules(bugs: &[BugRecord]) -> Vec<WarningRule> {
    // Group bugs by category and extract patterns
    let mut by_category: HashMap<BugCategory, Vec<&BugRecord>> = HashMap::new();
    for bug in bugs {
        by_category.entry(bug.category).or_default().push(bug);
    }

    let mut rules = Vec::new();
    for (category, records) in &by_category {
        let total_occurrences: u32 = records.iter().map(|r| r.occurrences).sum();
        let max_severity = records.iter().map(|r| r.severity).max().unwrap_or(3);

        // Collect all trigger keywords across records
        let mut keywords: Vec<String> = Vec::new();
        for record in records {
            for word in record.trigger_pattern.split_whitespace() {
                let w = word.to_lowercase();
                if w.len() > 2 && !keywords.contains(&w) {
                    keywords.push(w);
                }
            }
        }

        // Confidence scales with evidence
        let confidence = (total_occurrences as f64 / 10.0).min(0.95).max(0.2);

        let message = format!(
            "Potential {} — {} historical occurrences in this project",
            category, total_occurrences
        );

        let suggestion = records
            .iter()
            .find(|r| !r.fix_description.is_empty())
            .map(|r| r.fix_description.clone())
            .unwrap_or_else(|| format!("Review for {category} patterns"));

        rules.push(WarningRule {
            id: format!("aci-warn-{category}"),
            category: *category,
            trigger_keywords: keywords,
            message,
            suggestion,
            confidence,
            severity: max_severity,
            evidence_count: total_occurrences,
            false_positive_rate: 0.0,
        });
    }

    // Sort by priority (highest first)
    rules.sort_by(|a, b| {
        b.priority().partial_cmp(&a.priority()).unwrap_or(std::cmp::Ordering::Equal)
    });
    rules
}

/// Calibrate rules using swarm session feedback.
pub fn calibrate_with_swarm_feedback(rules: &mut [WarningRule], sessions: &[SwarmSessionRecord]) {
    for rule in rules.iter_mut() {
        let rule_id = &rule.id;
        let mut total_generated = 0u32;
        let mut total_fp = 0u32;

        for session in sessions {
            let generated =
                session.warnings_generated.iter().filter(|w| w.contains(rule_id.as_str())).count()
                    as u32;
            let fp = session.false_positives.iter().filter(|w| w.contains(rule_id.as_str())).count()
                as u32;
            total_generated += generated;
            total_fp += fp;
        }

        if total_generated > 0 {
            rule.false_positive_rate = total_fp as f64 / total_generated as f64;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Code Analyzer
// ═══════════════════════════════════════════════════════════════════════════

/// A warning emitted by the dynamic warning engine.
#[derive(Debug, Clone)]
pub struct DynamicWarning {
    pub rule_id: String,
    pub category: BugCategory,
    pub message: String,
    pub suggestion: String,
    pub severity: u8,
    pub confidence: f64,
    pub location: CodeLocation,
}

impl DynamicWarning {
    pub fn is_actionable(&self) -> bool {
        self.confidence > 0.3 && !self.suggestion.is_empty()
    }
}

impl fmt::Display for DynamicWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "warning[{}]: {} at {} (confidence: {:.0}%)",
            self.category,
            self.message,
            self.location,
            self.confidence * 100.0
        )
    }
}

/// Location in source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl CodeLocation {
    pub fn new(file: &str, line: u32, column: u32) -> Self {
        CodeLocation { file: file.to_string(), line, column }
    }
}

impl fmt::Display for CodeLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

/// A code snippet to analyze.
#[derive(Debug, Clone)]
pub struct CodeSnippet {
    pub file: String,
    pub start_line: u32,
    pub content: String,
}

impl CodeSnippet {
    pub fn new(file: &str, start_line: u32, content: &str) -> Self {
        CodeSnippet { file: file.to_string(), start_line, content: content.to_string() }
    }
}

/// Analyze a code snippet against learned warning rules.
pub fn analyze_code(snippet: &CodeSnippet, rules: &[WarningRule]) -> Vec<DynamicWarning> {
    let mut warnings = Vec::new();
    let content_lower = snippet.content.to_lowercase();

    for rule in rules {
        if rule.is_suppressed() {
            continue;
        }

        // Check if any trigger keywords match
        let matching_keywords: Vec<&String> =
            rule.trigger_keywords.iter().filter(|kw| content_lower.contains(kw.as_str())).collect();

        if matching_keywords.is_empty() {
            continue;
        }

        // More keyword matches → higher confidence
        let match_ratio =
            matching_keywords.len() as f64 / rule.trigger_keywords.len().max(1) as f64;
        let effective_confidence = rule.confidence * match_ratio;

        // Find approximate line of first match
        let line_offset = content_lower
            .lines()
            .enumerate()
            .find(|(_, line)| matching_keywords.iter().any(|kw| line.contains(kw.as_str())))
            .map(|(i, _)| i as u32)
            .unwrap_or(0);

        warnings.push(DynamicWarning {
            rule_id: rule.id.clone(),
            category: rule.category,
            message: rule.message.clone(),
            suggestion: rule.suggestion.clone(),
            severity: rule.severity,
            confidence: effective_confidence,
            location: CodeLocation::new(&snippet.file, snippet.start_line + line_offset, 1),
        });
    }

    // Sort by severity (highest first), then confidence
    warnings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal))
    });

    warnings
}

// ═══════════════════════════════════════════════════════════════════════════
// Warning Engine (Full Pipeline)
// ═══════════════════════════════════════════════════════════════════════════

/// Configuration for the warning engine.
#[derive(Debug, Clone)]
pub struct WarningEngineConfig {
    /// Minimum confidence threshold for emitting warnings.
    pub min_confidence: f64,
    /// Maximum number of warnings per file.
    pub max_warnings_per_file: usize,
    /// Whether to include suggestions.
    pub include_suggestions: bool,
    /// Minimum severity (1–5) to emit.
    pub min_severity: u8,
}

impl Default for WarningEngineConfig {
    fn default() -> Self {
        WarningEngineConfig {
            min_confidence: 0.2,
            max_warnings_per_file: 50,
            include_suggestions: true,
            min_severity: 1,
        }
    }
}

/// The dynamic warning engine state.
#[derive(Debug)]
pub struct WarningEngine {
    pub rules: Vec<WarningRule>,
    pub config: WarningEngineConfig,
    /// Feedback: rule_id → (true_positives, false_positives).
    feedback: HashMap<String, (u32, u32)>,
}

impl WarningEngine {
    /// Build from bug history and optional swarm feedback.
    pub fn build(
        bugs: &[BugRecord],
        sessions: &[SwarmSessionRecord],
        config: WarningEngineConfig,
    ) -> Self {
        let mut rules = extract_warning_rules(bugs);
        if !sessions.is_empty() {
            calibrate_with_swarm_feedback(&mut rules, sessions);
        }
        WarningEngine { rules, config, feedback: HashMap::new() }
    }

    /// Analyze a code snippet.
    pub fn analyze(&self, snippet: &CodeSnippet) -> Vec<DynamicWarning> {
        let raw = analyze_code(snippet, &self.rules);
        raw.into_iter()
            .filter(|w| w.confidence >= self.config.min_confidence)
            .filter(|w| w.severity >= self.config.min_severity)
            .take(self.config.max_warnings_per_file)
            .collect()
    }

    /// Analyze multiple snippets (e.g., all files in a project).
    pub fn analyze_all(&self, snippets: &[CodeSnippet]) -> Vec<DynamicWarning> {
        snippets.iter().flat_map(|s| self.analyze(s)).collect()
    }

    /// Record feedback: was this warning a true positive or false positive?
    pub fn record_feedback(&mut self, rule_id: &str, is_true_positive: bool) {
        let entry = self.feedback.entry(rule_id.to_string()).or_insert((0, 0));
        if is_true_positive {
            entry.0 += 1;
        } else {
            entry.1 += 1;
        }

        // Update rule's false positive rate
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == rule_id) {
            let (tp, fp) = self.feedback[rule_id];
            let total = tp + fp;
            if total > 0 {
                rule.false_positive_rate = fp as f64 / total as f64;
            }
        }
    }

    /// Get summary statistics.
    pub fn stats(&self) -> WarningEngineStats {
        let active_rules = self.rules.iter().filter(|r| !r.is_suppressed()).count();
        let suppressed_rules = self.rules.len() - active_rules;
        let total_evidence: u32 = self.rules.iter().map(|r| r.evidence_count).sum();

        WarningEngineStats {
            total_rules: self.rules.len(),
            active_rules,
            suppressed_rules,
            total_evidence,
            categories: self.rules.iter().map(|r| r.category).collect(),
        }
    }
}

/// Summary statistics for the warning engine.
#[derive(Debug)]
pub struct WarningEngineStats {
    pub total_rules: usize,
    pub active_rules: usize,
    pub suppressed_rules: usize,
    pub total_evidence: u32,
    pub categories: Vec<BugCategory>,
}

impl WarningEngineStats {
    pub fn unique_categories(&self) -> usize {
        let mut cats = self.categories.clone();
        cats.sort_by_key(|c| format!("{c}"));
        cats.dedup();
        cats.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bugs() -> Vec<BugRecord> {
        vec![
            BugRecord::new(1, BugCategory::OffByOne, "loop index len minus", "off-by-one in loop")
                .with_fix("Use < len instead of <= len")
                .with_severity(4)
                .with_occurrences(5),
            BugRecord::new(2, BugCategory::NullDeref, "unwrap option none", "null deref on unwrap")
                .with_fix("Use match or if-let instead of unwrap")
                .with_severity(5)
                .with_occurrences(3),
            BugRecord::new(
                3,
                BugCategory::RaceCondition,
                "shared mutex lock unlock",
                "race in shared state",
            )
            .with_fix("Hold lock for entire critical section")
            .with_severity(5)
            .with_occurrences(2),
            BugRecord::new(
                4,
                BugCategory::ResourceLeak,
                "open file close drop",
                "file handle leak",
            )
            .with_fix("Use RAII pattern or explicit close")
            .with_severity(3)
            .with_occurrences(4),
        ]
    }

    fn sample_sessions() -> Vec<SwarmSessionRecord> {
        vec![SwarmSessionRecord {
            session_id: "s1".to_string(),
            warnings_generated: vec![
                "aci-warn-off-by-one".to_string(),
                "aci-warn-null-deref".to_string(),
            ],
            warnings_that_caught_bugs: vec!["aci-warn-null-deref".to_string()],
            false_positives: vec!["aci-warn-off-by-one".to_string()],
        }]
    }

    // ── Bug Category ─────────────────────────────────────────────────────

    #[test]
    fn bug_category_display() {
        assert_eq!(BugCategory::OffByOne.to_string(), "off-by-one");
        assert_eq!(BugCategory::RaceCondition.to_string(), "race-condition");
    }

    // ── Bug Record ───────────────────────────────────────────────────────

    #[test]
    fn bug_record_severity_clamp() {
        let bug = BugRecord::new(1, BugCategory::LogicError, "test", "test").with_severity(10);
        assert_eq!(bug.severity, 5);
    }

    // ── Swarm Session ────────────────────────────────────────────────────

    #[test]
    fn session_precision() {
        let session = &sample_sessions()[0];
        assert_eq!(session.precision(), 0.5); // 1 caught / 2 generated
    }

    // ── Rule Extraction ──────────────────────────────────────────────────

    #[test]
    fn extract_rules_from_bugs() {
        let rules = extract_warning_rules(&sample_bugs());
        assert!(!rules.is_empty());
        // Should have one rule per bug category
        let categories: Vec<BugCategory> = rules.iter().map(|r| r.category).collect();
        assert!(categories.contains(&BugCategory::OffByOne));
        assert!(categories.contains(&BugCategory::NullDeref));
    }

    #[test]
    fn rules_sorted_by_priority() {
        let rules = extract_warning_rules(&sample_bugs());
        for w in rules.windows(2) {
            assert!(w[0].priority() >= w[1].priority());
        }
    }

    #[test]
    fn rule_has_keywords() {
        let rules = extract_warning_rules(&sample_bugs());
        for rule in &rules {
            assert!(!rule.trigger_keywords.is_empty(), "rule {} should have keywords", rule.id);
        }
    }

    // ── Calibration ──────────────────────────────────────────────────────

    #[test]
    fn calibrate_updates_fp_rate() {
        let mut rules = extract_warning_rules(&sample_bugs());
        let sessions = sample_sessions();
        calibrate_with_swarm_feedback(&mut rules, &sessions);

        let obo = rules.iter().find(|r| r.category == BugCategory::OffByOne).unwrap();
        assert!(obo.false_positive_rate > 0.0, "should have FP rate from feedback");
    }

    // ── Warning Rule ─────────────────────────────────────────────────────

    #[test]
    fn rule_priority() {
        let rule = WarningRule {
            id: "test".to_string(),
            category: BugCategory::NullDeref,
            trigger_keywords: vec!["unwrap".to_string()],
            message: "test".to_string(),
            suggestion: "fix".to_string(),
            confidence: 0.9,
            severity: 5,
            evidence_count: 10,
            false_positive_rate: 0.1,
        };
        assert!(rule.priority() > 0.0);
    }

    #[test]
    fn rule_suppression() {
        let mut rule = WarningRule {
            id: "test".to_string(),
            category: BugCategory::LogicError,
            trigger_keywords: vec![],
            message: "test".to_string(),
            suggestion: String::new(),
            confidence: 0.5,
            severity: 2,
            evidence_count: 1,
            false_positive_rate: 0.3,
        };
        assert!(!rule.is_suppressed());
        rule.false_positive_rate = 0.8;
        assert!(rule.is_suppressed());
    }

    #[test]
    fn rule_display() {
        let rule = WarningRule {
            id: "test".to_string(),
            category: BugCategory::OffByOne,
            trigger_keywords: vec![],
            message: "potential off-by-one".to_string(),
            suggestion: String::new(),
            confidence: 0.85,
            severity: 4,
            evidence_count: 1,
            false_positive_rate: 0.0,
        };
        let s = format!("{rule}");
        assert!(s.contains("off-by-one"));
        assert!(s.contains("85%"));
    }

    // ── Code Analysis ────────────────────────────────────────────────────

    #[test]
    fn analyze_triggers_warning() {
        let rules = extract_warning_rules(&sample_bugs());
        let snippet = CodeSnippet::new("main.rdx", 10, "for i in 0..len minus one loop index");
        let warnings = analyze_code(&snippet, &rules);
        assert!(!warnings.is_empty(), "should detect pattern match");
    }

    #[test]
    fn analyze_no_match() {
        let rules = extract_warning_rules(&sample_bugs());
        let snippet = CodeSnippet::new("clean.rdx", 1, "let x = 42;");
        let warnings = analyze_code(&snippet, &rules);
        assert!(warnings.is_empty(), "clean code should not trigger warnings");
    }

    #[test]
    fn analyze_sorts_by_severity() {
        let rules = extract_warning_rules(&sample_bugs());
        // Snippet matches multiple categories
        let snippet = CodeSnippet::new("bad.rdx", 1, "loop index unwrap option mutex lock");
        let warnings = analyze_code(&snippet, &rules);
        if warnings.len() >= 2 {
            assert!(warnings[0].severity >= warnings[1].severity);
        }
    }

    // ── Dynamic Warning ──────────────────────────────────────────────────

    #[test]
    fn warning_is_actionable() {
        let w = DynamicWarning {
            rule_id: "test".to_string(),
            category: BugCategory::NullDeref,
            message: "possible null deref".to_string(),
            suggestion: "use if-let".to_string(),
            severity: 4,
            confidence: 0.8,
            location: CodeLocation::new("main.rdx", 10, 1),
        };
        assert!(w.is_actionable());
    }

    #[test]
    fn warning_display() {
        let w = DynamicWarning {
            rule_id: "test".to_string(),
            category: BugCategory::ResourceLeak,
            message: "possible leak".to_string(),
            suggestion: String::new(),
            severity: 3,
            confidence: 0.6,
            location: CodeLocation::new("file.rdx", 25, 5),
        };
        let s = format!("{w}");
        assert!(s.contains("resource-leak"));
        assert!(s.contains("file.rdx:25:5"));
    }

    // ── Code Location ────────────────────────────────────────────────────

    #[test]
    fn code_location_display() {
        let loc = CodeLocation::new("src/main.rdx", 42, 10);
        assert_eq!(loc.to_string(), "src/main.rdx:42:10");
    }

    // ── Warning Engine ───────────────────────────────────────────────────

    #[test]
    fn engine_build() {
        let engine = WarningEngine::build(
            &sample_bugs(),
            &sample_sessions(),
            WarningEngineConfig::default(),
        );
        let stats = engine.stats();
        assert!(stats.active_rules > 0);
        assert!(stats.total_evidence > 0);
    }

    #[test]
    fn engine_analyze() {
        let engine = WarningEngine::build(&sample_bugs(), &[], WarningEngineConfig::default());
        let snippet = CodeSnippet::new("test.rdx", 1, "unwrap option none");
        let warnings = engine.analyze(&snippet);
        assert!(!warnings.is_empty());
    }

    #[test]
    fn engine_analyze_all() {
        let engine = WarningEngine::build(&sample_bugs(), &[], WarningEngineConfig::default());
        let snippets = vec![
            CodeSnippet::new("a.rdx", 1, "loop index minus"),
            CodeSnippet::new("b.rdx", 1, "unwrap option none"),
        ];
        let warnings = engine.analyze_all(&snippets);
        assert!(warnings.len() >= 2);
    }

    #[test]
    fn engine_min_severity_filter() {
        let config = WarningEngineConfig { min_severity: 5, ..Default::default() };
        let engine = WarningEngine::build(&sample_bugs(), &[], config);
        let snippet = CodeSnippet::new("test.rdx", 1, "loop index len minus unwrap open file");
        let warnings = engine.analyze(&snippet);
        for w in &warnings {
            assert!(w.severity >= 5);
        }
    }

    #[test]
    fn engine_feedback() {
        let mut engine = WarningEngine::build(&sample_bugs(), &[], WarningEngineConfig::default());
        let obo_id =
            engine.rules.iter().find(|r| r.category == BugCategory::OffByOne).map(|r| r.id.clone());
        if let Some(id) = obo_id {
            engine.record_feedback(&id, true);
            engine.record_feedback(&id, false);
            engine.record_feedback(&id, false);
            // FP rate should be 2/3
            let rule = engine.rules.iter().find(|r| r.id == id).unwrap();
            assert!((rule.false_positive_rate - 2.0 / 3.0).abs() < 0.01);
        }
    }

    // ── Stats ────────────────────────────────────────────────────────────

    #[test]
    fn engine_stats() {
        let engine = WarningEngine::build(&sample_bugs(), &[], WarningEngineConfig::default());
        let stats = engine.stats();
        assert_eq!(stats.total_rules, 4); // 4 bug categories
        assert!(stats.unique_categories() >= 4);
    }

    // ── Edge Cases ───────────────────────────────────────────────────────

    #[test]
    fn empty_bugs_no_rules() {
        let rules = extract_warning_rules(&[]);
        assert!(rules.is_empty());
    }

    #[test]
    fn empty_snippet() {
        let rules = extract_warning_rules(&sample_bugs());
        let snippet = CodeSnippet::new("empty.rdx", 1, "");
        let warnings = analyze_code(&snippet, &rules);
        assert!(warnings.is_empty());
    }
}

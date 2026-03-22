//! # Frequency-Driven Abbreviation Promotion in ACI
//!
//! Analyzes token frequency across a corpus and suggests new abbreviations for
//! frequently used identifiers. The system tracks usage patterns and
//! recommends short forms that minimize keystroke cost.

use std::collections::HashMap;
use std::fmt;

// ── Token Corpus ─────────────────────────────────────────────────────

/// A token occurrence in the corpus.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenOccurrence {
    pub token: String,
    pub context: TokenContext,
}

/// The syntactic context of a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenContext {
    FunctionName,
    TypeName,
    VariableName,
    FieldName,
    ModuleName,
    Keyword,
    Other,
}

impl fmt::Display for TokenContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FunctionName => write!(f, "fn"),
            Self::TypeName => write!(f, "type"),
            Self::VariableName => write!(f, "var"),
            Self::FieldName => write!(f, "field"),
            Self::ModuleName => write!(f, "mod"),
            Self::Keyword => write!(f, "kw"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// Corpus of token frequencies.
#[derive(Debug, Default)]
pub struct TokenCorpus {
    frequencies: HashMap<String, u64>,
    context_frequencies: HashMap<(String, TokenContext), u64>,
    total_tokens: u64,
}

impl TokenCorpus {
    pub fn new() -> Self { Self::default() }

    /// Record a token occurrence.
    pub fn record(&mut self, token: impl Into<String>, context: TokenContext) {
        let token = token.into();
        *self.frequencies.entry(token.clone()).or_insert(0) += 1;
        *self.context_frequencies.entry((token, context)).or_insert(0) += 1;
        self.total_tokens += 1;
    }

    /// Record multiple occurrences of the same token.
    pub fn record_n(&mut self, token: impl Into<String>, context: TokenContext, count: u64) {
        let token = token.into();
        *self.frequencies.entry(token.clone()).or_insert(0) += count;
        *self.context_frequencies.entry((token, context)).or_insert(0) += count;
        self.total_tokens += count;
    }

    /// Get frequency of a token.
    pub fn frequency(&self, token: &str) -> u64 {
        self.frequencies.get(token).copied().unwrap_or(0)
    }

    /// Get frequency of a token in a specific context.
    pub fn context_frequency(&self, token: &str, context: TokenContext) -> u64 {
        self.context_frequencies.get(&(token.to_string(), context)).copied().unwrap_or(0)
    }

    /// Get total number of token occurrences.
    pub fn total(&self) -> u64 {
        self.total_tokens
    }

    /// Get number of unique tokens.
    pub fn unique_count(&self) -> usize {
        self.frequencies.len()
    }

    /// Get tokens sorted by frequency (highest first).
    pub fn top_tokens(&self, n: usize) -> Vec<(&str, u64)> {
        let mut entries: Vec<(&str, u64)> = self.frequencies.iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(n);
        entries
    }

    /// Get relative frequency (0.0 to 1.0).
    pub fn relative_frequency(&self, token: &str) -> f64 {
        if self.total_tokens == 0 { return 0.0; }
        self.frequency(token) as f64 / self.total_tokens as f64
    }
}

// ── Abbreviation Candidate ───────────────────────────────────────────

/// A candidate abbreviation.
#[derive(Debug, Clone)]
pub struct AbbrevCandidate {
    pub full_form: String,
    pub short_form: String,
    pub frequency: u64,
    pub savings: u64,
    pub score: f64,
    pub contexts: Vec<TokenContext>,
}

impl fmt::Display for AbbrevCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {} (freq={}, savings={}, score={:.2})",
            self.full_form, self.short_form, self.frequency, self.savings, self.score)
    }
}

// ── Abbreviation Generation ──────────────────────────────────────────

/// Configuration for abbreviation generation.
#[derive(Debug, Clone)]
pub struct AbbrevConfig {
    /// Minimum frequency for a token to be considered.
    pub min_frequency: u64,
    /// Minimum token length to consider.
    pub min_token_length: usize,
    /// Maximum abbreviation length.
    pub max_abbrev_length: usize,
    /// Maximum number of suggestions.
    pub max_suggestions: usize,
    /// Minimum savings per occurrence (characters).
    pub min_savings_per_use: usize,
}

impl Default for AbbrevConfig {
    fn default() -> Self {
        Self {
            min_frequency: 5,
            min_token_length: 6,
            max_abbrev_length: 4,
            max_suggestions: 20,
            min_savings_per_use: 2,
        }
    }
}

/// Strategy for generating abbreviations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbbrevStrategy {
    /// First N characters.
    Prefix,
    /// Consonants only.
    Consonants,
    /// Camel-case initials.
    Initials,
    /// Best of all strategies.
    Auto,
}

/// Generate abbreviation using prefix strategy.
fn abbrev_prefix(token: &str, max_len: usize) -> String {
    token.chars().take(max_len).collect()
}

/// Generate abbreviation using consonant strategy.
fn abbrev_consonants(token: &str, max_len: usize) -> String {
    let vowels = ['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U'];
    let mut result = String::new();
    let mut chars = token.chars();
    // Always keep first character
    if let Some(c) = chars.next() {
        result.push(c);
    }
    for c in chars {
        if result.len() >= max_len { break; }
        if !vowels.contains(&c) {
            result.push(c);
        }
    }
    // If too short, fill with remaining chars
    if result.len() < 2 {
        for c in token.chars().skip(1) {
            if result.len() >= max_len { break; }
            if !result.contains(c) {
                result.push(c);
            }
        }
    }
    result
}

/// Generate abbreviation using CamelCase initials.
fn abbrev_initials(token: &str) -> String {
    let mut result = String::new();
    let mut prev_was_sep = true;
    for c in token.chars() {
        if c == '_' || c == '-' {
            prev_was_sep = true;
        } else if prev_was_sep || c.is_uppercase() {
            result.push(c.to_ascii_lowercase());
            prev_was_sep = false;
        }
    }
    if result.len() < 2 && !token.is_empty() {
        result = token.chars().take(2).collect();
    }
    result
}

/// Generate the best abbreviation for a token.
pub fn generate_abbreviation(token: &str, strategy: AbbrevStrategy, max_len: usize) -> String {
    match strategy {
        AbbrevStrategy::Prefix => abbrev_prefix(token, max_len),
        AbbrevStrategy::Consonants => abbrev_consonants(token, max_len),
        AbbrevStrategy::Initials => abbrev_initials(token),
        AbbrevStrategy::Auto => {
            let candidates = [
                abbrev_prefix(token, max_len),
                abbrev_consonants(token, max_len),
                abbrev_initials(token),
            ];
            // Pick the shortest unique one that saves the most
            candidates.into_iter()
                .filter(|c| c.len() < token.len())
                .min_by_key(|c| c.len())
                .unwrap_or_else(|| abbrev_prefix(token, max_len))
        }
    }
}

// ── Existing Abbreviations ───────────────────────────────────────────

/// Registry of existing abbreviations to avoid conflicts.
#[derive(Debug, Default)]
pub struct AbbrevRegistry {
    abbrevs: HashMap<String, String>, // short -> full
    reverse: HashMap<String, String>, // full -> short
}

impl AbbrevRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, short: impl Into<String>, full: impl Into<String>) {
        let short = short.into();
        let full = full.into();
        self.reverse.insert(full.clone(), short.clone());
        self.abbrevs.insert(short, full);
    }

    pub fn has_short(&self, short: &str) -> bool {
        self.abbrevs.contains_key(short)
    }

    pub fn has_full(&self, full: &str) -> bool {
        self.reverse.contains_key(full)
    }

    pub fn resolve(&self, short: &str) -> Option<&str> {
        self.abbrevs.get(short).map(|s| s.as_str())
    }

    pub fn abbreviation_of(&self, full: &str) -> Option<&str> {
        self.reverse.get(full).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.abbrevs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.abbrevs.is_empty()
    }
}

// ── Suggestion Engine ────────────────────────────────────────────────

/// Suggest abbreviations for a corpus.
pub fn suggest_abbreviations(
    corpus: &TokenCorpus,
    config: &AbbrevConfig,
    existing: &AbbrevRegistry,
    strategy: AbbrevStrategy,
) -> Vec<AbbrevCandidate> {
    let mut candidates = Vec::new();

    for (token, freq) in corpus.top_tokens(config.max_suggestions * 5) {
        if freq < config.min_frequency { continue; }
        if token.len() < config.min_token_length { continue; }
        if existing.has_full(token) { continue; }

        let short = generate_abbreviation(token, strategy, config.max_abbrev_length);
        if short.len() >= token.len() { continue; }

        let savings_per = token.len() - short.len();
        if savings_per < config.min_savings_per_use { continue; }

        // Check for conflicts
        if existing.has_short(&short) { continue; }

        let total_savings = savings_per as u64 * freq;
        let score = total_savings as f64 * corpus.relative_frequency(token);

        // Collect contexts
        let contexts: Vec<TokenContext> = [
            TokenContext::FunctionName,
            TokenContext::TypeName,
            TokenContext::VariableName,
            TokenContext::FieldName,
            TokenContext::ModuleName,
        ]
        .into_iter()
        .filter(|ctx| corpus.context_frequency(token, *ctx) > 0)
        .collect();

        candidates.push(AbbrevCandidate {
            full_form: token.to_string(),
            short_form: short,
            frequency: freq,
            savings: total_savings,
            score,
            contexts,
        });
    }

    // Sort by score descending
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(config.max_suggestions);
    candidates
}

// ── Promotion Decision ───────────────────────────────────────────────

/// Promotion threshold config.
#[derive(Debug, Clone)]
pub struct PromotionThreshold {
    pub min_score: f64,
    pub min_frequency: u64,
    pub min_savings: u64,
}

impl Default for PromotionThreshold {
    fn default() -> Self {
        Self {
            min_score: 0.001,
            min_frequency: 10,
            min_savings: 50,
        }
    }
}

/// A promotion decision.
#[derive(Debug, Clone)]
pub struct PromotionDecision {
    pub candidate: AbbrevCandidate,
    pub promoted: bool,
    pub reason: String,
}

impl fmt::Display for PromotionDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.promoted { "PROMOTE" } else { "SKIP" };
        write!(f, "[{status}] {} -> {}: {}", self.candidate.full_form, self.candidate.short_form, self.reason)
    }
}

/// Evaluate promotion candidates.
pub fn evaluate_promotions(
    candidates: &[AbbrevCandidate],
    threshold: &PromotionThreshold,
) -> Vec<PromotionDecision> {
    candidates.iter().map(|c| {
        let promoted = c.score >= threshold.min_score
            && c.frequency >= threshold.min_frequency
            && c.savings >= threshold.min_savings;
        let reason = if promoted {
            format!("score={:.4}, freq={}, savings={}", c.score, c.frequency, c.savings)
        } else {
            let mut reasons = Vec::new();
            if c.score < threshold.min_score { reasons.push("low score"); }
            if c.frequency < threshold.min_frequency { reasons.push("low frequency"); }
            if c.savings < threshold.min_savings { reasons.push("low savings"); }
            reasons.join(", ")
        };
        PromotionDecision {
            candidate: c.clone(),
            promoted,
            reason,
        }
    }).collect()
}

// ── Report ───────────────────────────────────────────────────────────

/// Analysis report.
#[derive(Debug)]
pub struct PromotionReport {
    pub total_candidates: usize,
    pub promoted_count: usize,
    pub total_savings: u64,
    pub decisions: Vec<PromotionDecision>,
}

impl PromotionReport {
    pub fn from_decisions(decisions: Vec<PromotionDecision>) -> Self {
        let promoted_count = decisions.iter().filter(|d| d.promoted).count();
        let total_savings: u64 = decisions.iter()
            .filter(|d| d.promoted)
            .map(|d| d.candidate.savings)
            .sum();
        Self {
            total_candidates: decisions.len(),
            promoted_count,
            total_savings,
            decisions,
        }
    }
}

impl fmt::Display for PromotionReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Abbreviation Promotion Report")?;
        writeln!(f, "  candidates: {}", self.total_candidates)?;
        writeln!(f, "  promoted:   {}", self.promoted_count)?;
        writeln!(f, "  savings:    {} chars", self.total_savings)?;
        for d in &self.decisions {
            writeln!(f, "  {d}")?;
        }
        Ok(())
    }
}

// ── Full Pipeline ────────────────────────────────────────────────────

/// Run the full abbreviation promotion pipeline.
pub fn promote_abbreviations(
    corpus: &TokenCorpus,
    existing: &AbbrevRegistry,
) -> PromotionReport {
    let config = AbbrevConfig::default();
    let threshold = PromotionThreshold::default();
    let candidates = suggest_abbreviations(corpus, &config, existing, AbbrevStrategy::Auto);
    let decisions = evaluate_promotions(&candidates, &threshold);
    PromotionReport::from_decisions(decisions)
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_corpus() -> TokenCorpus {
        let mut c = TokenCorpus::new();
        c.record_n("transform", TokenContext::FunctionName, 100);
        c.record_n("validate", TokenContext::FunctionName, 80);
        c.record_n("serialize", TokenContext::FunctionName, 60);
        c.record_n("configuration", TokenContext::TypeName, 50);
        c.record_n("identifier", TokenContext::VariableName, 40);
        c.record_n("response", TokenContext::TypeName, 30);
        c.record_n("fn", TokenContext::Keyword, 500);
        c.record_n("let", TokenContext::Keyword, 400);
        c.record_n("x", TokenContext::VariableName, 200);
        c
    }

    #[test]
    fn test_corpus_record() {
        let mut c = TokenCorpus::new();
        c.record("hello", TokenContext::VariableName);
        assert_eq!(c.frequency("hello"), 1);
        assert_eq!(c.total(), 1);
    }

    #[test]
    fn test_corpus_record_n() {
        let mut c = TokenCorpus::new();
        c.record_n("test", TokenContext::FunctionName, 10);
        assert_eq!(c.frequency("test"), 10);
    }

    #[test]
    fn test_corpus_context_frequency() {
        let mut c = TokenCorpus::new();
        c.record_n("foo", TokenContext::FunctionName, 5);
        c.record_n("foo", TokenContext::TypeName, 3);
        assert_eq!(c.context_frequency("foo", TokenContext::FunctionName), 5);
        assert_eq!(c.context_frequency("foo", TokenContext::TypeName), 3);
        assert_eq!(c.context_frequency("foo", TokenContext::VariableName), 0);
    }

    #[test]
    fn test_corpus_top_tokens() {
        let c = sample_corpus();
        let top = c.top_tokens(3);
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].0, "fn");
        assert_eq!(top[0].1, 500);
    }

    #[test]
    fn test_corpus_relative_frequency() {
        let mut c = TokenCorpus::new();
        c.record_n("a", TokenContext::Other, 25);
        c.record_n("b", TokenContext::Other, 75);
        assert!((c.relative_frequency("a") - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_corpus_unique_count() {
        let c = sample_corpus();
        assert_eq!(c.unique_count(), 9);
    }

    #[test]
    fn test_abbrev_prefix() {
        assert_eq!(abbrev_prefix("transform", 4), "tran");
        assert_eq!(abbrev_prefix("ab", 4), "ab");
    }

    #[test]
    fn test_abbrev_consonants() {
        let r = abbrev_consonants("transform", 4);
        assert!(r.len() <= 4);
        assert!(r.starts_with('t'));
    }

    #[test]
    fn test_abbrev_initials_snake_case() {
        assert_eq!(abbrev_initials("get_user_name"), "gun");
    }

    #[test]
    fn test_abbrev_initials_camel_case() {
        let r = abbrev_initials("getUserName");
        assert!(r.contains('g'));
        assert!(r.contains('u'));
        assert!(r.contains('n'));
    }

    #[test]
    fn test_generate_abbreviation_auto() {
        let r = generate_abbreviation("configuration", AbbrevStrategy::Auto, 4);
        assert!(r.len() <= 4);
        assert!(r.len() < "configuration".len());
    }

    #[test]
    fn test_abbrev_candidate_display() {
        let c = AbbrevCandidate {
            full_form: "transform".into(),
            short_form: "trn".into(),
            frequency: 100,
            savings: 600,
            score: 1.5,
            contexts: vec![TokenContext::FunctionName],
        };
        let s = format!("{c}");
        assert!(s.contains("transform"));
        assert!(s.contains("trn"));
    }

    #[test]
    fn test_abbrev_registry() {
        let mut reg = AbbrevRegistry::new();
        reg.register("fn", "function");
        assert!(reg.has_short("fn"));
        assert!(reg.has_full("function"));
        assert_eq!(reg.resolve("fn"), Some("function"));
        assert_eq!(reg.abbreviation_of("function"), Some("fn"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn test_abbrev_registry_empty() {
        let reg = AbbrevRegistry::new();
        assert!(reg.is_empty());
        assert!(!reg.has_short("x"));
    }

    #[test]
    fn test_suggest_abbreviations() {
        let corpus = sample_corpus();
        let config = AbbrevConfig {
            min_frequency: 5,
            min_token_length: 6,
            max_abbrev_length: 4,
            max_suggestions: 10,
            min_savings_per_use: 2,
        };
        let existing = AbbrevRegistry::new();
        let candidates = suggest_abbreviations(&corpus, &config, &existing, AbbrevStrategy::Prefix);
        assert!(!candidates.is_empty());
        // Should include long, frequent tokens
        assert!(candidates.iter().any(|c| c.full_form == "transform"));
    }

    #[test]
    fn test_suggest_skips_existing() {
        let corpus = sample_corpus();
        let config = AbbrevConfig::default();
        let mut existing = AbbrevRegistry::new();
        existing.register("trn", "transform");
        let candidates = suggest_abbreviations(&corpus, &config, &existing, AbbrevStrategy::Prefix);
        assert!(!candidates.iter().any(|c| c.full_form == "transform"));
    }

    #[test]
    fn test_suggest_skips_short_tokens() {
        let corpus = sample_corpus();
        let config = AbbrevConfig { min_token_length: 6, ..Default::default() };
        let existing = AbbrevRegistry::new();
        let candidates = suggest_abbreviations(&corpus, &config, &existing, AbbrevStrategy::Prefix);
        assert!(!candidates.iter().any(|c| c.full_form == "fn"));
        assert!(!candidates.iter().any(|c| c.full_form == "x"));
    }

    #[test]
    fn test_evaluate_promotions_promote() {
        let candidates = vec![AbbrevCandidate {
            full_form: "transform".into(),
            short_form: "trn".into(),
            frequency: 100,
            savings: 600,
            score: 0.05,
            contexts: vec![TokenContext::FunctionName],
        }];
        let threshold = PromotionThreshold::default();
        let decisions = evaluate_promotions(&candidates, &threshold);
        assert_eq!(decisions.len(), 1);
        assert!(decisions[0].promoted);
    }

    #[test]
    fn test_evaluate_promotions_skip_low_freq() {
        let candidates = vec![AbbrevCandidate {
            full_form: "transform".into(),
            short_form: "trn".into(),
            frequency: 2,
            savings: 12,
            score: 0.0001,
            contexts: vec![],
        }];
        let threshold = PromotionThreshold::default();
        let decisions = evaluate_promotions(&candidates, &threshold);
        assert!(!decisions[0].promoted);
    }

    #[test]
    fn test_promotion_decision_display() {
        let d = PromotionDecision {
            candidate: AbbrevCandidate {
                full_form: "config".into(),
                short_form: "cfg".into(),
                frequency: 50,
                savings: 150,
                score: 1.0,
                contexts: vec![],
            },
            promoted: true,
            reason: "good".into(),
        };
        let s = format!("{d}");
        assert!(s.contains("PROMOTE"));
        assert!(s.contains("config"));
    }

    #[test]
    fn test_promotion_report() {
        let decisions = vec![
            PromotionDecision {
                candidate: AbbrevCandidate {
                    full_form: "a".into(), short_form: "b".into(),
                    frequency: 10, savings: 50, score: 1.0, contexts: vec![],
                },
                promoted: true,
                reason: "ok".into(),
            },
            PromotionDecision {
                candidate: AbbrevCandidate {
                    full_form: "c".into(), short_form: "d".into(),
                    frequency: 1, savings: 2, score: 0.0, contexts: vec![],
                },
                promoted: false,
                reason: "low".into(),
            },
        ];
        let report = PromotionReport::from_decisions(decisions);
        assert_eq!(report.total_candidates, 2);
        assert_eq!(report.promoted_count, 1);
        assert_eq!(report.total_savings, 50);
    }

    #[test]
    fn test_promotion_report_display() {
        let report = PromotionReport {
            total_candidates: 5,
            promoted_count: 2,
            total_savings: 300,
            decisions: vec![],
        };
        let s = format!("{report}");
        assert!(s.contains("5"));
        assert!(s.contains("300"));
    }

    #[test]
    fn test_promote_abbreviations_pipeline() {
        let corpus = sample_corpus();
        let existing = AbbrevRegistry::new();
        let report = promote_abbreviations(&corpus, &existing);
        assert!(report.total_candidates > 0);
    }

    #[test]
    fn test_token_context_display() {
        assert_eq!(format!("{}", TokenContext::FunctionName), "fn");
        assert_eq!(format!("{}", TokenContext::TypeName), "type");
        assert_eq!(format!("{}", TokenContext::Other), "other");
    }

    #[test]
    fn test_empty_corpus() {
        let c = TokenCorpus::new();
        assert_eq!(c.total(), 0);
        assert_eq!(c.unique_count(), 0);
        assert!(c.top_tokens(5).is_empty());
        assert_eq!(c.relative_frequency("x"), 0.0);
    }

    #[test]
    fn test_config_default() {
        let cfg = AbbrevConfig::default();
        assert_eq!(cfg.min_frequency, 5);
        assert_eq!(cfg.min_token_length, 6);
        assert_eq!(cfg.max_abbrev_length, 4);
    }

    #[test]
    fn test_threshold_default() {
        let t = PromotionThreshold::default();
        assert!(t.min_score > 0.0);
        assert!(t.min_frequency > 0);
    }
}

//! # Corpus-Wide Token Frequency Analysis
//!
//! Analyzes token frequencies across a crate ecosystem corpus for abbreviation
//! optimization. Tracks identifiers, keywords, operators, and structural tokens
//! with contextual metadata.

use std::collections::HashMap;
use std::fmt;

// ── Token Classification ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Keyword,
    Identifier,
    Type,
    Lifetime,
    Operator,
    Punctuation,
    Literal,
    Attribute,
    Macro,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keyword => write!(f, "keyword"),
            Self::Identifier => write!(f, "ident"),
            Self::Type => write!(f, "type"),
            Self::Lifetime => write!(f, "lifetime"),
            Self::Operator => write!(f, "operator"),
            Self::Punctuation => write!(f, "punct"),
            Self::Literal => write!(f, "literal"),
            Self::Attribute => write!(f, "attr"),
            Self::Macro => write!(f, "macro"),
        }
    }
}

/// Context where a token appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenContext {
    FunctionDef,
    FunctionCall,
    StructDef,
    EnumDef,
    TraitDef,
    ImplBlock,
    TypeAnnotation,
    LetBinding,
    MatchArm,
    ClosureBody,
    ModuleLevel,
    UseStatement,
    Other,
}

impl fmt::Display for TokenContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FunctionDef => write!(f, "fn_def"),
            Self::FunctionCall => write!(f, "fn_call"),
            Self::StructDef => write!(f, "struct_def"),
            Self::EnumDef => write!(f, "enum_def"),
            Self::TraitDef => write!(f, "trait_def"),
            Self::ImplBlock => write!(f, "impl"),
            Self::TypeAnnotation => write!(f, "type_ann"),
            Self::LetBinding => write!(f, "let"),
            Self::MatchArm => write!(f, "match"),
            Self::ClosureBody => write!(f, "closure"),
            Self::ModuleLevel => write!(f, "module"),
            Self::UseStatement => write!(f, "use"),
            Self::Other => write!(f, "other"),
        }
    }
}

// ── Token Record ─────────────────────────────────────────────────────

/// A record of one token occurrence in the corpus.
#[derive(Debug, Clone)]
pub struct TokenRecord {
    pub text: String,
    pub kind: TokenKind,
    pub context: TokenContext,
    pub crate_name: String,
}

// ── Frequency Table ──────────────────────────────────────────────────

/// Accumulates token frequency data.
#[derive(Debug, Clone)]
pub struct FrequencyEntry {
    pub text: String,
    pub kind: TokenKind,
    pub total_count: u64,
    pub crate_count: u64,
    pub context_counts: HashMap<TokenContext, u64>,
    pub crates_seen: Vec<String>,
}

impl FrequencyEntry {
    fn new(text: String, kind: TokenKind) -> Self {
        Self {
            text,
            kind,
            total_count: 0,
            crate_count: 0,
            context_counts: HashMap::new(),
            crates_seen: Vec::new(),
        }
    }

    pub fn frequency_rank(&self, corpus_total: u64) -> f64 {
        if corpus_total == 0 {
            return 0.0;
        }
        self.total_count as f64 / corpus_total as f64
    }

    pub fn dispersion(&self, total_crates: usize) -> f64 {
        if total_crates == 0 {
            return 0.0;
        }
        self.crate_count as f64 / total_crates as f64
    }

    pub fn dominant_context(&self) -> Option<TokenContext> {
        self.context_counts.iter().max_by_key(|(_, count)| *count).map(|(ctx, _)| *ctx)
    }
}

impl fmt::Display for FrequencyEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}): {} total, {} crates",
            self.text, self.kind, self.total_count, self.crate_count
        )
    }
}

// ── Corpus Analyzer ──────────────────────────────────────────────────

/// Main corpus analyzer that accumulates token data across crates.
#[derive(Debug)]
pub struct CorpusAnalyzer {
    entries: HashMap<String, FrequencyEntry>,
    crate_names: Vec<String>,
    total_tokens: u64,
}

impl CorpusAnalyzer {
    pub fn new() -> Self {
        Self { entries: HashMap::new(), crate_names: Vec::new(), total_tokens: 0 }
    }

    /// Record a batch of tokens from one crate.
    pub fn ingest_crate(&mut self, crate_name: &str, tokens: &[TokenRecord]) {
        if !self.crate_names.contains(&crate_name.to_string()) {
            self.crate_names.push(crate_name.to_string());
        }

        // Track which tokens appear in this crate for crate_count
        let mut seen_in_crate: HashMap<String, bool> = HashMap::new();

        for tok in tokens {
            self.total_tokens += 1;
            let entry = self
                .entries
                .entry(tok.text.clone())
                .or_insert_with(|| FrequencyEntry::new(tok.text.clone(), tok.kind));
            entry.total_count += 1;
            *entry.context_counts.entry(tok.context).or_insert(0) += 1;

            if !seen_in_crate.contains_key(&tok.text) {
                seen_in_crate.insert(tok.text.clone(), true);
                entry.crate_count += 1;
                if !entry.crates_seen.contains(&crate_name.to_string()) {
                    entry.crates_seen.push(crate_name.to_string());
                }
            }
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.total_tokens
    }

    pub fn unique_tokens(&self) -> usize {
        self.entries.len()
    }

    pub fn total_crates(&self) -> usize {
        self.crate_names.len()
    }

    pub fn get_entry(&self, text: &str) -> Option<&FrequencyEntry> {
        self.entries.get(text)
    }

    /// Get top N tokens by total count.
    pub fn top_by_count(&self, n: usize) -> Vec<&FrequencyEntry> {
        let mut sorted: Vec<&FrequencyEntry> = self.entries.values().collect();
        sorted.sort_by(|a, b| b.total_count.cmp(&a.total_count));
        sorted.truncate(n);
        sorted
    }

    /// Get top N tokens by dispersion (appearing in the most crates).
    pub fn top_by_dispersion(&self, n: usize) -> Vec<&FrequencyEntry> {
        let mut sorted: Vec<&FrequencyEntry> = self.entries.values().collect();
        sorted.sort_by(|a, b| b.crate_count.cmp(&a.crate_count));
        sorted.truncate(n);
        sorted
    }

    /// Get tokens of a specific kind.
    pub fn by_kind(&self, kind: TokenKind) -> Vec<&FrequencyEntry> {
        self.entries.values().filter(|e| e.kind == kind).collect()
    }

    /// Tokens that appear in at least min_crates crates.
    pub fn widespread(&self, min_crates: u64) -> Vec<&FrequencyEntry> {
        self.entries.values().filter(|e| e.crate_count >= min_crates).collect()
    }
}

impl Default for CorpusAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Abbreviation Candidate Scoring ───────────────────────────────────

/// A score for how beneficial abbreviating a token would be.
#[derive(Debug, Clone)]
pub struct AbbrevScore {
    pub token: String,
    pub frequency: u64,
    pub dispersion: f64,
    pub length: usize,
    pub savings_score: f64,
    pub recommendation: AbbrevRecommendation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbbrevRecommendation {
    StronglyRecommended,
    Recommended,
    Neutral,
    NotRecommended,
}

impl fmt::Display for AbbrevRecommendation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StronglyRecommended => write!(f, "strongly recommended"),
            Self::Recommended => write!(f, "recommended"),
            Self::Neutral => write!(f, "neutral"),
            Self::NotRecommended => write!(f, "not recommended"),
        }
    }
}

impl fmt::Display for AbbrevScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: savings={:.2}, disp={:.2}, freq={} ({})",
            self.token, self.savings_score, self.dispersion, self.frequency, self.recommendation
        )
    }
}

/// Score tokens for abbreviation benefit.
pub fn score_abbreviations(analyzer: &CorpusAnalyzer, min_length: usize) -> Vec<AbbrevScore> {
    let mut scores = Vec::new();

    for entry in analyzer.entries.values() {
        if entry.text.len() < min_length {
            continue;
        }

        let dispersion = entry.dispersion(analyzer.total_crates());
        let _freq_ratio = entry.frequency_rank(analyzer.total_tokens());

        // Savings = frequency * (length - estimated_abbrev_length)
        let est_abbrev_len = (entry.text.len() / 2).max(2);
        let char_savings = entry.text.len().saturating_sub(est_abbrev_len) as f64;
        let savings_score = char_savings * entry.total_count as f64 * dispersion;

        let recommendation = if savings_score > 1000.0 && dispersion > 0.5 {
            AbbrevRecommendation::StronglyRecommended
        } else if savings_score > 100.0 && dispersion > 0.3 {
            AbbrevRecommendation::Recommended
        } else if savings_score > 10.0 {
            AbbrevRecommendation::Neutral
        } else {
            AbbrevRecommendation::NotRecommended
        };

        scores.push(AbbrevScore {
            token: entry.text.clone(),
            frequency: entry.total_count,
            dispersion,
            length: entry.text.len(),
            savings_score,
            recommendation,
        });
    }

    scores.sort_by(|a, b| {
        b.savings_score.partial_cmp(&a.savings_score).unwrap_or(std::cmp::Ordering::Equal)
    });
    scores
}

// ── Analysis Report ──────────────────────────────────────────────────

/// Summary statistics for a corpus analysis.
#[derive(Debug)]
pub struct CorpusReport {
    pub total_tokens: u64,
    pub unique_tokens: usize,
    pub total_crates: usize,
    pub top_10_by_count: Vec<(String, u64)>,
    pub top_10_by_dispersion: Vec<(String, u64)>,
    pub abbrev_candidates: usize,
}

pub fn generate_report(analyzer: &CorpusAnalyzer) -> CorpusReport {
    let top_count: Vec<(String, u64)> =
        analyzer.top_by_count(10).iter().map(|e| (e.text.clone(), e.total_count)).collect();

    let top_disp: Vec<(String, u64)> =
        analyzer.top_by_dispersion(10).iter().map(|e| (e.text.clone(), e.crate_count)).collect();

    let abbrev_scores = score_abbreviations(analyzer, 5);
    let candidates = abbrev_scores
        .iter()
        .filter(|s| s.recommendation != AbbrevRecommendation::NotRecommended)
        .count();

    CorpusReport {
        total_tokens: analyzer.total_tokens(),
        unique_tokens: analyzer.unique_tokens(),
        total_crates: analyzer.total_crates(),
        top_10_by_count: top_count,
        top_10_by_dispersion: top_disp,
        abbrev_candidates: candidates,
    }
}

impl fmt::Display for CorpusReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Corpus Token Frequency Report ===")?;
        writeln!(f, "Total tokens: {}", self.total_tokens)?;
        writeln!(f, "Unique tokens: {}", self.unique_tokens)?;
        writeln!(f, "Crates analyzed: {}", self.total_crates)?;
        writeln!(f, "Abbreviation candidates: {}", self.abbrev_candidates)?;
        writeln!(f, "\nTop by count:")?;
        for (tok, count) in &self.top_10_by_count {
            writeln!(f, "  {tok}: {count}")?;
        }
        writeln!(f, "\nTop by dispersion:")?;
        for (tok, crate_count) in &self.top_10_by_dispersion {
            writeln!(f, "  {tok}: {crate_count} crates")?;
        }
        Ok(())
    }
}

// ── Simple Token Classifier ──────────────────────────────────────────

/// Classify a raw token string into a TokenKind (heuristic).
pub fn classify_token(text: &str) -> TokenKind {
    const KEYWORDS: &[&str] = &[
        "fn", "let", "mut", "pub", "struct", "enum", "trait", "impl", "use", "mod", "if", "else",
        "match", "for", "while", "loop", "return", "break", "continue", "const", "static", "type",
        "where", "as", "ref", "self", "super", "crate", "async", "await", "move", "unsafe",
        "extern", "dyn", "box", "in",
    ];

    if KEYWORDS.contains(&text) {
        return TokenKind::Keyword;
    }
    if text.starts_with('\'') {
        return TokenKind::Lifetime;
    }
    if text.starts_with('#') {
        return TokenKind::Attribute;
    }
    if text.ends_with('!') && text.len() > 1 {
        return TokenKind::Macro;
    }
    if text.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '_') && !text.is_empty() {
        return TokenKind::Literal;
    }
    if text.len() <= 3 && text.chars().all(|c| !c.is_alphanumeric()) {
        return TokenKind::Operator;
    }
    if text.chars().next().map_or(false, |c| c.is_uppercase()) {
        return TokenKind::Type;
    }
    TokenKind::Identifier
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tokens(
        crate_name: &str,
        tokens: &[(&str, TokenKind, TokenContext)],
    ) -> Vec<TokenRecord> {
        tokens
            .iter()
            .map(|(text, kind, ctx)| TokenRecord {
                text: text.to_string(),
                kind: *kind,
                context: *ctx,
                crate_name: crate_name.to_string(),
            })
            .collect()
    }

    fn sample_analyzer() -> CorpusAnalyzer {
        let mut a = CorpusAnalyzer::new();

        let t1 = make_tokens(
            "serde",
            &[
                ("derive", TokenKind::Keyword, TokenContext::ModuleLevel),
                ("struct", TokenKind::Keyword, TokenContext::StructDef),
                ("Serialize", TokenKind::Type, TokenContext::TraitDef),
                ("Deserialize", TokenKind::Type, TokenContext::TraitDef),
                ("fn", TokenKind::Keyword, TokenContext::FunctionDef),
                ("serialize", TokenKind::Identifier, TokenContext::FunctionDef),
                ("deserialize", TokenKind::Identifier, TokenContext::FunctionDef),
            ],
        );
        a.ingest_crate("serde", &t1);

        let t2 = make_tokens(
            "tokio",
            &[
                ("fn", TokenKind::Keyword, TokenContext::FunctionDef),
                ("async", TokenKind::Keyword, TokenContext::FunctionDef),
                ("struct", TokenKind::Keyword, TokenContext::StructDef),
                ("Runtime", TokenKind::Type, TokenContext::StructDef),
                ("spawn", TokenKind::Identifier, TokenContext::FunctionCall),
                ("serialize", TokenKind::Identifier, TokenContext::FunctionCall),
            ],
        );
        a.ingest_crate("tokio", &t2);

        let t3 = make_tokens(
            "rand",
            &[
                ("fn", TokenKind::Keyword, TokenContext::FunctionDef),
                ("struct", TokenKind::Keyword, TokenContext::StructDef),
                ("thread_rng", TokenKind::Identifier, TokenContext::FunctionCall),
                ("serialize", TokenKind::Identifier, TokenContext::FunctionCall),
            ],
        );
        a.ingest_crate("rand", &t3);

        a
    }

    #[test]
    fn test_token_kind_display() {
        assert_eq!(format!("{}", TokenKind::Keyword), "keyword");
        assert_eq!(format!("{}", TokenKind::Identifier), "ident");
    }

    #[test]
    fn test_token_context_display() {
        assert_eq!(format!("{}", TokenContext::FunctionDef), "fn_def");
    }

    #[test]
    fn test_analyzer_new() {
        let a = CorpusAnalyzer::new();
        assert_eq!(a.total_tokens(), 0);
        assert_eq!(a.unique_tokens(), 0);
    }

    #[test]
    fn test_ingest_crate() {
        let a = sample_analyzer();
        assert_eq!(a.total_crates(), 3);
        assert!(a.total_tokens() > 0);
    }

    #[test]
    fn test_frequency_entry_rank() {
        let a = sample_analyzer();
        let entry = a.get_entry("fn").unwrap();
        let rank = entry.frequency_rank(a.total_tokens());
        assert!(rank > 0.0);
    }

    #[test]
    fn test_frequency_entry_dispersion() {
        let a = sample_analyzer();
        let entry = a.get_entry("fn").unwrap();
        let disp = entry.dispersion(a.total_crates());
        assert!((disp - 1.0).abs() < f64::EPSILON); // fn appears in all 3 crates
    }

    #[test]
    fn test_dominant_context() {
        let a = sample_analyzer();
        let entry = a.get_entry("fn").unwrap();
        assert_eq!(entry.dominant_context(), Some(TokenContext::FunctionDef));
    }

    #[test]
    fn test_frequency_entry_display() {
        let a = sample_analyzer();
        let entry = a.get_entry("fn").unwrap();
        let s = format!("{entry}");
        assert!(s.contains("fn"));
        assert!(s.contains("keyword"));
    }

    #[test]
    fn test_top_by_count() {
        let a = sample_analyzer();
        let top = a.top_by_count(3);
        assert!(!top.is_empty());
        assert!(top[0].total_count >= top.last().unwrap().total_count);
    }

    #[test]
    fn test_top_by_dispersion() {
        let a = sample_analyzer();
        let top = a.top_by_dispersion(3);
        assert!(!top.is_empty());
    }

    #[test]
    fn test_by_kind() {
        let a = sample_analyzer();
        let keywords = a.by_kind(TokenKind::Keyword);
        assert!(!keywords.is_empty());
        for kw in &keywords {
            assert_eq!(kw.kind, TokenKind::Keyword);
        }
    }

    #[test]
    fn test_widespread() {
        let a = sample_analyzer();
        let wide = a.widespread(2);
        for entry in &wide {
            assert!(entry.crate_count >= 2);
        }
    }

    #[test]
    fn test_score_abbreviations() {
        let a = sample_analyzer();
        let scores = score_abbreviations(&a, 5);
        assert!(!scores.is_empty());
        // Should be sorted by savings
        if scores.len() >= 2 {
            assert!(scores[0].savings_score >= scores[1].savings_score);
        }
    }

    #[test]
    fn test_abbrev_recommendation_display() {
        assert_eq!(
            format!("{}", AbbrevRecommendation::StronglyRecommended),
            "strongly recommended"
        );
    }

    #[test]
    fn test_abbrev_score_display() {
        let s = AbbrevScore {
            token: "test".into(),
            frequency: 10,
            dispersion: 0.5,
            length: 4,
            savings_score: 20.0,
            recommendation: AbbrevRecommendation::Neutral,
        };
        let d = format!("{s}");
        assert!(d.contains("test"));
        assert!(d.contains("neutral"));
    }

    #[test]
    fn test_generate_report() {
        let a = sample_analyzer();
        let report = generate_report(&a);
        assert_eq!(report.total_crates, 3);
        assert!(report.total_tokens > 0);
        assert!(report.unique_tokens > 0);
    }

    #[test]
    fn test_report_display() {
        let a = sample_analyzer();
        let report = generate_report(&a);
        let s = format!("{report}");
        assert!(s.contains("Corpus Token Frequency Report"));
        assert!(s.contains("Crates analyzed: 3"));
    }

    #[test]
    fn test_classify_keyword() {
        assert_eq!(classify_token("fn"), TokenKind::Keyword);
        assert_eq!(classify_token("struct"), TokenKind::Keyword);
    }

    #[test]
    fn test_classify_type() {
        assert_eq!(classify_token("String"), TokenKind::Type);
    }

    #[test]
    fn test_classify_lifetime() {
        assert_eq!(classify_token("'a"), TokenKind::Lifetime);
    }

    #[test]
    fn test_classify_attribute() {
        assert_eq!(classify_token("#[derive]"), TokenKind::Attribute);
    }

    #[test]
    fn test_classify_macro() {
        assert_eq!(classify_token("println!"), TokenKind::Macro);
    }

    #[test]
    fn test_classify_literal() {
        assert_eq!(classify_token("42"), TokenKind::Literal);
    }

    #[test]
    fn test_classify_operator() {
        assert_eq!(classify_token("->"), TokenKind::Operator);
    }

    #[test]
    fn test_classify_identifier() {
        assert_eq!(classify_token("my_var"), TokenKind::Identifier);
    }

    #[test]
    fn test_crate_count_per_token() {
        let a = sample_analyzer();
        let entry = a.get_entry("serialize").unwrap();
        assert_eq!(entry.crate_count, 3); // appears in serde, tokio, rand
    }

    #[test]
    fn test_empty_corpus_report() {
        let a = CorpusAnalyzer::new();
        let report = generate_report(&a);
        assert_eq!(report.total_tokens, 0);
    }

    #[test]
    fn test_frequency_rank_empty() {
        let entry = FrequencyEntry::new("x".into(), TokenKind::Identifier);
        assert_eq!(entry.frequency_rank(0), 0.0);
    }

    #[test]
    fn test_dispersion_empty() {
        let entry = FrequencyEntry::new("x".into(), TokenKind::Identifier);
        assert_eq!(entry.dispersion(0), 0.0);
    }
}

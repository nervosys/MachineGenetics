//! # Memory Recall API
//!
//! RAP endpoints for memory operations:
//! - `memory.store` — store a value in tiers
//! - `memory.recall` — recall values with relevance ranking
//! - `memory.suggest` — get suggestions based on context

use std::collections::HashMap;
use std::fmt;

// ── Memory Tiers (mirrors agent_memory) ──────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryTier {
    Ephemeral,
    Session,
    Project,
    Global,
}

impl MemoryTier {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "ephemeral" => Some(Self::Ephemeral),
            "session" => Some(Self::Session),
            "project" => Some(Self::Project),
            "global" => Some(Self::Global),
            _ => None,
        }
    }
}

impl fmt::Display for MemoryTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ephemeral => write!(f, "ephemeral"),
            Self::Session => write!(f, "session"),
            Self::Project => write!(f, "project"),
            Self::Global => write!(f, "global"),
        }
    }
}

// ── Memory Values ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum MemValue {
    Text(String),
    Number(f64),
    Bool(bool),
    List(Vec<MemValue>),
    Object(Vec<(String, MemValue)>),
    Null,
}

impl fmt::Display for MemValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(s) => write!(f, "\"{s}\""),
            Self::Number(n) => write!(f, "{n}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::List(items) => {
                let s: Vec<String> = items.iter().map(|i| format!("{i}")).collect();
                write!(f, "[{}]", s.join(", "))
            }
            Self::Object(fields) => {
                let s: Vec<String> = fields.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                write!(f, "{{{}}}", s.join(", "))
            }
            Self::Null => write!(f, "null"),
        }
    }
}

// ── Store Entry ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct StoreEntry {
    pub key: String,
    pub value: MemValue,
    pub tier: MemoryTier,
    pub tags: Vec<String>,
    pub access_count: u64,
    pub relevance_score: f64,
}

impl StoreEntry {
    pub fn new(key: impl Into<String>, value: MemValue, tier: MemoryTier) -> Self {
        Self {
            key: key.into(),
            value,
            tier,
            tags: Vec::new(),
            access_count: 0,
            relevance_score: 0.0,
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn touch(&mut self) {
        self.access_count += 1;
    }
}

impl fmt::Display for StoreEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.tier, self.key, self.value)
    }
}

// ── RAP Request/Response Types ───────────────────────────────────────

/// Request to store memory.
#[derive(Debug, Clone)]
pub struct StoreRequest {
    pub key: String,
    pub value: MemValue,
    pub tier: MemoryTier,
    pub tags: Vec<String>,
}

/// Response from memory.store.
#[derive(Debug, Clone)]
pub struct StoreResponse {
    pub success: bool,
    pub key: String,
    pub tier: MemoryTier,
    pub message: String,
}

impl fmt::Display for StoreResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.success { "OK" } else { "ERR" };
        write!(f, "[{status}] memory.store {}: {} ({})", self.key, self.tier, self.message)
    }
}

/// Request to recall memory.
#[derive(Debug, Clone)]
pub struct RecallRequest {
    pub query: String,
    pub tier: Option<MemoryTier>,
    pub tags: Vec<String>,
    pub max_results: usize,
}

/// A single recall result.
#[derive(Debug, Clone)]
pub struct RecallResult {
    pub key: String,
    pub value: MemValue,
    pub tier: MemoryTier,
    pub relevance: f64,
}

impl fmt::Display for RecallResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:.2}] {}: {} ({})", self.relevance, self.key, self.value, self.tier)
    }
}

/// Response from memory.recall.
#[derive(Debug, Clone)]
pub struct RecallResponse {
    pub results: Vec<RecallResult>,
    pub total_searched: usize,
}

impl fmt::Display for RecallResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "memory.recall: {} results (searched {})",
            self.results.len(),
            self.total_searched
        )?;
        for r in &self.results {
            writeln!(f, "  {r}")?;
        }
        Ok(())
    }
}

/// Request for memory suggestions.
#[derive(Debug, Clone)]
pub struct SuggestRequest {
    pub context: String,
    pub max_suggestions: usize,
}

/// A memory suggestion.
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub key: String,
    pub value: MemValue,
    pub tier: MemoryTier,
    pub reason: String,
    pub confidence: f64,
}

impl fmt::Display for Suggestion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:.2}] {}: {} - {}", self.confidence, self.key, self.value, self.reason)
    }
}

/// Response from memory.suggest.
#[derive(Debug, Clone)]
pub struct SuggestResponse {
    pub suggestions: Vec<Suggestion>,
}

impl fmt::Display for SuggestResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "memory.suggest: {} suggestions", self.suggestions.len())?;
        for s in &self.suggestions {
            writeln!(f, "  {s}")?;
        }
        Ok(())
    }
}

// ── Memory Backend ───────────────────────────────────────────────────

/// In-memory backend for the recall API.
#[derive(Debug, Default)]
pub struct MemoryBackend {
    stores: HashMap<MemoryTier, HashMap<String, StoreEntry>>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        let mut stores = HashMap::new();
        stores.insert(MemoryTier::Ephemeral, HashMap::new());
        stores.insert(MemoryTier::Session, HashMap::new());
        stores.insert(MemoryTier::Project, HashMap::new());
        stores.insert(MemoryTier::Global, HashMap::new());
        Self { stores }
    }

    fn tier_store(&self, tier: MemoryTier) -> &HashMap<String, StoreEntry> {
        self.stores.get(&tier).unwrap()
    }

    fn tier_store_mut(&mut self, tier: MemoryTier) -> &mut HashMap<String, StoreEntry> {
        self.stores.get_mut(&tier).unwrap()
    }

    pub fn entry_count(&self, tier: MemoryTier) -> usize {
        self.tier_store(tier).len()
    }

    pub fn total_entries(&self) -> usize {
        self.stores.values().map(|s| s.len()).sum()
    }
}

// ── API Endpoints ────────────────────────────────────────────────────

/// Process a `memory.store` request.
pub fn handle_store(backend: &mut MemoryBackend, req: StoreRequest) -> StoreResponse {
    let entry = StoreEntry::new(&req.key, req.value, req.tier).with_tags(req.tags);
    backend.tier_store_mut(req.tier).insert(req.key.clone(), entry);
    StoreResponse { success: true, key: req.key, tier: req.tier, message: "stored".into() }
}

/// Compute relevance between a query and a store entry.
fn compute_relevance(query: &str, entry: &StoreEntry) -> f64 {
    let mut score = 0.0;
    let query_lower = query.to_lowercase();

    // Key match
    if entry.key.to_lowercase().contains(&query_lower) {
        score += 0.5;
    }

    // Value text match
    if let MemValue::Text(ref text) = entry.value {
        if text.to_lowercase().contains(&query_lower) {
            score += 0.3;
        }
    }

    // Tag match
    for tag in &entry.tags {
        if tag.to_lowercase().contains(&query_lower) {
            score += 0.2;
        }
    }

    // Access frequency bonus
    score += (entry.access_count as f64).ln().max(0.0) * 0.05;

    // Tier priority only if there is already a base match
    if score > 0.0 {
        score += match entry.tier {
            MemoryTier::Global => 0.1,
            MemoryTier::Project => 0.05,
            MemoryTier::Session => 0.02,
            MemoryTier::Ephemeral => 0.0,
        };
    }

    score
}
/// Process a `memory.recall` request.

pub fn handle_recall(backend: &mut MemoryBackend, req: RecallRequest) -> RecallResponse {
    let tiers: Vec<MemoryTier> = if let Some(tier) = req.tier {
        vec![tier]
    } else {
        vec![MemoryTier::Ephemeral, MemoryTier::Session, MemoryTier::Project, MemoryTier::Global]
    };

    let mut total_searched = 0;
    let mut results = Vec::new();

    for tier in tiers {
        let store = backend.tier_store(tier);
        for entry in store.values() {
            total_searched += 1;

            // Tag filter
            if !req.tags.is_empty() && !req.tags.iter().any(|t| entry.tags.contains(t)) {
                continue;
            }

            let relevance = compute_relevance(&req.query, entry);
            if relevance > 0.0 {
                results.push(RecallResult {
                    key: entry.key.clone(),
                    value: entry.value.clone(),
                    tier,
                    relevance,
                });
            }
        }
    }

    // Sort by relevance
    results
        .sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(req.max_results);

    // Touch accessed entries
    for r in &results {
        if let Some(entry) = backend.tier_store_mut(r.tier).get_mut(&r.key) {
            entry.touch();
        }
    }

    RecallResponse { results, total_searched }
}

/// Process a `memory.suggest` request.
pub fn handle_suggest(backend: &MemoryBackend, req: SuggestRequest) -> SuggestResponse {
    let context_lower = req.context.to_lowercase();
    let context_words: Vec<&str> = context_lower.split_whitespace().collect();

    let mut suggestions = Vec::new();

    for tier in
        [MemoryTier::Ephemeral, MemoryTier::Session, MemoryTier::Project, MemoryTier::Global]
    {
        for entry in backend.tier_store(tier).values() {
            let mut score = 0.0;
            let mut reasons = Vec::new();

            // Key word overlap
            let key_lower = entry.key.to_lowercase();
            let key_words: Vec<&str> = key_lower.split(|c: char| !c.is_alphanumeric()).collect();
            let overlap: usize = context_words
                .iter()
                .filter(|w| key_words.iter().any(|kw| kw.contains(*w) || w.contains(kw)))
                .count();
            if overlap > 0 {
                score += overlap as f64 * 0.3;
                reasons.push(format!("{overlap} word overlap"));
            }

            // Tag overlap
            for tag in &entry.tags {
                if context_words.iter().any(|w| tag.to_lowercase().contains(*w)) {
                    score += 0.2;
                    reasons.push(format!("tag match: {tag}"));
                }
            }

            // Frequency bonus
            if entry.access_count > 5 {
                score += 0.1;
                reasons.push("frequently accessed".into());
            }

            if score > 0.0 {
                suggestions.push(Suggestion {
                    key: entry.key.clone(),
                    value: entry.value.clone(),
                    tier,
                    reason: reasons.join(", "),
                    confidence: score.min(1.0),
                });
            }
        }
    }

    suggestions.sort_by(|a, b| {
        b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal)
    });
    suggestions.truncate(req.max_suggestions);
    SuggestResponse { suggestions }
}

// ── RAP Endpoint Dispatcher ──────────────────────────────────────────

/// RAP method name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RapMethod {
    MemoryStore,
    MemoryRecall,
    MemorySuggest,
}

impl RapMethod {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "memory.store" => Some(Self::MemoryStore),
            "memory.recall" => Some(Self::MemoryRecall),
            "memory.suggest" => Some(Self::MemorySuggest),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::MemoryStore => "memory.store",
            Self::MemoryRecall => "memory.recall",
            Self::MemorySuggest => "memory.suggest",
        }
    }
}

impl fmt::Display for RapMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// RAP response envelope.
#[derive(Debug)]
pub enum RapResponse {
    Store(StoreResponse),
    Recall(RecallResponse),
    Suggest(SuggestResponse),
    Error(String),
}

impl fmt::Display for RapResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(r) => write!(f, "{r}"),
            Self::Recall(r) => write!(f, "{r}"),
            Self::Suggest(r) => write!(f, "{r}"),
            Self::Error(msg) => write!(f, "ERROR: {msg}"),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_backend() -> MemoryBackend {
        let mut b = MemoryBackend::new();
        handle_store(
            &mut b,
            StoreRequest {
                key: "indent_style".into(),
                value: MemValue::Text("spaces".into()),
                tier: MemoryTier::Project,
                tags: vec!["convention".into(), "formatting".into()],
            },
        );
        handle_store(
            &mut b,
            StoreRequest {
                key: "max_line_length".into(),
                value: MemValue::Number(120.0),
                tier: MemoryTier::Project,
                tags: vec!["convention".into()],
            },
        );
        handle_store(
            &mut b,
            StoreRequest {
                key: "debug_mode".into(),
                value: MemValue::Bool(true),
                tier: MemoryTier::Session,
                tags: vec!["debug".into()],
            },
        );
        handle_store(
            &mut b,
            StoreRequest {
                key: "user_preference".into(),
                value: MemValue::Text("dark_theme".into()),
                tier: MemoryTier::Global,
                tags: vec!["ui".into()],
            },
        );
        b
    }

    #[test]
    fn test_tier_from_str() {
        assert_eq!(MemoryTier::from_str("ephemeral"), Some(MemoryTier::Ephemeral));
        assert_eq!(MemoryTier::from_str("global"), Some(MemoryTier::Global));
        assert_eq!(MemoryTier::from_str("invalid"), None);
    }

    #[test]
    fn test_tier_display() {
        assert_eq!(format!("{}", MemoryTier::Project), "project");
    }

    #[test]
    fn test_mem_value_display() {
        assert_eq!(format!("{}", MemValue::Null), "null");
        assert_eq!(format!("{}", MemValue::Bool(true)), "true");
        assert_eq!(format!("{}", MemValue::Number(42.0)), "42");
    }

    #[test]
    fn test_mem_value_list() {
        let v = MemValue::List(vec![MemValue::Number(1.0)]);
        assert!(format!("{v}").contains("1"));
    }

    #[test]
    fn test_mem_value_object() {
        let v = MemValue::Object(vec![("k".into(), MemValue::Text("v".into()))]);
        assert!(format!("{v}").contains("k: \"v\""));
    }

    #[test]
    fn test_store_entry() {
        let e = StoreEntry::new("k", MemValue::Bool(true), MemoryTier::Session)
            .with_tags(vec!["t".into()]);
        assert_eq!(e.key, "k");
        assert_eq!(e.tags, vec!["t"]);
    }

    #[test]
    fn test_store_entry_touch() {
        let mut e = StoreEntry::new("k", MemValue::Null, MemoryTier::Ephemeral);
        e.touch();
        e.touch();
        assert_eq!(e.access_count, 2);
    }

    #[test]
    fn test_store_entry_display() {
        let e = StoreEntry::new("key", MemValue::Text("val".into()), MemoryTier::Global);
        let s = format!("{e}");
        assert!(s.contains("global"));
        assert!(s.contains("key"));
    }

    #[test]
    fn test_handle_store() {
        let mut b = MemoryBackend::new();
        let resp = handle_store(
            &mut b,
            StoreRequest {
                key: "test".into(),
                value: MemValue::Bool(true),
                tier: MemoryTier::Ephemeral,
                tags: vec![],
            },
        );
        assert!(resp.success);
        assert_eq!(resp.key, "test");
        assert_eq!(b.entry_count(MemoryTier::Ephemeral), 1);
    }

    #[test]
    fn test_store_response_display() {
        let r = StoreResponse {
            success: true,
            key: "k".into(),
            tier: MemoryTier::Session,
            message: "ok".into(),
        };
        let s = format!("{r}");
        assert!(s.contains("OK"));
    }

    #[test]
    fn test_handle_recall_by_key() {
        let mut b = setup_backend();
        let resp = handle_recall(
            &mut b,
            RecallRequest { query: "indent".into(), tier: None, tags: vec![], max_results: 10 },
        );
        assert!(!resp.results.is_empty());
        assert!(resp.results[0].key.contains("indent"));
    }

    #[test]
    fn test_handle_recall_by_tier() {
        let mut b = setup_backend();
        let resp = handle_recall(
            &mut b,
            RecallRequest {
                query: "debug".into(),
                tier: Some(MemoryTier::Session),
                tags: vec![],
                max_results: 10,
            },
        );
        assert!(!resp.results.is_empty());
        assert_eq!(resp.results[0].tier, MemoryTier::Session);
    }

    #[test]
    fn test_handle_recall_by_tag() {
        let mut b = setup_backend();
        let resp = handle_recall(
            &mut b,
            RecallRequest {
                query: "convention".into(),
                tier: None,
                tags: vec!["convention".into()],
                max_results: 10,
            },
        );
        assert!(resp.results.len() >= 1);
    }

    #[test]
    fn test_handle_recall_no_match() {
        let mut b = setup_backend();
        let resp = handle_recall(
            &mut b,
            RecallRequest {
                query: "zzzzz_nonexistent_zzzzz".into(),
                tier: None,
                tags: vec![],
                max_results: 10,
            },
        );
        assert!(resp.results.is_empty());
    }

    #[test]
    fn test_recall_response_display() {
        let r = RecallResponse {
            results: vec![RecallResult {
                key: "k".into(),
                value: MemValue::Bool(true),
                tier: MemoryTier::Global,
                relevance: 0.8,
            }],
            total_searched: 5,
        };
        let s = format!("{r}");
        assert!(s.contains("1 results"));
    }

    #[test]
    fn test_recall_result_display() {
        let r = RecallResult {
            key: "k".into(),
            value: MemValue::Number(42.0),
            tier: MemoryTier::Project,
            relevance: 0.75,
        };
        let s = format!("{r}");
        assert!(s.contains("0.75"));
        assert!(s.contains("k"));
    }

    #[test]
    fn test_handle_suggest() {
        let b = setup_backend();
        let resp = handle_suggest(
            &b,
            SuggestRequest { context: "formatting convention indent".into(), max_suggestions: 5 },
        );
        assert!(!resp.suggestions.is_empty());
    }

    #[test]
    fn test_handle_suggest_no_match() {
        let b = setup_backend();
        let resp = handle_suggest(
            &b,
            SuggestRequest { context: "zzzzz_nothing_zzzzz".into(), max_suggestions: 5 },
        );
        assert!(resp.suggestions.is_empty());
    }

    #[test]
    fn test_suggest_response_display() {
        let r = SuggestResponse {
            suggestions: vec![Suggestion {
                key: "k".into(),
                value: MemValue::Bool(true),
                tier: MemoryTier::Project,
                reason: "match".into(),
                confidence: 0.9,
            }],
        };
        let s = format!("{r}");
        assert!(s.contains("1 suggestions"));
    }

    #[test]
    fn test_suggestion_display() {
        let s = Suggestion {
            key: "k".into(),
            value: MemValue::Text("v".into()),
            tier: MemoryTier::Session,
            reason: "relevant".into(),
            confidence: 0.65,
        };
        let d = format!("{s}");
        assert!(d.contains("0.65"));
        assert!(d.contains("relevant"));
    }

    #[test]
    fn test_rap_method_from_str() {
        assert_eq!(RapMethod::from_str("memory.store"), Some(RapMethod::MemoryStore));
        assert_eq!(RapMethod::from_str("memory.recall"), Some(RapMethod::MemoryRecall));
        assert_eq!(RapMethod::from_str("memory.suggest"), Some(RapMethod::MemorySuggest));
        assert_eq!(RapMethod::from_str("other"), None);
    }

    #[test]
    fn test_rap_method_display() {
        assert_eq!(format!("{}", RapMethod::MemoryStore), "memory.store");
    }

    #[test]
    fn test_rap_response_display() {
        let r = RapResponse::Error("bad".into());
        assert!(format!("{r}").contains("ERROR"));
    }

    #[test]
    fn test_backend_new() {
        let b = MemoryBackend::new();
        assert_eq!(b.total_entries(), 0);
    }

    #[test]
    fn test_backend_counts() {
        let b = setup_backend();
        assert_eq!(b.entry_count(MemoryTier::Project), 2);
        assert_eq!(b.entry_count(MemoryTier::Session), 1);
        assert_eq!(b.entry_count(MemoryTier::Global), 1);
        assert_eq!(b.total_entries(), 4);
    }

    #[test]
    fn test_recall_sorts_by_relevance() {
        let mut b = setup_backend();
        let resp = handle_recall(
            &mut b,
            RecallRequest { query: "convention".into(), tier: None, tags: vec![], max_results: 10 },
        );
        if resp.results.len() >= 2 {
            assert!(resp.results[0].relevance >= resp.results[1].relevance);
        }
    }

    #[test]
    fn test_store_overwrites() {
        let mut b = MemoryBackend::new();
        handle_store(
            &mut b,
            StoreRequest {
                key: "k".into(),
                value: MemValue::Number(1.0),
                tier: MemoryTier::Ephemeral,
                tags: vec![],
            },
        );
        handle_store(
            &mut b,
            StoreRequest {
                key: "k".into(),
                value: MemValue::Number(2.0),
                tier: MemoryTier::Ephemeral,
                tags: vec![],
            },
        );
        assert_eq!(b.entry_count(MemoryTier::Ephemeral), 1);
    }

    #[test]
    fn test_recall_max_results() {
        let mut b = MemoryBackend::new();
        for i in 0..10 {
            handle_store(
                &mut b,
                StoreRequest {
                    key: format!("item_{i}"),
                    value: MemValue::Text("item".into()),
                    tier: MemoryTier::Project,
                    tags: vec![],
                },
            );
        }
        let resp = handle_recall(
            &mut b,
            RecallRequest { query: "item".into(), tier: None, tags: vec![], max_results: 3 },
        );
        assert!(resp.results.len() <= 3);
    }

    #[test]
    fn test_compute_relevance_key_match() {
        let entry = StoreEntry::new("test_key", MemValue::Null, MemoryTier::Ephemeral);
        let rel = compute_relevance("test", &entry);
        assert!(rel > 0.0);
    }

    #[test]
    fn test_compute_relevance_no_match() {
        let entry = StoreEntry::new("abc", MemValue::Null, MemoryTier::Ephemeral);
        let rel = compute_relevance("xyz", &entry);
        assert!(rel < 0.01);
    }
}

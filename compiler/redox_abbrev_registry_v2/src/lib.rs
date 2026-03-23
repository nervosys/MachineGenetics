//! # Standard Abbreviation Registry v2
//!
//! Frequency-weighted abbreviation registry with full ecosystem coverage.
//! Provides canonical abbreviations for common Rust/Redox tokens, weighted
//! by corpus frequency data.

use std::collections::HashMap;
use std::fmt;

// ── Abbreviation Entry ───────────────────────────────────────────────

/// One abbreviation mapping in the registry.
#[derive(Debug, Clone)]
pub struct AbbrevEntry {
    pub full: String,
    pub abbrev: String,
    pub category: AbbrevCategory,
    pub frequency_weight: f64,
    pub ecosystem_coverage: f64,
    pub source: AbbrevSource,
    pub is_stable: bool,
}

impl AbbrevEntry {
    pub fn new(full: impl Into<String>, abbrev: impl Into<String>, category: AbbrevCategory) -> Self {
        Self {
            full: full.into(),
            abbrev: abbrev.into(),
            category,
            frequency_weight: 1.0,
            ecosystem_coverage: 0.0,
            source: AbbrevSource::Manual,
            is_stable: false,
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.frequency_weight = weight;
        self
    }

    pub fn with_coverage(mut self, coverage: f64) -> Self {
        self.ecosystem_coverage = coverage;
        self
    }

    pub fn with_source(mut self, source: AbbrevSource) -> Self {
        self.source = source;
        self
    }

    pub fn stabilize(mut self) -> Self {
        self.is_stable = true;
        self
    }

    /// Character savings per occurrence.
    pub fn savings(&self) -> usize {
        self.full.len().saturating_sub(self.abbrev.len())
    }

    /// Weighted savings = savings * frequency_weight.
    pub fn weighted_savings(&self) -> f64 {
        self.savings() as f64 * self.frequency_weight
    }
}

impl fmt::Display for AbbrevEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stable = if self.is_stable { " [stable]" } else { "" };
        write!(f, "{} -> {} ({}){}", self.full, self.abbrev, self.category, stable)
    }
}

// ── Categories ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbbrevCategory {
    Keyword,
    Type,
    Trait,
    Function,
    Module,
    Lifetime,
    Attribute,
    Macro,
    Pattern,
    Operator,
    ControlFlow,
    Concurrency,
    Memory,
    Error,
    Collection,
    IO,
}

impl fmt::Display for AbbrevCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keyword => write!(f, "keyword"),
            Self::Type => write!(f, "type"),
            Self::Trait => write!(f, "trait"),
            Self::Function => write!(f, "function"),
            Self::Module => write!(f, "module"),
            Self::Lifetime => write!(f, "lifetime"),
            Self::Attribute => write!(f, "attribute"),
            Self::Macro => write!(f, "macro"),
            Self::Pattern => write!(f, "pattern"),
            Self::Operator => write!(f, "operator"),
            Self::ControlFlow => write!(f, "control_flow"),
            Self::Concurrency => write!(f, "concurrency"),
            Self::Memory => write!(f, "memory"),
            Self::Error => write!(f, "error"),
            Self::Collection => write!(f, "collection"),
            Self::IO => write!(f, "io"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbbrevSource {
    Manual,
    FrequencyDerived,
    CommunityVoted,
    EcosystemAdopted,
}

impl fmt::Display for AbbrevSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manual => write!(f, "manual"),
            Self::FrequencyDerived => write!(f, "freq"),
            Self::CommunityVoted => write!(f, "voted"),
            Self::EcosystemAdopted => write!(f, "adopted"),
        }
    }
}

// ── Registry ─────────────────────────────────────────────────────────

/// The v2 abbreviation registry.
#[derive(Debug)]
pub struct AbbrevRegistryV2 {
    entries: HashMap<String, AbbrevEntry>,
    reverse: HashMap<String, String>,
    version: String,
}

impl AbbrevRegistryV2 {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            reverse: HashMap::new(),
            version: "2.0.0".into(),
        }
    }

    pub fn register(&mut self, entry: AbbrevEntry) {
        self.reverse.insert(entry.abbrev.clone(), entry.full.clone());
        self.entries.insert(entry.full.clone(), entry);
    }

    pub fn lookup(&self, full: &str) -> Option<&AbbrevEntry> {
        self.entries.get(full)
    }

    pub fn resolve(&self, abbrev: &str) -> Option<&str> {
        self.reverse.get(abbrev).map(|s| s.as_str())
    }

    pub fn abbreviate(&self, full: &str) -> Option<&str> {
        self.entries.get(full).map(|e| e.abbrev.as_str())
    }

    pub fn contains(&self, full: &str) -> bool {
        self.entries.contains_key(full)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn by_category(&self, cat: AbbrevCategory) -> Vec<&AbbrevEntry> {
        self.entries.values().filter(|e| e.category == cat).collect()
    }

    pub fn stable_entries(&self) -> Vec<&AbbrevEntry> {
        self.entries.values().filter(|e| e.is_stable).collect()
    }

    pub fn unstable_entries(&self) -> Vec<&AbbrevEntry> {
        self.entries.values().filter(|e| !e.is_stable).collect()
    }

    /// Top N entries by weighted savings.
    pub fn top_by_savings(&self, n: usize) -> Vec<&AbbrevEntry> {
        let mut sorted: Vec<&AbbrevEntry> = self.entries.values().collect();
        sorted.sort_by(|a, b| b.weighted_savings().partial_cmp(&a.weighted_savings())
            .unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(n);
        sorted
    }

    /// Entries with ecosystem coverage above the threshold.
    pub fn well_covered(&self, min_coverage: f64) -> Vec<&AbbrevEntry> {
        self.entries.values().filter(|e| e.ecosystem_coverage >= min_coverage).collect()
    }

    pub fn remove(&mut self, full: &str) -> Option<AbbrevEntry> {
        if let Some(entry) = self.entries.remove(full) {
            self.reverse.remove(&entry.abbrev);
            Some(entry)
        } else {
            None
        }
    }

    pub fn all_fulls(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    pub fn all_abbrevs(&self) -> Vec<&str> {
        self.reverse.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for AbbrevRegistryV2 {
    fn default() -> Self {
        Self::new()
    }
}

// ── Standard Registry Builder ────────────────────────────────────────

/// Build the standard abbreviation registry with ecosystem-wide defaults.
pub fn build_standard_registry() -> AbbrevRegistryV2 {
    let mut reg = AbbrevRegistryV2::new();

    // Keywords (high frequency, stabilized)
    let keywords = [
        ("function", "fn", 0.95),
        ("public", "pub", 0.92),
        ("mutable", "mut", 0.88),
        ("structure", "struct", 0.90),
        ("enumeration", "enum", 0.85),
        ("implementation", "impl", 0.87),
        ("reference", "ref", 0.80),
        ("constant", "const", 0.78),
        ("module", "mod", 0.75),
        ("return", "ret", 0.70),
    ];
    for (full, abbrev, coverage) in keywords {
        reg.register(AbbrevEntry::new(full, abbrev, AbbrevCategory::Keyword)
            .with_weight(10.0).with_coverage(coverage)
            .with_source(AbbrevSource::EcosystemAdopted).stabilize());
    }

    // Types (high frequency)
    let types = [
        ("String", "Str", 0.90),
        ("Vector", "Vec", 0.88),
        ("HashMap", "HMap", 0.70),
        ("Option", "Opt", 0.65),
        ("Result", "Res", 0.60),
        ("Boolean", "Bool", 0.82),
        ("Integer", "Int", 0.80),
        ("Character", "Char", 0.75),
    ];
    for (full, abbrev, coverage) in types {
        reg.register(AbbrevEntry::new(full, abbrev, AbbrevCategory::Type)
            .with_weight(8.0).with_coverage(coverage)
            .with_source(AbbrevSource::FrequencyDerived).stabilize());
    }

    // Traits
    let traits = [
        ("Iterator", "Iter", 0.85),
        ("Display", "Disp", 0.60),
        ("Debug", "Dbg", 0.72),
        ("Default", "Def", 0.68),
        ("Clone", "Cln", 0.65),
        ("Serialize", "Ser", 0.55),
        ("Deserialize", "Deser", 0.50),
    ];
    for (full, abbrev, coverage) in traits {
        reg.register(AbbrevEntry::new(full, abbrev, AbbrevCategory::Trait)
            .with_weight(6.0).with_coverage(coverage)
            .with_source(AbbrevSource::FrequencyDerived));
    }

    // Functions
    let functions = [
        ("collect", "coll", 0.80),
        ("unwrap", "unw", 0.75),
        ("expect", "exp", 0.70),
        ("contains", "has", 0.65),
        ("push_back", "pb", 0.50),
        ("to_string", "to_str", 0.72),
    ];
    for (full, abbrev, coverage) in functions {
        reg.register(AbbrevEntry::new(full, abbrev, AbbrevCategory::Function)
            .with_weight(5.0).with_coverage(coverage)
            .with_source(AbbrevSource::FrequencyDerived));
    }

    // Control flow
    let control = [
        ("continue", "cont", 0.65),
        ("break", "brk", 0.60),
        ("match", "mtch", 0.55),
    ];
    for (full, abbrev, coverage) in control {
        reg.register(AbbrevEntry::new(full, abbrev, AbbrevCategory::ControlFlow)
            .with_weight(4.0).with_coverage(coverage)
            .with_source(AbbrevSource::FrequencyDerived));
    }

    // Concurrency
    let concurrency = [
        ("async", "asnc", 0.55),
        ("await", "awt", 0.50),
        ("spawn", "spn", 0.45),
        ("channel", "chan", 0.50),
        ("mutex", "mtx", 0.48),
    ];
    for (full, abbrev, coverage) in concurrency {
        reg.register(AbbrevEntry::new(full, abbrev, AbbrevCategory::Concurrency)
            .with_weight(3.0).with_coverage(coverage)
            .with_source(AbbrevSource::FrequencyDerived));
    }

    // Memory
    let memory = [
        ("allocate", "alloc", 0.60),
        ("deallocate", "dealloc", 0.55),
        ("reference", "ref", 0.80),
        ("pointer", "ptr", 0.70),
    ];
    for (full, abbrev, coverage) in memory {
        reg.register(AbbrevEntry::new(full, abbrev, AbbrevCategory::Memory)
            .with_weight(4.0).with_coverage(coverage)
            .with_source(AbbrevSource::EcosystemAdopted));
    }

    // Collections
    let collections = [
        ("BTreeMap", "BMap", 0.45),
        ("HashSet", "HSet", 0.55),
        ("LinkedList", "LList", 0.30),
        ("VecDeque", "VDeq", 0.40),
    ];
    for (full, abbrev, coverage) in collections {
        reg.register(AbbrevEntry::new(full, abbrev, AbbrevCategory::Collection)
            .with_weight(3.0).with_coverage(coverage)
            .with_source(AbbrevSource::FrequencyDerived));
    }

    // Error handling
    let errors = [
        ("Error", "Err", 0.85),
        ("panic", "pnc", 0.50),
        ("unwrap_or", "unw_or", 0.55),
    ];
    for (full, abbrev, coverage) in errors {
        reg.register(AbbrevEntry::new(full, abbrev, AbbrevCategory::Error)
            .with_weight(5.0).with_coverage(coverage)
            .with_source(AbbrevSource::FrequencyDerived));
    }

    reg
}

// ── Registry Stats ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct RegistryStats {
    pub total_entries: usize,
    pub stable_count: usize,
    pub unstable_count: usize,
    pub categories: HashMap<AbbrevCategory, usize>,
    pub avg_savings: f64,
    pub avg_coverage: f64,
}

pub fn compute_stats(registry: &AbbrevRegistryV2) -> RegistryStats {
    let total = registry.len();
    let stable = registry.stable_entries().len();
    let mut categories: HashMap<AbbrevCategory, usize> = HashMap::new();
    let mut total_savings = 0.0;
    let mut total_coverage = 0.0;

    for entry in registry.entries.values() {
        *categories.entry(entry.category).or_insert(0) += 1;
        total_savings += entry.savings() as f64;
        total_coverage += entry.ecosystem_coverage;
    }

    RegistryStats {
        total_entries: total,
        stable_count: stable,
        unstable_count: total - stable,
        categories,
        avg_savings: if total > 0 { total_savings / total as f64 } else { 0.0 },
        avg_coverage: if total > 0 { total_coverage / total as f64 } else { 0.0 },
    }
}

impl fmt::Display for RegistryStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Registry v2 Stats:")?;
        writeln!(f, "  Total: {}", self.total_entries)?;
        writeln!(f, "  Stable: {}", self.stable_count)?;
        writeln!(f, "  Unstable: {}", self.unstable_count)?;
        writeln!(f, "  Avg savings: {:.1} chars", self.avg_savings)?;
        writeln!(f, "  Avg coverage: {:.1}%", self.avg_coverage * 100.0)?;
        for (cat, count) in &self.categories {
            writeln!(f, "  {cat}: {count}")?;
        }
        Ok(())
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abbrev_entry_new() {
        let e = AbbrevEntry::new("function", "fn", AbbrevCategory::Keyword);
        assert_eq!(e.full, "function");
        assert_eq!(e.abbrev, "fn");
    }

    #[test]
    fn test_abbrev_entry_savings() {
        let e = AbbrevEntry::new("function", "fn", AbbrevCategory::Keyword);
        assert_eq!(e.savings(), 6);
    }

    #[test]
    fn test_abbrev_entry_weighted_savings() {
        let e = AbbrevEntry::new("function", "fn", AbbrevCategory::Keyword).with_weight(10.0);
        assert!((e.weighted_savings() - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_abbrev_entry_display() {
        let e = AbbrevEntry::new("function", "fn", AbbrevCategory::Keyword).stabilize();
        let s = format!("{e}");
        assert!(s.contains("function -> fn"));
        assert!(s.contains("[stable]"));
    }

    #[test]
    fn test_abbrev_category_display() {
        assert_eq!(format!("{}", AbbrevCategory::Keyword), "keyword");
        assert_eq!(format!("{}", AbbrevCategory::Concurrency), "concurrency");
    }

    #[test]
    fn test_abbrev_source_display() {
        assert_eq!(format!("{}", AbbrevSource::FrequencyDerived), "freq");
    }

    #[test]
    fn test_registry_new() {
        let r = AbbrevRegistryV2::new();
        assert!(r.is_empty());
        assert_eq!(r.version(), "2.0.0");
    }

    #[test]
    fn test_registry_register_lookup() {
        let mut r = AbbrevRegistryV2::new();
        r.register(AbbrevEntry::new("function", "fn", AbbrevCategory::Keyword));
        assert!(r.contains("function"));
        assert_eq!(r.abbreviate("function"), Some("fn"));
    }

    #[test]
    fn test_registry_resolve() {
        let mut r = AbbrevRegistryV2::new();
        r.register(AbbrevEntry::new("function", "fn", AbbrevCategory::Keyword));
        assert_eq!(r.resolve("fn"), Some("function"));
    }

    #[test]
    fn test_registry_remove() {
        let mut r = AbbrevRegistryV2::new();
        r.register(AbbrevEntry::new("function", "fn", AbbrevCategory::Keyword));
        assert!(r.remove("function").is_some());
        assert!(!r.contains("function"));
    }

    #[test]
    fn test_registry_by_category() {
        let mut r = AbbrevRegistryV2::new();
        r.register(AbbrevEntry::new("function", "fn", AbbrevCategory::Keyword));
        r.register(AbbrevEntry::new("String", "Str", AbbrevCategory::Type));
        assert_eq!(r.by_category(AbbrevCategory::Keyword).len(), 1);
    }

    #[test]
    fn test_registry_stable_unstable() {
        let mut r = AbbrevRegistryV2::new();
        r.register(AbbrevEntry::new("a", "b", AbbrevCategory::Keyword).stabilize());
        r.register(AbbrevEntry::new("c", "d", AbbrevCategory::Type));
        assert_eq!(r.stable_entries().len(), 1);
        assert_eq!(r.unstable_entries().len(), 1);
    }

    #[test]
    fn test_registry_top_by_savings() {
        let mut r = AbbrevRegistryV2::new();
        r.register(AbbrevEntry::new("xx", "x", AbbrevCategory::Keyword).with_weight(1.0));
        r.register(AbbrevEntry::new("implementation", "impl", AbbrevCategory::Keyword).with_weight(10.0));
        let top = r.top_by_savings(1);
        assert_eq!(top[0].full, "implementation");
    }

    #[test]
    fn test_registry_well_covered() {
        let mut r = AbbrevRegistryV2::new();
        r.register(AbbrevEntry::new("a", "b", AbbrevCategory::Keyword).with_coverage(0.9));
        r.register(AbbrevEntry::new("c", "d", AbbrevCategory::Type).with_coverage(0.1));
        assert_eq!(r.well_covered(0.5).len(), 1);
    }

    #[test]
    fn test_registry_all_fulls() {
        let mut r = AbbrevRegistryV2::new();
        r.register(AbbrevEntry::new("foo", "f", AbbrevCategory::Keyword));
        let fulls = r.all_fulls();
        assert!(fulls.contains(&"foo"));
    }

    #[test]
    fn test_registry_all_abbrevs() {
        let mut r = AbbrevRegistryV2::new();
        r.register(AbbrevEntry::new("foo", "f", AbbrevCategory::Keyword));
        let abbrevs = r.all_abbrevs();
        assert!(abbrevs.contains(&"f"));
    }

    #[test]
    fn test_build_standard_registry() {
        let reg = build_standard_registry();
        assert!(reg.len() > 30);
        assert!(reg.contains("function"));
        assert_eq!(reg.abbreviate("function"), Some("fn"));
    }

    #[test]
    fn test_standard_has_types() {
        let reg = build_standard_registry();
        assert!(reg.contains("String"));
        assert!(reg.contains("Vector"));
    }

    #[test]
    fn test_standard_has_traits() {
        let reg = build_standard_registry();
        assert!(reg.contains("Iterator"));
        assert_eq!(reg.abbreviate("Iterator"), Some("Iter"));
    }

    #[test]
    fn test_standard_has_functions() {
        let reg = build_standard_registry();
        assert!(reg.contains("collect"));
    }

    #[test]
    fn test_standard_has_concurrency() {
        let reg = build_standard_registry();
        assert!(reg.contains("channel"));
        assert_eq!(reg.abbreviate("channel"), Some("chan"));
    }

    #[test]
    fn test_standard_has_memory() {
        let reg = build_standard_registry();
        assert!(reg.contains("allocate"));
    }

    #[test]
    fn test_standard_has_collections() {
        let reg = build_standard_registry();
        assert!(reg.contains("HashSet"));
    }

    #[test]
    fn test_standard_has_errors() {
        let reg = build_standard_registry();
        assert!(reg.contains("Error"));
    }

    #[test]
    fn test_standard_stable_keywords() {
        let reg = build_standard_registry();
        let entry = reg.lookup("function").unwrap();
        assert!(entry.is_stable);
    }

    #[test]
    fn test_compute_stats() {
        let reg = build_standard_registry();
        let stats = compute_stats(&reg);
        assert!(stats.total_entries > 30);
        assert!(stats.stable_count > 0);
        assert!(stats.avg_savings > 0.0);
        assert!(stats.avg_coverage > 0.0);
    }

    #[test]
    fn test_stats_display() {
        let reg = build_standard_registry();
        let stats = compute_stats(&reg);
        let s = format!("{stats}");
        assert!(s.contains("Registry v2 Stats"));
    }

    #[test]
    fn test_entry_with_source() {
        let e = AbbrevEntry::new("a", "b", AbbrevCategory::Keyword)
            .with_source(AbbrevSource::CommunityVoted);
        assert_eq!(e.source, AbbrevSource::CommunityVoted);
    }

    #[test]
    fn test_empty_registry_stats() {
        let reg = AbbrevRegistryV2::new();
        let stats = compute_stats(&reg);
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.avg_savings, 0.0);
    }
}

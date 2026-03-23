// redox_global_memory: Global memory network for cross-project pattern sharing.
//
//  Provides opt-in, anonymized sharing of compilation patterns, error
//  frequencies, and successful fix strategies across projects. Includes
//  privacy controls, pattern deduplication, and query APIs.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Privacy level
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrivacyLevel {
    Public,       // fully shared
    Anonymized,   // shared with identifiers stripped
    ProjectOnly,  // only within the same project
    Private,      // never shared
}

impl PrivacyLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Anonymized => "anonymized",
            Self::ProjectOnly => "project-only",
            Self::Private => "private",
        }
    }

    pub fn is_shared(self) -> bool {
        matches!(self, Self::Public | Self::Anonymized)
    }
}

// ---------------------------------------------------------------------------
// Pattern kind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatternKind {
    ErrorFix,        // error code → fix strategy
    Refactoring,     // common refactoring pattern
    Performance,     // performance optimization
    Idiom,           // idiomatic code pattern
    AntiPattern,     // known bad pattern
    Migration,       // migration recipe
    SecurityFix,     // security-related fix
}

impl PatternKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::ErrorFix => "error-fix",
            Self::Refactoring => "refactoring",
            Self::Performance => "performance",
            Self::Idiom => "idiom",
            Self::AntiPattern => "anti-pattern",
            Self::Migration => "migration",
            Self::SecurityFix => "security-fix",
        }
    }
}

// ---------------------------------------------------------------------------
// Shared pattern
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PatternId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedPattern {
    pub id: PatternId,
    pub kind: PatternKind,
    pub title: String,
    pub description: String,
    pub privacy: PrivacyLevel,
    pub tags: Vec<String>,
    pub frequency: u64,     // how often observed
    pub success_rate: u32,  // 0-100 percentage
    pub source_project: Option<String>, // None if anonymized
}

impl SharedPattern {
    pub fn is_effective(&self) -> bool {
        self.success_rate >= 80
    }
}

// ---------------------------------------------------------------------------
// Memory network
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MemoryNetwork {
    patterns: Vec<SharedPattern>,
    index: HashMap<String, usize>,
}

impl MemoryNetwork {
    pub fn new() -> Self {
        Self { patterns: Vec::new(), index: HashMap::new() }
    }

    pub fn submit(&mut self, pattern: SharedPattern) {
        if pattern.privacy == PrivacyLevel::Private {
            return; // never store private patterns in the network
        }
        self.index.insert(pattern.id.0.clone(), self.patterns.len());
        self.patterns.push(pattern);
    }

    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&SharedPattern> {
        self.index.get(id).map(|&i| &self.patterns[i])
    }

    pub fn by_kind(&self, kind: PatternKind) -> Vec<&SharedPattern> {
        self.patterns.iter().filter(|p| p.kind == kind).collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&SharedPattern> {
        self.patterns.iter().filter(|p| p.tags.iter().any(|t| t == tag)).collect()
    }

    pub fn effective(&self) -> Vec<&SharedPattern> {
        self.patterns.iter().filter(|p| p.is_effective()).collect()
    }

    pub fn top_by_frequency(&self, n: usize) -> Vec<&SharedPattern> {
        let mut sorted: Vec<&SharedPattern> = self.patterns.iter().collect();
        sorted.sort_by(|a, b| b.frequency.cmp(&a.frequency));
        sorted.into_iter().take(n).collect()
    }

    pub fn search(&self, keyword: &str) -> Vec<&SharedPattern> {
        let kw = keyword.to_lowercase();
        self.patterns.iter().filter(|p| {
            p.title.to_lowercase().contains(&kw) || p.description.to_lowercase().contains(&kw)
        }).collect()
    }

    pub fn anonymize(&mut self) {
        for p in &mut self.patterns {
            if p.privacy == PrivacyLevel::Public {
                p.privacy = PrivacyLevel::Anonymized;
                p.source_project = None;
            }
        }
    }

    pub fn shared_patterns(&self) -> Vec<&SharedPattern> {
        self.patterns.iter().filter(|p| p.privacy.is_shared()).collect()
    }

    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.patterns.iter().flat_map(|p| p.tags.clone()).collect();
        tags.sort();
        tags.dedup();
        tags
    }

    pub fn stats(&self) -> NetworkStats {
        NetworkStats {
            total_patterns: self.patterns.len(),
            effective: self.effective().len(),
            avg_success_rate: if self.patterns.is_empty() {
                0.0
            } else {
                self.patterns.iter().map(|p| p.success_rate as f64).sum::<f64>()
                    / self.patterns.len() as f64
            },
            total_frequency: self.patterns.iter().map(|p| p.frequency).sum(),
        }
    }
}

impl Default for MemoryNetwork {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkStats {
    pub total_patterns: usize,
    pub effective: usize,
    pub avg_success_rate: f64,
    pub total_frequency: u64,
}

// ---------------------------------------------------------------------------
// Pre-built sample network
// ---------------------------------------------------------------------------

fn pat(id: &str, kind: PatternKind, title: &str, desc: &str, tags: &[&str], freq: u64, success: u32) -> SharedPattern {
    SharedPattern {
        id: PatternId(id.to_string()),
        kind,
        title: title.to_string(),
        description: desc.to_string(),
        privacy: PrivacyLevel::Anonymized,
        tags: tags.iter().map(|s| s.to_string()).collect(),
        frequency: freq,
        success_rate: success,
        source_project: None,
    }
}

pub fn build_sample_network() -> MemoryNetwork {
    let mut net = MemoryNetwork::new();
    net.submit(pat("GM-001", PatternKind::ErrorFix, "E0382 use-after-move fix",
        "Clone the value before the move or restructure ownership.", &["ownership", "E0382"], 5420, 92));
    net.submit(pat("GM-002", PatternKind::ErrorFix, "E0597 lifetime too short",
        "Extend the lifetime by moving the binding to an outer scope.", &["lifetime", "E0597"], 3100, 85));
    net.submit(pat("GM-003", PatternKind::Refactoring, "Extract method refactoring",
        "Extract repeated code into a helper function.", &["refactoring", "DRY"], 2800, 90));
    net.submit(pat("GM-004", PatternKind::Performance, "Avoid unnecessary collect",
        "Use iterator chaining instead of intermediate Vec collection.", &["performance", "iterator"], 4200, 88));
    net.submit(pat("GM-005", PatternKind::Idiom, "Use ? operator for error handling",
        "Replace match on Result with the ? operator.", &["idiom", "error-handling"], 6100, 95));
    net.submit(pat("GM-006", PatternKind::AntiPattern, "Unwrap in library code",
        "Avoid .unwrap() in library code; prefer returning Result.", &["anti-pattern", "error-handling"], 3500, 78));
    net.submit(pat("GM-007", PatternKind::SecurityFix, "SQL injection prevention",
        "Use parameterized queries instead of string concatenation.", &["security", "sql"], 1800, 97));
    net.submit(pat("GM-008", PatternKind::Migration, "Async migration recipe",
        "Convert sync I/O to async/await with tokio runtime.", &["migration", "async"], 2200, 82));
    net
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn net() -> MemoryNetwork {
        build_sample_network()
    }

    // -- PrivacyLevel --
    #[test]
    fn test_privacy_labels() {
        assert_eq!(PrivacyLevel::Public.label(), "public");
        assert_eq!(PrivacyLevel::Private.label(), "private");
    }

    #[test]
    fn test_privacy_is_shared() {
        assert!(PrivacyLevel::Public.is_shared());
        assert!(PrivacyLevel::Anonymized.is_shared());
        assert!(!PrivacyLevel::Private.is_shared());
        assert!(!PrivacyLevel::ProjectOnly.is_shared());
    }

    // -- PatternKind --
    #[test]
    fn test_pattern_kind_labels() {
        assert_eq!(PatternKind::ErrorFix.label(), "error-fix");
        assert_eq!(PatternKind::SecurityFix.label(), "security-fix");
    }

    // -- Network basics --
    #[test]
    fn test_network_len() {
        assert_eq!(net().len(), 8);
    }

    #[test]
    fn test_network_not_empty() {
        assert!(!net().is_empty());
    }

    // -- get --
    #[test]
    fn test_get_existing() {
        let n = net();
        let p = n.get("GM-001").unwrap();
        assert!(p.title.contains("E0382"));
    }

    #[test]
    fn test_get_missing() {
        assert!(net().get("NOPE").is_none());
    }

    // -- by_kind --
    #[test]
    fn test_by_kind_error_fix() {
        let n = net();
        assert_eq!(n.by_kind(PatternKind::ErrorFix).len(), 2);
    }

    // -- by_tag --
    #[test]
    fn test_by_tag() {
        let n = net();
        assert_eq!(n.by_tag("error-handling").len(), 2);
    }

    // -- effective --
    #[test]
    fn test_effective() {
        let n = net();
        let eff = n.effective();
        assert!(eff.len() >= 6);
    }

    // -- top_by_frequency --
    #[test]
    fn test_top_by_frequency() {
        let n = net();
        let top = n.top_by_frequency(3);
        assert_eq!(top.len(), 3);
        assert!(top[0].frequency >= top[1].frequency);
    }

    // -- search --
    #[test]
    fn test_search() {
        let n = net();
        let results = n.search("lifetime");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_case_insensitive() {
        let n = net();
        assert!(!n.search("SQL").is_empty());
    }

    // -- anonymize --
    #[test]
    fn test_anonymize() {
        let mut n = MemoryNetwork::new();
        n.submit(SharedPattern {
            id: PatternId("X".into()),
            kind: PatternKind::Idiom,
            title: "T".into(),
            description: "D".into(),
            privacy: PrivacyLevel::Public,
            tags: vec![],
            frequency: 1,
            success_rate: 90,
            source_project: Some("proj".into()),
        });
        n.anonymize();
        let p = n.get("X").unwrap();
        assert_eq!(p.privacy, PrivacyLevel::Anonymized);
        assert!(p.source_project.is_none());
    }

    // -- private patterns not stored --
    #[test]
    fn test_private_not_stored() {
        let mut n = MemoryNetwork::new();
        n.submit(SharedPattern {
            id: PatternId("P".into()),
            kind: PatternKind::Idiom,
            title: "T".into(),
            description: "D".into(),
            privacy: PrivacyLevel::Private,
            tags: vec![],
            frequency: 1,
            success_rate: 50,
            source_project: None,
        });
        assert!(n.is_empty());
    }

    // -- shared_patterns --
    #[test]
    fn test_shared_patterns() {
        let n = net();
        assert_eq!(n.shared_patterns().len(), 8); // all anonymized
    }

    // -- all_tags --
    #[test]
    fn test_all_tags() {
        let n = net();
        assert!(n.all_tags().len() >= 5);
    }

    // -- stats --
    #[test]
    fn test_stats() {
        let n = net();
        let s = n.stats();
        assert_eq!(s.total_patterns, 8);
        assert!(s.avg_success_rate > 0.0);
        assert!(s.total_frequency > 0);
    }

    // -- is_effective --
    #[test]
    fn test_is_effective() {
        let p = pat("X", PatternKind::Idiom, "T", "D", &[], 1, 90);
        assert!(p.is_effective());
        let p2 = pat("Y", PatternKind::Idiom, "T", "D", &[], 1, 50);
        assert!(!p2.is_effective());
    }

    // -- default --
    #[test]
    fn test_default() {
        let n = MemoryNetwork::default();
        assert!(n.is_empty());
    }

    // -- PatternId hash --
    #[test]
    fn test_pattern_id_hash() {
        let mut set = std::collections::HashSet::new();
        set.insert(PatternId("A".into()));
        set.insert(PatternId("B".into()));
        assert_eq!(set.len(), 2);
    }
}

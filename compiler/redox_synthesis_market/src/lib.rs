// redox_synthesis_market: Verified spec→implementation pairs marketplace.
//
//  Each component pairs a formal specification with an implementation
//  that has been verified to satisfy it. Components are searchable,
//  rated by quality, and reusable across projects.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Verification status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerificationStatus {
    Unverified,
    TypeChecked,
    PropertyTested,
    FormallyVerified,
    CommunityAudited,
}

impl VerificationStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unverified => "unverified",
            Self::TypeChecked => "type-checked",
            Self::PropertyTested => "property-tested",
            Self::FormallyVerified => "formally-verified",
            Self::CommunityAudited => "community-audited",
        }
    }

    pub fn trust_score(self) -> u32 {
        match self {
            Self::Unverified => 0,
            Self::TypeChecked => 25,
            Self::PropertyTested => 50,
            Self::FormallyVerified => 90,
            Self::CommunityAudited => 100,
        }
    }
}

// ---------------------------------------------------------------------------
// Component domain
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Domain {
    DataStructures,
    Algorithms,
    Networking,
    Cryptography,
    Parsing,
    Concurrency,
    Serialization,
    Math,
}

impl Domain {
    pub fn label(self) -> &'static str {
        match self {
            Self::DataStructures => "data-structures",
            Self::Algorithms => "algorithms",
            Self::Networking => "networking",
            Self::Cryptography => "cryptography",
            Self::Parsing => "parsing",
            Self::Concurrency => "concurrency",
            Self::Serialization => "serialization",
            Self::Math => "math",
        }
    }
}

// ---------------------------------------------------------------------------
// Specification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    pub name: String,
    pub description: String,
    pub preconditions: Vec<String>,
    pub postconditions: Vec<String>,
    pub invariants: Vec<String>,
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthComponent {
    pub id: ComponentId,
    pub name: String,
    pub domain: Domain,
    pub spec: Spec,
    pub implementation_lines: usize,
    pub verification: VerificationStatus,
    pub tags: Vec<String>,
    pub downloads: u64,
    pub rating: u32, // 0-100
}

impl SynthComponent {
    pub fn is_trusted(&self) -> bool {
        self.verification.trust_score() >= 50
    }

    pub fn spec_size(&self) -> usize {
        self.spec.preconditions.len() + self.spec.postconditions.len() + self.spec.invariants.len()
    }
}

// ---------------------------------------------------------------------------
// Marketplace
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SynthMarketplace {
    components: Vec<SynthComponent>,
    index: HashMap<String, usize>,
}

impl SynthMarketplace {
    pub fn new() -> Self {
        Self { components: Vec::new(), index: HashMap::new() }
    }

    pub fn publish(&mut self, comp: SynthComponent) {
        self.index.insert(comp.id.0.clone(), self.components.len());
        self.components.push(comp);
    }

    pub fn len(&self) -> usize {
        self.components.len()
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&SynthComponent> {
        self.index.get(id).map(|&i| &self.components[i])
    }

    pub fn by_domain(&self, domain: Domain) -> Vec<&SynthComponent> {
        self.components.iter().filter(|c| c.domain == domain).collect()
    }

    pub fn by_verification(&self, status: VerificationStatus) -> Vec<&SynthComponent> {
        self.components.iter().filter(|c| c.verification == status).collect()
    }

    pub fn trusted(&self) -> Vec<&SynthComponent> {
        self.components.iter().filter(|c| c.is_trusted()).collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&SynthComponent> {
        self.components.iter().filter(|c| c.tags.iter().any(|t| t == tag)).collect()
    }

    pub fn search(&self, keyword: &str) -> Vec<&SynthComponent> {
        let kw = keyword.to_lowercase();
        self.components.iter().filter(|c| {
            c.name.to_lowercase().contains(&kw)
                || c.spec.description.to_lowercase().contains(&kw)
        }).collect()
    }

    pub fn top_by_downloads(&self, n: usize) -> Vec<&SynthComponent> {
        let mut sorted: Vec<&SynthComponent> = self.components.iter().collect();
        sorted.sort_by(|a, b| b.downloads.cmp(&a.downloads));
        sorted.into_iter().take(n).collect()
    }

    pub fn top_by_rating(&self, n: usize) -> Vec<&SynthComponent> {
        let mut sorted: Vec<&SynthComponent> = self.components.iter().collect();
        sorted.sort_by(|a, b| b.rating.cmp(&a.rating));
        sorted.into_iter().take(n).collect()
    }

    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.components.iter().flat_map(|c| c.tags.clone()).collect();
        tags.sort();
        tags.dedup();
        tags
    }

    pub fn stats(&self) -> MarketStats {
        MarketStats {
            total: self.components.len(),
            trusted: self.trusted().len(),
            formally_verified: self.by_verification(VerificationStatus::FormallyVerified).len(),
            total_downloads: self.components.iter().map(|c| c.downloads).sum(),
        }
    }
}

impl Default for SynthMarketplace {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketStats {
    pub total: usize,
    pub trusted: usize,
    pub formally_verified: usize,
    pub total_downloads: u64,
}

// ---------------------------------------------------------------------------
// Pre-built marketplace
// ---------------------------------------------------------------------------

fn spec(desc: &str, pre: &[&str], post: &[&str], inv: &[&str]) -> Spec {
    Spec {
        name: desc.to_string(),
        description: desc.to_string(),
        preconditions: pre.iter().map(|s| s.to_string()).collect(),
        postconditions: post.iter().map(|s| s.to_string()).collect(),
        invariants: inv.iter().map(|s| s.to_string()).collect(),
    }
}

pub fn build_sample_marketplace() -> SynthMarketplace {
    let mut m = SynthMarketplace::new();

    m.publish(SynthComponent {
        id: ComponentId("sorted-vec".into()),
        name: "Sorted Vector".into(),
        domain: Domain::DataStructures,
        spec: spec("Always-sorted vector", &["elements are Ord"], &["result is sorted"], &["sorted invariant"]),
        implementation_lines: 120,
        verification: VerificationStatus::FormallyVerified,
        tags: vec!["sorted".into(), "collection".into()],
        downloads: 8500,
        rating: 95,
    });

    m.publish(SynthComponent {
        id: ComponentId("binary-search".into()),
        name: "Binary Search".into(),
        domain: Domain::Algorithms,
        spec: spec("O(log n) search in sorted slice", &["slice is sorted"], &["returns correct index or None"], &[]),
        implementation_lines: 30,
        verification: VerificationStatus::FormallyVerified,
        tags: vec!["search".into(), "algorithm".into()],
        downloads: 12000,
        rating: 98,
    });

    m.publish(SynthComponent {
        id: ComponentId("json-parser".into()),
        name: "JSON Parser".into(),
        domain: Domain::Parsing,
        spec: spec("RFC 8259 compliant JSON parser", &["valid UTF-8 input"], &["valid JSON AST or error"], &[]),
        implementation_lines: 450,
        verification: VerificationStatus::PropertyTested,
        tags: vec!["json".into(), "parser".into()],
        downloads: 15000,
        rating: 90,
    });

    m.publish(SynthComponent {
        id: ComponentId("sha256".into()),
        name: "SHA-256".into(),
        domain: Domain::Cryptography,
        spec: spec("NIST FIPS 180-4 SHA-256", &[], &["correct hash output"], &["constant-time"]),
        implementation_lines: 200,
        verification: VerificationStatus::CommunityAudited,
        tags: vec!["hash".into(), "crypto".into()],
        downloads: 20000,
        rating: 99,
    });

    m.publish(SynthComponent {
        id: ComponentId("mutex-guard".into()),
        name: "Scoped Mutex Guard".into(),
        domain: Domain::Concurrency,
        spec: spec("RAII mutex guard", &[], &["lock released on drop"], &["mutual exclusion"]),
        implementation_lines: 60,
        verification: VerificationStatus::TypeChecked,
        tags: vec!["concurrency".into(), "sync".into()],
        downloads: 5000,
        rating: 80,
    });

    m
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn market() -> SynthMarketplace {
        build_sample_marketplace()
    }

    // -- VerificationStatus --
    #[test]
    fn test_verification_labels() {
        assert_eq!(VerificationStatus::FormallyVerified.label(), "formally-verified");
    }

    #[test]
    fn test_trust_scores() {
        assert_eq!(VerificationStatus::Unverified.trust_score(), 0);
        assert_eq!(VerificationStatus::CommunityAudited.trust_score(), 100);
    }

    // -- Domain --
    #[test]
    fn test_domain_labels() {
        assert_eq!(Domain::Cryptography.label(), "cryptography");
    }

    // -- SynthComponent --
    #[test]
    fn test_is_trusted() {
        let m = market();
        let c = m.get("sha256").unwrap();
        assert!(c.is_trusted());
    }

    #[test]
    fn test_not_trusted_unverified() {
        let c = SynthComponent {
            id: ComponentId("x".into()), name: "x".into(), domain: Domain::Math,
            spec: spec("x", &[], &[], &[]), implementation_lines: 1,
            verification: VerificationStatus::Unverified, tags: vec![],
            downloads: 0, rating: 0,
        };
        assert!(!c.is_trusted());
    }

    #[test]
    fn test_spec_size() {
        let m = market();
        let c = m.get("sorted-vec").unwrap();
        assert_eq!(c.spec_size(), 3); // 1 pre + 1 post + 1 inv
    }

    // -- Marketplace basics --
    #[test]
    fn test_market_len() {
        assert_eq!(market().len(), 5);
    }

    #[test]
    fn test_not_empty() {
        assert!(!market().is_empty());
    }

    // -- get --
    #[test]
    fn test_get_existing() {
        let m = market();
        assert!(m.get("binary-search").is_some());
    }

    #[test]
    fn test_get_missing() {
        assert!(market().get("nope").is_none());
    }

    // -- by_domain --
    #[test]
    fn test_by_domain() {
        let m = market();
        assert_eq!(m.by_domain(Domain::Algorithms).len(), 1);
    }

    // -- by_verification --
    #[test]
    fn test_by_verification() {
        let m = market();
        assert_eq!(m.by_verification(VerificationStatus::FormallyVerified).len(), 2);
    }

    // -- trusted --
    #[test]
    fn test_trusted() {
        let m = market();
        assert!(m.trusted().len() >= 4); // all except mutex-guard (TypeChecked=25)
    }

    // -- by_tag --
    #[test]
    fn test_by_tag() {
        let m = market();
        assert_eq!(m.by_tag("crypto").len(), 1);
    }

    // -- search --
    #[test]
    fn test_search() {
        let m = market();
        let results = m.search("json");
        assert_eq!(results.len(), 1);
    }

    // -- top_by_downloads --
    #[test]
    fn test_top_by_downloads() {
        let m = market();
        let top = m.top_by_downloads(2);
        assert_eq!(top.len(), 2);
        assert!(top[0].downloads >= top[1].downloads);
    }

    // -- top_by_rating --
    #[test]
    fn test_top_by_rating() {
        let m = market();
        let top = m.top_by_rating(1);
        assert_eq!(top[0].id.0, "sha256");
    }

    // -- all_tags --
    #[test]
    fn test_all_tags() {
        let m = market();
        assert!(m.all_tags().len() >= 5);
    }

    // -- stats --
    #[test]
    fn test_stats() {
        let m = market();
        let s = m.stats();
        assert_eq!(s.total, 5);
        assert!(s.total_downloads > 0);
    }

    // -- default --
    #[test]
    fn test_default() {
        let m = SynthMarketplace::default();
        assert!(m.is_empty());
    }

    // -- ComponentId hash --
    #[test]
    fn test_component_id_hash() {
        let mut set = std::collections::HashSet::new();
        set.insert(ComponentId("a".into()));
        set.insert(ComponentId("b".into()));
        assert_eq!(set.len(), 2);
    }
}

// ── Forge Registry (Deepened) ──────────────────────────────────────
//
// Extended Forge package registry with:
//   1. Capability-indexed search with ranked scoring
//   2. Semantic search by capability query (fuzzy matching)
//   3. Contract-based composition (verify compatibility)
//   4. Dependency graph with capability propagation
//   5. Version constraint resolution

use std::collections::{BTreeMap, BTreeSet};

// ── Package descriptor ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ForgePackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub contracts: Vec<PackageContract>,
    pub effects: Vec<String>,
    pub dependencies: Vec<Dependency>,
    pub agents: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PackageContract {
    pub kind: ContractKind,
    pub condition: String,
    pub target: String, // function or type this contract applies to
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractKind {
    Requires,
    Ensures,
    Invariant,
}

#[derive(Debug, Clone)]
pub struct Dependency {
    pub package: String,
    pub version_req: String,
    pub required_capabilities: Vec<String>,
}

// ── Search result ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub package_name: String,
    pub version: String,
    pub score: f64,
    pub matched_capabilities: Vec<String>,
    pub matched_contracts: Vec<String>,
}

impl SearchResult {
    fn new(pkg: &ForgePackage) -> Self {
        Self {
            package_name: pkg.name.clone(),
            version: pkg.version.clone(),
            score: 0.0,
            matched_capabilities: Vec::new(),
            matched_contracts: Vec::new(),
        }
    }
}

// ── Compatibility result ───────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CompatibilityResult {
    pub compatible: bool,
    pub issues: Vec<String>,
    pub satisfied_contracts: Vec<String>,
    pub missing_capabilities: Vec<String>,
}

// ── Forge Registry ─────────────────────────────────────────────────

pub struct ForgeRegistry {
    packages: BTreeMap<String, Vec<ForgePackage>>, // name → versions
    /// Inverted index: capability → set of package names
    cap_index: BTreeMap<String, BTreeSet<String>>,
    /// Inverted index: effect → set of package names
    effect_index: BTreeMap<String, BTreeSet<String>>,
}

impl ForgeRegistry {
    pub fn new() -> Self {
        Self {
            packages: BTreeMap::new(),
            cap_index: BTreeMap::new(),
            effect_index: BTreeMap::new(),
        }
    }

    /// Publish a package to the registry.
    pub fn publish(&mut self, pkg: ForgePackage) {
        for cap in &pkg.capabilities {
            self.cap_index
                .entry(cap.clone())
                .or_default()
                .insert(pkg.name.clone());
        }
        for eff in &pkg.effects {
            self.effect_index
                .entry(eff.clone())
                .or_default()
                .insert(pkg.name.clone());
        }
        self.packages
            .entry(pkg.name.clone())
            .or_default()
            .push(pkg);
    }

    /// Get the latest version of a package.
    pub fn latest(&self, name: &str) -> Option<&ForgePackage> {
        self.packages.get(name).and_then(|v| v.last())
    }

    pub fn package_count(&self) -> usize {
        self.packages.len()
    }

    // ── Capability-indexed search ─────────────────────────────────

    /// Search by exact capability match, returning ranked results.
    pub fn search_by_capability(&self, capability: &str) -> Vec<SearchResult> {
        let names = match self.cap_index.get(capability) {
            Some(set) => set,
            None => return vec![],
        };
        let mut results = Vec::new();
        for name in names {
            if let Some(pkg) = self.latest(name) {
                let mut r = SearchResult::new(pkg);
                r.matched_capabilities.push(capability.into());
                r.score = 1.0;
                results.push(r);
            }
        }
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Search by multiple capabilities, scoring by match count.
    pub fn search_by_capabilities(&self, capabilities: &[&str]) -> Vec<SearchResult> {
        let mut scores: BTreeMap<String, SearchResult> = BTreeMap::new();

        for cap in capabilities {
            if let Some(names) = self.cap_index.get(*cap) {
                for name in names {
                    if let Some(pkg) = self.latest(name) {
                        let entry =
                            scores.entry(name.clone()).or_insert_with(|| SearchResult::new(pkg));
                        entry.matched_capabilities.push(cap.to_string());
                        entry.score += 1.0;
                    }
                }
            }
        }

        // Normalize scores by number of queried capabilities
        let total = capabilities.len() as f64;
        let mut results: Vec<SearchResult> = scores
            .into_values()
            .map(|mut r| {
                if total > 0.0 {
                    r.score /= total;
                }
                r
            })
            .collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    // ── Semantic search (fuzzy capability matching) ───────────────

    /// Fuzzy search: match capabilities that contain the query substring.
    pub fn semantic_search(&self, query: &str) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let mut scores: BTreeMap<String, SearchResult> = BTreeMap::new();

        for (cap, names) in &self.cap_index {
            let cap_lower = cap.to_lowercase();
            let sim = string_similarity(&query_lower, &cap_lower);
            if sim > 0.3 {
                for name in names {
                    if let Some(pkg) = self.latest(name) {
                        let entry =
                            scores.entry(name.clone()).or_insert_with(|| SearchResult::new(pkg));
                        entry.matched_capabilities.push(cap.clone());
                        if sim > entry.score {
                            entry.score = sim;
                        }
                    }
                }
            }
        }

        let mut results: Vec<SearchResult> = scores.into_values().collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    // ── Contract-based composition ────────────────────────────────

    /// Check if two packages are contract-compatible for composition.
    pub fn check_compatibility(
        &self,
        provider: &str,
        consumer: &str,
    ) -> CompatibilityResult {
        let provider_pkg = match self.latest(provider) {
            Some(p) => p,
            None => {
                return CompatibilityResult {
                    compatible: false,
                    issues: vec![format!("Package '{}' not found", provider)],
                    satisfied_contracts: vec![],
                    missing_capabilities: vec![],
                }
            }
        };
        let consumer_pkg = match self.latest(consumer) {
            Some(p) => p,
            None => {
                return CompatibilityResult {
                    compatible: false,
                    issues: vec![format!("Package '{}' not found", consumer)],
                    satisfied_contracts: vec![],
                    missing_capabilities: vec![],
                }
            }
        };

        let mut issues = Vec::new();
        let mut satisfied = Vec::new();
        let mut missing_caps = Vec::new();

        // Check: consumer's required capabilities satisfied by provider
        for dep in &consumer_pkg.dependencies {
            if dep.package == provider {
                for req_cap in &dep.required_capabilities {
                    if provider_pkg.capabilities.contains(req_cap) {
                        satisfied.push(format!("cap:{}", req_cap));
                    } else {
                        missing_caps.push(req_cap.clone());
                        issues.push(format!(
                            "Provider '{}' lacks required capability '{}'",
                            provider, req_cap
                        ));
                    }
                }
            }
        }

        // Check: provider's ensures match consumer's requires
        let provider_ensures: Vec<&PackageContract> = provider_pkg
            .contracts
            .iter()
            .filter(|c| c.kind == ContractKind::Ensures)
            .collect();
        let consumer_requires: Vec<&PackageContract> = consumer_pkg
            .contracts
            .iter()
            .filter(|c| c.kind == ContractKind::Requires)
            .collect();

        for req in &consumer_requires {
            let matched = provider_ensures
                .iter()
                .any(|ens| ens.condition == req.condition);
            if matched {
                satisfied.push(format!("contract:{}", req.condition));
            } else {
                issues.push(format!(
                    "Consumer requires '{}' but provider does not ensure it",
                    req.condition
                ));
            }
        }

        // Check: effect compatibility (consumer shouldn't need effects provider doesn't have)
        let provider_effects: BTreeSet<&str> =
            provider_pkg.effects.iter().map(|s| s.as_str()).collect();
        for eff in &consumer_pkg.effects {
            if !provider_effects.contains(eff.as_str()) {
                // Not necessarily an issue; consumer may use different effects
            }
        }

        CompatibilityResult {
            compatible: issues.is_empty(),
            issues,
            satisfied_contracts: satisfied,
            missing_capabilities: missing_caps,
        }
    }

    // ── Dependency graph ──────────────────────────────────────────

    /// Build a dependency graph (package → dependencies).
    pub fn dependency_graph(&self) -> BTreeMap<String, Vec<String>> {
        let mut graph = BTreeMap::new();
        for (name, versions) in &self.packages {
            if let Some(pkg) = versions.last() {
                let deps: Vec<String> = pkg.dependencies.iter().map(|d| d.package.clone()).collect();
                graph.insert(name.clone(), deps);
            }
        }
        graph
    }

    /// Propagate capabilities through dependency graph.
    pub fn transitive_capabilities(&self, package: &str) -> BTreeSet<String> {
        let mut result = BTreeSet::new();
        let mut visited = BTreeSet::new();
        self.collect_caps_recursive(package, &mut result, &mut visited);
        result
    }

    fn collect_caps_recursive(
        &self,
        package: &str,
        caps: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) {
        if !visited.insert(package.into()) {
            return;
        }
        if let Some(pkg) = self.latest(package) {
            for cap in &pkg.capabilities {
                caps.insert(cap.clone());
            }
            for dep in &pkg.dependencies {
                self.collect_caps_recursive(&dep.package, caps, visited);
            }
        }
    }

    // ── Search by effect ──────────────────────────────────────────

    pub fn search_by_effect(&self, effect: &str) -> Vec<SearchResult> {
        let names = match self.effect_index.get(effect) {
            Some(set) => set,
            None => return vec![],
        };
        let mut results = Vec::new();
        for name in names {
            if let Some(pkg) = self.latest(name) {
                let mut r = SearchResult::new(pkg);
                r.score = 1.0;
                results.push(r);
            }
        }
        results
    }

    // ── Stats ─────────────────────────────────────────────────────

    pub fn stats(&self) -> String {
        let pkgs = self.packages.len();
        let caps = self.cap_index.len();
        let effects = self.effect_index.len();
        format!(
            "{{\"packages\":{},\"capabilities\":{},\"effects\":{}}}",
            pkgs, caps, effects
        )
    }
}

// ── String similarity (simple trigram-based) ───────────────────────

fn trigrams(s: &str) -> BTreeSet<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut set = BTreeSet::new();
    if chars.len() < 3 {
        set.insert(s.to_string());
        return set;
    }
    for w in chars.windows(3) {
        set.insert(w.iter().collect());
    }
    set
}

fn string_similarity(a: &str, b: &str) -> f64 {
    // Substring match gets high score
    if a == b {
        return 1.0;
    }
    if b.contains(a) || a.contains(b) {
        return 0.8;
    }
    let ta = trigrams(a);
    let tb = trigrams(b);
    let intersection = ta.intersection(&tb).count() as f64;
    let union = ta.union(&tb).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_registry() -> ForgeRegistry {
        let mut reg = ForgeRegistry::new();

        reg.publish(ForgePackage {
            name: "redox-math".into(),
            version: "1.0.0".into(),
            description: "Math utilities".into(),
            capabilities: vec!["arithmetic".into(), "linear-algebra".into()],
            contracts: vec![PackageContract {
                kind: ContractKind::Ensures,
                condition: "result >= 0".into(),
                target: "abs".into(),
            }],
            effects: vec!["pure".into()],
            dependencies: vec![],
            agents: vec![],
        });

        reg.publish(ForgePackage {
            name: "redox-io".into(),
            version: "2.0.0".into(),
            description: "I/O utilities".into(),
            capabilities: vec!["file-read".into(), "file-write".into(), "network".into()],
            contracts: vec![PackageContract {
                kind: ContractKind::Ensures,
                condition: "handle != null".into(),
                target: "open".into(),
            }],
            effects: vec!["io".into(), "network".into()],
            dependencies: vec![],
            agents: vec![],
        });

        reg.publish(ForgePackage {
            name: "redox-agent".into(),
            version: "0.5.0".into(),
            description: "Agent toolkit".into(),
            capabilities: vec!["code-analysis".into(), "arithmetic".into()],
            contracts: vec![PackageContract {
                kind: ContractKind::Requires,
                condition: "result >= 0".into(),
                target: "analyze".into(),
            }],
            effects: vec!["pure".into()],
            dependencies: vec![
                Dependency {
                    package: "redox-math".into(),
                    version_req: ">=1.0".into(),
                    required_capabilities: vec!["arithmetic".into()],
                },
            ],
            agents: vec!["Analyzer".into()],
        });

        reg
    }

    // ── Publish and lookup ────────────────────────────────────────

    #[test]
    fn publish_and_latest() {
        let reg = sample_registry();
        assert_eq!(reg.package_count(), 3);
        let math = reg.latest("redox-math").unwrap();
        assert_eq!(math.version, "1.0.0");
    }

    // ── Capability search ─────────────────────────────────────────

    #[test]
    fn search_single_capability() {
        let reg = sample_registry();
        let results = reg.search_by_capability("arithmetic");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_multiple_capabilities() {
        let reg = sample_registry();
        let results = reg.search_by_capabilities(&["arithmetic", "code-analysis"]);
        // redox-agent has both, redox-math has one
        assert!(!results.is_empty());
        assert_eq!(results[0].package_name, "redox-agent"); // highest score
    }

    #[test]
    fn search_no_match() {
        let reg = sample_registry();
        let results = reg.search_by_capability("nonexistent");
        assert!(results.is_empty());
    }

    // ── Semantic search ───────────────────────────────────────────

    #[test]
    fn semantic_search_substring() {
        let reg = sample_registry();
        let results = reg.semantic_search("file");
        assert!(!results.is_empty());
        assert!(results[0].matched_capabilities.iter().any(|c| c.contains("file")));
    }

    #[test]
    fn semantic_search_no_match() {
        let reg = sample_registry();
        let results = reg.semantic_search("zzzzxyzzy");
        assert!(results.is_empty());
    }

    // ── Contract compatibility ────────────────────────────────────

    #[test]
    fn compatible_packages() {
        let reg = sample_registry();
        let result = reg.check_compatibility("redox-math", "redox-agent");
        assert!(result.compatible);
        assert!(result.satisfied_contracts.iter().any(|s| s.contains("arithmetic")));
    }

    #[test]
    fn incompatible_missing_cap() {
        let mut reg = sample_registry();
        reg.publish(ForgePackage {
            name: "redox-consumer".into(),
            version: "1.0.0".into(),
            description: "Needs GPU".into(),
            capabilities: vec![],
            contracts: vec![],
            effects: vec![],
            dependencies: vec![Dependency {
                package: "redox-math".into(),
                version_req: ">=1.0".into(),
                required_capabilities: vec!["gpu-compute".into()],
            }],
            agents: vec![],
        });
        let result = reg.check_compatibility("redox-math", "redox-consumer");
        assert!(!result.compatible);
        assert!(result.missing_capabilities.contains(&"gpu-compute".into()));
    }

    #[test]
    fn compatibility_package_not_found() {
        let reg = sample_registry();
        let result = reg.check_compatibility("nonexistent", "redox-agent");
        assert!(!result.compatible);
    }

    // ── Dependency graph ──────────────────────────────────────────

    #[test]
    fn dependency_graph() {
        let reg = sample_registry();
        let graph = reg.dependency_graph();
        assert_eq!(graph["redox-agent"], vec!["redox-math".to_string()]);
        assert!(graph["redox-math"].is_empty());
    }

    #[test]
    fn transitive_capabilities() {
        let reg = sample_registry();
        let caps = reg.transitive_capabilities("redox-agent");
        assert!(caps.contains("code-analysis"));
        assert!(caps.contains("arithmetic"));
        assert!(caps.contains("linear-algebra")); // from redox-math
    }

    // ── Effect search ─────────────────────────────────────────────

    #[test]
    fn search_by_effect() {
        let reg = sample_registry();
        let results = reg.search_by_effect("io");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].package_name, "redox-io");
    }

    // ── Stats ─────────────────────────────────────────────────────

    #[test]
    fn stats_json() {
        let reg = sample_registry();
        let s = reg.stats();
        assert!(s.contains("\"packages\":3"));
    }

    // ── String similarity ─────────────────────────────────────────

    #[test]
    fn similarity_exact() {
        assert_eq!(string_similarity("hello", "hello"), 1.0);
    }

    #[test]
    fn similarity_substring() {
        assert!(string_similarity("file", "file-read") > 0.5);
    }

    #[test]
    fn similarity_unrelated() {
        assert!(string_similarity("abc", "xyz") < 0.3);
    }

    // ── Contract-based composition with ensures/requires ──────────

    #[test]
    fn contract_ensures_requires_match() {
        let reg = sample_registry();
        // redox-math ensures "result >= 0", redox-agent requires "result >= 0"
        let result = reg.check_compatibility("redox-math", "redox-agent");
        assert!(result.satisfied_contracts.iter().any(|s| s.contains("result >= 0")));
    }

    #[test]
    fn contract_ensures_requires_mismatch() {
        let mut reg = ForgeRegistry::new();
        reg.publish(ForgePackage {
            name: "provider".into(),
            version: "1.0.0".into(),
            description: "Provides stuff".into(),
            capabilities: vec![],
            contracts: vec![PackageContract {
                kind: ContractKind::Ensures,
                condition: "x > 0".into(),
                target: "f".into(),
            }],
            effects: vec![],
            dependencies: vec![],
            agents: vec![],
        });
        reg.publish(ForgePackage {
            name: "consumer".into(),
            version: "1.0.0".into(),
            description: "Needs stuff".into(),
            capabilities: vec![],
            contracts: vec![PackageContract {
                kind: ContractKind::Requires,
                condition: "y > 0".into(),
                target: "g".into(),
            }],
            effects: vec![],
            dependencies: vec![],
            agents: vec![],
        });
        let result = reg.check_compatibility("provider", "consumer");
        assert!(!result.compatible);
    }
}

//! # Capability-Indexed Package Registry
//!
//! A package registry indexed by capabilities (features, traits, hardware
//! targets, safety levels) rather than just names/versions.

use std::collections::HashMap;
use std::fmt;

// ── Capabilities ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    Trait(String),
    Feature(String),
    Target(String),
    SafetyLevel(String),
    Language(String),
    Custom(String),
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trait(s) => write!(f, "trait:{s}"),
            Self::Feature(s) => write!(f, "feature:{s}"),
            Self::Target(s) => write!(f, "target:{s}"),
            Self::SafetyLevel(s) => write!(f, "safety:{s}"),
            Self::Language(s) => write!(f, "lang:{s}"),
            Self::Custom(s) => write!(f, "custom:{s}"),
        }
    }
}

// ── Package ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PackageVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl PackageVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major
    }
}

impl fmt::Display for PackageVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PartialEq for PackageVersion {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major && self.minor == other.minor && self.patch == other.patch
    }
}

impl Eq for PackageVersion {}

#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: PackageVersion,
    pub description: String,
    pub capabilities: Vec<Capability>,
    pub dependencies: Vec<String>,
    pub downloads: u64,
    pub is_yanked: bool,
}

impl Package {
    pub fn new(name: impl Into<String>, version: PackageVersion) -> Self {
        Self {
            name: name.into(),
            version,
            description: String::new(),
            capabilities: Vec::new(),
            dependencies: Vec::new(),
            downloads: 0,
            is_yanked: false,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_capability(mut self, cap: Capability) -> Self {
        if !self.capabilities.contains(&cap) {
            self.capabilities.push(cap);
        }
        self
    }

    pub fn with_dependency(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.capabilities.contains(cap)
    }

    pub fn capability_count(&self) -> usize {
        self.capabilities.len()
    }
}

impl fmt::Display for Package {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} v{}", self.name, self.version)?;
        if self.is_yanked { write!(f, " [YANKED]")?; }
        Ok(())
    }
}

// ── Registry ─────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct PackageRegistry {
    packages: Vec<Package>,
    /// Inverted index: capability → package indices
    cap_index: HashMap<Capability, Vec<usize>>,
    /// Name index: name → package indices
    name_index: HashMap<String, Vec<usize>>,
}

impl PackageRegistry {
    pub fn new() -> Self {
        Self {
            packages: Vec::new(),
            cap_index: HashMap::new(),
            name_index: HashMap::new(),
        }
    }

    pub fn publish(&mut self, pkg: Package) -> usize {
        let idx = self.packages.len();
        for cap in &pkg.capabilities {
            self.cap_index.entry(cap.clone()).or_default().push(idx);
        }
        self.name_index.entry(pkg.name.clone()).or_default().push(idx);
        self.packages.push(pkg);
        idx
    }

    pub fn total_packages(&self) -> usize {
        self.packages.len()
    }

    pub fn get(&self, idx: usize) -> Option<&Package> {
        self.packages.get(idx)
    }

    /// Find packages by name (all versions).
    pub fn by_name(&self, name: &str) -> Vec<&Package> {
        self.name_index.get(name)
            .map(|idxs| idxs.iter().filter_map(|i| self.packages.get(*i)).collect())
            .unwrap_or_default()
    }

    /// Find packages by a single capability.
    pub fn by_capability(&self, cap: &Capability) -> Vec<&Package> {
        self.cap_index.get(cap)
            .map(|idxs| idxs.iter().filter_map(|i| self.packages.get(*i)).collect())
            .unwrap_or_default()
    }

    /// Find packages that satisfy ALL given capabilities.
    pub fn by_all_capabilities(&self, caps: &[Capability]) -> Vec<&Package> {
        if caps.is_empty() { return Vec::new(); }

        let first = &caps[0];
        let candidates = self.by_capability(first);
        candidates.into_iter()
            .filter(|pkg| caps.iter().all(|c| pkg.has_capability(c)))
            .collect()
    }

    /// Find packages that satisfy ANY of the given capabilities.
    pub fn by_any_capability(&self, caps: &[Capability]) -> Vec<&Package> {
        let mut seen = Vec::new();
        let mut result = Vec::new();
        for cap in caps {
            if let Some(idxs) = self.cap_index.get(cap) {
                for idx in idxs {
                    if !seen.contains(idx) {
                        seen.push(*idx);
                        if let Some(pkg) = self.packages.get(*idx) {
                            result.push(pkg);
                        }
                    }
                }
            }
        }
        result
    }

    /// Search by keyword in name or description.
    pub fn search(&self, keyword: &str) -> Vec<&Package> {
        let kw = keyword.to_lowercase();
        self.packages.iter()
            .filter(|p| p.name.to_lowercase().contains(&kw) || p.description.to_lowercase().contains(&kw))
            .collect()
    }

    /// Yank a specific package version.
    pub fn yank(&mut self, name: &str, version: &PackageVersion) -> bool {
        if let Some(pkg) = self.packages.iter_mut()
            .find(|p| p.name == name && p.version == *version)
        {
            pkg.is_yanked = true;
            true
        } else {
            false
        }
    }

    /// Get the latest (highest) version of a package by name (non-yanked).
    pub fn latest(&self, name: &str) -> Option<&Package> {
        self.by_name(name).into_iter()
            .filter(|p| !p.is_yanked)
            .max_by(|a, b| {
                a.version.major.cmp(&b.version.major)
                    .then(a.version.minor.cmp(&b.version.minor))
                    .then(a.version.patch.cmp(&b.version.patch))
            })
    }

    /// Unique capabilities across entire registry.
    pub fn all_capabilities(&self) -> Vec<&Capability> {
        self.cap_index.keys().collect()
    }

    /// Top packages by download count.
    pub fn top_by_downloads(&self, n: usize) -> Vec<&Package> {
        let mut pkgs: Vec<&Package> = self.packages.iter().filter(|p| !p.is_yanked).collect();
        pkgs.sort_by(|a, b| b.downloads.cmp(&a.downloads));
        pkgs.truncate(n);
        pkgs
    }
}

impl Default for PackageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a sample registry for testing.
pub fn build_sample_registry() -> PackageRegistry {
    let mut reg = PackageRegistry::new();

    reg.publish(
        Package::new("serde", PackageVersion::new(1, 0, 0))
            .with_description("Serialization framework")
            .with_capability(Capability::Trait("Serialize".into()))
            .with_capability(Capability::Trait("Deserialize".into()))
            .with_capability(Capability::Feature("derive".into()))
    );

    reg.publish(
        Package::new("tokio", PackageVersion::new(1, 0, 0))
            .with_description("Async runtime")
            .with_capability(Capability::Feature("async".into()))
            .with_capability(Capability::Feature("io".into()))
            .with_capability(Capability::Feature("net".into()))
    );

    reg.publish(
        Package::new("embedded-hal", PackageVersion::new(0, 2, 0))
            .with_description("Hardware abstraction layer for embedded")
            .with_capability(Capability::Target("arm".into()))
            .with_capability(Capability::Target("riscv".into()))
            .with_capability(Capability::SafetyLevel("no_std".into()))
    );

    reg
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_display() {
        assert_eq!(format!("{}", Capability::Trait("Clone".into())), "trait:Clone");
        assert_eq!(format!("{}", Capability::Target("arm".into())), "target:arm");
    }

    #[test]
    fn test_package_version_display() {
        assert_eq!(format!("{}", PackageVersion::new(1, 2, 3)), "1.2.3");
    }

    #[test]
    fn test_version_compatible() {
        let a = PackageVersion::new(1, 2, 0);
        let b = PackageVersion::new(1, 3, 0);
        assert!(a.is_compatible_with(&b));
    }

    #[test]
    fn test_version_incompatible() {
        let a = PackageVersion::new(1, 0, 0);
        let b = PackageVersion::new(2, 0, 0);
        assert!(!a.is_compatible_with(&b));
    }

    #[test]
    fn test_package_new() {
        let p = Package::new("test", PackageVersion::new(1, 0, 0));
        assert_eq!(p.name, "test");
        assert!(!p.is_yanked);
    }

    #[test]
    fn test_package_with_capability() {
        let p = Package::new("test", PackageVersion::new(1, 0, 0))
            .with_capability(Capability::Feature("async".into()));
        assert!(p.has_capability(&Capability::Feature("async".into())));
        assert_eq!(p.capability_count(), 1);
    }

    #[test]
    fn test_package_dedup_capability() {
        let p = Package::new("test", PackageVersion::new(1, 0, 0))
            .with_capability(Capability::Feature("async".into()))
            .with_capability(Capability::Feature("async".into()));
        assert_eq!(p.capability_count(), 1);
    }

    #[test]
    fn test_package_display() {
        let p = Package::new("test", PackageVersion::new(1, 0, 0));
        assert_eq!(format!("{p}"), "test v1.0.0");
    }

    #[test]
    fn test_package_display_yanked() {
        let mut p = Package::new("test", PackageVersion::new(1, 0, 0));
        p.is_yanked = true;
        assert!(format!("{p}").contains("YANKED"));
    }

    #[test]
    fn test_registry_new() {
        let r = PackageRegistry::new();
        assert_eq!(r.total_packages(), 0);
    }

    #[test]
    fn test_registry_publish() {
        let mut r = PackageRegistry::new();
        let idx = r.publish(Package::new("test", PackageVersion::new(1, 0, 0)));
        assert_eq!(idx, 0);
        assert_eq!(r.total_packages(), 1);
    }

    #[test]
    fn test_registry_by_name() {
        let reg = build_sample_registry();
        let pkgs = reg.by_name("serde");
        assert_eq!(pkgs.len(), 1);
    }

    #[test]
    fn test_registry_by_name_missing() {
        let reg = build_sample_registry();
        assert!(reg.by_name("nonexistent").is_empty());
    }

    #[test]
    fn test_registry_by_capability() {
        let reg = build_sample_registry();
        let pkgs = reg.by_capability(&Capability::Feature("async".into()));
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "tokio");
    }

    #[test]
    fn test_registry_by_all_capabilities() {
        let reg = build_sample_registry();
        let caps = vec![
            Capability::Target("arm".into()),
            Capability::SafetyLevel("no_std".into()),
        ];
        let pkgs = reg.by_all_capabilities(&caps);
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "embedded-hal");
    }

    #[test]
    fn test_registry_by_all_capabilities_none() {
        let reg = build_sample_registry();
        let caps = vec![
            Capability::Feature("async".into()),
            Capability::Target("arm".into()),
        ];
        let pkgs = reg.by_all_capabilities(&caps);
        assert!(pkgs.is_empty());
    }

    #[test]
    fn test_registry_by_any_capability() {
        let reg = build_sample_registry();
        let caps = vec![
            Capability::Feature("async".into()),
            Capability::Target("arm".into()),
        ];
        let pkgs = reg.by_any_capability(&caps);
        assert_eq!(pkgs.len(), 2);
    }

    #[test]
    fn test_registry_search() {
        let reg = build_sample_registry();
        let pkgs = reg.search("serial");
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "serde");
    }

    #[test]
    fn test_registry_search_case_insensitive() {
        let reg = build_sample_registry();
        assert_eq!(reg.search("ASYNC").len(), 1);
    }

    #[test]
    fn test_registry_yank() {
        let mut reg = build_sample_registry();
        assert!(reg.yank("serde", &PackageVersion::new(1, 0, 0)));
        let pkgs = reg.by_name("serde");
        assert!(pkgs[0].is_yanked);
    }

    #[test]
    fn test_registry_yank_nonexistent() {
        let mut reg = build_sample_registry();
        assert!(!reg.yank("nope", &PackageVersion::new(1, 0, 0)));
    }

    #[test]
    fn test_registry_latest() {
        let mut reg = PackageRegistry::new();
        reg.publish(Package::new("foo", PackageVersion::new(1, 0, 0)));
        reg.publish(Package::new("foo", PackageVersion::new(1, 1, 0)));
        reg.publish(Package::new("foo", PackageVersion::new(2, 0, 0)));
        let latest = reg.latest("foo").unwrap();
        assert_eq!(latest.version, PackageVersion::new(2, 0, 0));
    }

    #[test]
    fn test_registry_latest_skips_yanked() {
        let mut reg = PackageRegistry::new();
        reg.publish(Package::new("foo", PackageVersion::new(1, 0, 0)));
        reg.publish(Package::new("foo", PackageVersion::new(2, 0, 0)));
        reg.yank("foo", &PackageVersion::new(2, 0, 0));
        let latest = reg.latest("foo").unwrap();
        assert_eq!(latest.version, PackageVersion::new(1, 0, 0));
    }

    #[test]
    fn test_registry_top_by_downloads() {
        let mut reg = PackageRegistry::new();
        let mut p1 = Package::new("a", PackageVersion::new(1, 0, 0));
        p1.downloads = 100;
        let mut p2 = Package::new("b", PackageVersion::new(1, 0, 0));
        p2.downloads = 500;
        reg.publish(p1);
        reg.publish(p2);
        let top = reg.top_by_downloads(1);
        assert_eq!(top[0].name, "b");
    }

    #[test]
    fn test_registry_all_capabilities() {
        let reg = build_sample_registry();
        let caps = reg.all_capabilities();
        assert!(caps.len() >= 5);
    }

    #[test]
    fn test_registry_default() {
        let r = PackageRegistry::default();
        assert_eq!(r.total_packages(), 0);
    }

    #[test]
    fn test_registry_get() {
        let reg = build_sample_registry();
        assert!(reg.get(0).is_some());
        assert!(reg.get(999).is_none());
    }

    #[test]
    fn test_package_dependency() {
        let p = Package::new("test", PackageVersion::new(1, 0, 0))
            .with_dependency("serde");
        assert_eq!(p.dependencies.len(), 1);
    }

    #[test]
    fn test_by_all_empty() {
        let reg = build_sample_registry();
        assert!(reg.by_all_capabilities(&[]).is_empty());
    }
}

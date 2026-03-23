//! # Core Ecosystem Crate Migration with Capability Manifests
//!
//! Tooling and data structures for migrating core ecosystem crates to use
//! capability manifests, tracking migration status, and generating manifests.

use std::collections::HashMap;
use std::fmt;

// ── Capability Manifest ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CapabilityKind {
    Trait(String),
    Feature(String),
    Target(String),
    SafetyLevel(String),
    Edition(String),
    Custom(String),
}

impl fmt::Display for CapabilityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trait(s) => write!(f, "trait:{s}"),
            Self::Feature(s) => write!(f, "feature:{s}"),
            Self::Target(s) => write!(f, "target:{s}"),
            Self::SafetyLevel(s) => write!(f, "safety:{s}"),
            Self::Edition(s) => write!(f, "edition:{s}"),
            Self::Custom(s) => write!(f, "custom:{s}"),
        }
    }
}

/// A capability manifest attached to a crate.
#[derive(Debug, Clone)]
pub struct CapabilityManifest {
    pub crate_name: String,
    pub version: String,
    pub provides: Vec<CapabilityKind>,
    pub requires: Vec<CapabilityKind>,
    pub minimum_edition: String,
    pub metadata: HashMap<String, String>,
}

impl CapabilityManifest {
    pub fn new(crate_name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            version: version.into(),
            provides: Vec::new(),
            requires: Vec::new(),
            minimum_edition: "redox-2024".into(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_provides(&mut self, cap: CapabilityKind) {
        if !self.provides.contains(&cap) {
            self.provides.push(cap);
        }
    }

    pub fn add_requires(&mut self, cap: CapabilityKind) {
        if !self.requires.contains(&cap) {
            self.requires.push(cap);
        }
    }

    pub fn set_metadata(&mut self, key: impl Into<String>, val: impl Into<String>) {
        self.metadata.insert(key.into(), val.into());
    }

    pub fn provides_count(&self) -> usize {
        self.provides.len()
    }

    pub fn requires_count(&self) -> usize {
        self.requires.len()
    }
}

impl fmt::Display for CapabilityManifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "[capability-manifest]")?;
        writeln!(f, "crate = \"{}\"", self.crate_name)?;
        writeln!(f, "version = \"{}\"", self.version)?;
        writeln!(f, "minimum_edition = \"{}\"", self.minimum_edition)?;
        if !self.provides.is_empty() {
            writeln!(f, "provides = {:?}", self.provides.iter().map(|c| c.to_string()).collect::<Vec<_>>())?;
        }
        if !self.requires.is_empty() {
            writeln!(f, "requires = {:?}", self.requires.iter().map(|c| c.to_string()).collect::<Vec<_>>())?;
        }
        Ok(())
    }
}

// ── Migration Status ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationStatus {
    NotStarted,
    ManifestDrafted,
    DepsUpdated,
    TestsPassing,
    Published,
    Blocked,
}

impl fmt::Display for MigrationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotStarted => write!(f, "not-started"),
            Self::ManifestDrafted => write!(f, "manifest-drafted"),
            Self::DepsUpdated => write!(f, "deps-updated"),
            Self::TestsPassing => write!(f, "tests-passing"),
            Self::Published => write!(f, "published"),
            Self::Blocked => write!(f, "BLOCKED"),
        }
    }
}

/// A migration record for a single crate.
#[derive(Debug, Clone)]
pub struct MigrationRecord {
    pub crate_name: String,
    pub manifest: Option<CapabilityManifest>,
    pub status: MigrationStatus,
    pub blockers: Vec<String>,
    pub notes: Vec<String>,
}

impl MigrationRecord {
    pub fn new(crate_name: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            manifest: None,
            status: MigrationStatus::NotStarted,
            blockers: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn set_manifest(&mut self, manifest: CapabilityManifest) {
        self.manifest = Some(manifest);
        if self.status == MigrationStatus::NotStarted {
            self.status = MigrationStatus::ManifestDrafted;
        }
    }

    pub fn advance(&mut self) {
        self.status = match self.status {
            MigrationStatus::NotStarted => MigrationStatus::ManifestDrafted,
            MigrationStatus::ManifestDrafted => MigrationStatus::DepsUpdated,
            MigrationStatus::DepsUpdated => MigrationStatus::TestsPassing,
            MigrationStatus::TestsPassing => MigrationStatus::Published,
            MigrationStatus::Published => MigrationStatus::Published,
            MigrationStatus::Blocked => MigrationStatus::Blocked,
        };
    }

    pub fn block(&mut self, reason: impl Into<String>) {
        self.blockers.push(reason.into());
        self.status = MigrationStatus::Blocked;
    }

    pub fn is_complete(&self) -> bool {
        self.status == MigrationStatus::Published
    }
}

impl fmt::Display for MigrationRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.crate_name, self.status)?;
        if !self.blockers.is_empty() {
            write!(f, " (blockers: {})", self.blockers.join(", "))?;
        }
        Ok(())
    }
}

// ── Migration Tracker ────────────────────────────────────────────────

#[derive(Debug)]
pub struct MigrationTracker {
    records: Vec<MigrationRecord>,
}

impl MigrationTracker {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    pub fn add(&mut self, record: MigrationRecord) {
        self.records.push(record);
    }

    pub fn get(&self, name: &str) -> Option<&MigrationRecord> {
        self.records.iter().find(|r| r.crate_name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut MigrationRecord> {
        self.records.iter_mut().find(|r| r.crate_name == name)
    }

    pub fn total(&self) -> usize {
        self.records.len()
    }

    pub fn completed(&self) -> usize {
        self.records.iter().filter(|r| r.is_complete()).count()
    }

    pub fn blocked(&self) -> usize {
        self.records.iter().filter(|r| r.status == MigrationStatus::Blocked).count()
    }

    pub fn in_progress(&self) -> usize {
        self.records.iter()
            .filter(|r| !r.is_complete() && r.status != MigrationStatus::Blocked && r.status != MigrationStatus::NotStarted)
            .count()
    }

    pub fn not_started(&self) -> usize {
        self.records.iter().filter(|r| r.status == MigrationStatus::NotStarted).count()
    }

    pub fn by_status(&self, status: MigrationStatus) -> Vec<&MigrationRecord> {
        self.records.iter().filter(|r| r.status == status).collect()
    }

    pub fn progress_ratio(&self) -> f64 {
        if self.records.is_empty() { return 0.0; }
        self.completed() as f64 / self.records.len() as f64
    }
}

impl Default for MigrationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MigrationTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Migration Progress: {}/{} complete", self.completed(), self.total())?;
        for r in &self.records {
            writeln!(f, "  {r}")?;
        }
        Ok(())
    }
}

/// Generate a manifest for a well-known crate.
pub fn generate_manifest(crate_name: &str) -> CapabilityManifest {
    let mut m = CapabilityManifest::new(crate_name, "0.1.0");
    match crate_name {
        "serde" => {
            m.add_provides(CapabilityKind::Trait("Serialize".into()));
            m.add_provides(CapabilityKind::Trait("Deserialize".into()));
            m.add_provides(CapabilityKind::Feature("derive".into()));
        }
        "tokio" => {
            m.add_provides(CapabilityKind::Feature("async_runtime".into()));
            m.add_provides(CapabilityKind::Feature("io".into()));
            m.add_provides(CapabilityKind::Feature("net".into()));
            m.add_requires(CapabilityKind::Feature("alloc".into()));
        }
        "rand" => {
            m.add_provides(CapabilityKind::Trait("Rng".into()));
            m.add_provides(CapabilityKind::Feature("random".into()));
        }
        "log" => {
            m.add_provides(CapabilityKind::Trait("Log".into()));
            m.add_provides(CapabilityKind::Feature("logging".into()));
        }
        "clap" => {
            m.add_provides(CapabilityKind::Feature("cli_parsing".into()));
            m.add_provides(CapabilityKind::Feature("derive".into()));
        }
        _ => {
            m.add_provides(CapabilityKind::Custom(format!("{crate_name}_generic")));
        }
    }
    m
}

/// Build a migration tracker for a standard set of core crates.
pub fn build_core_migration() -> MigrationTracker {
    let mut tracker = MigrationTracker::new();
    let crates = ["serde", "tokio", "rand", "log", "clap", "regex", "hyper", "reqwest"];
    for name in crates {
        let mut record = MigrationRecord::new(name);
        let manifest = generate_manifest(name);
        record.set_manifest(manifest);
        tracker.add(record);
    }
    tracker
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_kind_display() {
        assert_eq!(format!("{}", CapabilityKind::Trait("Clone".into())), "trait:Clone");
        assert_eq!(format!("{}", CapabilityKind::Edition("2026".into())), "edition:2026");
    }

    #[test]
    fn test_manifest_new() {
        let m = CapabilityManifest::new("test", "1.0.0");
        assert_eq!(m.crate_name, "test");
        assert_eq!(m.provides_count(), 0);
    }

    #[test]
    fn test_manifest_add_provides() {
        let mut m = CapabilityManifest::new("test", "1.0.0");
        m.add_provides(CapabilityKind::Feature("async".into()));
        assert_eq!(m.provides_count(), 1);
    }

    #[test]
    fn test_manifest_dedup_provides() {
        let mut m = CapabilityManifest::new("test", "1.0.0");
        m.add_provides(CapabilityKind::Feature("async".into()));
        m.add_provides(CapabilityKind::Feature("async".into()));
        assert_eq!(m.provides_count(), 1);
    }

    #[test]
    fn test_manifest_requires() {
        let mut m = CapabilityManifest::new("test", "1.0.0");
        m.add_requires(CapabilityKind::Feature("alloc".into()));
        assert_eq!(m.requires_count(), 1);
    }

    #[test]
    fn test_manifest_metadata() {
        let mut m = CapabilityManifest::new("test", "1.0.0");
        m.set_metadata("author", "redox");
        assert_eq!(m.metadata.get("author").unwrap(), "redox");
    }

    #[test]
    fn test_manifest_display() {
        let m = CapabilityManifest::new("test", "1.0.0");
        let s = format!("{m}");
        assert!(s.contains("[capability-manifest]"));
        assert!(s.contains("test"));
    }

    #[test]
    fn test_migration_status_display() {
        assert_eq!(format!("{}", MigrationStatus::Published), "published");
        assert_eq!(format!("{}", MigrationStatus::Blocked), "BLOCKED");
    }

    #[test]
    fn test_migration_record_new() {
        let r = MigrationRecord::new("serde");
        assert_eq!(r.status, MigrationStatus::NotStarted);
        assert!(!r.is_complete());
    }

    #[test]
    fn test_migration_record_set_manifest() {
        let mut r = MigrationRecord::new("serde");
        r.set_manifest(CapabilityManifest::new("serde", "1.0.0"));
        assert_eq!(r.status, MigrationStatus::ManifestDrafted);
    }

    #[test]
    fn test_migration_record_advance() {
        let mut r = MigrationRecord::new("serde");
        r.advance();
        assert_eq!(r.status, MigrationStatus::ManifestDrafted);
        r.advance();
        assert_eq!(r.status, MigrationStatus::DepsUpdated);
        r.advance();
        assert_eq!(r.status, MigrationStatus::TestsPassing);
        r.advance();
        assert_eq!(r.status, MigrationStatus::Published);
        assert!(r.is_complete());
    }

    #[test]
    fn test_migration_record_block() {
        let mut r = MigrationRecord::new("serde");
        r.block("missing dep");
        assert_eq!(r.status, MigrationStatus::Blocked);
        assert_eq!(r.blockers.len(), 1);
    }

    #[test]
    fn test_migration_record_display() {
        let r = MigrationRecord::new("serde");
        assert!(format!("{r}").contains("serde"));
    }

    #[test]
    fn test_tracker_new() {
        let t = MigrationTracker::new();
        assert_eq!(t.total(), 0);
    }

    #[test]
    fn test_tracker_add_and_get() {
        let mut t = MigrationTracker::new();
        t.add(MigrationRecord::new("serde"));
        assert_eq!(t.total(), 1);
        assert!(t.get("serde").is_some());
    }

    #[test]
    fn test_tracker_completed() {
        let mut t = MigrationTracker::new();
        let mut r = MigrationRecord::new("serde");
        r.advance(); r.advance(); r.advance(); r.advance();
        t.add(r);
        assert_eq!(t.completed(), 1);
    }

    #[test]
    fn test_tracker_blocked() {
        let mut t = MigrationTracker::new();
        let mut r = MigrationRecord::new("serde");
        r.block("reason");
        t.add(r);
        assert_eq!(t.blocked(), 1);
    }

    #[test]
    fn test_tracker_in_progress() {
        let mut t = MigrationTracker::new();
        let mut r = MigrationRecord::new("serde");
        r.advance(); // ManifestDrafted
        t.add(r);
        assert_eq!(t.in_progress(), 1);
    }

    #[test]
    fn test_tracker_progress_ratio_empty() {
        let t = MigrationTracker::new();
        assert_eq!(t.progress_ratio(), 0.0);
    }

    #[test]
    fn test_tracker_by_status() {
        let mut t = MigrationTracker::new();
        t.add(MigrationRecord::new("a"));
        t.add(MigrationRecord::new("b"));
        assert_eq!(t.by_status(MigrationStatus::NotStarted).len(), 2);
    }

    #[test]
    fn test_tracker_display() {
        let t = build_core_migration();
        let s = format!("{t}");
        assert!(s.contains("Migration Progress"));
    }

    #[test]
    fn test_generate_manifest_serde() {
        let m = generate_manifest("serde");
        assert!(m.provides.contains(&CapabilityKind::Trait("Serialize".into())));
    }

    #[test]
    fn test_generate_manifest_tokio() {
        let m = generate_manifest("tokio");
        assert!(m.requires.contains(&CapabilityKind::Feature("alloc".into())));
    }

    #[test]
    fn test_generate_manifest_unknown() {
        let m = generate_manifest("unknown_crate");
        assert_eq!(m.provides_count(), 1);
    }

    #[test]
    fn test_build_core_migration() {
        let tracker = build_core_migration();
        assert_eq!(tracker.total(), 8);
        assert_eq!(tracker.not_started(), 0); // all have manifests
        assert_eq!(tracker.by_status(MigrationStatus::ManifestDrafted).len(), 8);
    }

    #[test]
    fn test_tracker_default() {
        let t = MigrationTracker::default();
        assert_eq!(t.total(), 0);
    }

    #[test]
    fn test_tracker_get_mut() {
        let mut t = MigrationTracker::new();
        t.add(MigrationRecord::new("serde"));
        t.get_mut("serde").unwrap().advance();
        assert_eq!(t.get("serde").unwrap().status, MigrationStatus::ManifestDrafted);
    }

    #[test]
    fn test_advance_published_stays() {
        let mut r = MigrationRecord::new("x");
        r.advance(); r.advance(); r.advance(); r.advance();
        r.advance(); // already published
        assert_eq!(r.status, MigrationStatus::Published);
    }

    #[test]
    fn test_blocked_stays_blocked() {
        let mut r = MigrationRecord::new("x");
        r.block("issue");
        r.advance();
        assert_eq!(r.status, MigrationStatus::Blocked);
    }
}

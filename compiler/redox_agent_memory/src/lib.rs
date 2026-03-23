//! # Agent Memory Model
//!
//! Four-tier memory system for ACI agents:
//! - **Ephemeral**: per-task, discarded when task completes
//! - **Session**: per-swarm-session, shared among agents in a session
//! - **Project**: conventions, persisted per-project
//! - **Global**: cross-project knowledge, persisted globally

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, SystemTime};

// ── Memory Tiers ─────────────────────────────────────────────────────

/// The four memory tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MemoryTier {
    Ephemeral,
    Session,
    Project,
    Global,
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

impl MemoryTier {
    /// Returns whether this tier persists across tasks.
    pub fn is_persistent(&self) -> bool {
        matches!(self, Self::Project | Self::Global)
    }

    /// Returns whether this tier is shared across sessions.
    pub fn is_shared(&self) -> bool {
        matches!(self, Self::Global)
    }
}

// ── Memory Entry ─────────────────────────────────────────────────────

/// A single memory entry.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: MemoryValue,
    pub tier: MemoryTier,
    pub created_at: SystemTime,
    pub accessed_at: SystemTime,
    pub access_count: u64,
    pub tags: Vec<String>,
}

/// Value stored in memory.
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryValue {
    Text(String),
    Number(f64),
    Bool(bool),
    List(Vec<MemoryValue>),
    Map(Vec<(String, MemoryValue)>),
}

impl fmt::Display for MemoryValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(s) => write!(f, "\"{s}\""),
            Self::Number(n) => write!(f, "{n}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::List(items) => {
                let s: Vec<String> = items.iter().map(|i| format!("{i}")).collect();
                write!(f, "[{}]", s.join(", "))
            }
            Self::Map(entries) => {
                let s: Vec<String> = entries.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                write!(f, "{{{}}}", s.join(", "))
            }
        }
    }
}

impl MemoryEntry {
    pub fn new(key: impl Into<String>, value: MemoryValue, tier: MemoryTier) -> Self {
        let now = SystemTime::now();
        Self {
            key: key.into(),
            value,
            tier,
            created_at: now,
            accessed_at: now,
            access_count: 0,
            tags: Vec::new(),
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn touch(&mut self) {
        self.accessed_at = SystemTime::now();
        self.access_count += 1;
    }

    pub fn age(&self) -> Duration {
        self.created_at.elapsed().unwrap_or(Duration::ZERO)
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

impl fmt::Display for MemoryEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.tier, self.key, self.value)
    }
}

// ── Memory Store (per-tier) ──────────────────────────────────────────

/// Storage for a single memory tier.
#[derive(Debug, Default)]
pub struct TierStore {
    entries: HashMap<String, MemoryEntry>,
}

impl TierStore {
    pub fn new() -> Self { Self::default() }

    pub fn get(&self, key: &str) -> Option<&MemoryEntry> {
        self.entries.get(key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut MemoryEntry> {
        self.entries.get_mut(key)
    }

    pub fn set(&mut self, entry: MemoryEntry) {
        self.entries.insert(entry.key.clone(), entry);
    }

    pub fn remove(&mut self, key: &str) -> Option<MemoryEntry> {
        self.entries.remove(key)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    /// Find entries by tag.
    pub fn find_by_tag(&self, tag: &str) -> Vec<&MemoryEntry> {
        self.entries.values().filter(|e| e.has_tag(tag)).collect()
    }

    /// Get entries sorted by access count (most accessed first).
    pub fn most_accessed(&self, n: usize) -> Vec<&MemoryEntry> {
        let mut entries: Vec<&MemoryEntry> = self.entries.values().collect();
        entries.sort_by(|a, b| b.access_count.cmp(&a.access_count));
        entries.truncate(n);
        entries
    }
}

// ── Agent Memory Model ───────────────────────────────────────────────

/// The four-tier agent memory model.
#[derive(Debug, Default)]
pub struct AgentMemory {
    ephemeral: TierStore,
    session: TierStore,
    project: TierStore,
    global: TierStore,
}

impl AgentMemory {
    pub fn new() -> Self { Self::default() }

    fn store(&self, tier: MemoryTier) -> &TierStore {
        match tier {
            MemoryTier::Ephemeral => &self.ephemeral,
            MemoryTier::Session => &self.session,
            MemoryTier::Project => &self.project,
            MemoryTier::Global => &self.global,
        }
    }

    fn store_mut(&mut self, tier: MemoryTier) -> &mut TierStore {
        match tier {
            MemoryTier::Ephemeral => &mut self.ephemeral,
            MemoryTier::Session => &mut self.session,
            MemoryTier::Project => &mut self.project,
            MemoryTier::Global => &mut self.global,
        }
    }

    /// Store a value in the specified tier.
    pub fn set(&mut self, tier: MemoryTier, key: impl Into<String>, value: MemoryValue) {
        let key = key.into();
        let entry = MemoryEntry::new(key, value, tier);
        self.store_mut(tier).set(entry);
    }

    /// Store a value with tags.
    pub fn set_tagged(&mut self, tier: MemoryTier, key: impl Into<String>, value: MemoryValue, tags: Vec<String>) {
        let key = key.into();
        let entry = MemoryEntry::new(key, value, tier).with_tags(tags);
        self.store_mut(tier).set(entry);
    }

    /// Retrieve a value from a specific tier.
    pub fn get(&mut self, tier: MemoryTier, key: &str) -> Option<&MemoryValue> {
        // Touch the entry to track access
        if let Some(entry) = self.store_mut(tier).get_mut(key) {
            entry.touch();
            Some(&entry.value)
        } else {
            None
        }
    }

    /// Retrieve a value, searching tiers from ephemeral to global.
    pub fn resolve(&mut self, key: &str) -> Option<(MemoryTier, &MemoryValue)> {
        for tier in [MemoryTier::Ephemeral, MemoryTier::Session, MemoryTier::Project, MemoryTier::Global] {
            if self.store(tier).contains(key) {
                let entry = self.store_mut(tier).get_mut(key).unwrap();
                entry.touch();
                return Some((tier, &entry.value));
            }
        }
        None
    }

    /// Remove a key from a tier.
    pub fn remove(&mut self, tier: MemoryTier, key: &str) -> Option<MemoryEntry> {
        self.store_mut(tier).remove(key)
    }

    /// Clear all ephemeral memory (called at task end).
    pub fn clear_ephemeral(&mut self) {
        self.ephemeral.clear();
    }

    /// Clear session memory (called at session end).
    pub fn clear_session(&mut self) {
        self.session.clear();
    }

    /// Get entry count for a tier.
    pub fn tier_len(&self, tier: MemoryTier) -> usize {
        self.store(tier).len()
    }

    /// Get total entry count across all tiers.
    pub fn total_entries(&self) -> usize {
        self.ephemeral.len() + self.session.len() + self.project.len() + self.global.len()
    }

    /// Find entries by tag across all tiers.
    pub fn find_by_tag(&self, tag: &str) -> Vec<&MemoryEntry> {
        let mut results = Vec::new();
        for tier in [MemoryTier::Ephemeral, MemoryTier::Session, MemoryTier::Project, MemoryTier::Global] {
            results.extend(self.store(tier).find_by_tag(tag));
        }
        results
    }

    /// Promote an entry from one tier to another.
    pub fn promote(&mut self, key: &str, from: MemoryTier, to: MemoryTier) -> bool {
        if let Some(mut entry) = self.store_mut(from).remove(key) {
            entry.tier = to;
            self.store_mut(to).set(entry);
            true
        } else {
            false
        }
    }
}

// ── Memory Snapshot ──────────────────────────────────────────────────

/// Serializable snapshot of memory state.
#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    pub tier: MemoryTier,
    pub entries: Vec<(String, String)>, // key, value display
    pub entry_count: usize,
}

impl fmt::Display for MemorySnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Memory snapshot [{}]: {} entries", self.tier, self.entry_count)?;
        for (k, v) in &self.entries {
            writeln!(f, "  {k}: {v}")?;
        }
        Ok(())
    }
}

/// Take a snapshot of a tier.
pub fn snapshot_tier(memory: &AgentMemory, tier: MemoryTier) -> MemorySnapshot {
    let store = memory.store(tier);
    let entries: Vec<(String, String)> = store.entries.iter()
        .map(|(k, v)| (k.clone(), format!("{}", v.value)))
        .collect();
    MemorySnapshot {
        tier,
        entry_count: entries.len(),
        entries,
    }
}

// ── Memory Policy ────────────────────────────────────────────────────

/// Policy for automatic memory management.
#[derive(Debug, Clone)]
pub struct MemoryPolicy {
    /// Max entries per tier before eviction.
    pub max_ephemeral: usize,
    pub max_session: usize,
    pub max_project: usize,
    pub max_global: usize,
    /// Whether to auto-promote frequently accessed entries.
    pub auto_promote: bool,
    /// Access count threshold for auto-promotion.
    pub promote_threshold: u64,
}

impl Default for MemoryPolicy {
    fn default() -> Self {
        Self {
            max_ephemeral: 100,
            max_session: 500,
            max_project: 1000,
            max_global: 5000,
            auto_promote: true,
            promote_threshold: 10,
        }
    }
}

impl MemoryPolicy {
    pub fn max_for_tier(&self, tier: MemoryTier) -> usize {
        match tier {
            MemoryTier::Ephemeral => self.max_ephemeral,
            MemoryTier::Session => self.max_session,
            MemoryTier::Project => self.max_project,
            MemoryTier::Global => self.max_global,
        }
    }
}

/// Check if a tier needs eviction.
pub fn needs_eviction(memory: &AgentMemory, tier: MemoryTier, policy: &MemoryPolicy) -> bool {
    memory.tier_len(tier) > policy.max_for_tier(tier)
}

/// Find candidates for auto-promotion from a tier.
pub fn promotion_candidates(memory: &AgentMemory, tier: MemoryTier, policy: &MemoryPolicy) -> Vec<String> {
    if !policy.auto_promote { return Vec::new(); }
    let next_tier = match tier {
        MemoryTier::Ephemeral => MemoryTier::Session,
        MemoryTier::Session => MemoryTier::Project,
        MemoryTier::Project => MemoryTier::Global,
        MemoryTier::Global => return Vec::new(),
    };
    let _ = next_tier; // used conceptually for the target tier
    memory.store(tier).entries.values()
        .filter(|e| e.access_count >= policy.promote_threshold)
        .map(|e| e.key.clone())
        .collect()
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_display() {
        assert_eq!(format!("{}", MemoryTier::Ephemeral), "ephemeral");
        assert_eq!(format!("{}", MemoryTier::Global), "global");
    }

    #[test]
    fn test_tier_persistent() {
        assert!(!MemoryTier::Ephemeral.is_persistent());
        assert!(!MemoryTier::Session.is_persistent());
        assert!(MemoryTier::Project.is_persistent());
        assert!(MemoryTier::Global.is_persistent());
    }

    #[test]
    fn test_tier_shared() {
        assert!(!MemoryTier::Project.is_shared());
        assert!(MemoryTier::Global.is_shared());
    }

    #[test]
    fn test_memory_value_display() {
        assert_eq!(format!("{}", MemoryValue::Text("hello".into())), "\"hello\"");
        assert_eq!(format!("{}", MemoryValue::Number(3.14)), "3.14");
        assert_eq!(format!("{}", MemoryValue::Bool(true)), "true");
    }

    #[test]
    fn test_memory_value_list() {
        let v = MemoryValue::List(vec![MemoryValue::Number(1.0), MemoryValue::Number(2.0)]);
        let s = format!("{v}");
        assert!(s.contains("1"));
        assert!(s.contains("2"));
    }

    #[test]
    fn test_memory_value_map() {
        let v = MemoryValue::Map(vec![("key".into(), MemoryValue::Text("val".into()))]);
        let s = format!("{v}");
        assert!(s.contains("key"));
        assert!(s.contains("val"));
    }

    #[test]
    fn test_memory_entry_new() {
        let e = MemoryEntry::new("test", MemoryValue::Bool(true), MemoryTier::Ephemeral);
        assert_eq!(e.key, "test");
        assert_eq!(e.tier, MemoryTier::Ephemeral);
        assert_eq!(e.access_count, 0);
    }

    #[test]
    fn test_memory_entry_touch() {
        let mut e = MemoryEntry::new("test", MemoryValue::Bool(false), MemoryTier::Session);
        e.touch();
        e.touch();
        assert_eq!(e.access_count, 2);
    }

    #[test]
    fn test_memory_entry_tags() {
        let e = MemoryEntry::new("test", MemoryValue::Bool(true), MemoryTier::Project)
            .with_tags(vec!["code".into(), "convention".into()]);
        assert!(e.has_tag("code"));
        assert!(!e.has_tag("other"));
    }

    #[test]
    fn test_memory_entry_display() {
        let e = MemoryEntry::new("config", MemoryValue::Text("val".into()), MemoryTier::Project);
        let s = format!("{e}");
        assert!(s.contains("project"));
        assert!(s.contains("config"));
    }

    #[test]
    fn test_tier_store() {
        let mut store = TierStore::new();
        assert!(store.is_empty());
        store.set(MemoryEntry::new("k1", MemoryValue::Number(42.0), MemoryTier::Ephemeral));
        assert_eq!(store.len(), 1);
        assert!(store.contains("k1"));
        assert!(!store.contains("k2"));
    }

    #[test]
    fn test_tier_store_remove() {
        let mut store = TierStore::new();
        store.set(MemoryEntry::new("k", MemoryValue::Bool(true), MemoryTier::Session));
        assert!(store.remove("k").is_some());
        assert!(store.is_empty());
    }

    #[test]
    fn test_tier_store_find_by_tag() {
        let mut store = TierStore::new();
        store.set(MemoryEntry::new("a", MemoryValue::Bool(true), MemoryTier::Project)
            .with_tags(vec!["x".into()]));
        store.set(MemoryEntry::new("b", MemoryValue::Bool(false), MemoryTier::Project)
            .with_tags(vec!["y".into()]));
        assert_eq!(store.find_by_tag("x").len(), 1);
    }

    #[test]
    fn test_tier_store_most_accessed() {
        let mut store = TierStore::new();
        let mut e1 = MemoryEntry::new("a", MemoryValue::Bool(true), MemoryTier::Ephemeral);
        e1.access_count = 10;
        let mut e2 = MemoryEntry::new("b", MemoryValue::Bool(false), MemoryTier::Ephemeral);
        e2.access_count = 5;
        store.set(e1);
        store.set(e2);
        let top = store.most_accessed(1);
        assert_eq!(top[0].key, "a");
    }

    #[test]
    fn test_agent_memory_set_get() {
        let mut mem = AgentMemory::new();
        mem.set(MemoryTier::Ephemeral, "task_id", MemoryValue::Text("123".into()));
        let val = mem.get(MemoryTier::Ephemeral, "task_id");
        assert_eq!(val, Some(&MemoryValue::Text("123".into())));
    }

    #[test]
    fn test_agent_memory_resolve_order() {
        let mut mem = AgentMemory::new();
        mem.set(MemoryTier::Global, "name", MemoryValue::Text("global".into()));
        mem.set(MemoryTier::Ephemeral, "name", MemoryValue::Text("ephemeral".into()));
        let (tier, val) = mem.resolve("name").unwrap();
        assert_eq!(tier, MemoryTier::Ephemeral);
        assert_eq!(val, &MemoryValue::Text("ephemeral".into()));
    }

    #[test]
    fn test_agent_memory_resolve_fallback() {
        let mut mem = AgentMemory::new();
        mem.set(MemoryTier::Project, "setting", MemoryValue::Bool(true));
        let (tier, _) = mem.resolve("setting").unwrap();
        assert_eq!(tier, MemoryTier::Project);
    }

    #[test]
    fn test_agent_memory_resolve_missing() {
        let mut mem = AgentMemory::new();
        assert!(mem.resolve("nope").is_none());
    }

    #[test]
    fn test_agent_memory_clear_ephemeral() {
        let mut mem = AgentMemory::new();
        mem.set(MemoryTier::Ephemeral, "tmp", MemoryValue::Number(1.0));
        mem.set(MemoryTier::Session, "sess", MemoryValue::Number(2.0));
        mem.clear_ephemeral();
        assert_eq!(mem.tier_len(MemoryTier::Ephemeral), 0);
        assert_eq!(mem.tier_len(MemoryTier::Session), 1);
    }

    #[test]
    fn test_agent_memory_clear_session() {
        let mut mem = AgentMemory::new();
        mem.set(MemoryTier::Session, "s", MemoryValue::Bool(true));
        mem.clear_session();
        assert_eq!(mem.tier_len(MemoryTier::Session), 0);
    }

    #[test]
    fn test_agent_memory_total_entries() {
        let mut mem = AgentMemory::new();
        mem.set(MemoryTier::Ephemeral, "a", MemoryValue::Bool(true));
        mem.set(MemoryTier::Session, "b", MemoryValue::Bool(true));
        mem.set(MemoryTier::Project, "c", MemoryValue::Bool(true));
        assert_eq!(mem.total_entries(), 3);
    }

    #[test]
    fn test_agent_memory_remove() {
        let mut mem = AgentMemory::new();
        mem.set(MemoryTier::Global, "x", MemoryValue::Number(1.0));
        assert!(mem.remove(MemoryTier::Global, "x").is_some());
        assert!(mem.remove(MemoryTier::Global, "x").is_none());
    }

    #[test]
    fn test_agent_memory_promote() {
        let mut mem = AgentMemory::new();
        mem.set(MemoryTier::Ephemeral, "k", MemoryValue::Text("val".into()));
        assert!(mem.promote("k", MemoryTier::Ephemeral, MemoryTier::Session));
        assert_eq!(mem.tier_len(MemoryTier::Ephemeral), 0);
        assert_eq!(mem.tier_len(MemoryTier::Session), 1);
    }

    #[test]
    fn test_agent_memory_promote_missing() {
        let mut mem = AgentMemory::new();
        assert!(!mem.promote("nope", MemoryTier::Ephemeral, MemoryTier::Session));
    }

    #[test]
    fn test_agent_memory_find_by_tag() {
        let mut mem = AgentMemory::new();
        mem.set_tagged(MemoryTier::Project, "a", MemoryValue::Bool(true), vec!["conv".into()]);
        mem.set_tagged(MemoryTier::Global, "b", MemoryValue::Bool(true), vec!["conv".into()]);
        mem.set_tagged(MemoryTier::Ephemeral, "c", MemoryValue::Bool(true), vec!["other".into()]);
        let found = mem.find_by_tag("conv");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_snapshot_tier() {
        let mut mem = AgentMemory::new();
        mem.set(MemoryTier::Project, "x", MemoryValue::Number(1.0));
        mem.set(MemoryTier::Project, "y", MemoryValue::Text("hi".into()));
        let snap = snapshot_tier(&mem, MemoryTier::Project);
        assert_eq!(snap.entry_count, 2);
        assert_eq!(snap.tier, MemoryTier::Project);
    }

    #[test]
    fn test_snapshot_display() {
        let snap = MemorySnapshot {
            tier: MemoryTier::Session,
            entry_count: 1,
            entries: vec![("k".into(), "v".into())],
        };
        let s = format!("{snap}");
        assert!(s.contains("session"));
        assert!(s.contains("k: v"));
    }

    #[test]
    fn test_memory_policy_defaults() {
        let p = MemoryPolicy::default();
        assert_eq!(p.max_ephemeral, 100);
        assert!(p.auto_promote);
    }

    #[test]
    fn test_needs_eviction() {
        let mut mem = AgentMemory::new();
        let policy = MemoryPolicy { max_ephemeral: 1, ..Default::default() };
        mem.set(MemoryTier::Ephemeral, "a", MemoryValue::Bool(true));
        mem.set(MemoryTier::Ephemeral, "b", MemoryValue::Bool(true));
        assert!(needs_eviction(&mem, MemoryTier::Ephemeral, &policy));
    }

    #[test]
    fn test_promotion_candidates() {
        let mut mem = AgentMemory::new();
        let mut entry = MemoryEntry::new("hot", MemoryValue::Bool(true), MemoryTier::Ephemeral);
        entry.access_count = 20;
        mem.ephemeral.set(entry);
        mem.set(MemoryTier::Ephemeral, "cold", MemoryValue::Bool(false));
        let policy = MemoryPolicy { promote_threshold: 10, ..Default::default() };
        let candidates = promotion_candidates(&mem, MemoryTier::Ephemeral, &policy);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], "hot");
    }

    #[test]
    fn test_tier_store_keys() {
        let mut store = TierStore::new();
        store.set(MemoryEntry::new("a", MemoryValue::Bool(true), MemoryTier::Ephemeral));
        store.set(MemoryEntry::new("b", MemoryValue::Bool(false), MemoryTier::Ephemeral));
        let keys = store.keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_memory_entry_age() {
        let e = MemoryEntry::new("k", MemoryValue::Bool(true), MemoryTier::Ephemeral);
        assert!(e.age() < Duration::from_secs(1));
    }
}

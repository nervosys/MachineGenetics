//! # Agent Swarm Marketplace & Pre-composed Swarm Templates
//!
//! A marketplace for discovering, publishing, and instantiating reusable
//! swarm templates with configurable agent compositions.

use std::collections::HashMap;
use std::fmt;

// ── Agent Role ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentRole {
    Parser,
    TypeChecker,
    BorrowChecker,
    Optimizer,
    CodeGen,
    Linter,
    Formatter,
    Auditor,
    Migrator,
    TestRunner,
    Coordinator,
    Custom(String),
}

impl fmt::Display for AgentRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parser => write!(f, "parser"),
            Self::TypeChecker => write!(f, "type_checker"),
            Self::BorrowChecker => write!(f, "borrow_checker"),
            Self::Optimizer => write!(f, "optimizer"),
            Self::CodeGen => write!(f, "code_gen"),
            Self::Linter => write!(f, "linter"),
            Self::Formatter => write!(f, "formatter"),
            Self::Auditor => write!(f, "auditor"),
            Self::Migrator => write!(f, "migrator"),
            Self::TestRunner => write!(f, "test_runner"),
            Self::Coordinator => write!(f, "coordinator"),
            Self::Custom(s) => write!(f, "custom:{s}"),
        }
    }
}

// ── Agent Slot ───────────────────────────────────────────────────────

/// A slot in a swarm template for one or more agents of a role.
#[derive(Debug, Clone)]
pub struct AgentSlot {
    pub role: AgentRole,
    pub min_count: u32,
    pub max_count: u32,
    pub description: String,
}

impl AgentSlot {
    pub fn new(role: AgentRole, min: u32, max: u32) -> Self {
        Self { role, min_count: min, max_count: max, description: String::new() }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn is_required(&self) -> bool {
        self.min_count > 0
    }
}

impl fmt::Display for AgentSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}-{}]", self.role, self.min_count, self.max_count)
    }
}

// ── Swarm Template ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateCategory {
    Audit,
    Migration,
    Greenfield,
    CI,
    Security,
    Performance,
    Custom,
}

impl fmt::Display for TemplateCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Audit => write!(f, "audit"),
            Self::Migration => write!(f, "migration"),
            Self::Greenfield => write!(f, "greenfield"),
            Self::CI => write!(f, "ci"),
            Self::Security => write!(f, "security"),
            Self::Performance => write!(f, "performance"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// A reusable swarm template.
#[derive(Debug, Clone)]
pub struct SwarmTemplate {
    pub name: String,
    pub version: String,
    pub category: TemplateCategory,
    pub description: String,
    pub slots: Vec<AgentSlot>,
    pub tags: Vec<String>,
    pub author: String,
    pub downloads: u64,
}

impl SwarmTemplate {
    pub fn new(name: impl Into<String>, category: TemplateCategory) -> Self {
        Self {
            name: name.into(),
            version: "0.1.0".into(),
            category,
            description: String::new(),
            slots: Vec::new(),
            tags: Vec::new(),
            author: String::new(),
            downloads: 0,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn add_slot(&mut self, slot: AgentSlot) {
        self.slots.push(slot);
    }

    pub fn required_agents(&self) -> u32 {
        self.slots.iter().map(|s| s.min_count).sum()
    }

    pub fn max_agents(&self) -> u32 {
        self.slots.iter().map(|s| s.max_count).sum()
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }
}

impl fmt::Display for SwarmTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} v{} [{}]", self.name, self.version, self.category)?;
        if !self.description.is_empty() {
            write!(f, " — {}", self.description)?;
        }
        Ok(())
    }
}

// ── Marketplace ──────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SwarmMarketplace {
    templates: Vec<SwarmTemplate>,
    tag_index: HashMap<String, Vec<usize>>,
}

impl SwarmMarketplace {
    pub fn new() -> Self {
        Self { templates: Vec::new(), tag_index: HashMap::new() }
    }

    pub fn publish(&mut self, template: SwarmTemplate) -> usize {
        let idx = self.templates.len();
        for tag in &template.tags {
            self.tag_index.entry(tag.clone()).or_default().push(idx);
        }
        self.templates.push(template);
        idx
    }

    pub fn total(&self) -> usize {
        self.templates.len()
    }

    pub fn get(&self, idx: usize) -> Option<&SwarmTemplate> {
        self.templates.get(idx)
    }

    pub fn by_name(&self, name: &str) -> Option<&SwarmTemplate> {
        self.templates.iter().find(|t| t.name == name)
    }

    pub fn by_category(&self, cat: TemplateCategory) -> Vec<&SwarmTemplate> {
        self.templates.iter().filter(|t| t.category == cat).collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&SwarmTemplate> {
        self.tag_index.get(tag)
            .map(|idxs| idxs.iter().filter_map(|i| self.templates.get(*i)).collect())
            .unwrap_or_default()
    }

    pub fn search(&self, keyword: &str) -> Vec<&SwarmTemplate> {
        let kw = keyword.to_lowercase();
        self.templates.iter()
            .filter(|t| t.name.to_lowercase().contains(&kw) || t.description.to_lowercase().contains(&kw))
            .collect()
    }

    pub fn top_by_downloads(&self, n: usize) -> Vec<&SwarmTemplate> {
        let mut sorted: Vec<&SwarmTemplate> = self.templates.iter().collect();
        sorted.sort_by(|a, b| b.downloads.cmp(&a.downloads));
        sorted.truncate(n);
        sorted
    }

    pub fn all_tags(&self) -> Vec<&String> {
        self.tag_index.keys().collect()
    }
}

impl Default for SwarmMarketplace {
    fn default() -> Self {
        Self::new()
    }
}

/// Build pre-composed templates.
pub fn build_standard_templates() -> SwarmMarketplace {
    let mut mp = SwarmMarketplace::new();

    // Audit swarm
    let mut audit = SwarmTemplate::new("audit-swarm", TemplateCategory::Audit)
        .with_description("Full codebase audit with linting and security checks")
        .with_author("redox-team")
        .with_tag("audit").with_tag("security");
    audit.add_slot(AgentSlot::new(AgentRole::Coordinator, 1, 1));
    audit.add_slot(AgentSlot::new(AgentRole::Linter, 2, 4));
    audit.add_slot(AgentSlot::new(AgentRole::Auditor, 1, 3));
    audit.add_slot(AgentSlot::new(AgentRole::TestRunner, 1, 2));
    mp.publish(audit);

    // Migration swarm
    let mut migration = SwarmTemplate::new("migration-swarm", TemplateCategory::Migration)
        .with_description("Automated codebase migration to Redox edition")
        .with_author("redox-team")
        .with_tag("migration").with_tag("upgrade");
    migration.add_slot(AgentSlot::new(AgentRole::Coordinator, 1, 1));
    migration.add_slot(AgentSlot::new(AgentRole::Parser, 2, 6));
    migration.add_slot(AgentSlot::new(AgentRole::Migrator, 2, 8));
    migration.add_slot(AgentSlot::new(AgentRole::TestRunner, 1, 4));
    mp.publish(migration);

    // Greenfield swarm
    let mut greenfield = SwarmTemplate::new("greenfield-swarm", TemplateCategory::Greenfield)
        .with_description("New project scaffolding and code generation")
        .with_author("redox-team")
        .with_tag("new-project").with_tag("scaffold");
    greenfield.add_slot(AgentSlot::new(AgentRole::Coordinator, 1, 1));
    greenfield.add_slot(AgentSlot::new(AgentRole::CodeGen, 2, 6));
    greenfield.add_slot(AgentSlot::new(AgentRole::TypeChecker, 1, 2));
    greenfield.add_slot(AgentSlot::new(AgentRole::Formatter, 1, 1));
    mp.publish(greenfield);

    // CI swarm
    let mut ci = SwarmTemplate::new("ci-swarm", TemplateCategory::CI)
        .with_description("Continuous integration pipeline agents")
        .with_author("redox-team")
        .with_tag("ci").with_tag("testing");
    ci.add_slot(AgentSlot::new(AgentRole::Coordinator, 1, 1));
    ci.add_slot(AgentSlot::new(AgentRole::TestRunner, 3, 10));
    ci.add_slot(AgentSlot::new(AgentRole::Linter, 1, 2));
    mp.publish(ci);

    mp
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_role_display() {
        assert_eq!(format!("{}", AgentRole::Parser), "parser");
        assert_eq!(format!("{}", AgentRole::Custom("x".into())), "custom:x");
    }

    #[test]
    fn test_agent_slot_new() {
        let s = AgentSlot::new(AgentRole::Parser, 1, 4);
        assert!(s.is_required());
    }

    #[test]
    fn test_agent_slot_optional() {
        let s = AgentSlot::new(AgentRole::Linter, 0, 2);
        assert!(!s.is_required());
    }

    #[test]
    fn test_agent_slot_display() {
        let s = AgentSlot::new(AgentRole::Parser, 1, 4);
        assert!(format!("{s}").contains("parser[1-4]"));
    }

    #[test]
    fn test_template_category_display() {
        assert_eq!(format!("{}", TemplateCategory::Audit), "audit");
    }

    #[test]
    fn test_swarm_template_new() {
        let t = SwarmTemplate::new("test", TemplateCategory::Audit);
        assert_eq!(t.slot_count(), 0);
        assert_eq!(t.required_agents(), 0);
    }

    #[test]
    fn test_swarm_template_add_slot() {
        let mut t = SwarmTemplate::new("test", TemplateCategory::Audit);
        t.add_slot(AgentSlot::new(AgentRole::Coordinator, 1, 1));
        t.add_slot(AgentSlot::new(AgentRole::Linter, 2, 4));
        assert_eq!(t.slot_count(), 2);
        assert_eq!(t.required_agents(), 3);
        assert_eq!(t.max_agents(), 5);
    }

    #[test]
    fn test_swarm_template_display() {
        let t = SwarmTemplate::new("my-swarm", TemplateCategory::Greenfield)
            .with_description("A test swarm");
        let s = format!("{t}");
        assert!(s.contains("my-swarm"));
        assert!(s.contains("greenfield"));
    }

    #[test]
    fn test_marketplace_new() {
        let mp = SwarmMarketplace::new();
        assert_eq!(mp.total(), 0);
    }

    #[test]
    fn test_marketplace_publish() {
        let mut mp = SwarmMarketplace::new();
        let idx = mp.publish(SwarmTemplate::new("test", TemplateCategory::Custom));
        assert_eq!(idx, 0);
        assert_eq!(mp.total(), 1);
    }

    #[test]
    fn test_marketplace_by_name() {
        let mut mp = SwarmMarketplace::new();
        mp.publish(SwarmTemplate::new("my-swarm", TemplateCategory::Audit));
        assert!(mp.by_name("my-swarm").is_some());
        assert!(mp.by_name("missing").is_none());
    }

    #[test]
    fn test_marketplace_by_category() {
        let mp = build_standard_templates();
        let audits = mp.by_category(TemplateCategory::Audit);
        assert_eq!(audits.len(), 1);
    }

    #[test]
    fn test_marketplace_by_tag() {
        let mp = build_standard_templates();
        let security = mp.by_tag("security");
        assert_eq!(security.len(), 1);
    }

    #[test]
    fn test_marketplace_search() {
        let mp = build_standard_templates();
        let results = mp.search("migration");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_marketplace_search_case_insensitive() {
        let mp = build_standard_templates();
        assert!(!mp.search("AUDIT").is_empty());
    }

    #[test]
    fn test_marketplace_top_by_downloads() {
        let mut mp = SwarmMarketplace::new();
        let mut t1 = SwarmTemplate::new("a", TemplateCategory::Custom);
        t1.downloads = 100;
        let mut t2 = SwarmTemplate::new("b", TemplateCategory::Custom);
        t2.downloads = 500;
        mp.publish(t1);
        mp.publish(t2);
        let top = mp.top_by_downloads(1);
        assert_eq!(top[0].name, "b");
    }

    #[test]
    fn test_marketplace_all_tags() {
        let mp = build_standard_templates();
        let tags = mp.all_tags();
        assert!(tags.len() >= 4);
    }

    #[test]
    fn test_marketplace_get() {
        let mp = build_standard_templates();
        assert!(mp.get(0).is_some());
        assert!(mp.get(999).is_none());
    }

    #[test]
    fn test_standard_templates_count() {
        let mp = build_standard_templates();
        assert_eq!(mp.total(), 4);
    }

    #[test]
    fn test_audit_template_agents() {
        let mp = build_standard_templates();
        let audit = mp.by_name("audit-swarm").unwrap();
        assert!(audit.required_agents() >= 5);
    }

    #[test]
    fn test_migration_template_agents() {
        let mp = build_standard_templates();
        let migration = mp.by_name("migration-swarm").unwrap();
        assert!(migration.required_agents() >= 6);
    }

    #[test]
    fn test_greenfield_template() {
        let mp = build_standard_templates();
        let gf = mp.by_name("greenfield-swarm").unwrap();
        assert_eq!(gf.category, TemplateCategory::Greenfield);
    }

    #[test]
    fn test_marketplace_default() {
        let mp = SwarmMarketplace::default();
        assert_eq!(mp.total(), 0);
    }

    #[test]
    fn test_slot_description() {
        let s = AgentSlot::new(AgentRole::Parser, 1, 2).with_description("Parse sources");
        assert_eq!(s.description, "Parse sources");
    }

    #[test]
    fn test_template_author() {
        let t = SwarmTemplate::new("t", TemplateCategory::Audit).with_author("alice");
        assert_eq!(t.author, "alice");
    }
}

// redox_swarm_configs: Reference swarm configurations.
//
//  Provides ready-to-use swarm blueprints for common workflows:
//  audit swarm, migration swarm, and greenfield swarm. Each config
//  defines agent roles, communication topology, and scheduling policy.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Agent roles
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentRole {
    Coordinator,
    Analyzer,
    Transformer,
    Validator,
    Reporter,
    Planner,
    Implementer,
    Reviewer,
    Tester,
    Deployer,
    Auditor,
    SecurityScanner,
}

impl AgentRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Coordinator => "coordinator",
            Self::Analyzer => "analyzer",
            Self::Transformer => "transformer",
            Self::Validator => "validator",
            Self::Reporter => "reporter",
            Self::Planner => "planner",
            Self::Implementer => "implementer",
            Self::Reviewer => "reviewer",
            Self::Tester => "tester",
            Self::Deployer => "deployer",
            Self::Auditor => "auditor",
            Self::SecurityScanner => "security-scanner",
        }
    }
}

// ---------------------------------------------------------------------------
// Communication topology
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Topology {
    Star,        // coordinator hub, agents are spokes
    Ring,        // each agent passes to next
    Mesh,        // all-to-all
    Pipeline,    // linear chain
    Tree,        // hierarchical
}

impl Topology {
    pub fn label(self) -> &'static str {
        match self {
            Self::Star => "star",
            Self::Ring => "ring",
            Self::Mesh => "mesh",
            Self::Pipeline => "pipeline",
            Self::Tree => "tree",
        }
    }
}

// ---------------------------------------------------------------------------
// Scheduling policy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchedulingPolicy {
    RoundRobin,
    Priority,
    LoadBalanced,
    Adaptive,
}

impl SchedulingPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::RoundRobin => "round-robin",
            Self::Priority => "priority",
            Self::LoadBalanced => "load-balanced",
            Self::Adaptive => "adaptive",
        }
    }
}

// ---------------------------------------------------------------------------
// Agent spec within a config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpec {
    pub role: AgentRole,
    pub count: usize,
    pub capabilities: Vec<String>,
}

impl AgentSpec {
    pub fn new(role: AgentRole, count: usize, caps: &[&str]) -> Self {
        Self {
            role,
            count,
            capabilities: caps.iter().map(|s| s.to_string()).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Swarm configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmConfig {
    pub name: String,
    pub description: String,
    pub topology: Topology,
    pub scheduling: SchedulingPolicy,
    pub agents: Vec<AgentSpec>,
    pub max_concurrent: usize,
    pub timeout_secs: u64,
    pub tags: Vec<String>,
}

impl SwarmConfig {
    pub fn total_agents(&self) -> usize {
        self.agents.iter().map(|a| a.count).sum()
    }

    pub fn roles(&self) -> Vec<AgentRole> {
        self.agents.iter().map(|a| a.role).collect()
    }

    pub fn has_role(&self, role: AgentRole) -> bool {
        self.agents.iter().any(|a| a.role == role)
    }

    pub fn agents_for_role(&self, role: AgentRole) -> usize {
        self.agents.iter().filter(|a| a.role == role).map(|a| a.count).sum()
    }
}

// ---------------------------------------------------------------------------
// Reference configurations
// ---------------------------------------------------------------------------

pub fn audit_swarm() -> SwarmConfig {
    SwarmConfig {
        name: "audit-swarm".to_string(),
        description: "Security and compliance audit for an existing codebase.".to_string(),
        topology: Topology::Star,
        scheduling: SchedulingPolicy::Priority,
        agents: vec![
            AgentSpec::new(AgentRole::Coordinator, 1, &["orchestrate", "report"]),
            AgentSpec::new(AgentRole::Auditor, 3, &["lint", "compliance"]),
            AgentSpec::new(AgentRole::SecurityScanner, 2, &["vuln-scan", "dependency-check"]),
            AgentSpec::new(AgentRole::Reporter, 1, &["summarize", "format"]),
        ],
        max_concurrent: 6,
        timeout_secs: 3600,
        tags: vec!["audit".into(), "security".into(), "compliance".into()],
    }
}

pub fn migration_swarm() -> SwarmConfig {
    SwarmConfig {
        name: "migration-swarm".to_string(),
        description: "Migrate a codebase from one language or framework to Redox.".to_string(),
        topology: Topology::Pipeline,
        scheduling: SchedulingPolicy::Adaptive,
        agents: vec![
            AgentSpec::new(AgentRole::Analyzer, 2, &["parse", "dependency-graph"]),
            AgentSpec::new(AgentRole::Planner, 1, &["plan", "prioritize"]),
            AgentSpec::new(AgentRole::Transformer, 4, &["rewrite", "translate"]),
            AgentSpec::new(AgentRole::Validator, 2, &["typecheck", "test"]),
            AgentSpec::new(AgentRole::Reviewer, 1, &["review", "approve"]),
        ],
        max_concurrent: 8,
        timeout_secs: 7200,
        tags: vec!["migration".into(), "refactor".into()],
    }
}

pub fn greenfield_swarm() -> SwarmConfig {
    SwarmConfig {
        name: "greenfield-swarm".to_string(),
        description: "Scaffold and build a new Redox project from scratch.".to_string(),
        topology: Topology::Tree,
        scheduling: SchedulingPolicy::LoadBalanced,
        agents: vec![
            AgentSpec::new(AgentRole::Planner, 1, &["architecture", "design"]),
            AgentSpec::new(AgentRole::Implementer, 4, &["codegen", "scaffold"]),
            AgentSpec::new(AgentRole::Tester, 2, &["unit-test", "integration-test"]),
            AgentSpec::new(AgentRole::Reviewer, 1, &["code-review"]),
            AgentSpec::new(AgentRole::Deployer, 1, &["package", "publish"]),
        ],
        max_concurrent: 6,
        timeout_secs: 5400,
        tags: vec!["greenfield".into(), "scaffold".into(), "new-project".into()],
    }
}

pub fn ci_swarm() -> SwarmConfig {
    SwarmConfig {
        name: "ci-swarm".to_string(),
        description: "Continuous integration swarm for build, test, and deploy.".to_string(),
        topology: Topology::Pipeline,
        scheduling: SchedulingPolicy::RoundRobin,
        agents: vec![
            AgentSpec::new(AgentRole::Analyzer, 1, &["lint", "format-check"]),
            AgentSpec::new(AgentRole::Tester, 3, &["unit-test", "integration-test", "fuzz"]),
            AgentSpec::new(AgentRole::Validator, 1, &["typecheck"]),
            AgentSpec::new(AgentRole::Deployer, 1, &["deploy", "publish"]),
        ],
        max_concurrent: 4,
        timeout_secs: 1800,
        tags: vec!["ci".into(), "cd".into(), "automation".into()],
    }
}

/// All reference configs.
pub fn all_reference_configs() -> Vec<SwarmConfig> {
    vec![audit_swarm(), migration_swarm(), greenfield_swarm(), ci_swarm()]
}

// ---------------------------------------------------------------------------
// Config registry
// ---------------------------------------------------------------------------

pub struct ConfigRegistry {
    configs: HashMap<String, SwarmConfig>,
}

impl ConfigRegistry {
    pub fn new() -> Self {
        Self { configs: HashMap::new() }
    }

    pub fn from_defaults() -> Self {
        let mut reg = Self::new();
        for c in all_reference_configs() {
            reg.register(c);
        }
        reg
    }

    pub fn register(&mut self, config: SwarmConfig) {
        self.configs.insert(config.name.clone(), config);
    }

    pub fn get(&self, name: &str) -> Option<&SwarmConfig> {
        self.configs.get(name)
    }

    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.configs.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    pub fn by_topology(&self, topo: Topology) -> Vec<&SwarmConfig> {
        self.configs.values().filter(|c| c.topology == topo).collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&SwarmConfig> {
        self.configs.values().filter(|c| c.tags.iter().any(|t| t == tag)).collect()
    }

    pub fn len(&self) -> usize {
        self.configs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.configs.is_empty()
    }
}

impl Default for ConfigRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- AgentRole --
    #[test]
    fn test_agent_role_labels() {
        assert_eq!(AgentRole::Coordinator.label(), "coordinator");
        assert_eq!(AgentRole::SecurityScanner.label(), "security-scanner");
    }

    // -- Topology --
    #[test]
    fn test_topology_labels() {
        assert_eq!(Topology::Star.label(), "star");
        assert_eq!(Topology::Pipeline.label(), "pipeline");
    }

    // -- SchedulingPolicy --
    #[test]
    fn test_scheduling_labels() {
        assert_eq!(SchedulingPolicy::Adaptive.label(), "adaptive");
    }

    // -- AgentSpec --
    #[test]
    fn test_agent_spec() {
        let a = AgentSpec::new(AgentRole::Tester, 2, &["unit", "fuzz"]);
        assert_eq!(a.count, 2);
        assert_eq!(a.capabilities.len(), 2);
    }

    // -- SwarmConfig --
    #[test]
    fn test_audit_swarm_total_agents() {
        assert_eq!(audit_swarm().total_agents(), 7);
    }

    #[test]
    fn test_migration_swarm_total_agents() {
        assert_eq!(migration_swarm().total_agents(), 10);
    }

    #[test]
    fn test_greenfield_swarm_total_agents() {
        assert_eq!(greenfield_swarm().total_agents(), 9);
    }

    #[test]
    fn test_ci_swarm_total_agents() {
        assert_eq!(ci_swarm().total_agents(), 6);
    }

    #[test]
    fn test_has_role() {
        let a = audit_swarm();
        assert!(a.has_role(AgentRole::Auditor));
        assert!(!a.has_role(AgentRole::Deployer));
    }

    #[test]
    fn test_agents_for_role() {
        assert_eq!(audit_swarm().agents_for_role(AgentRole::SecurityScanner), 2);
    }

    #[test]
    fn test_roles() {
        let roles = ci_swarm().roles();
        assert_eq!(roles.len(), 4);
    }

    // -- all_reference_configs --
    #[test]
    fn test_all_reference_configs_count() {
        assert_eq!(all_reference_configs().len(), 4);
    }

    // -- ConfigRegistry --
    #[test]
    fn test_registry_from_defaults() {
        let reg = ConfigRegistry::from_defaults();
        assert_eq!(reg.len(), 4);
        assert!(!reg.is_empty());
    }

    #[test]
    fn test_registry_get() {
        let reg = ConfigRegistry::from_defaults();
        assert!(reg.get("audit-swarm").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_list_sorted() {
        let reg = ConfigRegistry::from_defaults();
        let names = reg.list();
        assert_eq!(names[0], "audit-swarm");
        assert_eq!(names[1], "ci-swarm");
    }

    #[test]
    fn test_registry_by_topology() {
        let reg = ConfigRegistry::from_defaults();
        let pipelines = reg.by_topology(Topology::Pipeline);
        assert_eq!(pipelines.len(), 2); // migration + ci
    }

    #[test]
    fn test_registry_by_tag() {
        let reg = ConfigRegistry::from_defaults();
        let sec = reg.by_tag("security");
        assert_eq!(sec.len(), 1);
    }

    #[test]
    fn test_registry_empty() {
        let reg = ConfigRegistry::new();
        assert!(reg.is_empty());
    }

    #[test]
    fn test_registry_register_custom() {
        let mut reg = ConfigRegistry::new();
        reg.register(SwarmConfig {
            name: "custom".into(),
            description: "Custom swarm".into(),
            topology: Topology::Mesh,
            scheduling: SchedulingPolicy::RoundRobin,
            agents: vec![AgentSpec::new(AgentRole::Coordinator, 1, &[])],
            max_concurrent: 1,
            timeout_secs: 60,
            tags: vec![],
        });
        assert_eq!(reg.len(), 1);
        assert!(reg.get("custom").is_some());
    }

    #[test]
    fn test_registry_default() {
        let reg = ConfigRegistry::default();
        assert!(reg.is_empty());
    }

    // -- Topology coverage --
    #[test]
    fn test_audit_topology_star() {
        assert_eq!(audit_swarm().topology, Topology::Star);
    }

    #[test]
    fn test_greenfield_topology_tree() {
        assert_eq!(greenfield_swarm().topology, Topology::Tree);
    }

    // -- Config descriptions --
    #[test]
    fn test_configs_have_descriptions() {
        for c in all_reference_configs() {
            assert!(!c.description.is_empty());
        }
    }

    // -- Tags --
    #[test]
    fn test_configs_have_tags() {
        for c in all_reference_configs() {
            assert!(!c.tags.is_empty());
        }
    }
}

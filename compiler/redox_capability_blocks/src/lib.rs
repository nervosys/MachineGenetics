//! Capability blocks for Redox HIR lowering.
//!
//! Scoped capability grants that limit what code within a block can do.
//! Capabilities are hierarchical, composable, and enforced at compile time.

use std::collections::{HashMap, HashSet};
use std::fmt;

// ---------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Read from the filesystem
    FileRead,
    /// Write to the filesystem
    FileWrite,
    /// Network access (inbound)
    NetListen,
    /// Network access (outbound)
    NetConnect,
    /// Spawn sub-processes
    ProcessSpawn,
    /// Access environment variables
    EnvAccess,
    /// Use unsafe code
    Unsafe,
    /// Allocate heap memory
    HeapAlloc,
    /// Perform I/O operations
    Io,
    /// Access system clock / timers
    Clock,
    /// Use FFI (foreign function interface)
    Ffi,
    /// Custom capability
    Custom(String),
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Capability::FileRead => write!(f, "file_read"),
            Capability::FileWrite => write!(f, "file_write"),
            Capability::NetListen => write!(f, "net_listen"),
            Capability::NetConnect => write!(f, "net_connect"),
            Capability::ProcessSpawn => write!(f, "process_spawn"),
            Capability::EnvAccess => write!(f, "env_access"),
            Capability::Unsafe => write!(f, "unsafe"),
            Capability::HeapAlloc => write!(f, "heap_alloc"),
            Capability::Io => write!(f, "io"),
            Capability::Clock => write!(f, "clock"),
            Capability::Ffi => write!(f, "ffi"),
            Capability::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

/// A set of capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySet {
    caps: HashSet<Capability>,
}

impl CapabilitySet {
    pub fn new() -> Self {
        Self { caps: HashSet::new() }
    }

    pub fn full() -> Self {
        let mut caps = HashSet::new();
        caps.insert(Capability::FileRead);
        caps.insert(Capability::FileWrite);
        caps.insert(Capability::NetListen);
        caps.insert(Capability::NetConnect);
        caps.insert(Capability::ProcessSpawn);
        caps.insert(Capability::EnvAccess);
        caps.insert(Capability::Unsafe);
        caps.insert(Capability::HeapAlloc);
        caps.insert(Capability::Io);
        caps.insert(Capability::Clock);
        caps.insert(Capability::Ffi);
        Self { caps }
    }

    pub fn with(mut self, cap: Capability) -> Self {
        self.caps.insert(cap);
        self
    }

    pub fn grant(&mut self, cap: Capability) {
        self.caps.insert(cap);
    }

    pub fn revoke(&mut self, cap: &Capability) {
        self.caps.remove(cap);
    }

    pub fn has(&self, cap: &Capability) -> bool {
        self.caps.contains(cap)
    }

    pub fn is_empty(&self) -> bool {
        self.caps.is_empty()
    }

    pub fn len(&self) -> usize {
        self.caps.len()
    }

    /// Intersection: capabilities in both sets
    pub fn intersect(&self, other: &CapabilitySet) -> CapabilitySet {
        CapabilitySet { caps: self.caps.intersection(&other.caps).cloned().collect() }
    }

    /// Union: capabilities in either set
    pub fn union(&self, other: &CapabilitySet) -> CapabilitySet {
        CapabilitySet { caps: self.caps.union(&other.caps).cloned().collect() }
    }

    /// Difference: capabilities in self but not in other
    pub fn difference(&self, other: &CapabilitySet) -> CapabilitySet {
        CapabilitySet { caps: self.caps.difference(&other.caps).cloned().collect() }
    }

    /// Is self a subset of other?
    pub fn is_subset_of(&self, other: &CapabilitySet) -> bool {
        self.caps.is_subset(&other.caps)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.caps.iter()
    }

    pub fn to_sorted_vec(&self) -> Vec<Capability> {
        let mut v: Vec<_> = self.caps.iter().cloned().collect();
        v.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
        v
    }
}

impl Default for CapabilitySet {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CapabilitySet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sorted = self.to_sorted_vec();
        let parts: Vec<String> = sorted.iter().map(|c| c.to_string()).collect();
        write!(f, "{{{}}}", parts.join(", "))
    }
}

// ---------------------------------------------------------------------------
// Capability block — HIR node
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapBlockMode {
    /// Grant only these capabilities (whitelist)
    Grant,
    /// Revoke these capabilities from the ambient set (blacklist)
    Deny,
}

#[derive(Debug, Clone)]
pub struct CapabilityBlock {
    pub id: u64,
    pub mode: CapBlockMode,
    pub capabilities: CapabilitySet,
    pub children: Vec<HirNode>,
}

impl CapabilityBlock {
    pub fn effective_caps(&self, ambient: &CapabilitySet) -> CapabilitySet {
        match self.mode {
            CapBlockMode::Grant => {
                // Only grant capabilities that are available in the ambient set
                self.capabilities.intersect(ambient)
            }
            CapBlockMode::Deny => ambient.difference(&self.capabilities),
        }
    }
}

// ---------------------------------------------------------------------------
// Simplified HIR nodes (for testing capability checking)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum HirNode {
    /// A literal value (no capabilities needed)
    Literal(String),
    /// Variable reference
    VarRef(String),
    /// Function call with required capabilities
    Call { name: String, required_caps: CapabilitySet },
    /// A block of statements
    Block(Vec<HirNode>),
    /// A capability block: with cap { ... }
    CapBlock(CapabilityBlock),
    /// Let binding
    Let { name: String, value: Box<HirNode> },
    /// If expression
    If { cond: Box<HirNode>, then_branch: Box<HirNode>, else_branch: Option<Box<HirNode>> },
}

// ---------------------------------------------------------------------------
// Capability checker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityViolation {
    pub required: Capability,
    pub location: String,
    pub available: CapabilitySet,
    pub message: String,
}

impl fmt::Display for CapabilityViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "capability violation at {}: {} — {}", self.location, self.required, self.message)
    }
}

pub struct CapabilityChecker {
    violations: Vec<CapabilityViolation>,
    /// Function capability requirements: fn_name -> required capabilities
    fn_caps: HashMap<String, CapabilitySet>,
}

impl CapabilityChecker {
    pub fn new() -> Self {
        Self { violations: Vec::new(), fn_caps: HashMap::new() }
    }

    pub fn violations(&self) -> &[CapabilityViolation] {
        &self.violations
    }

    pub fn has_violations(&self) -> bool {
        !self.violations.is_empty()
    }

    /// Register a function's capability requirements.
    pub fn register_fn(&mut self, name: String, required: CapabilitySet) {
        self.fn_caps.insert(name, required);
    }

    /// Check an HIR node against the available capabilities.
    pub fn check(&mut self, node: &HirNode, available: &CapabilitySet, path: &str) {
        match node {
            HirNode::Literal(_) | HirNode::VarRef(_) => {}
            HirNode::Call { name, required_caps } => {
                // Merge with registered function requirements
                let mut required = required_caps.clone();
                if let Some(fn_reqs) = self.fn_caps.get(name) {
                    required = required.union(fn_reqs);
                }

                for cap in required.iter() {
                    if !available.has(cap) {
                        self.violations.push(CapabilityViolation {
                            required: cap.clone(),
                            location: format!("{path}/call:{name}"),
                            available: available.clone(),
                            message: format!(
                                "function '{name}' requires {cap} but it is not available"
                            ),
                        });
                    }
                }
            }
            HirNode::Block(stmts) => {
                for (i, stmt) in stmts.iter().enumerate() {
                    self.check(stmt, available, &format!("{path}/stmt:{i}"));
                }
            }
            HirNode::CapBlock(cap_block) => {
                let effective = cap_block.effective_caps(available);
                for (i, child) in cap_block.children.iter().enumerate() {
                    self.check(
                        child,
                        &effective,
                        &format!("{path}/cap_block:{}/child:{i}", cap_block.id),
                    );
                }
            }
            HirNode::Let { name, value } => {
                self.check(value, available, &format!("{path}/let:{name}"));
            }
            HirNode::If { cond, then_branch, else_branch } => {
                self.check(cond, available, &format!("{path}/if:cond"));
                self.check(then_branch, available, &format!("{path}/if:then"));
                if let Some(else_br) = else_branch {
                    self.check(else_br, available, &format!("{path}/if:else"));
                }
            }
        }
    }
}

impl Default for CapabilityChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Capability inference
// ---------------------------------------------------------------------------

/// Infer the minimum capability set needed for an HIR node.
pub fn infer_required_caps(
    node: &HirNode,
    fn_caps: &HashMap<String, CapabilitySet>,
) -> CapabilitySet {
    match node {
        HirNode::Literal(_) | HirNode::VarRef(_) => CapabilitySet::new(),
        HirNode::Call { name, required_caps } => {
            let mut caps = required_caps.clone();
            if let Some(fn_reqs) = fn_caps.get(name) {
                caps = caps.union(fn_reqs);
            }
            caps
        }
        HirNode::Block(stmts) => {
            let mut caps = CapabilitySet::new();
            for stmt in stmts {
                caps = caps.union(&infer_required_caps(stmt, fn_caps));
            }
            caps
        }
        HirNode::CapBlock(cap_block) => {
            // The cap block itself doesn't add requirements — it constrains them
            let mut caps = CapabilitySet::new();
            for child in &cap_block.children {
                caps = caps.union(&infer_required_caps(child, fn_caps));
            }
            caps
        }
        HirNode::Let { value, .. } => infer_required_caps(value, fn_caps),
        HirNode::If { cond, then_branch, else_branch } => {
            let mut caps = infer_required_caps(cond, fn_caps);
            caps = caps.union(&infer_required_caps(then_branch, fn_caps));
            if let Some(else_br) = else_branch {
                caps = caps.union(&infer_required_caps(else_br, fn_caps));
            }
            caps
        }
    }
}

// ---------------------------------------------------------------------------
// HIR lowering: capability block desugaring
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LoweredBlock {
    pub original_id: u64,
    pub effective_caps: CapabilitySet,
    pub children: Vec<LoweredNode>,
}

#[derive(Debug, Clone)]
pub enum LoweredNode {
    Literal(String),
    VarRef(String),
    Call {
        name: String,
        required_caps: CapabilitySet,
    },
    Block(Vec<LoweredNode>),
    CapBlock(LoweredBlock),
    Let {
        name: String,
        value: Box<LoweredNode>,
    },
    If {
        cond: Box<LoweredNode>,
        then_branch: Box<LoweredNode>,
        else_branch: Option<Box<LoweredNode>>,
    },
}

/// Lower HIR capability blocks: compute effective caps and propagate.
pub fn lower_cap_blocks(node: &HirNode, ambient: &CapabilitySet) -> LoweredNode {
    match node {
        HirNode::Literal(v) => LoweredNode::Literal(v.clone()),
        HirNode::VarRef(v) => LoweredNode::VarRef(v.clone()),
        HirNode::Call { name, required_caps } => {
            LoweredNode::Call { name: name.clone(), required_caps: required_caps.clone() }
        }
        HirNode::Block(stmts) => {
            let lowered: Vec<LoweredNode> =
                stmts.iter().map(|s| lower_cap_blocks(s, ambient)).collect();
            LoweredNode::Block(lowered)
        }
        HirNode::CapBlock(cap_block) => {
            let effective = cap_block.effective_caps(ambient);
            let children: Vec<LoweredNode> =
                cap_block.children.iter().map(|c| lower_cap_blocks(c, &effective)).collect();
            LoweredNode::CapBlock(LoweredBlock {
                original_id: cap_block.id,
                effective_caps: effective,
                children,
            })
        }
        HirNode::Let { name, value } => LoweredNode::Let {
            name: name.clone(),
            value: Box::new(lower_cap_blocks(value, ambient)),
        },
        HirNode::If { cond, then_branch, else_branch } => LoweredNode::If {
            cond: Box::new(lower_cap_blocks(cond, ambient)),
            then_branch: Box::new(lower_cap_blocks(then_branch, ambient)),
            else_branch: else_branch.as_ref().map(|e| Box::new(lower_cap_blocks(e, ambient))),
        },
    }
}

// ---------------------------------------------------------------------------
// Summary / report
// ---------------------------------------------------------------------------

pub struct CapabilityReport {
    pub ambient: CapabilitySet,
    pub violations: Vec<CapabilityViolation>,
    pub inferred_required: CapabilitySet,
}

impl CapabilityReport {
    pub fn is_ok(&self) -> bool {
        self.violations.is_empty()
    }

    pub fn summary(&self) -> String {
        format!(
            "Capability check: {} violations, {} ambient caps, {} required caps",
            self.violations.len(),
            self.ambient.len(),
            self.inferred_required.len(),
        )
    }
}

/// Full capability analysis pipeline.
pub fn analyze_capabilities(
    node: &HirNode,
    ambient: &CapabilitySet,
    fn_caps: &HashMap<String, CapabilitySet>,
) -> CapabilityReport {
    let mut checker = CapabilityChecker::new();
    for (name, caps) in fn_caps {
        checker.register_fn(name.clone(), caps.clone());
    }
    checker.check(node, ambient, "root");

    let inferred = infer_required_caps(node, fn_caps);

    CapabilityReport {
        ambient: ambient.clone(),
        violations: checker.violations().to_vec(),
        inferred_required: inferred,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn io_caps() -> CapabilitySet {
        CapabilitySet::new()
            .with(Capability::Io)
            .with(Capability::FileRead)
            .with(Capability::FileWrite)
    }

    fn net_caps() -> CapabilitySet {
        CapabilitySet::new().with(Capability::NetConnect).with(Capability::NetListen)
    }

    // -- CapabilitySet tests --

    #[test]
    fn test_empty_set() {
        let cs = CapabilitySet::new();
        assert!(cs.is_empty());
        assert_eq!(cs.len(), 0);
    }

    #[test]
    fn test_set_with() {
        let cs = CapabilitySet::new().with(Capability::Io);
        assert!(cs.has(&Capability::Io));
        assert!(!cs.has(&Capability::Ffi));
    }

    #[test]
    fn test_set_grant_revoke() {
        let mut cs = CapabilitySet::new();
        cs.grant(Capability::FileRead);
        assert!(cs.has(&Capability::FileRead));
        cs.revoke(&Capability::FileRead);
        assert!(!cs.has(&Capability::FileRead));
    }

    #[test]
    fn test_set_intersect() {
        let a = io_caps();
        let b = CapabilitySet::new().with(Capability::FileRead).with(Capability::NetConnect);
        let inter = a.intersect(&b);
        assert!(inter.has(&Capability::FileRead));
        assert!(!inter.has(&Capability::FileWrite));
        assert!(!inter.has(&Capability::NetConnect));
    }

    #[test]
    fn test_set_union() {
        let a = io_caps();
        let b = net_caps();
        let u = a.union(&b);
        assert!(u.has(&Capability::Io));
        assert!(u.has(&Capability::NetConnect));
    }

    #[test]
    fn test_set_difference() {
        let full = io_caps();
        let remove = CapabilitySet::new().with(Capability::FileWrite);
        let diff = full.difference(&remove);
        assert!(diff.has(&Capability::FileRead));
        assert!(diff.has(&Capability::Io));
        assert!(!diff.has(&Capability::FileWrite));
    }

    #[test]
    fn test_set_subset() {
        let small = CapabilitySet::new().with(Capability::FileRead);
        let big = io_caps();
        assert!(small.is_subset_of(&big));
        assert!(!big.is_subset_of(&small));
    }

    #[test]
    fn test_full_set() {
        let full = CapabilitySet::full();
        assert!(full.has(&Capability::FileRead));
        assert!(full.has(&Capability::Ffi));
        assert!(full.has(&Capability::HeapAlloc));
    }

    #[test]
    fn test_display() {
        let cs = CapabilitySet::new().with(Capability::Io);
        assert_eq!(cs.to_string(), "{io}");
    }

    #[test]
    fn test_custom_capability() {
        let cs = CapabilitySet::new().with(Capability::Custom("gpu".to_string()));
        assert!(cs.has(&Capability::Custom("gpu".to_string())));
        assert!(!cs.has(&Capability::Custom("tpu".to_string())));
    }

    // -- CapabilityBlock tests --

    #[test]
    fn test_cap_block_grant_mode() {
        let ambient = io_caps().union(&net_caps());
        let block = CapabilityBlock {
            id: 1,
            mode: CapBlockMode::Grant,
            capabilities: io_caps(),
            children: vec![],
        };
        let effective = block.effective_caps(&ambient);
        assert!(effective.has(&Capability::FileRead));
        assert!(!effective.has(&Capability::NetConnect));
    }

    #[test]
    fn test_cap_block_deny_mode() {
        let ambient = io_caps().union(&net_caps());
        let block = CapabilityBlock {
            id: 2,
            mode: CapBlockMode::Deny,
            capabilities: net_caps(),
            children: vec![],
        };
        let effective = block.effective_caps(&ambient);
        assert!(effective.has(&Capability::FileRead));
        assert!(!effective.has(&Capability::NetConnect));
    }

    #[test]
    fn test_cap_block_grant_limited_by_ambient() {
        let ambient = CapabilitySet::new().with(Capability::FileRead);
        let block = CapabilityBlock {
            id: 3,
            mode: CapBlockMode::Grant,
            capabilities: io_caps(), // grants FileRead, FileWrite, Io
            children: vec![],
        };
        let effective = block.effective_caps(&ambient);
        // Can only grant what ambient has
        assert!(effective.has(&Capability::FileRead));
        assert!(!effective.has(&Capability::FileWrite));
        assert!(!effective.has(&Capability::Io));
    }

    // -- Checker tests --

    #[test]
    fn test_checker_no_violations() {
        let mut checker = CapabilityChecker::new();
        let node = HirNode::Call {
            name: "read_file".to_string(),
            required_caps: CapabilitySet::new().with(Capability::FileRead),
        };
        checker.check(&node, &io_caps(), "test");
        assert!(!checker.has_violations());
    }

    #[test]
    fn test_checker_violation() {
        let mut checker = CapabilityChecker::new();
        let node = HirNode::Call { name: "connect".to_string(), required_caps: net_caps() };
        checker.check(&node, &io_caps(), "test");
        assert!(checker.has_violations());
        assert_eq!(checker.violations().len(), 2); // NetConnect + NetListen
    }

    #[test]
    fn test_checker_nested_cap_block() {
        let mut checker = CapabilityChecker::new();
        let node = HirNode::CapBlock(CapabilityBlock {
            id: 10,
            mode: CapBlockMode::Grant,
            capabilities: CapabilitySet::new().with(Capability::FileRead),
            children: vec![
                HirNode::Call {
                    name: "read".to_string(),
                    required_caps: CapabilitySet::new().with(Capability::FileRead),
                },
                HirNode::Call {
                    name: "write".to_string(),
                    required_caps: CapabilitySet::new().with(Capability::FileWrite),
                },
            ],
        });
        checker.check(&node, &io_caps(), "test");
        assert!(checker.has_violations());
        assert_eq!(checker.violations().len(), 1);
        assert_eq!(checker.violations()[0].required, Capability::FileWrite);
    }

    #[test]
    fn test_checker_registered_fn_caps() {
        let mut checker = CapabilityChecker::new();
        checker.register_fn("dangerous".to_string(), CapabilitySet::new().with(Capability::Unsafe));
        let node =
            HirNode::Call { name: "dangerous".to_string(), required_caps: CapabilitySet::new() };
        let ambient = io_caps();
        checker.check(&node, &ambient, "test");
        assert!(checker.has_violations());
        assert_eq!(checker.violations()[0].required, Capability::Unsafe);
    }

    #[test]
    fn test_checker_let_binding() {
        let mut checker = CapabilityChecker::new();
        let node = HirNode::Let {
            name: "x".to_string(),
            value: Box::new(HirNode::Call {
                name: "net_call".to_string(),
                required_caps: net_caps(),
            }),
        };
        checker.check(&node, &io_caps(), "test");
        assert!(checker.has_violations());
    }

    #[test]
    fn test_checker_if_expr() {
        let mut checker = CapabilityChecker::new();
        let node = HirNode::If {
            cond: Box::new(HirNode::Literal("true".to_string())),
            then_branch: Box::new(HirNode::Call {
                name: "read".to_string(),
                required_caps: CapabilitySet::new().with(Capability::FileRead),
            }),
            else_branch: Some(Box::new(HirNode::Call {
                name: "spawn".to_string(),
                required_caps: CapabilitySet::new().with(Capability::ProcessSpawn),
            })),
        };
        let ambient = CapabilitySet::new().with(Capability::FileRead);
        checker.check(&node, &ambient, "test");
        assert!(checker.has_violations());
        assert_eq!(checker.violations().len(), 1);
        assert_eq!(checker.violations()[0].required, Capability::ProcessSpawn);
    }

    // -- Inference tests --

    #[test]
    fn test_infer_caps_literal() {
        let caps = infer_required_caps(&HirNode::Literal("42".to_string()), &HashMap::new());
        assert!(caps.is_empty());
    }

    #[test]
    fn test_infer_caps_call() {
        let node = HirNode::Call {
            name: "read".to_string(),
            required_caps: CapabilitySet::new().with(Capability::FileRead),
        };
        let caps = infer_required_caps(&node, &HashMap::new());
        assert!(caps.has(&Capability::FileRead));
        assert_eq!(caps.len(), 1);
    }

    #[test]
    fn test_infer_caps_block() {
        let node = HirNode::Block(vec![
            HirNode::Call {
                name: "read".to_string(),
                required_caps: CapabilitySet::new().with(Capability::FileRead),
            },
            HirNode::Call {
                name: "connect".to_string(),
                required_caps: CapabilitySet::new().with(Capability::NetConnect),
            },
        ]);
        let caps = infer_required_caps(&node, &HashMap::new());
        assert!(caps.has(&Capability::FileRead));
        assert!(caps.has(&Capability::NetConnect));
    }

    // -- Lowering tests --

    #[test]
    fn test_lower_cap_block() {
        let ambient = io_caps().union(&net_caps());
        let node = HirNode::CapBlock(CapabilityBlock {
            id: 1,
            mode: CapBlockMode::Grant,
            capabilities: io_caps(),
            children: vec![HirNode::Literal("ok".to_string())],
        });
        let lowered = lower_cap_blocks(&node, &ambient);
        match lowered {
            LoweredNode::CapBlock(lb) => {
                assert_eq!(lb.original_id, 1);
                assert!(lb.effective_caps.has(&Capability::FileRead));
                assert!(!lb.effective_caps.has(&Capability::NetConnect));
                assert_eq!(lb.children.len(), 1);
            }
            _ => panic!("expected LoweredNode::CapBlock"),
        }
    }

    // -- Full pipeline tests --

    #[test]
    fn test_analyze_capabilities_ok() {
        let node = HirNode::Call {
            name: "read".to_string(),
            required_caps: CapabilitySet::new().with(Capability::FileRead),
        };
        let ambient = io_caps();
        let report = analyze_capabilities(&node, &ambient, &HashMap::new());
        assert!(report.is_ok());
        assert!(report.summary().contains("0 violations"));
    }

    #[test]
    fn test_analyze_capabilities_violation() {
        let node = HirNode::Call {
            name: "spawn".to_string(),
            required_caps: CapabilitySet::new().with(Capability::ProcessSpawn),
        };
        let ambient = io_caps();
        let report = analyze_capabilities(&node, &ambient, &HashMap::new());
        assert!(!report.is_ok());
    }

    #[test]
    fn test_violation_display() {
        let v = CapabilityViolation {
            required: Capability::Unsafe,
            location: "main/call:foo".to_string(),
            available: io_caps(),
            message: "unsafe not available".to_string(),
        };
        let s = v.to_string();
        assert!(s.contains("capability violation"));
        assert!(s.contains("unsafe"));
    }

    // -- Nested capability blocks --

    #[test]
    fn test_nested_deny_blocks() {
        let mut checker = CapabilityChecker::new();
        // Deny NetConnect, then inside deny FileWrite
        let node = HirNode::CapBlock(CapabilityBlock {
            id: 1,
            mode: CapBlockMode::Deny,
            capabilities: net_caps(),
            children: vec![HirNode::CapBlock(CapabilityBlock {
                id: 2,
                mode: CapBlockMode::Deny,
                capabilities: CapabilitySet::new().with(Capability::FileWrite),
                children: vec![HirNode::Call {
                    name: "read".to_string(),
                    required_caps: CapabilitySet::new().with(Capability::FileRead),
                }],
            })],
        });
        let ambient = io_caps().union(&net_caps());
        checker.check(&node, &ambient, "test");
        assert!(!checker.has_violations());
    }
}

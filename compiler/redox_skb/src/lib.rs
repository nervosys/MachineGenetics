//! # Safety Knowledge Base (SKB)
//!
//! The SKB is the central database of safety rules for the Redox compiler.
//! Every rule conforms to a common schema and is stored as a structured record.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │                 SafetyKnowledgeBase               │
//! │  ┌────────┐ ┌────────┐ ┌────────┐ ┌──────────┐  │
//! │  │Ownership│ │Borrow  │ │Lifetime│ │TypeSafety│  │
//! │  │ 2,847   │ │ 1,203  │ │  894   │ │  3,412   │  │
//! │  └────────┘ └────────┘ └────────┘ └──────────┘  │
//! │  ┌──────────┐ ┌─────┐                            │
//! │  │Concurrency│ │ FFI │                            │
//! │  │   567     │ │ 234 │                            │
//! │  └──────────┘ └─────┘                            │
//! └──────────────────────────────────────────────────┘
//! ```
//!
//! Reference: REDOX_PROPOSAL.md §15

use std::collections::HashMap;
use std::fmt;

// ===========================================================================
// Core types
// ===========================================================================

/// Unique rule identifier (e.g., "OWN-0042", "BR-1203").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleId(pub String);

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Semantic version for rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemanticVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The six safety databases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Database {
    Ownership,
    Borrow,
    Lifetime,
    TypeSafety,
    Concurrency,
    Ffi,
}

impl Database {
    /// ID prefix for rules in this database.
    pub fn prefix(&self) -> &'static str {
        match self {
            Database::Ownership => "OWN",
            Database::Borrow => "BR",
            Database::Lifetime => "LT",
            Database::TypeSafety => "TS",
            Database::Concurrency => "CON",
            Database::Ffi => "FFI",
        }
    }

    /// All database variants.
    pub fn all() -> &'static [Database] {
        &[
            Database::Ownership,
            Database::Borrow,
            Database::Lifetime,
            Database::TypeSafety,
            Database::Concurrency,
            Database::Ffi,
        ]
    }

    /// Target rule count for the seed corpus.
    pub fn seed_count(&self) -> usize {
        match self {
            Database::Ownership => 2_847,
            Database::Borrow => 1_203,
            Database::Lifetime => 894,
            Database::TypeSafety => 3_412,
            Database::Concurrency => 567,
            Database::Ffi => 234,
        }
    }
}

impl fmt::Display for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Database::Ownership => write!(f, "ownership"),
            Database::Borrow => write!(f, "borrow"),
            Database::Lifetime => write!(f, "lifetime"),
            Database::TypeSafety => write!(f, "type_safety"),
            Database::Concurrency => write!(f, "concurrency"),
            Database::Ffi => write!(f, "ffi"),
        }
    }
}

/// Rule severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Hint,
    Info,
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Hint => write!(f, "hint"),
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

/// Rule scope — where the rule applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Function,
    Module,
    Crate,
    Global,
}

/// Rule source — who created the rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleSource {
    BuiltIn,
    ProjectCustom,
    CommunityContributed,
}

/// Rule lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Proposed,
    Staged,
    Active,
    Deprecated,
}

impl fmt::Display for Lifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Lifecycle::Proposed => write!(f, "proposed"),
            Lifecycle::Staged => write!(f, "staged"),
            Lifecycle::Active => write!(f, "active"),
            Lifecycle::Deprecated => write!(f, "deprecated"),
        }
    }
}

// ===========================================================================
// Pattern language (§15.2)
// ===========================================================================

/// Structural patterns that match against Redox MIR nodes.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    // Ownership patterns
    UseAfterMove { var: VarPattern, move_site: SitePattern, use_site: SitePattern },
    DoubleMove { var: VarPattern, sites: (SitePattern, SitePattern) },
    MoveInLoop { var: VarPattern, loop_kind: LoopKind },

    // Borrow patterns
    MutableBorrow { target_type: TypePattern, context: ContextSpec },
    AliasingBorrow { refs: Vec<RefPattern>, overlap: OverlapKind },
    BorrowEscapes { ref_pattern: RefPattern, escapes_to: ScopePattern },

    // Lifetime patterns
    DanglingRef { ref_pattern: RefPattern, referent_scope: ScopePattern },
    LifetimeMismatch { expected: RegionPattern, actual: RegionPattern },
    SelfReferential { struct_type: TypePattern },

    // Type safety patterns
    TypeConversion { from: TypePattern, to: TypePattern, target_arch: ArchPattern },
    NarrowingCast { from: TypePattern, to: TypePattern },
    UnsoundTransmute { from: TypePattern, to: TypePattern },
    UninitRead { var: VarPattern, site: SitePattern },

    // Concurrency patterns
    DataRace { var: VarPattern, threads: Vec<ThreadPattern> },
    DeadlockRisk { locks: Vec<LockPattern>, order: OrderPattern },
    SendViolation { type_pattern: TypePattern, context: ContextSpec },

    // FFI patterns
    NullPointerDeref { source: FfiSource, site: SitePattern },
    LayoutMismatch { redox_type: TypePattern, foreign_type: TypePattern },
    MissingFree { alloc_site: SitePattern, foreign_allocator: String },
}

impl Pattern {
    /// The pattern category name.
    pub fn name(&self) -> &'static str {
        match self {
            Pattern::UseAfterMove { .. } => "UseAfterMove",
            Pattern::DoubleMove { .. } => "DoubleMove",
            Pattern::MoveInLoop { .. } => "MoveInLoop",
            Pattern::MutableBorrow { .. } => "MutableBorrow",
            Pattern::AliasingBorrow { .. } => "AliasingBorrow",
            Pattern::BorrowEscapes { .. } => "BorrowEscapes",
            Pattern::DanglingRef { .. } => "DanglingRef",
            Pattern::LifetimeMismatch { .. } => "LifetimeMismatch",
            Pattern::SelfReferential { .. } => "SelfReferential",
            Pattern::TypeConversion { .. } => "TypeConversion",
            Pattern::NarrowingCast { .. } => "NarrowingCast",
            Pattern::UnsoundTransmute { .. } => "UnsoundTransmute",
            Pattern::UninitRead { .. } => "UninitRead",
            Pattern::DataRace { .. } => "DataRace",
            Pattern::DeadlockRisk { .. } => "DeadlockRisk",
            Pattern::SendViolation { .. } => "SendViolation",
            Pattern::NullPointerDeref { .. } => "NullPointerDeref",
            Pattern::LayoutMismatch { .. } => "LayoutMismatch",
            Pattern::MissingFree { .. } => "MissingFree",
        }
    }

    /// Which database this pattern belongs to.
    pub fn database(&self) -> Database {
        match self {
            Pattern::UseAfterMove { .. }
            | Pattern::DoubleMove { .. }
            | Pattern::MoveInLoop { .. } => Database::Ownership,

            Pattern::MutableBorrow { .. }
            | Pattern::AliasingBorrow { .. }
            | Pattern::BorrowEscapes { .. } => Database::Borrow,

            Pattern::DanglingRef { .. }
            | Pattern::LifetimeMismatch { .. }
            | Pattern::SelfReferential { .. } => Database::Lifetime,

            Pattern::TypeConversion { .. }
            | Pattern::NarrowingCast { .. }
            | Pattern::UnsoundTransmute { .. }
            | Pattern::UninitRead { .. } => Database::TypeSafety,

            Pattern::DataRace { .. }
            | Pattern::DeadlockRisk { .. }
            | Pattern::SendViolation { .. } => Database::Concurrency,

            Pattern::NullPointerDeref { .. }
            | Pattern::LayoutMismatch { .. }
            | Pattern::MissingFree { .. } => Database::Ffi,
        }
    }
}

/// Variable pattern with wildcard support.
#[derive(Debug, Clone, PartialEq)]
pub enum VarPattern {
    Exact(String),
    AnyVar,
    Typed(TypePattern),
}

/// Type pattern with wildcard support.
#[derive(Debug, Clone, PartialEq)]
pub enum TypePattern {
    Exact(String),
    AnyType,
    Generic(String, Vec<TypePattern>),
    Wildcard,
    OneOf(Vec<TypePattern>),
}

/// Source site pattern.
#[derive(Debug, Clone, PartialEq)]
pub enum SitePattern {
    Line(u32),
    AnySite,
    InScope(ScopePattern),
}

/// Scope pattern for matching.
#[derive(Debug, Clone, PartialEq)]
pub enum ScopePattern {
    Function(String),
    Module(String),
    Crate,
    Any,
}

/// Region (lifetime) pattern.
#[derive(Debug, Clone, PartialEq)]
pub enum RegionPattern {
    Named(String),
    Static,
    Any,
}

/// Context specification for patterns.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ContextSpec {
    pub constraints: HashMap<String, String>,
}

/// Reference pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct RefPattern {
    pub mutable: bool,
    pub target_type: TypePattern,
}

/// Loop kind for MoveInLoop.
#[derive(Debug, Clone, PartialEq)]
pub enum LoopKind {
    For,
    While,
    Loop,
    Any,
}

/// Overlap kind for aliasing borrows.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlapKind {
    ReadWrite,
    WriteWrite,
    Any,
}

/// Architecture pattern for type conversions.
#[derive(Debug, Clone, PartialEq)]
pub enum ArchPattern {
    Exact(String),
    Any,
}

/// Thread pattern for concurrency analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadPattern {
    pub name: VarPattern,
}

/// Lock pattern for deadlock analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct LockPattern {
    pub lock_type: TypePattern,
    pub site: SitePattern,
}

/// Lock ordering pattern.
#[derive(Debug, Clone, PartialEq)]
pub enum OrderPattern {
    Reversed,
    Any,
}

/// FFI source descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct FfiSource {
    pub language: String,
    pub function: String,
}

// ===========================================================================
// Fix templates
// ===========================================================================

/// Automated fix template for a rule.
#[derive(Debug, Clone, PartialEq)]
pub struct FixTemplate {
    /// Human-readable description of the fix.
    pub description: String,
    /// Template code for the fix (with placeholders).
    pub template: String,
}

/// Alternative resolution approach.
#[derive(Debug, Clone, PartialEq)]
pub struct Alternative {
    pub description: String,
    pub trade_offs: String,
}

// ===========================================================================
// Rule (§15.1)
// ===========================================================================

/// A single safety rule in the SKB.
#[derive(Debug, Clone)]
pub struct Rule {
    // Identity
    pub id: RuleId,
    pub database: Database,
    pub version: SemanticVersion,

    // Pattern matching
    pub pattern: Pattern,
    pub context: ContextSpec,
    pub scope: Scope,

    // Semantics
    pub severity: Severity,
    pub category: String,
    pub description: String,
    pub rationale: String,

    // Resolution
    pub fix_template: Option<FixTemplate>,
    pub fix_confidence: f64,
    pub alternatives: Vec<Alternative>,

    // Metadata
    pub source: RuleSource,
    pub lifecycle: Lifecycle,
    pub frequency: u64,
    pub false_positive_rate: f64,
    pub tags: Vec<String>,
}

impl Rule {
    /// Convenience check: is this rule currently active?
    pub fn is_active(&self) -> bool {
        self.lifecycle == Lifecycle::Active
    }

    /// Does this rule have a reliable auto-fix (confidence > threshold)?
    pub fn has_reliable_fix(&self, threshold: f64) -> bool {
        self.fix_template.is_some() && self.fix_confidence >= threshold
    }
}

// ===========================================================================
// Safety Knowledge Base
// ===========================================================================

/// The Safety Knowledge Base — central repository of all safety rules.
pub struct SafetyKnowledgeBase {
    rules: Vec<Rule>,
    /// B-tree index: (database, category) → rule indices.
    category_index: HashMap<(Database, String), Vec<usize>>,
    /// Hash index: pattern name → rule indices.
    pattern_index: HashMap<String, Vec<usize>>,
    /// Severity index: severity → rule indices.
    severity_index: HashMap<Severity, Vec<usize>>,
}

impl SafetyKnowledgeBase {
    /// Create an empty SKB.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            category_index: HashMap::new(),
            pattern_index: HashMap::new(),
            severity_index: HashMap::new(),
        }
    }

    /// Total number of rules.
    pub fn total_rules(&self) -> usize {
        self.rules.len()
    }

    /// Number of rules in a specific database.
    pub fn rules_in_database(&self, db: Database) -> usize {
        self.rules.iter().filter(|r| r.database == db).count()
    }

    /// Add a rule to the SKB and update all indices.
    pub fn add_rule(&mut self, rule: Rule) {
        let idx = self.rules.len();

        // Update category index
        self.category_index
            .entry((rule.database, rule.category.clone()))
            .or_default()
            .push(idx);

        // Update pattern index
        self.pattern_index
            .entry(rule.pattern.name().to_string())
            .or_default()
            .push(idx);

        // Update severity index
        self.severity_index
            .entry(rule.severity)
            .or_default()
            .push(idx);

        self.rules.push(rule);
    }

    /// Look up a rule by its ID.
    pub fn get_rule(&self, id: &RuleId) -> Option<&Rule> {
        self.rules.iter().find(|r| &r.id == id)
    }

    /// Query rules by database and category.
    pub fn query_by_category(&self, db: Database, category: &str) -> Vec<&Rule> {
        self.category_index
            .get(&(db, category.to_string()))
            .map(|indices| indices.iter().map(|&i| &self.rules[i]).collect())
            .unwrap_or_default()
    }

    /// Query rules by pattern name.
    pub fn query_by_pattern(&self, pattern_name: &str) -> Vec<&Rule> {
        self.pattern_index
            .get(pattern_name)
            .map(|indices| indices.iter().map(|&i| &self.rules[i]).collect())
            .unwrap_or_default()
    }

    /// Query rules by severity (exact match).
    pub fn query_by_severity(&self, severity: Severity) -> Vec<&Rule> {
        self.severity_index
            .get(&severity)
            .map(|indices| indices.iter().map(|&i| &self.rules[i]).collect())
            .unwrap_or_default()
    }

    /// Query rules by severity >= threshold.
    pub fn query_by_min_severity(&self, min: Severity) -> Vec<&Rule> {
        self.rules.iter().filter(|r| r.severity >= min).collect()
    }

    /// Query rules with high-confidence auto-fixes.
    pub fn query_fixable(&self, min_confidence: f64) -> Vec<&Rule> {
        self.rules
            .iter()
            .filter(|r| r.has_reliable_fix(min_confidence))
            .collect()
    }

    /// Query active rules only from a specific database.
    pub fn query_active_in_database(&self, db: Database) -> Vec<&Rule> {
        self.rules
            .iter()
            .filter(|r| r.database == db && r.is_active())
            .collect()
    }

    /// Summary statistics per database.
    pub fn statistics(&self) -> Vec<DatabaseStats> {
        Database::all()
            .iter()
            .map(|&db| {
                let db_rules: Vec<&Rule> = self.rules.iter().filter(|r| r.database == db).collect();
                let active = db_rules.iter().filter(|r| r.is_active()).count();
                let with_fix = db_rules.iter().filter(|r| r.fix_template.is_some()).count();
                DatabaseStats {
                    database: db,
                    total: db_rules.len(),
                    active,
                    with_fix,
                    target: db.seed_count(),
                }
            })
            .collect()
    }
}

impl Default for SafetyKnowledgeBase {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for a single database.
#[derive(Debug)]
pub struct DatabaseStats {
    pub database: Database,
    pub total: usize,
    pub active: usize,
    pub with_fix: usize,
    pub target: usize,
}

// ===========================================================================
// Seed corpus generator
// ===========================================================================

/// Seed corpus configuration per database.
struct SeedConfig {
    database: Database,
    categories: Vec<(&'static str, usize)>,
}

/// Generate the seed corpus for the SKB.
///
/// This creates representative rules across all six databases.
/// The seed corpus provides a structural foundation — real rule bodies
/// will be refined as the compiler matures.
pub fn seed_corpus() -> SafetyKnowledgeBase {
    let configs = seed_configs();
    let mut skb = SafetyKnowledgeBase::new();

    for config in &configs {
        let mut rule_num = 0u32;
        for &(category, count) in &config.categories {
            for i in 0..count {
                rule_num += 1;
                let id = format!("{}-{:04}", config.database.prefix(), rule_num);
                let pattern = seed_pattern(config.database, category, i);
                let severity = seed_severity(i);
                let has_fix = i % 3 != 0; // ~66% have auto-fixes

                skb.add_rule(Rule {
                    id: RuleId(id),
                    database: config.database,
                    version: SemanticVersion::new(1, 0, 0),
                    pattern,
                    context: ContextSpec::default(),
                    scope: seed_scope(i),
                    severity,
                    category: category.to_string(),
                    description: format!("{} rule #{} in {}", category, i + 1, config.database),
                    rationale: format!("Prevents {} violations", category),
                    fix_template: if has_fix {
                        Some(FixTemplate {
                            description: format!("Auto-fix for {category}"),
                            template: format!("/* fix {category} #{} */", i + 1),
                        })
                    } else {
                        None
                    },
                    fix_confidence: if has_fix { 0.85 } else { 0.0 },
                    alternatives: Vec::new(),
                    source: RuleSource::BuiltIn,
                    lifecycle: Lifecycle::Active,
                    frequency: 1000 - (i as u64 % 1000),
                    false_positive_rate: 0.02,
                    tags: vec![category.to_string(), config.database.to_string()],
                });
            }
        }
    }

    skb
}

fn seed_configs() -> Vec<SeedConfig> {
    vec![
        // Ownership: 2,847 rules
        SeedConfig {
            database: Database::Ownership,
            categories: vec![
                ("use-after-move", 712),
                ("double-move", 534),
                ("move-in-loop", 356),
                ("partial-move", 445),
                ("implicit-copy", 400),
                ("ownership-transfer", 400),
            ],
        },
        // Borrow: 1,203 rules
        SeedConfig {
            database: Database::Borrow,
            categories: vec![
                ("mutable-borrow", 301),
                ("aliasing-borrow", 251),
                ("borrow-escapes", 201),
                ("reborrow", 225),
                ("borrow-scope", 225),
            ],
        },
        // Lifetime: 894 rules
        SeedConfig {
            database: Database::Lifetime,
            categories: vec![
                ("dangling-ref", 224),
                ("lifetime-mismatch", 223),
                ("self-referential", 149),
                ("elision-mismatch", 149),
                ("lifetime-bound", 149),
            ],
        },
        // Type Safety: 3,412 rules
        SeedConfig {
            database: Database::TypeSafety,
            categories: vec![
                ("type-conversion", 682),
                ("narrowing-cast", 512),
                ("unsound-transmute", 341),
                ("uninit-read", 512),
                ("type-mismatch", 512),
                ("integer-overflow", 341),
                ("enum-exhaustiveness", 512),
            ],
        },
        // Concurrency: 567 rules
        SeedConfig {
            database: Database::Concurrency,
            categories: vec![
                ("data-race", 142),
                ("deadlock-risk", 113),
                ("send-violation", 113),
                ("sync-violation", 99),
                ("atomic-ordering", 100),
            ],
        },
        // FFI: 234 rules
        SeedConfig {
            database: Database::Ffi,
            categories: vec![
                ("null-pointer-deref", 59),
                ("layout-mismatch", 58),
                ("missing-free", 39),
                ("abi-mismatch", 39),
                ("callback-safety", 39),
            ],
        },
    ]
}

fn seed_pattern(db: Database, category: &str, index: usize) -> Pattern {
    match (db, category) {
        (Database::Ownership, "use-after-move") => Pattern::UseAfterMove {
            var: VarPattern::AnyVar,
            move_site: SitePattern::AnySite,
            use_site: SitePattern::AnySite,
        },
        (Database::Ownership, "double-move") => Pattern::DoubleMove {
            var: VarPattern::AnyVar,
            sites: (SitePattern::AnySite, SitePattern::AnySite),
        },
        (Database::Ownership, "move-in-loop") => Pattern::MoveInLoop {
            var: VarPattern::AnyVar,
            loop_kind: match index % 3 {
                0 => LoopKind::For,
                1 => LoopKind::While,
                _ => LoopKind::Loop,
            },
        },
        (Database::Ownership, _) => Pattern::UseAfterMove {
            var: VarPattern::AnyVar,
            move_site: SitePattern::AnySite,
            use_site: SitePattern::AnySite,
        },

        (Database::Borrow, "mutable-borrow") => Pattern::MutableBorrow {
            target_type: TypePattern::AnyType,
            context: ContextSpec::default(),
        },
        (Database::Borrow, "aliasing-borrow") => Pattern::AliasingBorrow {
            refs: vec![
                RefPattern { mutable: false, target_type: TypePattern::AnyType },
                RefPattern { mutable: true, target_type: TypePattern::AnyType },
            ],
            overlap: OverlapKind::ReadWrite,
        },
        (Database::Borrow, "borrow-escapes") => Pattern::BorrowEscapes {
            ref_pattern: RefPattern { mutable: false, target_type: TypePattern::AnyType },
            escapes_to: ScopePattern::Any,
        },
        (Database::Borrow, _) => Pattern::MutableBorrow {
            target_type: TypePattern::AnyType,
            context: ContextSpec::default(),
        },

        (Database::Lifetime, "dangling-ref") => Pattern::DanglingRef {
            ref_pattern: RefPattern { mutable: false, target_type: TypePattern::AnyType },
            referent_scope: ScopePattern::Any,
        },
        (Database::Lifetime, "lifetime-mismatch") => Pattern::LifetimeMismatch {
            expected: RegionPattern::Any,
            actual: RegionPattern::Any,
        },
        (Database::Lifetime, "self-referential") => Pattern::SelfReferential {
            struct_type: TypePattern::AnyType,
        },
        (Database::Lifetime, _) => Pattern::DanglingRef {
            ref_pattern: RefPattern { mutable: false, target_type: TypePattern::AnyType },
            referent_scope: ScopePattern::Any,
        },

        (Database::TypeSafety, "type-conversion") => Pattern::TypeConversion {
            from: TypePattern::AnyType,
            to: TypePattern::AnyType,
            target_arch: ArchPattern::Any,
        },
        (Database::TypeSafety, "narrowing-cast") => Pattern::NarrowingCast {
            from: TypePattern::AnyType,
            to: TypePattern::AnyType,
        },
        (Database::TypeSafety, "unsound-transmute") => Pattern::UnsoundTransmute {
            from: TypePattern::AnyType,
            to: TypePattern::AnyType,
        },
        (Database::TypeSafety, "uninit-read") => Pattern::UninitRead {
            var: VarPattern::AnyVar,
            site: SitePattern::AnySite,
        },
        (Database::TypeSafety, _) => Pattern::TypeConversion {
            from: TypePattern::AnyType,
            to: TypePattern::AnyType,
            target_arch: ArchPattern::Any,
        },

        (Database::Concurrency, "data-race") => Pattern::DataRace {
            var: VarPattern::AnyVar,
            threads: vec![ThreadPattern { name: VarPattern::AnyVar }],
        },
        (Database::Concurrency, "deadlock-risk") => Pattern::DeadlockRisk {
            locks: vec![LockPattern {
                lock_type: TypePattern::AnyType,
                site: SitePattern::AnySite,
            }],
            order: OrderPattern::Reversed,
        },
        (Database::Concurrency, "send-violation") => Pattern::SendViolation {
            type_pattern: TypePattern::AnyType,
            context: ContextSpec::default(),
        },
        (Database::Concurrency, _) => Pattern::DataRace {
            var: VarPattern::AnyVar,
            threads: vec![ThreadPattern { name: VarPattern::AnyVar }],
        },

        (Database::Ffi, "null-pointer-deref") => Pattern::NullPointerDeref {
            source: FfiSource { language: "C".into(), function: "*".into() },
            site: SitePattern::AnySite,
        },
        (Database::Ffi, "layout-mismatch") => Pattern::LayoutMismatch {
            redox_type: TypePattern::AnyType,
            foreign_type: TypePattern::AnyType,
        },
        (Database::Ffi, "missing-free") => Pattern::MissingFree {
            alloc_site: SitePattern::AnySite,
            foreign_allocator: "malloc".into(),
        },
        (Database::Ffi, _) => Pattern::NullPointerDeref {
            source: FfiSource { language: "C".into(), function: "*".into() },
            site: SitePattern::AnySite,
        },
    }
}

fn seed_severity(index: usize) -> Severity {
    match index % 4 {
        0 => Severity::Error,
        1 => Severity::Warning,
        2 => Severity::Error,
        _ => Severity::Info,
    }
}

fn seed_scope(index: usize) -> Scope {
    match index % 4 {
        0 => Scope::Function,
        1 => Scope::Module,
        2 => Scope::Function,
        _ => Scope::Crate,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Core type tests -----------------------------------------------------

    #[test]
    fn rule_id_display() {
        let id = RuleId("OWN-0042".into());
        assert_eq!(id.to_string(), "OWN-0042");
    }

    #[test]
    fn semantic_version_display() {
        let v = SemanticVersion::new(1, 4, 2);
        assert_eq!(v.to_string(), "1.4.2");
    }

    #[test]
    fn database_prefixes() {
        assert_eq!(Database::Ownership.prefix(), "OWN");
        assert_eq!(Database::Borrow.prefix(), "BR");
        assert_eq!(Database::Lifetime.prefix(), "LT");
        assert_eq!(Database::TypeSafety.prefix(), "TS");
        assert_eq!(Database::Concurrency.prefix(), "CON");
        assert_eq!(Database::Ffi.prefix(), "FFI");
    }

    #[test]
    fn database_seed_counts() {
        assert_eq!(Database::Ownership.seed_count(), 2_847);
        assert_eq!(Database::Borrow.seed_count(), 1_203);
        assert_eq!(Database::Lifetime.seed_count(), 894);
        assert_eq!(Database::TypeSafety.seed_count(), 3_412);
        assert_eq!(Database::Concurrency.seed_count(), 567);
        assert_eq!(Database::Ffi.seed_count(), 234);
    }

    #[test]
    fn total_seed_count_is_9157() {
        let total: usize = Database::all().iter().map(|db| db.seed_count()).sum();
        assert_eq!(total, 9_157);
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Hint < Severity::Info);
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }

    #[test]
    fn lifecycle_display() {
        assert_eq!(Lifecycle::Active.to_string(), "active");
        assert_eq!(Lifecycle::Deprecated.to_string(), "deprecated");
    }

    // -- Pattern tests -------------------------------------------------------

    #[test]
    fn pattern_name_ownership() {
        let p = Pattern::UseAfterMove {
            var: VarPattern::AnyVar,
            move_site: SitePattern::AnySite,
            use_site: SitePattern::AnySite,
        };
        assert_eq!(p.name(), "UseAfterMove");
        assert_eq!(p.database(), Database::Ownership);
    }

    #[test]
    fn pattern_name_borrow() {
        let p = Pattern::MutableBorrow {
            target_type: TypePattern::Exact("Vec<u8>".into()),
            context: ContextSpec::default(),
        };
        assert_eq!(p.name(), "MutableBorrow");
        assert_eq!(p.database(), Database::Borrow);
    }

    #[test]
    fn pattern_name_concurrency() {
        let p = Pattern::DataRace {
            var: VarPattern::Exact("shared".into()),
            threads: vec![],
        };
        assert_eq!(p.name(), "DataRace");
        assert_eq!(p.database(), Database::Concurrency);
    }

    #[test]
    fn pattern_name_ffi() {
        let p = Pattern::NullPointerDeref {
            source: FfiSource { language: "C".into(), function: "malloc".into() },
            site: SitePattern::AnySite,
        };
        assert_eq!(p.name(), "NullPointerDeref");
        assert_eq!(p.database(), Database::Ffi);
    }

    // -- Rule tests ----------------------------------------------------------

    #[test]
    fn rule_is_active() {
        let rule = make_test_rule("OWN-0001", Database::Ownership, Lifecycle::Active);
        assert!(rule.is_active());
    }

    #[test]
    fn rule_not_active_when_deprecated() {
        let rule = make_test_rule("OWN-0001", Database::Ownership, Lifecycle::Deprecated);
        assert!(!rule.is_active());
    }

    #[test]
    fn rule_has_reliable_fix() {
        let mut rule = make_test_rule("OWN-0001", Database::Ownership, Lifecycle::Active);
        rule.fix_template = Some(FixTemplate {
            description: "Use clone()".into(),
            template: "x.clone()".into(),
        });
        rule.fix_confidence = 0.95;
        assert!(rule.has_reliable_fix(0.9));
        assert!(!rule.has_reliable_fix(0.99));
    }

    // -- SKB tests -----------------------------------------------------------

    #[test]
    fn empty_skb() {
        let skb = SafetyKnowledgeBase::new();
        assert_eq!(skb.total_rules(), 0);
    }

    #[test]
    fn add_and_get_rule() {
        let mut skb = SafetyKnowledgeBase::new();
        let rule = make_test_rule("OWN-0001", Database::Ownership, Lifecycle::Active);
        skb.add_rule(rule);
        assert_eq!(skb.total_rules(), 1);

        let found = skb.get_rule(&RuleId("OWN-0001".into()));
        assert!(found.is_some());
        assert_eq!(found.unwrap().id.0, "OWN-0001");
    }

    #[test]
    fn query_by_category() {
        let mut skb = SafetyKnowledgeBase::new();
        skb.add_rule(make_test_rule_cat("OWN-0001", Database::Ownership, "use-after-move"));
        skb.add_rule(make_test_rule_cat("OWN-0002", Database::Ownership, "double-move"));
        skb.add_rule(make_test_rule_cat("OWN-0003", Database::Ownership, "use-after-move"));

        let results = skb.query_by_category(Database::Ownership, "use-after-move");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_by_pattern_name() {
        let mut skb = SafetyKnowledgeBase::new();
        skb.add_rule(make_test_rule("OWN-0001", Database::Ownership, Lifecycle::Active));
        skb.add_rule(make_test_rule("BR-0001", Database::Borrow, Lifecycle::Active));

        let results = skb.query_by_pattern("UseAfterMove");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn query_by_severity() {
        let mut skb = SafetyKnowledgeBase::new();
        let mut r1 = make_test_rule("OWN-0001", Database::Ownership, Lifecycle::Active);
        r1.severity = Severity::Error;
        let mut r2 = make_test_rule("OWN-0002", Database::Ownership, Lifecycle::Active);
        r2.severity = Severity::Warning;
        skb.add_rule(r1);
        skb.add_rule(r2);

        assert_eq!(skb.query_by_severity(Severity::Error).len(), 1);
        assert_eq!(skb.query_by_min_severity(Severity::Warning).len(), 2);
    }

    #[test]
    fn query_fixable_rules() {
        let mut skb = SafetyKnowledgeBase::new();
        let mut r1 = make_test_rule("OWN-0001", Database::Ownership, Lifecycle::Active);
        r1.fix_template = Some(FixTemplate {
            description: "fix".into(),
            template: "x.clone()".into(),
        });
        r1.fix_confidence = 0.95;
        skb.add_rule(r1);
        skb.add_rule(make_test_rule("OWN-0002", Database::Ownership, Lifecycle::Active));

        assert_eq!(skb.query_fixable(0.9).len(), 1);
        assert_eq!(skb.query_fixable(0.99).len(), 0);
    }

    #[test]
    fn query_active_in_database() {
        let mut skb = SafetyKnowledgeBase::new();
        skb.add_rule(make_test_rule("OWN-0001", Database::Ownership, Lifecycle::Active));
        skb.add_rule(make_test_rule("OWN-0002", Database::Ownership, Lifecycle::Deprecated));
        skb.add_rule(make_test_rule("BR-0001", Database::Borrow, Lifecycle::Active));

        let results = skb.query_active_in_database(Database::Ownership);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn statistics_empty() {
        let skb = SafetyKnowledgeBase::new();
        let stats = skb.statistics();
        assert_eq!(stats.len(), 6);
        for s in &stats {
            assert_eq!(s.total, 0);
        }
    }

    // -- Seed corpus tests ---------------------------------------------------

    #[test]
    fn seed_corpus_total_count() {
        let skb = seed_corpus();
        assert_eq!(skb.total_rules(), 9_157);
    }

    #[test]
    fn seed_corpus_ownership_count() {
        let skb = seed_corpus();
        assert_eq!(skb.rules_in_database(Database::Ownership), 2_847);
    }

    #[test]
    fn seed_corpus_borrow_count() {
        let skb = seed_corpus();
        assert_eq!(skb.rules_in_database(Database::Borrow), 1_203);
    }

    #[test]
    fn seed_corpus_lifetime_count() {
        let skb = seed_corpus();
        assert_eq!(skb.rules_in_database(Database::Lifetime), 894);
    }

    #[test]
    fn seed_corpus_type_safety_count() {
        let skb = seed_corpus();
        assert_eq!(skb.rules_in_database(Database::TypeSafety), 3_412);
    }

    #[test]
    fn seed_corpus_concurrency_count() {
        let skb = seed_corpus();
        assert_eq!(skb.rules_in_database(Database::Concurrency), 567);
    }

    #[test]
    fn seed_corpus_ffi_count() {
        let skb = seed_corpus();
        assert_eq!(skb.rules_in_database(Database::Ffi), 234);
    }

    #[test]
    fn seed_corpus_all_active() {
        let skb = seed_corpus();
        let active: usize = Database::all()
            .iter()
            .map(|&db| skb.query_active_in_database(db).len())
            .sum();
        assert_eq!(active, 9_157);
    }

    #[test]
    fn seed_corpus_has_indices() {
        let skb = seed_corpus();
        // Category index should have entries
        assert!(!skb.category_index.is_empty());
        // Pattern index should have entries
        assert!(!skb.pattern_index.is_empty());
        // Severity index should have entries
        assert!(!skb.severity_index.is_empty());
    }

    #[test]
    fn seed_corpus_rule_ids_unique() {
        let skb = seed_corpus();
        let mut ids: Vec<&str> = skb.rules.iter().map(|r| r.id.0.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 9_157, "all rule IDs must be unique");
    }

    #[test]
    fn seed_corpus_query_use_after_move() {
        let skb = seed_corpus();
        let results = skb.query_by_category(Database::Ownership, "use-after-move");
        assert_eq!(results.len(), 712);
    }

    #[test]
    fn seed_corpus_query_by_pattern() {
        let skb = seed_corpus();
        let results = skb.query_by_pattern("UseAfterMove");
        // UseAfterMove used for: use-after-move(712) + partial-move(445) + implicit-copy(400) + ownership-transfer(400) = 1957
        assert!(!results.is_empty());
    }

    #[test]
    fn seed_corpus_statistics() {
        let skb = seed_corpus();
        let stats = skb.statistics();
        let ownership = stats.iter().find(|s| s.database == Database::Ownership).unwrap();
        assert_eq!(ownership.total, 2_847);
        assert_eq!(ownership.target, 2_847);
        assert_eq!(ownership.active, 2_847);
    }

    // -- Helpers -------------------------------------------------------------

    fn make_test_rule(id: &str, db: Database, lifecycle: Lifecycle) -> Rule {
        let pattern = match db {
            Database::Ownership => Pattern::UseAfterMove {
                var: VarPattern::AnyVar,
                move_site: SitePattern::AnySite,
                use_site: SitePattern::AnySite,
            },
            Database::Borrow => Pattern::MutableBorrow {
                target_type: TypePattern::AnyType,
                context: ContextSpec::default(),
            },
            _ => Pattern::UseAfterMove {
                var: VarPattern::AnyVar,
                move_site: SitePattern::AnySite,
                use_site: SitePattern::AnySite,
            },
        };

        Rule {
            id: RuleId(id.into()),
            database: db,
            version: SemanticVersion::new(1, 0, 0),
            pattern,
            context: ContextSpec::default(),
            scope: Scope::Function,
            severity: Severity::Error,
            category: "test".into(),
            description: "Test rule".into(),
            rationale: "Testing".into(),
            fix_template: None,
            fix_confidence: 0.0,
            alternatives: Vec::new(),
            source: RuleSource::BuiltIn,
            lifecycle,
            frequency: 100,
            false_positive_rate: 0.02,
            tags: vec!["test".into()],
        }
    }

    fn make_test_rule_cat(id: &str, db: Database, category: &str) -> Rule {
        let mut rule = make_test_rule(id, db, Lifecycle::Active);
        rule.category = category.into();
        rule
    }
}

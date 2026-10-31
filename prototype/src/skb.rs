/// SKB — Safety Knowledge Base (proposal §15).
///
/// Eight rule databases:
///   1. Ownership Rules  (OWN-*)
///   2. Borrow Patterns  (BOR-*)
///   3. Lifetime Rules   (LIF-*)
///   4. Type Safety      (TYP-*)
///   5. Concurrency      (CON-*)
///   6. FFI Safety       (FFI-*)
///   7. Agent Elision    (AEL-*) — rules for safety constructs the compiler
///      handles automatically in agent mode so they can be elided from syntax.
///   8. Swarm Safety     (SWM-*) — rules for multi-agent swarm coordination,
///      consensus, topology, fault tolerance, and deadlock prevention.
///
/// Plus the original symbol metadata entries used by the query API.
///
/// SKB-QL syntax (simplified):
///   SELECT effects FROM std.io.read_file
///   SELECT cost WHERE construct = "Vec::push" AND target = "x86_64"
///   MATCH UseAfterMove IN function("process_data")
use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════════════
//  Rule Data Model (proposal §15.1)
// ══════════════════════════════════════════════════════════════════════

/// Which of the eight rule databases a rule belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleDatabase {
    Ownership,
    Borrow,
    Lifetime,
    TypeSafety,
    Concurrency,
    FFI,
    /// Agent elision — rules the compiler enforces so agent mode doesn't
    /// need explicit safety syntax.
    AgentElision,
    /// Swarm safety — rules for multi-agent coordination, consensus,
    /// topology, fault tolerance, and deadlock prevention.
    SwarmSafety,
}

impl RuleDatabase {
    fn named(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|d| format!("{d:?}") == name)
            .unwrap_or_else(|| panic!("no rule database {name:?}"))
    }

    /// Every database, in the order `--emit-skb` writes them.
    ///
    /// **One array, because there were two.** `rule_counts_by_db` held this
    /// list and `rmi_ontology_adapter::unified_search` held a second copy
    /// under a comment promising it scans "all rules across the eight
    /// databases". Both were right; nothing held them that way. An array
    /// literal is not exhaustive to the compiler the way a `match` on the
    /// enum is, so a ninth variant would have been added to neither and
    /// silently dropped from both the emitted `skb/` tree and the unified
    /// search.
    ///
    /// `per_database_counts_sum_to_the_total` is the guard, and it only
    /// guards what derives from here — which is now both callers. It catches
    /// a variant that exists and is not listed here; the compiler catches
    /// both the other directions (this array's fixed size, and the two
    /// exhaustive matches on the enum).
    pub const ALL: [RuleDatabase; 8] = [
        RuleDatabase::Ownership,
        RuleDatabase::Borrow,
        RuleDatabase::Lifetime,
        RuleDatabase::TypeSafety,
        RuleDatabase::Concurrency,
        RuleDatabase::FFI,
        RuleDatabase::AgentElision,
        RuleDatabase::SwarmSafety,
    ];
}

/// Severity of a rule violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A single safety rule in the SKB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Unique identifier, e.g. "OWN-0001".
    pub id: String,
    /// Which database this rule belongs to.
    pub database: RuleDatabase,
    /// Short category label, e.g. "use-after-move".
    pub category: String,
    /// How severe a violation is.
    pub severity: RuleSeverity,
    /// Human/agent-readable description.
    pub description: String,
    /// Why this rule exists.
    pub rationale: String,
    /// Auto-fix template (if applicable).
    pub fix_template: Option<String>,
    /// Confidence in the auto-fix (0.0–1.0).
    pub fix_confidence: f64,
    /// Searchable tags.
    pub tags: Vec<String>,
}

// ══════════════════════════════════════════════════════════════════════
//  Built-in Rule Databases — 255 rules across 8 databases, in kb/rules/safety.mg
// ══════════════════════════════════════════════════════════════════════

/// The compiled-in rules. `kb::tests` holds the file to the schema, so this
/// reads it alone: a compile that consults one rule should not pay to load the
/// whole knowledge base.
fn builtin_rules() -> Vec<Rule> {
    static RULES: std::sync::OnceLock<Vec<Rule>> = std::sync::OnceLock::new();
    RULES.get_or_init(|| read_rules(crate::kb::SAFETY)).clone()
}

fn read_rules(src: &str) -> Vec<Rule> {
    let module = crate::parser::parse(&crate::lexer::lex(src))
        .unwrap_or_else(|e| panic!("kb/rules/safety.mg does not parse: {e:?}"));
    let mut rules = Vec::new();
    let mut fixes = std::collections::HashMap::new();
    let mut tags: std::collections::HashMap<String, Vec<(usize, String)>> = Default::default();
    for item in &module.items {
        let crate::ast::ItemKind::Kb(def) = &item.kind else { continue };
        for fact in &def.facts {
            let args = crate::logic::read_fact(fact)
                .unwrap_or_else(|n| panic!("{}: argument {n} is not a constant", fact.name));
            match (fact.name.as_str(), args.as_slice()) {
                ("safety_rule", [id, db, category, severity, description, rationale, confidence]) => {
                    rules.push(Rule {
                        id: id.clone(),
                        database: RuleDatabase::named(db),
                        category: category.clone(),
                        severity: RuleSeverity::named(severity),
                        description: description.clone(),
                        rationale: rationale.clone(),
                        fix_template: None,
                        fix_confidence: confidence
                            .parse()
                            .unwrap_or_else(|_| panic!("{id}: confidence {confidence:?}")),
                        tags: Vec::new(),
                    })
                }
                ("rule_fix", [id, fix]) => {
                    fixes.insert(id.clone(), fix.clone());
                }
                ("rule_tag", [id, position, tag]) => {
                    let position = position
                        .parse()
                        .unwrap_or_else(|_| panic!("{id}: tag position {position:?}"));
                    tags.entry(id.clone()).or_default().push((position, tag.clone()));
                }
                _ => {}
            }
        }
    }
    for r in &mut rules {
        r.fix_template = fixes.remove(&r.id);
        let mut held = tags.remove(&r.id).unwrap_or_default();
        held.sort();
        r.tags = held.into_iter().map(|(_, t)| t).collect();
    }
    assert!(fixes.is_empty() && tags.is_empty(), "a rule_fix or rule_tag names no safety_rule");
    // The order is part of the policy digest and of `skb/`.
    rules.sort_by_key(|r| {
        (RuleDatabase::ALL.iter().position(|d| *d == r.database), r.id.clone())
    });
    rules
}

// ══════════════════════════════════════════════════════════════════════
//  Rule Query API
// ══════════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════════
//  The SKB as loaded policy (ARCHITECTURE.md, "The improvable surface")
// ══════════════════════════════════════════════════════════════════════

/// Where the active rule set came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicySource {
    /// `builtin_rules()` alone — no overlay directory exists.
    Builtin,
    /// `builtin_rules()` plus an overlay, which may only have made it stricter.
    Overlay {
        dir: std::path::PathBuf,
        /// Rules the overlay introduced that no builtin id covers.
        added: usize,
        /// Builtin rules the overlay raised the severity of.
        strengthened: usize,
    },
}

/// Why an overlay was refused.
///
/// Scope, so the refusals below are not read as more than they are: **no SKB
/// rule is executed.** These 255 rules are knowledge — `codegen_bridge` keeps
/// their descriptions as strings and `rmi_ontology_adapter` substring-matches
/// them for agent search. Nothing matches a rule id against an AST. An overlay
/// that lowered a severity would change the advice a synthesising agent
/// receives, with no diagnostic anywhere; it would not turn off a check,
/// because there is no check to turn off.
///
/// Every variant halts. None of them falls back to the builtin set, and the
/// distinction from an *absent* overlay is the whole point: absence means
/// nobody has written a policy, while any of these means someone did and it
/// cannot be honoured. Treating the second as the first would let deleting or
/// corrupting a policy file silently change which rules are enforced, which is
/// the same failure as an unchecked gene reading as a pure one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    /// The overlay directory or a file in it could not be read.
    Unreadable { path: String, why: String },
    /// A file was read and is not a rule array.
    Malformed { path: String, why: String },
    /// Two overlay rules claim the same id, so neither can be said to apply.
    DuplicateId { id: String, path: String },
    /// An overlay rule would make a builtin rule *less* severe.
    Weakened { id: String, from: RuleSeverity, to: RuleSeverity },
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyError::Unreadable { path, why } => {
                write!(f, "SKB overlay {path} could not be read: {why}")
            }
            PolicyError::Malformed { path, why } => {
                write!(f, "SKB overlay {path} is not a rule array: {why}")
            }
            PolicyError::DuplicateId { id, path } => {
                write!(f, "SKB overlay {path} declares rule {id} twice")
            }
            PolicyError::Weakened { id, from, to } => write!(
                f,
                "SKB overlay would lower {id} from {from:?} to {to:?}; \
                 the builtin rules are a floor, not a default"
            ),
        }
    }
}

impl RuleSeverity {
    fn named(name: &str) -> Self {
        [RuleSeverity::Error, RuleSeverity::Warning, RuleSeverity::Info, RuleSeverity::Hint]
            .into_iter()
            .find(|s| format!("{s:?}") == name)
            .unwrap_or_else(|| panic!("no rule severity {name:?}"))
    }

    /// Lower is stricter. `Error` < `Warning` < `Info` < `Hint`.
    fn strictness(self) -> u8 {
        match self {
            RuleSeverity::Error => 0,
            RuleSeverity::Warning => 1,
            RuleSeverity::Info => 2,
            RuleSeverity::Hint => 3,
        }
    }
}

/// The rules in force, where they came from, and which policy they are.
#[derive(Debug, Clone)]
pub struct RuleSet {
    rules: Vec<Rule>,
    source: PolicySource,
    digest: String,
}

/// SHA-256 over the rules in force, lowercase hex.
///
/// **Over the merged set, not the overlay files.** What a run needs to be able
/// to state afterwards is which policy was enforced, and that is the rules —
/// not the spelling of the directory that produced them. Two overlays that
/// differ in whitespace, filenames or how the rules are split across files are
/// the same policy and hash the same; one that moves a single severity does
/// not. The builtins have a digest too, so the question "which rules ran" has
/// an answer on every run rather than only on configured ones.
fn digest_of(rules: &[Rule]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    // Field-by-field rather than via serde, so a derive attribute changing the
    // JSON spelling cannot silently renumber every policy in the journal.
    for r in rules {
        h.update(r.id.as_bytes());
        h.update([0]);
        h.update(format!("{:?}", r.database).as_bytes());
        h.update([0]);
        h.update(format!("{:?}", r.severity).as_bytes());
        h.update([0]);
        h.update(r.category.as_bytes());
        h.update([0]);
        h.update(r.description.as_bytes());
        h.update([0]);
        h.update(r.rationale.as_bytes());
        h.update([0]);
        h.update(r.fix_template.as_deref().unwrap_or("").as_bytes());
        h.update([0]);
        h.update(r.fix_confidence.to_bits().to_le_bytes());
        for t in &r.tags {
            h.update(t.as_bytes());
            h.update([0]);
        }
        h.update([1]);
    }
    format!("{:x}", h.finalize())
}

impl RuleSet {
    /// The compiled-in rules.
    pub fn builtin() -> Self {
        let rules = builtin_rules();
        let digest = digest_of(&rules);
        RuleSet { rules, source: PolicySource::Builtin, digest }
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn source(&self) -> &PolicySource {
        &self.source
    }

    /// SHA-256 of the rules in force — the name of this policy.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Load the builtin rules with an overlay applied from `dir`.
    ///
    /// **The builtin rules are a floor, not a default.** The merge starts from
    /// them and an overlay may only add an id or raise a severity, so removing
    /// a rule is not something the format can express — the guarantee is
    /// structural rather than checked, which is the difference between a
    /// property and a test of one. That is what makes an absent overlay safe
    /// to accept: falling back to the builtins cannot be a downgrade, because
    /// no reachable policy is weaker than they are.
    ///
    /// A missing directory is therefore `Ok` and yields [`PolicySource::Builtin`].
    /// A directory that exists and cannot be honoured is an error, and the
    /// caller must halt rather than proceeding on the floor: something was
    /// written, and running as though it had not been is how a policy change
    /// disappears silently.
    pub fn load(dir: &std::path::Path) -> Result<Self, PolicyError> {
        if !dir.exists() {
            return Ok(RuleSet::builtin());
        }
        let entries = std::fs::read_dir(dir).map_err(|e| PolicyError::Unreadable {
            path: dir.display().to_string(),
            why: e.to_string(),
        })?;
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        for e in entries {
            let e = e.map_err(|e| PolicyError::Unreadable {
                path: dir.display().to_string(),
                why: e.to_string(),
            })?;
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("json") {
                files.push(p);
            }
        }
        // Read in a fixed order, so the same directory always produces the same
        // rule set and any error it reports is the same error.
        files.sort();

        let mut rules = builtin_rules();
        let mut index: std::collections::HashMap<String, usize> =
            rules.iter().enumerate().map(|(i, r)| (r.id.clone(), i)).collect();
        let mut seen_in_overlay: std::collections::HashSet<String> = Default::default();
        let mut added = 0usize;
        let mut strengthened = 0usize;

        for path in &files {
            let text = std::fs::read_to_string(path).map_err(|e| PolicyError::Unreadable {
                path: path.display().to_string(),
                why: e.to_string(),
            })?;
            let parsed: Vec<Rule> =
                serde_json::from_str(&text).map_err(|e| PolicyError::Malformed {
                    path: path.display().to_string(),
                    why: e.to_string(),
                })?;
            for r in parsed {
                if !seen_in_overlay.insert(r.id.clone()) {
                    return Err(PolicyError::DuplicateId {
                        id: r.id,
                        path: dir.display().to_string(),
                    });
                }
                match index.get(&r.id) {
                    Some(&i) => {
                        let from = rules[i].severity;
                        if r.severity.strictness() > from.strictness() {
                            return Err(PolicyError::Weakened {
                                id: r.id,
                                from,
                                to: r.severity,
                            });
                        }
                        if r.severity.strictness() < from.strictness() {
                            strengthened += 1;
                        }
                        rules[i] = r;
                    }
                    None => {
                        index.insert(r.id.clone(), rules.len());
                        rules.push(r);
                        added += 1;
                    }
                }
            }
        }

        let digest = digest_of(&rules);
        Ok(RuleSet {
            rules,
            source: PolicySource::Overlay { dir: dir.to_path_buf(), added, strengthened },
            digest,
        })
    }

    /// Load from `$MAGE_SKB_OVERLAY`, or the builtins if it is unset.
    pub fn from_env() -> Result<Self, PolicyError> {
        match std::env::var_os("MAGE_SKB_OVERLAY") {
            Some(dir) => RuleSet::load(std::path::Path::new(&dir)),
            None => Ok(RuleSet::builtin()),
        }
    }
}

static ACTIVE: std::sync::OnceLock<Vec<Rule>> = std::sync::OnceLock::new();

/// Install `set` as the rules every query in this module answers from.
///
/// Returns `false` if a set was already installed, in which case nothing
/// changes — the policy in force must not shift underneath a run that has
/// already consulted it.
pub fn install(set: RuleSet) -> bool {
    ACTIVE.set(set.rules).is_ok()
}

/// The rules in force: whatever was installed, else the compiled-in floor.
///
/// Nothing installed means nothing tried to, which is the state of every test
/// and every embedding that does not opt in. It is not the state of a failed
/// load, because [`RuleSet::load`] returns an error there and the caller halts.
fn active_rules() -> Vec<Rule> {
    match ACTIVE.get() {
        Some(rules) => rules.clone(),
        None => builtin_rules(),
    }
}

/// Result of a rule query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleQueryResult {
    pub matches: Vec<Rule>,
    pub query_text: String,
}

/// Query rules by database.
pub fn query_rules_by_db(db: RuleDatabase) -> RuleQueryResult {
    let matches: Vec<_> = active_rules().into_iter().filter(|r| r.database == db).collect();
    RuleQueryResult { query_text: format!("database = {:?}", db), matches }
}

/// Query rules by category (exact match).
pub fn query_rules_by_category(category: &str) -> RuleQueryResult {
    let matches: Vec<_> = active_rules().into_iter().filter(|r| r.category == category).collect();
    RuleQueryResult { query_text: format!("category = {category}"), matches }
}

/// Query rules by tag.
pub fn query_rules_by_tag(tag: &str) -> RuleQueryResult {
    let matches: Vec<_> =
        active_rules().into_iter().filter(|r| r.tags.iter().any(|t| t == tag)).collect();
    RuleQueryResult { query_text: format!("rule tag = {tag}"), matches }
}

/// Query a rule by its ID (e.g. "OWN-0001").
pub fn query_rule_by_id(id: &str) -> Option<Rule> {
    active_rules().into_iter().find(|r| r.id == id)
}

/// Query rules by severity.
pub fn query_rules_by_severity(sev: RuleSeverity) -> RuleQueryResult {
    let matches: Vec<_> = active_rules().into_iter().filter(|r| r.severity == sev).collect();
    RuleQueryResult { query_text: format!("severity = {:?}", sev), matches }
}

/// Count total rules in the SKB.
pub fn rule_count() -> usize {
    active_rules().len()
}

/// Count rules per database.
/// The on-disk file stem for a database, matching the names in `skb/rules/`.
pub fn database_file_stem(db: RuleDatabase) -> &'static str {
    match db {
        RuleDatabase::Ownership => "ownership",
        RuleDatabase::Borrow => "borrow",
        RuleDatabase::Lifetime => "lifetime",
        RuleDatabase::TypeSafety => "type_safety",
        RuleDatabase::Concurrency => "concurrency",
        RuleDatabase::FFI => "ffi",
        RuleDatabase::AgentElision => "agent_elision",
        RuleDatabase::SwarmSafety => "swarm_safety",
    }
}

/// Write `skb/` as a generated export of what the compiler actually serves.
///
/// **Open item 23 was that `skb/` held 56 rules in JSON that nothing read**,
/// while `builtin_rules()` served 255 from the binary — and the two were not
/// even the same corpus: the tree used `BR-`/`LT-`/`TS-`/`CC-` identifiers
/// against the binary's `BOR-`/`LIF-`/`TYP-`/`CON-`, plus two stray `MEM-` and
/// `TC-` rules, and two whole databases (`AgentElision`, `SwarmSafety`, 45
/// rules) had no file at all. So it was neither an input nor an export: a
/// parallel corpus, free to drift, and drifted.
///
/// The decision taken was to **generate** it rather than load it or delete it.
/// Loading would have made 56 rules authoritative and discarded 199 that run;
/// deleting would lose a machine-readable corpus an agent can read without
/// running the binary. Generating makes the tree true by construction, and a
/// CI check regenerates and diffs it — the same arrangement as
/// `MAGE_ONTOLOGY.json`.
///
/// The emitted rules carry the nine fields a `Rule` actually has. The old files
/// carried seventeen — `pattern`, `context`, `alternatives`,
/// `false_positive_rate`, `frequency`, `scope`, `source`, `version` — for a
/// corpus no code populated or consumed. Eight fields of metadata about rules
/// that were not the rules being enforced.
pub fn emit_tree(dir: &std::path::Path) -> std::io::Result<usize> {
    let rules_dir = dir.join("rules");
    std::fs::create_dir_all(&rules_dir)?;

    let mut total = 0usize;
    let mut databases = serde_json::Map::new();
    for (db, _) in rule_counts_by_db() {
        let stem = database_file_stem(db);
        // `builtin_rules()`, not the installed set. This tree is a projection
        // of the compiled-in floor — its manifest says so — and
        // `check-skb-tree.sh` regenerates it and fails on any difference. If
        // it emitted the active rules, the committed tree would depend on
        // whether \ happened to be set in the shell that ran
        // CI, and the checker would be testing the environment.
        let matches: Vec<Rule> =
            builtin_rules().into_iter().filter(|r| r.database == db).collect();
        total += matches.len();
        databases.insert(
            stem.to_string(),
            serde_json::json!({ "rules": matches.len(), "id_prefix": matches.first().map(|r| r.id.split('-').next().unwrap_or("").to_string()) }),
        );
        let json = serde_json::to_string_pretty(&matches)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(rules_dir.join(format!("{stem}.json")), json + "
")?;
    }

    let manifest = serde_json::json!({
        "generated_by": "mage-parse --emit-skb",
        "generated_from": "kb/rules/safety.mg",
        "schema_version": 2,
        "total_rules": total,
        "databases": databases,
        "note": "Generated. Edits here are overwritten; change kb/rules/safety.mg instead.                  CI regenerates this tree and fails if it differs (item 23).",
    });
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::write(dir.join("manifest.json"), json + "
")?;

    // The schema is emitted too, or it drifts from the thing it describes.
    //
    // It previously required `version` and `scope` and described seventeen
    // fields, against rules that now carry nine — so it validated nothing that
    // was actually there. A hand-written schema beside a generated corpus is
    // the same parallel-copy problem one level down, so it is generated from
    // the same source.
    let schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MAGE SKB rule",
        "description": "Generated by `mage-parse --emit-skb`; mirrors `Rule` in prototype/src/skb.rs.",
        "type": "array",
        "items": {
            "type": "object",
            "additionalProperties": false,
            "required": [
                "id", "database", "category", "severity",
                "description", "rationale", "fix_confidence", "tags"
            ],
            "properties": {
                "id": { "type": "string", "description": "e.g. OWN-0001" },
                "database": { "type": "string", "enum": [
                    "Ownership", "Borrow", "Lifetime", "TypeSafety",
                    "Concurrency", "FFI", "AgentElision", "SwarmSafety"
                ]},
                "category": { "type": "string" },
                "severity": { "type": "string" },
                "description": { "type": "string" },
                "rationale": { "type": "string" },
                "fix_template": { "type": ["string", "null"] },
                "fix_confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                "tags": { "type": "array", "items": { "type": "string" } }
            }
        }
    });
    let json = serde_json::to_string_pretty(&schema)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::write(dir.join("rule-schema.json"), json + "
")?;

    Ok(total)
}

pub fn rule_counts_by_db() -> Vec<(RuleDatabase, usize)> {
    RuleDatabase::ALL
        .into_iter()
        .map(|db| {
            let count = builtin_rules().iter().filter(|r| r.database == db).count();
            (db, count)
        })
        .collect()
}

// ══════════════════════════════════════════════════════════════════════
//  Symbol Metadata (original SKB entries for the query API)
// ══════════════════════════════════════════════════════════════════════

/// A single SKB entry for a symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkbEntry {
    /// Fully qualified name: e.g. "std.io.read_file".
    pub fqn: String,
    /// Kind of symbol.
    pub kind: SymbolKind,
    /// Effects declared by this symbol.
    pub effects: Vec<String>,
    /// Required capabilities to invoke.
    pub capabilities: Vec<String>,
    /// Spec block (pre/post conditions), if any.
    pub spec: Option<SpecBlock>,
    /// Whether the symbol is deprecated.
    pub deprecated: Option<String>,
    /// Aliases from the Rust ecosystem.
    pub rust_aliases: Vec<String>,
    /// Tags for semantic search.
    pub tags: Vec<String>,
}

/// Spec block attached to a function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecBlock {
    pub requires: Vec<String>,
    pub ensures: Vec<String>,
}

/// Symbol kind in the knowledge base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
    Constant,
    TypeAlias,
}

// ── Built-in SKB ─────────────────────────────────────────────────────

fn builtin_skb() -> Vec<SkbEntry> {
    vec![
        // I/O
        entry(
            "std.io.read_file",
            SymbolKind::Function,
            &["io", "fs"],
            &["fs.read"],
            Some(spec(&["path.exists()"], &["ret.is_ok() => ret.unwrap().len() >= 0"])),
            &["fs::read_to_string"],
            &["io", "file", "read"],
        ),
        entry(
            "std.io.write_file",
            SymbolKind::Function,
            &["io", "fs"],
            &["fs.write"],
            Some(spec(&["path.parent().exists()"], &["path.exists()"])),
            &["fs::write"],
            &["io", "file", "write"],
        ),
        entry(
            "std.io.stdin",
            SymbolKind::Function,
            &["io"],
            &[],
            None,
            &["io::stdin"],
            &["io", "input"],
        ),
        entry(
            "std.io.stdout",
            SymbolKind::Function,
            &["io"],
            &[],
            None,
            &["io::stdout"],
            &["io", "output"],
        ),
        // Net
        entry(
            "std.net.TcpStream",
            SymbolKind::Struct,
            &["io", "net"],
            &["network"],
            None,
            &["net::TcpStream"],
            &["network", "tcp", "stream"],
        ),
        entry(
            "std.net.listen",
            SymbolKind::Function,
            &["io", "net"],
            &["network"],
            Some(spec(&["port > 0", "port < 65536"], &["ret.is_ok() => listener.is_bound()"])),
            &["TcpListener::bind"],
            &["network", "tcp", "listen", "server"],
        ),
        // Collections
        entry(
            "std.collections.Vec",
            SymbolKind::Struct,
            &[],
            &[],
            None,
            &["Vec"],
            &["collection", "array", "vector"],
        ),
        entry(
            "std.collections.HashMap",
            SymbolKind::Struct,
            &[],
            &[],
            None,
            &["HashMap"],
            &["collection", "map", "hash"],
        ),
        // Agent primitives
        entry(
            "std.agent.Agent",
            SymbolKind::Trait,
            &[],
            &[],
            None,
            &[],
            &["agent", "trait", "handle"],
        ),
        entry(
            "std.agent.Swarm",
            SymbolKind::Struct,
            &["concurrency"],
            &[],
            None,
            &[],
            &["agent", "swarm", "multi-agent"],
        ),
        entry(
            "std.agent.Bus",
            SymbolKind::Struct,
            &["concurrency"],
            &[],
            None,
            &[],
            &["agent", "bus", "pubsub", "messaging"],
        ),
        entry(
            "std.agent.Memory",
            SymbolKind::Struct,
            &["io"],
            &[],
            None,
            &[],
            &["agent", "memory", "persist", "state"],
        ),
        entry(
            "std.agent.Lease",
            SymbolKind::Struct,
            &[],
            &[],
            None,
            &[],
            &["agent", "capability", "lease", "rbac"],
        ),
        // Sync
        entry(
            "std.sync.Mutex",
            SymbolKind::Struct,
            &["concurrency"],
            &[],
            None,
            &["Mutex"],
            &["sync", "mutex", "lock"],
        ),
        entry(
            "std.sync.RwLock",
            SymbolKind::Struct,
            &["concurrency"],
            &[],
            None,
            &["RwLock"],
            &["sync", "rwlock", "read-write"],
        ),
        entry(
            "std.sync.Channel",
            SymbolKind::Struct,
            &["concurrency"],
            &[],
            None,
            &["mpsc::channel"],
            &["sync", "channel", "mpsc"],
        ),
    ]
}

fn entry(
    fqn: &str,
    kind: SymbolKind,
    effects: &[&str],
    caps: &[&str],
    spec: Option<SpecBlock>,
    aliases: &[&str],
    tags: &[&str],
) -> SkbEntry {
    SkbEntry {
        fqn: fqn.into(),
        kind,
        effects: effects.iter().map(|s| s.to_string()).collect(),
        capabilities: caps.iter().map(|s| s.to_string()).collect(),
        spec,
        deprecated: None,
        rust_aliases: aliases.iter().map(|s| s.to_string()).collect(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
    }
}

fn spec(requires: &[&str], ensures: &[&str]) -> SpecBlock {
    SpecBlock {
        requires: requires.iter().map(|s| s.to_string()).collect(),
        ensures: ensures.iter().map(|s| s.to_string()).collect(),
    }
}

// ── Query Interface ──────────────────────────────────────────────────

/// Result of an SKB query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub matches: Vec<SkbEntry>,
    pub query_text: String,
}

/// Query the SKB by fully qualified name (exact or prefix match).
pub fn query_by_fqn(fqn: &str) -> QueryResult {
    let matches: Vec<_> = builtin_skb()
        .into_iter()
        .filter(|e| e.fqn == fqn || e.fqn.starts_with(&format!("{fqn}.")))
        .collect();
    QueryResult { matches, query_text: format!("fqn = {fqn}") }
}

/// Query the SKB by effect name: find all symbols that declare a given effect.
pub fn query_by_effect(effect: &str) -> QueryResult {
    let matches: Vec<_> =
        builtin_skb().into_iter().filter(|e| e.effects.iter().any(|eff| eff == effect)).collect();
    QueryResult { matches, query_text: format!("effect = {effect}") }
}

/// Query by required capability: find symbols that require a given capability.
pub fn query_by_capability(cap: &str) -> QueryResult {
    let matches: Vec<_> =
        builtin_skb().into_iter().filter(|e| e.capabilities.iter().any(|c| c == cap)).collect();
    QueryResult { matches, query_text: format!("capability = {cap}") }
}

/// Query by tag (semantic search).
pub fn query_by_tag(tag: &str) -> QueryResult {
    let matches: Vec<_> =
        builtin_skb().into_iter().filter(|e| e.tags.iter().any(|t| t == tag)).collect();
    QueryResult { matches, query_text: format!("tag = {tag}") }
}

/// Query by Rust alias: find the MAGE equivalent of a Rust symbol.
pub fn query_by_rust_alias(alias: &str) -> QueryResult {
    let matches: Vec<_> = builtin_skb()
        .into_iter()
        .filter(|e| e.rust_aliases.iter().any(|a| a.contains(alias)))
        .collect();
    QueryResult { matches, query_text: format!("rust_alias contains {alias}") }
}

/// Lookup the spec block for a symbol.
pub fn query_spec(fqn: &str) -> Option<SpecBlock> {
    builtin_skb().into_iter().find(|e| e.fqn == fqn).and_then(|e| e.spec)
}

/// List all symbols in a module (prefix match).
pub fn query_module(module_prefix: &str) -> QueryResult {
    let matches: Vec<_> =
        builtin_skb().into_iter().filter(|e| e.fqn.starts_with(module_prefix)).collect();
    QueryResult { matches, query_text: format!("module = {module_prefix}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_exact_fqn() {
        let result = query_by_fqn("std.io.read_file");
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].fqn, "std.io.read_file");
    }

    #[test]
    fn query_module_prefix() {
        let result = query_module("std.io");
        assert!(result.matches.len() >= 2);
    }

    #[test]
    fn query_io_effect() {
        let result = query_by_effect("io");
        assert!(result.matches.len() >= 3);
    }

    #[test]
    fn query_network_capability() {
        let result = query_by_capability("network");
        assert!(!result.matches.is_empty());
    }

    #[test]
    fn query_rust_alias() {
        let result = query_by_rust_alias("HashMap");
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].fqn, "std.collections.HashMap");
    }

    #[test]
    fn query_spec_read_file() {
        let spec = query_spec("std.io.read_file").unwrap();
        assert!(!spec.requires.is_empty());
        assert!(!spec.ensures.is_empty());
    }

    #[test]
    fn query_agent_primitives() {
        let result = query_by_tag("agent");
        assert!(result.matches.len() >= 4);
    }

    #[test]
    fn query_nonexistent() {
        let result = query_by_fqn("nonexistent.module");
        assert!(result.matches.is_empty());
    }

    // ── Rule database tests ──────────────────────────────────────────

    #[test]
    fn rule_total_count_255() {
        // The name said 210 while the assertion said 255, and the assertion was
        // the correct one. A test name is documentation that grep finds first:
        // anyone looking up how many safety rules exist hits the name, not the
        // body. `MAGE_ONTOLOGY.md` says 255 across 8 databases and is right.
        assert_eq!(rule_count(), 255);
    }

    /// Name *which* database went missing, not just that the total moved.
    ///
    /// `rule_count()` is `builtin_rules().len()` — the rules themselves.
    /// `rule_counts_by_db()` walks `RuleDatabase::ALL`, and `emit_tree`
    /// iterates the same list to write `skb/`. A variant that exists but is
    /// not in `ALL` therefore has its rules in `builtin_rules()` and nowhere
    /// on disk.
    ///
    /// **Measured 2026-09-15, with a ninth database holding one rule, wired
    /// through both exhaustive matches and left out of `ALL`:**
    ///
    /// - `check-skb-tree.sh` **passes**, printing `ok skb/ matches a fresh
    ///   --emit-skb: 255 rules across 8 databases` — while 256 rules across 9
    ///   databases exist. It diffs the committed tree against a fresh
    ///   `--emit-skb` that `emit_tree` builds from `ALL`, so both sides omit
    ///   the same database, and its summary counts what it emitted rather than
    ///   what exists.
    /// - `rule_total_count_255` **fails**. The suite is *not* blind, and an
    ///   earlier version of this comment said it was. What that failure says is
    ///   "the total changed", and its obvious remedy — bump 255 to 256 —
    ///   leaves `skb/` short with every check green.
    /// - This assertion fails with `reached 255 rule(s) but builtin_rules()
    ///   holds 256 — a RuleDatabase variant is missing from the array`.
    ///
    /// So the contribution here is **diagnosis, not detection**. Detection
    /// already existed; it pointed at a number instead of a cause.
    ///
    /// Two things do *not* need guarding, and claiming they did was the error:
    /// removing a variant from `ALL` is a compile error on its
    /// `[RuleDatabase; 8]` size, and adding one is a compile error twice over
    /// — `database_file_stem` here and the `air.skb.*` map in
    /// `rmi_ontology_adapter` are exhaustive matches. Only the last step, from
    /// "it compiles" to "it is in `ALL`", is unforced.
    #[test]
    fn per_database_counts_sum_to_the_total() {
        let per_db: usize = rule_counts_by_db().iter().map(|(_, n)| n).sum();
        assert_eq!(
            per_db,
            rule_count(),
            "rule_counts_by_db() reached {} rule(s) but builtin_rules() holds {} —              a RuleDatabase variant is missing from the array in rule_counts_by_db(),              and --emit-skb would write a tree short of those rules",
            per_db,
            rule_count()
        );
    }

    #[test]
    fn rule_counts_per_database() {
        let counts = rule_counts_by_db();
        let map: std::collections::HashMap<_, _> =
            counts.into_iter().map(|(db, n)| (format!("{:?}", db), n)).collect();
        assert_eq!(map["Ownership"], 40);
        assert_eq!(map["Borrow"], 40);
        assert_eq!(map["Lifetime"], 35);
        assert_eq!(map["TypeSafety"], 40);
        assert_eq!(map["Concurrency"], 35);
        assert_eq!(map["FFI"], 20);
        assert_eq!(map["AgentElision"], 30);
        assert_eq!(map["SwarmSafety"], 15);
    }

    #[test]
    fn query_rule_by_id_own0001() {
        let rule = query_rule_by_id("OWN-0001").expect("OWN-0001 should exist");
        assert_eq!(rule.database, RuleDatabase::Ownership);
        assert_eq!(rule.category, "use-after-move");
        assert_eq!(rule.severity, RuleSeverity::Error);
        assert!(rule.fix_template.is_some());
    }

    #[test]
    fn query_rule_by_id_nonexistent() {
        assert!(query_rule_by_id("XXX-9999").is_none());
    }

    #[test]
    fn query_rules_by_db_ownership() {
        let result = query_rules_by_db(RuleDatabase::Ownership);
        assert_eq!(result.matches.len(), 40);
        assert!(result.matches.iter().all(|r| r.database == RuleDatabase::Ownership));
    }

    #[test]
    fn query_rules_by_db_ffi() {
        let result = query_rules_by_db(RuleDatabase::FFI);
        assert_eq!(result.matches.len(), 20);
        assert!(result.matches[0].id.starts_with("FFI-"));
    }

    #[test]
    fn query_rules_by_category_data_race() {
        let result = query_rules_by_category("data-race");
        assert!(!result.matches.is_empty());
        assert!(result.matches.iter().all(|r| r.category == "data-race"));
    }

    #[test]
    fn query_rules_by_tag_concurrency() {
        let result = query_rules_by_tag("concurrency");
        assert!(result.matches.len() >= 5);
    }

    #[test]
    fn query_rules_by_severity_error() {
        let result = query_rules_by_severity(RuleSeverity::Error);
        assert!(result.matches.len() >= 20);
        assert!(result.matches.iter().all(|r| r.severity == RuleSeverity::Error));
    }

    #[test]
    fn all_rule_ids_unique() {
        let rules = builtin_rules();
        let mut ids: Vec<&str> = rules.iter().map(|r| r.id.as_str()).collect();
        ids.sort();
        let len = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), len, "duplicate rule IDs detected");
    }

    #[test]
    fn rules_serializable_to_json() {
        let rule = query_rule_by_id("BOR-0001").unwrap();
        let json = serde_json::to_string(&rule).unwrap();
        assert!(json.contains("BOR-0001"));
        assert!(json.contains("double-mutable-borrow"));
    }

    // ── The SKB as loaded policy ──────────────────────────────────────

    fn overlay_dir(tag: &str) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir()
            .join(format!("mage-skb-overlay-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// A rule as an overlay would write it.
    fn overlay_rule(id: &str, db: RuleDatabase, sev: RuleSeverity) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "database": db,
            "severity": sev,
            "category": "test",
            "description": "d",
            "rationale": "r",
            "fix_template": null,
            "fix_confidence": 0.0,
            "tags": ["test"],
        })
    }

    fn write_overlay(dir: &std::path::Path, name: &str, rules: &[serde_json::Value]) {
        std::fs::write(
            dir.join(name),
            serde_json::to_string_pretty(&rules).unwrap(),
        )
        .unwrap();
    }

    /// A one-line description of a load outcome.
    ///
    /// `{other:?}` on a `Result<RuleSet, _>` prints all 255 rules, which buries
    /// the assertion that failed under the corpus it was checking.
    fn outcome(r: &Result<RuleSet, PolicyError>) -> String {
        match r {
            Ok(set) => format!("Ok({:?}, {} rules)", set.source(), set.rules().len()),
            Err(e) => format!("Err({e})"),
        }
    }

    #[test]
    fn an_absent_overlay_is_the_builtin_floor() {
        // Absence is safe *because* removal is unrepresentable: no policy the
        // format can express is weaker than the builtins, so falling back to
        // them cannot be a downgrade.
        let missing = std::env::temp_dir().join("mage-skb-overlay-does-not-exist");
        let _ = std::fs::remove_dir_all(&missing);
        let set = RuleSet::load(&missing).expect("a missing overlay is not an error");
        assert_eq!(*set.source(), PolicySource::Builtin);
        assert_eq!(set.rules().len(), builtin_rules().len());
    }

    #[test]
    fn an_overlay_may_add_a_rule() {
        let dir = overlay_dir("add");
        write_overlay(
            &dir,
            "extra.json",
            &[overlay_rule("XTR-0001", RuleDatabase::Ownership, RuleSeverity::Error)],
        );
        let set = RuleSet::load(&dir).expect("adding is allowed");
        assert_eq!(
            *set.source(),
            PolicySource::Overlay { dir: dir.clone(), added: 1, strengthened: 0 }
        );
        assert_eq!(set.rules().len(), builtin_rules().len() + 1);
        assert!(set.rules().iter().any(|r| r.id == "XTR-0001"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_overlay_may_make_a_builtin_rule_stricter() {
        // Find a builtin rule that is not already at the strictest severity.
        let target = builtin_rules()
            .into_iter()
            .find(|r| r.severity != RuleSeverity::Error)
            .expect("some rule is below Error");
        let dir = overlay_dir("strengthen");
        write_overlay(
            &dir,
            "strict.json",
            &[overlay_rule(&target.id, target.database, RuleSeverity::Error)],
        );
        let set = RuleSet::load(&dir).expect("raising severity is allowed");
        assert_eq!(
            *set.source(),
            PolicySource::Overlay { dir: dir.clone(), added: 0, strengthened: 1 }
        );
        assert_eq!(set.rules().len(), builtin_rules().len(), "replaced, not appended");
        let now = set.rules().iter().find(|r| r.id == target.id).unwrap();
        assert_eq!(now.severity, RuleSeverity::Error);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_overlay_may_not_weaken_a_builtin_rule() {
        // The property the whole design exists for. Without it, the overlay is
        // a way to turn safety rules off, and "policy is data the loop can
        // change" means the loop can change what counts as unsafe.
        let target = builtin_rules()
            .into_iter()
            .find(|r| r.severity == RuleSeverity::Error)
            .expect("some rule is an Error");
        let dir = overlay_dir("weaken");
        write_overlay(
            &dir,
            "lax.json",
            &[overlay_rule(&target.id, target.database, RuleSeverity::Hint)],
        );
        match RuleSet::load(&dir) {
            Err(PolicyError::Weakened { id, from, to }) => {
                assert_eq!(id, target.id);
                assert_eq!(from, RuleSeverity::Error);
                assert_eq!(to, RuleSeverity::Hint);
            }
            other => panic!("a weakening overlay must be refused, got {}", outcome(&other)),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_overlay_halts_rather_than_falling_back() {
        // An overlay that exists and cannot be parsed is not the same fact as
        // no overlay. Reading it as absence would mean corrupting a policy file
        // silently restores different rules — an absence claim that cannot fail
        // loudly, one level up from `get_by_sha`.
        let dir = overlay_dir("corrupt");
        std::fs::write(dir.join("broken.json"), "{ not a rule array").unwrap();
        match RuleSet::load(&dir) {
            Err(PolicyError::Malformed { path, .. }) => {
                assert!(path.contains("broken.json"), "{path}");
            }
            other => panic!("a corrupt overlay must halt, got {}", outcome(&other)),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_overlay_may_not_declare_the_same_rule_twice() {
        let dir = overlay_dir("dup");
        write_overlay(
            &dir,
            "a.json",
            &[overlay_rule("XTR-0002", RuleDatabase::Borrow, RuleSeverity::Error)],
        );
        write_overlay(
            &dir,
            "b.json",
            &[overlay_rule("XTR-0002", RuleDatabase::Borrow, RuleSeverity::Warning)],
        );
        match RuleSet::load(&dir) {
            Err(PolicyError::DuplicateId { id, .. }) => assert_eq!(id, "XTR-0002"),
            other => panic!("two rules with one id cannot both apply, got {}", outcome(&other)),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_emitted_tree_is_the_builtin_floor_whatever_is_installed() {
        // `check-skb-tree.sh` regenerates this tree and fails on a difference.
        // If it tracked the installed set, the checker would be testing whether
        // $MAGE_SKB_OVERLAY happened to be set in CI's shell.
        let dir = overlay_dir("emit");
        let out = overlay_dir("emit-out");
        let n = emit_tree(&out).expect("emitted");
        assert_eq!(n, builtin_rules().len(), "the tree is the floor, not the policy");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn the_same_policy_has_the_same_name_however_it_was_spelled() {
        // The digest names the rules in force, not the directory that produced
        // them. Splitting one overlay file into two, or reformatting it, is the
        // same policy and must not read as a different one in a journal.
        let one = overlay_dir("spell-1");
        write_overlay(
            &one,
            "all.json",
            &[
                overlay_rule("XTR-1001", RuleDatabase::Ownership, RuleSeverity::Error),
                overlay_rule("XTR-1002", RuleDatabase::Borrow, RuleSeverity::Error),
            ],
        );
        let two = overlay_dir("spell-2");
        write_overlay(
            &two,
            "a.json",
            &[overlay_rule("XTR-1001", RuleDatabase::Ownership, RuleSeverity::Error)],
        );
        write_overlay(
            &two,
            "b.json",
            &[overlay_rule("XTR-1002", RuleDatabase::Borrow, RuleSeverity::Error)],
        );

        let a = RuleSet::load(&one).expect("loads");
        let b = RuleSet::load(&two).expect("loads");
        assert_eq!(a.digest(), b.digest(), "same rules, same policy");
        assert_ne!(a.digest(), RuleSet::builtin().digest(), "but not the floor");
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    #[test]
    fn moving_one_severity_changes_the_policy_name() {
        // The other half: a digest that did not move when the rules did would
        // make attribution worthless precisely when it matters.
        let target = builtin_rules()
            .into_iter()
            .find(|r| r.severity != RuleSeverity::Error)
            .expect("some rule is below Error");
        let dir = overlay_dir("rename");
        write_overlay(
            &dir,
            "strict.json",
            &[overlay_rule(&target.id, target.database, RuleSeverity::Error)],
        );
        let set = RuleSet::load(&dir).expect("strengthening is allowed");
        assert_eq!(set.rules().len(), builtin_rules().len(), "no rule added");
        assert_ne!(
            set.digest(),
            RuleSet::builtin().digest(),
            "one severity moved, so this is a different policy"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn severity_strictness_orders_the_way_the_floor_needs() {
        // `load` compares severities numerically; if this order were wrong the
        // weakening check would read backwards and admit exactly what it bars.
        assert!(RuleSeverity::Error.strictness() < RuleSeverity::Warning.strictness());
        assert!(RuleSeverity::Warning.strictness() < RuleSeverity::Info.strictness());
        assert!(RuleSeverity::Info.strictness() < RuleSeverity::Hint.strictness());
    }
}

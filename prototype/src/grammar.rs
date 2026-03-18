/// Grammar Extension System — namespace-scoped discovery, registration, frequency promotion.
///
/// `grammar_extension!` macro defines custom syntactic shorthands.
/// Extensions register in `Redox.toml` and can be promoted to built-in
/// based on usage frequency.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Extension Definition ─────────────────────────────────────────────

/// A registered grammar extension: maps a shorthand sigil to a Rust expansion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarExtension {
    /// The shorthand sigil or keyword (e.g. "?=", "@w", "+af").
    pub sigil: String,
    /// The Rust-equivalent expansion (e.g. "match", "while", "pub async fn").
    pub rust_equiv: String,
    /// Namespace the extension belongs to (e.g. "core", "async", "user").
    pub namespace: String,
    /// Number of times this sigil has been used (for promotion tracking).
    pub usage_count: u64,
    /// Description of the extension.
    pub description: String,
}

/// Frequency promotion threshold — extensions used more than this count
/// in a project are candidates for built-in promotion.
pub const PROMOTION_THRESHOLD: u64 = 100;

/// The extension registry holds all known grammar extensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionRegistry {
    pub extensions: Vec<GrammarExtension>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        let mut reg = Self { extensions: Vec::new() };
        reg.register_builtins();
        reg
    }

    /// Register the built-in core sigils that ship with Redox.
    fn register_builtins(&mut self) {
        let builtins = vec![
            // Core items
            ("f",   "fn",           "core", "Function definition"),
            ("+f",  "pub fn",       "core", "Public function definition"),
            ("af",  "async fn",     "core", "Async function"),
            ("+af", "pub async fn", "core", "Public async function"),
            ("uf",  "unsafe fn",    "core", "Unsafe function"),
            ("+uf", "pub unsafe fn","core", "Public unsafe function"),
            ("S",   "struct",       "core", "Struct definition"),
            ("+S",  "pub struct",   "core", "Public struct definition"),
            ("E",   "enum",         "core", "Enum definition"),
            ("+E",  "pub enum",     "core", "Public enum definition"),
            ("T",   "trait",        "core", "Trait definition"),
            ("+T",  "pub trait",    "core", "Public trait definition"),
            ("I",   "impl",         "core", "Impl block"),
            ("M",   "mod",          "core", "Module"),
            ("+M",  "pub mod",      "core", "Public module"),
            ("u",   "use",          "core", "Use import"),
            ("+u",  "pub use",      "core", "Public use import"),
            ("Y",   "type",         "core", "Type alias"),
            ("+Y",  "pub type",     "core", "Public type alias"),
            ("C",   "const",        "core", "Constant"),
            ("+C",  "pub const",    "core", "Public constant"),
            ("Z",   "static",       "core", "Static"),
            ("+Z",  "pub static",   "core", "Public static"),
            // Bindings
            ("v",   "let",          "core", "Immutable binding"),
            ("m",   "let mut",      "core", "Mutable binding"),
            // Control flow
            ("?",   "if",           "flow", "Conditional branch"),
            ("?=",  "match",        "flow", "Pattern matching"),
            ("@",   "for",          "flow", "For loop"),
            ("@@",  "loop",         "flow", "Infinite loop"),
            ("@w",  "while",        "flow", "While loop"),
            ("!",   "break",        "flow", "Break loop"),
            (">>",  "continue",     "flow", "Continue loop"),
            // Smart pointers / types
            ("^",   "Box<_>",       "ptr",  "Owned pointer"),
            ("$",   "Rc<_>",        "ptr",  "Reference-counted pointer"),
            ("@T",  "Arc<_>",       "ptr",  "Atomic reference-counted pointer"),
            ("&~",  "Cow<_>",       "ptr",  "Copy-on-write"),
            ("%",   "Cell<_>",      "ptr",  "Interior mutability (Cell)"),
            ("%!",  "RefCell<_>",   "ptr",  "Interior mutability (RefCell)"),
            ("#",   "Mutex<_>",     "ptr",  "Mutex lock"),
            ("#~",  "RwLock<_>",    "ptr",  "Read-write lock"),
            ("?T",  "Option<_>",    "ptr",  "Optional value"),
            ("R[,]","Result<_,_>",  "ptr",  "Result type"),
            // Annotations
            ("@req","requires",     "contract", "Precondition contract"),
            ("@ens","ensures",      "contract", "Postcondition contract"),
            ("@inv","invariant",    "contract", "Invariant contract"),
            ("@fx", "effects",      "contract", "Effect annotation"),
            ("@perf","performance", "contract", "Performance annotation"),
        ];

        for (sigil, rust, ns, desc) in builtins {
            self.extensions.push(GrammarExtension {
                sigil: sigil.into(),
                rust_equiv: rust.into(),
                namespace: ns.into(),
                usage_count: 0,
                description: desc.into(),
            });
        }
    }

    /// Register a user-defined grammar extension.
    pub fn register(&mut self, sigil: &str, rust_equiv: &str, namespace: &str, description: &str) {
        self.extensions.push(GrammarExtension {
            sigil: sigil.into(),
            rust_equiv: rust_equiv.into(),
            namespace: namespace.into(),
            usage_count: 0,
            description: description.into(),
        });
    }

    /// Record a usage of a sigil — increments the usage counter.
    pub fn record_usage(&mut self, sigil: &str) {
        if let Some(ext) = self.extensions.iter_mut().find(|e| e.sigil == sigil) {
            ext.usage_count += 1;
        }
    }

    /// Find the Rust equivalent for a sigil.
    pub fn lookup(&self, sigil: &str) -> Option<&GrammarExtension> {
        self.extensions.iter().find(|e| e.sigil == sigil)
    }

    /// List all extensions in a given namespace.
    pub fn list_namespace(&self, namespace: &str) -> Vec<&GrammarExtension> {
        self.extensions.iter().filter(|e| e.namespace == namespace).collect()
    }

    /// List all available namespaces.
    pub fn namespaces(&self) -> Vec<String> {
        let mut ns: Vec<String> = self.extensions.iter().map(|e| e.namespace.clone()).collect();
        ns.sort();
        ns.dedup();
        ns
    }

    /// Return extensions that have exceeded the promotion threshold.
    pub fn promotion_candidates(&self) -> Vec<&GrammarExtension> {
        self.extensions
            .iter()
            .filter(|e| e.namespace != "core" && e.usage_count >= PROMOTION_THRESHOLD)
            .collect()
    }

    /// Return all extensions as a JSON-serializable list.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!(self.extensions.iter().map(|e| {
            serde_json::json!({
                "sigil": e.sigil,
                "rust": e.rust_equiv,
                "namespace": e.namespace,
                "usage_count": e.usage_count,
                "description": e.description,
            })
        }).collect::<Vec<_>>())
    }
}

// ── Redox.toml parsing ──────────────────────────────────────────────

/// Represents a `[grammar_extensions]` section in Redox.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarExtensionConfig {
    pub sigil: String,
    pub rust_equiv: String,
    pub namespace: Option<String>,
    pub description: Option<String>,
}

/// Parse grammar extensions from a TOML-style map (simplified).
/// Expects a list of `{ sigil, rust_equiv, namespace?, description? }` entries.
pub fn parse_extension_configs(entries: &[HashMap<String, String>]) -> Vec<GrammarExtensionConfig> {
    entries
        .iter()
        .map(|e| GrammarExtensionConfig {
            sigil: e.get("sigil").cloned().unwrap_or_default(),
            rust_equiv: e.get("rust_equiv").cloned().unwrap_or_default(),
            namespace: e.get("namespace").cloned(),
            description: e.get("description").cloned(),
        })
        .collect()
}

/// Apply parsed configs to a registry.
pub fn apply_configs(registry: &mut ExtensionRegistry, configs: &[GrammarExtensionConfig]) {
    for cfg in configs {
        registry.register(
            &cfg.sigil,
            &cfg.rust_equiv,
            cfg.namespace.as_deref().unwrap_or("user"),
            cfg.description.as_deref().unwrap_or(""),
        );
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_registry_has_builtins() {
        let reg = ExtensionRegistry::new();
        assert!(reg.extensions.len() >= 40, "got {} builtins", reg.extensions.len());
    }

    #[test]
    fn lookup_core_function() {
        let reg = ExtensionRegistry::new();
        let ext = reg.lookup("f").expect("should find 'f'");
        assert_eq!(ext.rust_equiv, "fn");
        assert_eq!(ext.namespace, "core");
    }

    #[test]
    fn lookup_flow_match() {
        let reg = ExtensionRegistry::new();
        let ext = reg.lookup("?=").expect("should find '?='");
        assert_eq!(ext.rust_equiv, "match");
        assert_eq!(ext.namespace, "flow");
    }

    #[test]
    fn lookup_ptr_sigils() {
        let reg = ExtensionRegistry::new();
        assert_eq!(reg.lookup("^").unwrap().rust_equiv, "Box<_>");
        assert_eq!(reg.lookup("$").unwrap().rust_equiv, "Rc<_>");
        assert_eq!(reg.lookup("#").unwrap().rust_equiv, "Mutex<_>");
    }

    #[test]
    fn lookup_contract_sigils() {
        let reg = ExtensionRegistry::new();
        assert_eq!(reg.lookup("@req").unwrap().rust_equiv, "requires");
        assert_eq!(reg.lookup("@ens").unwrap().rust_equiv, "ensures");
        assert_eq!(reg.lookup("@inv").unwrap().rust_equiv, "invariant");
    }

    #[test]
    fn register_custom_extension() {
        let mut reg = ExtensionRegistry::new();
        let before = reg.extensions.len();
        reg.register("~>", "pipe", "user_flow", "Pipeline operator");
        assert_eq!(reg.extensions.len(), before + 1);
        let ext = reg.lookup("~>").unwrap();
        assert_eq!(ext.rust_equiv, "pipe");
        assert_eq!(ext.namespace, "user_flow");
    }

    #[test]
    fn record_usage_increments() {
        let mut reg = ExtensionRegistry::new();
        reg.register("$$", "alloc", "custom", "Custom alloc");
        reg.record_usage("$$");
        reg.record_usage("$$");
        reg.record_usage("$$");
        assert_eq!(reg.lookup("$$").unwrap().usage_count, 3);
    }

    #[test]
    fn list_namespace_core() {
        let reg = ExtensionRegistry::new();
        let core = reg.list_namespace("core");
        assert!(core.len() >= 20, "core namespace has {} entries", core.len());
    }

    #[test]
    fn list_namespace_flow() {
        let reg = ExtensionRegistry::new();
        let flow = reg.list_namespace("flow");
        assert!(flow.len() >= 5, "flow namespace has {} entries", flow.len());
    }

    #[test]
    fn namespaces_list() {
        let reg = ExtensionRegistry::new();
        let ns = reg.namespaces();
        assert!(ns.contains(&"core".into()));
        assert!(ns.contains(&"flow".into()));
        assert!(ns.contains(&"ptr".into()));
        assert!(ns.contains(&"contract".into()));
    }

    #[test]
    fn promotion_candidates_empty_initially() {
        let reg = ExtensionRegistry::new();
        assert!(reg.promotion_candidates().is_empty());
    }

    #[test]
    fn promotion_after_threshold() {
        let mut reg = ExtensionRegistry::new();
        reg.register("xx", "special", "user", "Test");
        for _ in 0..PROMOTION_THRESHOLD {
            reg.record_usage("xx");
        }
        let candidates = reg.promotion_candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].sigil, "xx");
    }

    #[test]
    fn core_not_promoted() {
        let mut reg = ExtensionRegistry::new();
        for _ in 0..200 {
            reg.record_usage("f");
        }
        // Core sigils are never promotion candidates
        assert!(reg.promotion_candidates().is_empty());
    }

    #[test]
    fn to_json_serializes() {
        let reg = ExtensionRegistry::new();
        let j = reg.to_json();
        let arr = j.as_array().unwrap();
        assert!(arr.len() >= 40);
        assert!(arr[0].get("sigil").is_some());
        assert!(arr[0].get("rust").is_some());
        assert!(arr[0].get("namespace").is_some());
    }

    #[test]
    fn parse_and_apply_configs() {
        let mut reg = ExtensionRegistry::new();
        let before = reg.extensions.len();
        let entries = vec![{
            let mut m = HashMap::new();
            m.insert("sigil".into(), "=>".into());
            m.insert("rust_equiv".into(), "fat_arrow".into());
            m.insert("namespace".into(), "user".into());
            m
        }];
        let configs = parse_extension_configs(&entries);
        apply_configs(&mut reg, &configs);
        assert_eq!(reg.extensions.len(), before + 1);
        assert_eq!(reg.lookup("=>").unwrap().rust_equiv, "fat_arrow");
    }

    #[test]
    fn unknown_sigil_returns_none() {
        let reg = ExtensionRegistry::new();
        assert!(reg.lookup("zzz_nonexistent").is_none());
    }

    #[test]
    fn record_usage_unknown_sigil_noop() {
        let mut reg = ExtensionRegistry::new();
        reg.record_usage("zzz_nonexistent");
        // Should not panic, just no-op
    }
}

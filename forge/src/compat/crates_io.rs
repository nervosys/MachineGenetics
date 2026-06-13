/// Compatibility layer between Forge and crates.io.
///
/// Supports:
/// - Auto-transpile Rust crates on first use via `rust2mg`
/// - Publish to both registries via `forge publish --also-crates-io`
/// - Unified dependency resolution across Rust + MAGE
/// - Version mapping (e.g. `u http.Client` → crates.io `reqwest`)
/// - FFI bridge for using Rust crates directly

/// A mapping from a MAGE module path to a crates.io crate.
#[derive(Debug, Clone)]
pub struct CrateAlias {
    /// MAGE-style import path, e.g. `http.Client`
    pub mage_path: String,
    /// Corresponding crates.io crate name, e.g. `reqwest`
    pub crate_name: String,
    /// Version requirement for the crate, e.g. `^0.12`
    pub version_req: String,
    /// Specific feature flags to enable
    pub features: Vec<String>,
}

impl CrateAlias {
    pub fn new(mage_path: &str, crate_name: &str, version_req: &str) -> Self {
        Self {
            mage_path: mage_path.to_string(),
            crate_name: crate_name.to_string(),
            version_req: version_req.to_string(),
            features: Vec::new(),
        }
    }

    pub fn with_features(mut self, features: &[&str]) -> Self {
        self.features = features.iter().map(|s| s.to_string()).collect();
        self
    }
}

/// The alias table that maps MAGE imports to crates.io crates.
#[derive(Debug, Default)]
pub struct AliasTable {
    aliases: Vec<CrateAlias>,
}

impl AliasTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a MAGE → crates.io alias.
    pub fn register(&mut self, alias: CrateAlias) {
        self.aliases.push(alias);
    }

    /// Look up the crates.io crate for a MAGE import path.
    pub fn resolve(&self, mage_path: &str) -> Option<&CrateAlias> {
        self.aliases.iter().find(|a| a.mage_path == mage_path)
    }

    /// Return all registered aliases.
    pub fn all(&self) -> &[CrateAlias] {
        &self.aliases
    }

    /// Build the default alias table with well-known mappings.
    pub fn with_defaults() -> Self {
        let mut table = Self::new();
        table.register(CrateAlias::new("http.Client", "reqwest", "^0.12"));
        table.register(CrateAlias::new("json", "serde_json", "^1"));
        table.register(CrateAlias::new("async.Runtime", "tokio", "^1").with_features(&["full"]));
        table.register(CrateAlias::new("cli.Args", "clap", "^4").with_features(&["derive"]));
        table.register(CrateAlias::new("regex", "regex", "^1"));
        table
    }
}

/// Check whether a crate name refers to a crates.io package rather than Forge.
pub fn is_crates_io_dependency(name: &str) -> bool {
    // Heuristic: names with underscores are typically Rust crates;
    // names with hyphens are typically Forge modules. This is a
    // simplification—the real check queries both registries.
    name.contains('_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_table_lookup() {
        let table = AliasTable::with_defaults();
        let alias = table.resolve("http.Client").unwrap();
        assert_eq!(alias.crate_name, "reqwest");
    }

    #[test]
    fn alias_table_miss() {
        let table = AliasTable::with_defaults();
        assert!(table.resolve("nonexistent.Module").is_none());
    }

    #[test]
    fn alias_with_features() {
        let table = AliasTable::with_defaults();
        let alias = table.resolve("async.Runtime").unwrap();
        assert!(alias.features.contains(&"full".to_string()));
    }
}

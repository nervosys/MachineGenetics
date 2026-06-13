use serde::{Deserialize, Serialize};
use semver::Version;

use super::dependency::Dependency;
use super::effect::EffectDecl;
use super::metadata::ModuleMetadata;
use super::skb_rule::SkbRule;
use super::spec::SpecBlock;

/// A published module in the Forge registry.
///
/// Modules are the primary unit of distribution in MAGE, analogous to
/// crates in Rust. Each module supports dual-format source (`.mg` + `.rs`),
/// MLIR artifact caching, and package-level safety rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    /// Module name (e.g., "http-client")
    pub name: String,
    /// Current version
    pub version: Version,
    /// Module source files
    pub source: ModuleSource,
    /// Module metadata
    pub metadata: ModuleMetadata,
    /// Dependencies
    pub dependencies: Vec<Dependency>,
    /// Package-specific SKB rules
    pub skb_rules: Vec<SkbRule>,
    /// Published API contracts
    pub specs: Vec<SpecBlock>,
    /// Declared effect signatures
    pub effects: Vec<EffectDecl>,
}

/// Source files for a module, supporting dual MAGE/Rust format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSource {
    /// Native MAGE source files (.mg)
    pub rdx_files: Vec<SourceFile>,
    /// Optional Rust compatibility source files (.rs)
    pub rs_files: Vec<SourceFile>,
    /// Pre-lowered MLIR artifacts (if cached)
    pub mlir_cache: Option<Vec<MlirArtifact>>,
}

/// A source file entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFile {
    /// Relative path within the module
    pub path: String,
    /// File size in bytes
    pub size: u64,
    /// SHA-256 hash of the file contents
    pub sha256: String,
}

/// A pre-lowered MLIR artifact for a specific target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlirArtifact {
    /// Target triple (e.g., "x86_64-unknown-linux-gnu")
    pub target: String,
    /// Dialect level
    pub dialect: MlirDialect,
    /// Artifact file path
    pub path: String,
    /// SHA-256 hash
    pub sha256: String,
}

/// MLIR dialect levels in the lowering pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MlirDialect {
    /// MAGE dialect (highest level)
    MAGE,
    /// After Linalg lowering
    Linalg,
    /// After Affine lowering
    Affine,
    /// After LLVM lowering
    Llvm,
    /// Pre-compiled native object
    Native,
    /// Pre-compiled WASM
    Wasm,
}

impl Module {
    /// Create a new module with the given name and version.
    pub fn new(name: impl Into<String>, version: Version) -> Self {
        Module {
            name: name.into(),
            version,
            source: ModuleSource {
                rdx_files: Vec::new(),
                rs_files: Vec::new(),
                mlir_cache: None,
            },
            metadata: ModuleMetadata::default(),
            dependencies: Vec::new(),
            skb_rules: Vec::new(),
            specs: Vec::new(),
            effects: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_module() {
        let module = Module::new("http-client", Version::new(1, 0, 0));
        assert_eq!(module.name, "http-client");
        assert_eq!(module.version, Version::new(1, 0, 0));
        assert!(module.dependencies.is_empty());
        assert!(module.source.rdx_files.is_empty());
    }

    #[test]
    fn test_module_serialization() {
        let module = Module::new("test-mod", Version::new(0, 1, 0));
        let json = serde_json::to_string(&module).unwrap();
        let deserialized: Module = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test-mod");
        assert_eq!(deserialized.version, Version::new(0, 1, 0));
    }
}

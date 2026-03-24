use std::path::PathBuf;

use crate::models::MlirDialect;

/// An entry in the MLIR artifact cache.
///
/// Cache structure (per module, per version):
/// ```text
/// module-name/1.3.0/
///   ├── MechGen-dialect.mlir
///   ├── linalg-dialect.mlir
///   ├── affine-dialect.mlir
///   ├── llvm-dialect.mlir
///   ├── x86_64.o
///   ├── aarch64.o
///   └── wasm32.wasm
/// ```
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub module_name: String,
    pub version: String,
    pub dialect: MlirDialect,
    pub path: PathBuf,
}

/// MLIR artifact cache manager.
///
/// Target hit rate: **> 95%** for published modules on common targets.
///
/// Invalidation triggers:
/// - Dependency updates
/// - Compiler version bumps
/// - SKB rule changes
pub struct MlirCache {
    root: PathBuf,
}

impl MlirCache {
    /// Create a cache rooted at the given directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Compute the cache directory for a module version.
    pub fn version_dir(&self, module: &str, version: &str) -> PathBuf {
        self.root.join(module).join(version)
    }

    /// Return the expected path for a dialect artifact.
    pub fn artifact_path(&self, module: &str, version: &str, dialect: &MlirDialect) -> PathBuf {
        self.version_dir(module, version).join(dialect_filename(dialect))
    }

    /// Check whether a cached artifact already exists.
    pub fn has_artifact(&self, module: &str, version: &str, dialect: &MlirDialect) -> bool {
        self.artifact_path(module, version, dialect).exists()
    }

    /// Invalidate all cached artifacts for a module version.
    pub fn invalidate(&self, module: &str, version: &str) -> std::io::Result<()> {
        let dir = self.version_dir(module, version);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    /// List all cached dialects for a module version.
    pub fn list_artifacts(&self, module: &str, version: &str) -> Vec<CacheEntry> {
        let dir = self.version_dir(module, version);
        if !dir.exists() {
            return Vec::new();
        }
        let mut entries = Vec::new();
        if let Ok(read_dir) = std::fs::read_dir(&dir) {
            for entry in read_dir.flatten() {
                if let Some(dialect) = dialect_from_filename(&entry.file_name().to_string_lossy()) {
                    entries.push(CacheEntry {
                        module_name: module.to_string(),
                        version: version.to_string(),
                        dialect,
                        path: entry.path(),
                    });
                }
            }
        }
        entries
    }
}

/// Map a dialect to its cache filename.
fn dialect_filename(dialect: &MlirDialect) -> &'static str {
    match dialect {
        MlirDialect::MechGen => "MechGen-dialect.mlir",
        MlirDialect::Linalg => "linalg-dialect.mlir",
        MlirDialect::Affine => "affine-dialect.mlir",
        MlirDialect::Llvm => "llvm-dialect.mlir",
        MlirDialect::Native => "native.o",
        MlirDialect::Wasm => "wasm32.wasm",
    }
}

/// Attempt to infer the dialect from a cache filename.
fn dialect_from_filename(name: &str) -> Option<MlirDialect> {
    match name {
        "MechGen-dialect.mlir" => Some(MlirDialect::MechGen),
        "linalg-dialect.mlir" => Some(MlirDialect::Linalg),
        "affine-dialect.mlir" => Some(MlirDialect::Affine),
        "llvm-dialect.mlir" => Some(MlirDialect::Llvm),
        _ if name.ends_with(".o") => Some(MlirDialect::Native),
        _ if name.ends_with(".wasm") => Some(MlirDialect::Wasm),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn version_dir_layout() {
        let cache = MlirCache::new(Path::new("/tmp/cache"));
        assert_eq!(
            cache.version_dir("http-client", "1.3.0"),
            PathBuf::from("/tmp/cache/http-client/1.3.0")
        );
    }

    #[test]
    fn artifact_path_matches_spec() {
        let cache = MlirCache::new(Path::new("/tmp/cache"));
        let path = cache.artifact_path("http-client", "1.3.0", &MlirDialect::MechGen);
        assert!(path.to_str().unwrap().ends_with("MechGen-dialect.mlir"));
    }

    #[test]
    fn dialect_roundtrip() {
        for dialect in &[
            MlirDialect::MechGen,
            MlirDialect::Linalg,
            MlirDialect::Affine,
            MlirDialect::Llvm,
        ] {
            let name = dialect_filename(dialect);
            let parsed = dialect_from_filename(name).unwrap();
            assert_eq!(
                std::mem::discriminant(&parsed),
                std::mem::discriminant(dialect)
            );
        }
    }
}

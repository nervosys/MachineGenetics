use serde::{Deserialize, Serialize};

/// A dependency specification for a module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Dependency module name
    pub name: String,
    /// Version requirement (semver range)
    pub version_req: String,
    /// Optional features to enable
    pub features: Vec<String>,
    /// Whether this is a dev-only dependency
    pub dev: bool,
    /// Source of the dependency
    pub source: DependencySource,
}

/// Where a dependency comes from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencySource {
    /// From the Forge registry
    Forge,
    /// From crates.io (Rust compatibility)
    CratesIo,
    /// From a Git repository
    Git { url: String, branch: Option<String> },
    /// From a local path
    Path { path: String },
}

impl Dependency {
    /// Create a Forge registry dependency.
    pub fn forge(name: impl Into<String>, version_req: impl Into<String>) -> Self {
        Dependency {
            name: name.into(),
            version_req: version_req.into(),
            features: Vec::new(),
            dev: false,
            source: DependencySource::Forge,
        }
    }

    /// Create a crates.io dependency.
    pub fn crates_io(name: impl Into<String>, version_req: impl Into<String>) -> Self {
        Dependency {
            name: name.into(),
            version_req: version_req.into(),
            features: Vec::new(),
            dev: false,
            source: DependencySource::CratesIo,
        }
    }

    /// Set this as a dev-only dependency.
    pub fn as_dev(mut self) -> Self {
        self.dev = true;
        self
    }

    /// Enable specific features.
    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forge_dependency() {
        let dep = Dependency::forge("http", "^1.0");
        assert_eq!(dep.name, "http");
        assert_eq!(dep.version_req, "^1.0");
        assert!(!dep.dev);
        assert!(matches!(dep.source, DependencySource::Forge));
    }

    #[test]
    fn test_crates_io_dependency() {
        let dep = Dependency::crates_io("serde", "1")
            .with_features(vec!["derive".to_string()]);
        assert_eq!(dep.name, "serde");
        assert_eq!(dep.features, vec!["derive"]);
        assert!(matches!(dep.source, DependencySource::CratesIo));
    }

    #[test]
    fn test_dev_dependency() {
        let dep = Dependency::forge("test-utils", "0.3").as_dev();
        assert!(dep.dev);
    }
}

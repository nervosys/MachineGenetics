//! RMIL Package Registry — share and discover RMIL modules across agents.
//!
//! The registry stores versioned RMIL packages (bundles of [`Expr`] programs
//! with metadata) and allows agents to publish, search, and resolve
//! dependencies.
//!
//! # Concepts
//!
//! - **Package**: a named, versioned bundle containing an RMIL expression,
//!   metadata (author, tags, description), and dependency declarations.
//! - **Registry**: an in-process store with publish / resolve / search.
//! - **SemVer**: versions follow `major.minor.patch` semantic versioning.
//! - **Content hashing**: each package is content-addressed via `Expr::content_hash`.
//!
//! # Example
//!
//! ```
//! use rmi::lang::registry::{Registry, PackageMeta, SemVer};
//! use rmi::lang::{Expr, Op};
//!
//! let mut reg = Registry::new();
//!
//! let meta = PackageMeta::new("my_block", SemVer::new(1, 0, 0))
//!     .with_description("A custom transformer block")
//!     .with_tag("transformer")
//!     .with_tag("nlp");
//!
//! let expr = Expr::op1(Op::LAYER_NORM)
//!     >> Expr::op1(Op::ATTN)
//!     >> Expr::op1(Op::LINEAR)
//!     >> Expr::op1(Op::GELU);
//!
//! reg.publish(meta, expr.clone()).unwrap();
//!
//! let found = reg.resolve("my_block", &">=1.0.0".parse().unwrap());
//! assert!(found.is_some());
//! ```

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use crate::lang::expr::Expr;

// ── SemVer ───────────────────────────────────────────────────────────────────

/// A semantic version: `major.minor.patch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SemVer {
    /// Breaking changes.
    pub major: u32,
    /// Backwards-compatible features.
    pub minor: u32,
    /// Bug fixes.
    pub patch: u32,
}

impl SemVer {
    /// Create a new version.
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for SemVer {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(format!(
                "expected 3 dot-separated parts, got {}",
                parts.len()
            ));
        }
        let major = parts[0]
            .parse()
            .map_err(|_| format!("invalid major: {}", parts[0]))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| format!("invalid minor: {}", parts[1]))?;
        let patch = parts[2]
            .parse()
            .map_err(|_| format!("invalid patch: {}", parts[2]))?;
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

// ── Version requirement ──────────────────────────────────────────────────────

/// A version requirement for dependency resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionReq {
    /// Exact match.
    Exact(SemVer),
    /// Greater or equal.
    Gte(SemVer),
    /// Compatible (`^` — same major).
    Compatible(SemVer),
    /// Any version.
    Any,
}

impl VersionReq {
    /// Check if a version satisfies this requirement.
    pub fn matches(&self, ver: &SemVer) -> bool {
        match self {
            VersionReq::Exact(v) => ver == v,
            VersionReq::Gte(v) => ver >= v,
            VersionReq::Compatible(v) => ver.major == v.major && ver >= v,
            VersionReq::Any => true,
        }
    }
}

impl FromStr for VersionReq {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s == "*" {
            return Ok(VersionReq::Any);
        }
        if let Some(rest) = s.strip_prefix(">=") {
            return Ok(VersionReq::Gte(rest.trim().parse()?));
        }
        if let Some(rest) = s.strip_prefix('^') {
            return Ok(VersionReq::Compatible(rest.trim().parse()?));
        }
        // Try exact
        Ok(VersionReq::Exact(s.parse()?))
    }
}

// ── Package metadata ─────────────────────────────────────────────────────────

/// Metadata for a published package.
#[derive(Debug, Clone)]
pub struct PackageMeta {
    /// Package name (must be unique within a major version).
    pub name: String,
    /// Semantic version.
    pub version: SemVer,
    /// Short description.
    pub description: Option<String>,
    /// Author / agent ID.
    pub author: Option<String>,
    /// Tags for search.
    pub tags: Vec<String>,
    /// Dependencies: `(name, version_req)`.
    pub dependencies: Vec<(String, VersionReq)>,
}

impl PackageMeta {
    /// Create minimal metadata.
    pub fn new(name: impl Into<String>, version: SemVer) -> Self {
        Self {
            name: name.into(),
            version,
            description: None,
            author: None,
            tags: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    /// Set description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set author.
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Add a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add a dependency.
    pub fn with_dependency(mut self, name: impl Into<String>, req: VersionReq) -> Self {
        self.dependencies.push((name.into(), req));
        self
    }
}

// ── Published package ────────────────────────────────────────────────────────

/// A published package in the registry.
#[derive(Debug, Clone)]
pub struct Package {
    /// Package metadata.
    pub meta: PackageMeta,
    /// The RMIL expression tree.
    pub expr: Expr,
    /// Content hash of the expression.
    pub content_hash: u64,
    /// Timestamp (seconds since epoch, or monotonic counter).
    pub published_at: u64,
}

// ── Registry errors ──────────────────────────────────────────────────────────

/// Errors from registry operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// A package with this name and version already exists.
    AlreadyExists {
        /// Package name.
        name: String,
        /// Version that already exists.
        version: SemVer,
    },
    /// Package name is invalid.
    InvalidName(String),
    /// Dependency not found.
    DependencyNotFound {
        /// The package requiring the dependency.
        requirer: String,
        /// The missing dependency name.
        dependency: String,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::AlreadyExists { name, version } => {
                write!(f, "package '{name}@{version}' already exists")
            }
            RegistryError::InvalidName(name) => {
                write!(f, "invalid package name: '{name}'")
            }
            RegistryError::DependencyNotFound {
                requirer,
                dependency,
            } => {
                write!(
                    f,
                    "dependency '{dependency}' required by '{requirer}' not found"
                )
            }
        }
    }
}

impl std::error::Error for RegistryError {}

// ── Registry ─────────────────────────────────────────────────────────────────

/// In-process RMIL package registry.
///
/// Agents publish versioned RMIL packages and resolve dependencies
/// from this registry. In a distributed setting, each node would
/// have a local registry that syncs with peers.
pub struct Registry {
    /// name → sorted vec of packages (by version, ascending).
    packages: HashMap<String, Vec<Package>>,
    /// Monotonic publish counter.
    counter: u64,
}

impl Registry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            packages: HashMap::new(),
            counter: 0,
        }
    }

    /// Publish a package.
    pub fn publish(&mut self, meta: PackageMeta, expr: Expr) -> Result<(), RegistryError> {
        // Validate name
        if meta.name.is_empty() || meta.name.len() > 128 {
            return Err(RegistryError::InvalidName(meta.name.clone()));
        }
        if !meta
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(RegistryError::InvalidName(meta.name.clone()));
        }

        let versions = self.packages.entry(meta.name.clone()).or_default();

        // Check for duplicate version
        if versions.iter().any(|p| p.meta.version == meta.version) {
            return Err(RegistryError::AlreadyExists {
                name: meta.name,
                version: meta.version,
            });
        }

        let content_hash = expr.content_hash();
        self.counter += 1;

        let package = Package {
            meta,
            expr,
            content_hash,
            published_at: self.counter,
        };

        versions.push(package);
        // Keep sorted by version
        versions.sort_by_key(|a| a.meta.version);

        Ok(())
    }

    /// Resolve the best matching package for a name and version requirement.
    ///
    /// Returns the highest version that satisfies the requirement.
    pub fn resolve(&self, name: &str, req: &VersionReq) -> Option<&Package> {
        let versions = self.packages.get(name)?;
        versions.iter().rev().find(|p| req.matches(&p.meta.version))
    }

    /// Get all versions of a package.
    pub fn versions(&self, name: &str) -> &[Package] {
        self.packages.get(name).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get a specific version of a package.
    pub fn get(&self, name: &str, version: &SemVer) -> Option<&Package> {
        let versions = self.packages.get(name)?;
        versions.iter().find(|p| &p.meta.version == version)
    }

    /// Search packages by tag.
    pub fn search_by_tag(&self, tag: &str) -> Vec<&Package> {
        let tag_lower = tag.to_ascii_lowercase();
        self.packages
            .values()
            .flat_map(|versions| versions.iter())
            .filter(|p| {
                p.meta
                    .tags
                    .iter()
                    .any(|t| t.to_ascii_lowercase() == tag_lower)
            })
            .collect()
    }

    /// Search packages by name substring.
    pub fn search_by_name(&self, query: &str) -> Vec<&Package> {
        let query_lower = query.to_ascii_lowercase();
        self.packages
            .values()
            .flat_map(|versions| versions.last()) // latest version only
            .filter(|p| p.meta.name.to_ascii_lowercase().contains(&query_lower))
            .collect()
    }

    /// List all package names.
    pub fn list(&self) -> Vec<&str> {
        self.packages.keys().map(|s| s.as_str()).collect()
    }

    /// Total number of packages (including all versions).
    pub fn total_packages(&self) -> usize {
        self.packages.values().map(|v| v.len()).sum()
    }

    /// Resolve all dependencies for a package, returning them in dependency order.
    pub fn resolve_deps(
        &self,
        name: &str,
        version: &SemVer,
    ) -> Result<Vec<&Package>, RegistryError> {
        let pkg = self
            .get(name, version)
            .ok_or(RegistryError::InvalidName(format!("{name}@{version}")))?;

        let mut resolved = Vec::new();
        let mut visited = std::collections::HashSet::new();
        self.resolve_deps_inner(pkg, &mut resolved, &mut visited)?;
        Ok(resolved)
    }

    fn resolve_deps_inner<'a>(
        &'a self,
        pkg: &'a Package,
        resolved: &mut Vec<&'a Package>,
        visited: &mut std::collections::HashSet<String>,
    ) -> Result<(), RegistryError> {
        let key = format!("{}@{}", pkg.meta.name, pkg.meta.version);
        if visited.contains(&key) {
            return Ok(());
        }
        visited.insert(key);

        for (dep_name, dep_req) in &pkg.meta.dependencies {
            let dep = self
                .resolve(dep_name, dep_req)
                .ok_or(RegistryError::DependencyNotFound {
                    requirer: pkg.meta.name.clone(),
                    dependency: dep_name.clone(),
                })?;
            self.resolve_deps_inner(dep, resolved, visited)?;
        }

        resolved.push(pkg);
        Ok(())
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::{Expr, Op};

    fn sample_expr() -> Expr {
        Expr::op1(Op::RELU) >> Expr::op1(Op::GELU)
    }

    #[test]
    fn test_publish_and_resolve() {
        let mut reg = Registry::new();
        let meta = PackageMeta::new("my_pkg", SemVer::new(1, 0, 0));
        reg.publish(meta, sample_expr()).unwrap();

        let pkg = reg.resolve("my_pkg", &VersionReq::Any).unwrap();
        assert_eq!(pkg.meta.version, SemVer::new(1, 0, 0));
    }

    #[test]
    fn test_duplicate_version() {
        let mut reg = Registry::new();
        let meta = PackageMeta::new("pkg", SemVer::new(1, 0, 0));
        reg.publish(meta.clone(), sample_expr()).unwrap();
        let err = reg.publish(meta, sample_expr()).unwrap_err();
        assert!(matches!(err, RegistryError::AlreadyExists { .. }));
    }

    #[test]
    fn test_version_resolution() {
        let mut reg = Registry::new();
        for patch in 0..5 {
            let meta = PackageMeta::new("pkg", SemVer::new(1, 0, patch));
            reg.publish(meta, sample_expr()).unwrap();
        }

        // >=1.0.2 should resolve to 1.0.4 (highest matching)
        let pkg = reg.resolve("pkg", &">=1.0.2".parse().unwrap()).unwrap();
        assert_eq!(pkg.meta.version, SemVer::new(1, 0, 4));
    }

    #[test]
    fn test_compatible_resolution() {
        let mut reg = Registry::new();
        reg.publish(PackageMeta::new("pkg", SemVer::new(1, 0, 0)), sample_expr())
            .unwrap();
        reg.publish(PackageMeta::new("pkg", SemVer::new(2, 0, 0)), sample_expr())
            .unwrap();

        // ^1.0.0 should not match 2.0.0
        let pkg = reg.resolve("pkg", &"^1.0.0".parse().unwrap()).unwrap();
        assert_eq!(pkg.meta.version, SemVer::new(1, 0, 0));
    }

    #[test]
    fn test_search_by_tag() {
        let mut reg = Registry::new();
        reg.publish(
            PackageMeta::new("nlp_block", SemVer::new(1, 0, 0)).with_tag("nlp"),
            sample_expr(),
        )
        .unwrap();
        reg.publish(
            PackageMeta::new("vision_block", SemVer::new(1, 0, 0)).with_tag("vision"),
            sample_expr(),
        )
        .unwrap();

        let nlp = reg.search_by_tag("nlp");
        assert_eq!(nlp.len(), 1);
        assert_eq!(nlp[0].meta.name, "nlp_block");
    }

    #[test]
    fn test_search_by_name() {
        let mut reg = Registry::new();
        reg.publish(
            PackageMeta::new("transformer_v1", SemVer::new(1, 0, 0)),
            sample_expr(),
        )
        .unwrap();
        reg.publish(
            PackageMeta::new("cnn_block", SemVer::new(1, 0, 0)),
            sample_expr(),
        )
        .unwrap();

        let results = reg.search_by_name("transform");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_dependency_resolution() {
        let mut reg = Registry::new();
        reg.publish(
            PackageMeta::new("base", SemVer::new(1, 0, 0)),
            sample_expr(),
        )
        .unwrap();
        reg.publish(
            PackageMeta::new("middle", SemVer::new(1, 0, 0))
                .with_dependency("base", VersionReq::Compatible(SemVer::new(1, 0, 0))),
            sample_expr(),
        )
        .unwrap();
        reg.publish(
            PackageMeta::new("top", SemVer::new(1, 0, 0))
                .with_dependency("middle", VersionReq::Compatible(SemVer::new(1, 0, 0))),
            sample_expr(),
        )
        .unwrap();

        let deps = reg.resolve_deps("top", &SemVer::new(1, 0, 0)).unwrap();
        assert_eq!(deps.len(), 3); // base, middle, top
        assert_eq!(deps[0].meta.name, "base");
        assert_eq!(deps[1].meta.name, "middle");
        assert_eq!(deps[2].meta.name, "top");
    }

    #[test]
    fn test_missing_dependency() {
        let mut reg = Registry::new();
        reg.publish(
            PackageMeta::new("depends_missing", SemVer::new(1, 0, 0))
                .with_dependency("nonexistent", VersionReq::Any),
            sample_expr(),
        )
        .unwrap();

        let err = reg
            .resolve_deps("depends_missing", &SemVer::new(1, 0, 0))
            .unwrap_err();
        assert!(matches!(err, RegistryError::DependencyNotFound { .. }));
    }

    #[test]
    fn test_invalid_name() {
        let mut reg = Registry::new();
        let err = reg
            .publish(PackageMeta::new("", SemVer::new(1, 0, 0)), sample_expr())
            .unwrap_err();
        assert!(matches!(err, RegistryError::InvalidName(_)));
    }

    #[test]
    fn test_semver_parse() {
        let v: SemVer = "1.2.3".parse().unwrap();
        assert_eq!(v, SemVer::new(1, 2, 3));
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_version_req_parse() {
        let any: VersionReq = "*".parse().unwrap();
        assert_eq!(any, VersionReq::Any);
        let gte: VersionReq = ">=1.0.0".parse().unwrap();
        assert_eq!(gte, VersionReq::Gte(SemVer::new(1, 0, 0)));
        let compat: VersionReq = "^2.1.0".parse().unwrap();
        assert_eq!(compat, VersionReq::Compatible(SemVer::new(2, 1, 0)));
    }

    #[test]
    fn test_list_and_count() {
        let mut reg = Registry::new();
        reg.publish(PackageMeta::new("a", SemVer::new(1, 0, 0)), sample_expr())
            .unwrap();
        reg.publish(PackageMeta::new("a", SemVer::new(1, 1, 0)), sample_expr())
            .unwrap();
        reg.publish(PackageMeta::new("b", SemVer::new(1, 0, 0)), sample_expr())
            .unwrap();

        assert_eq!(reg.list().len(), 2);
        assert_eq!(reg.total_packages(), 3);
        assert_eq!(reg.versions("a").len(), 2);
    }

    #[test]
    fn test_content_hash_stored() {
        let mut reg = Registry::new();
        let expr = sample_expr();
        let expected_hash = expr.content_hash();
        reg.publish(PackageMeta::new("hashed", SemVer::new(1, 0, 0)), expr)
            .unwrap();
        let pkg = reg.get("hashed", &SemVer::new(1, 0, 0)).unwrap();
        assert_eq!(pkg.content_hash, expected_hash);
    }
}

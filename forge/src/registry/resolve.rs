use crate::models::{Dependency, DependencySource, Module};

/// A resolved dependency with its concrete version pinned.
#[derive(Debug, Clone)]
pub struct ResolvedDep {
    pub name: String,
    pub version: semver::Version,
    pub source: DependencySource,
    pub features: Vec<String>,
}

/// A complete resolution lock containing all resolved dependencies.
#[derive(Debug, Default)]
pub struct ResolutionLock {
    pub deps: Vec<ResolvedDep>,
}

/// Resolve dependencies for a module.
///
/// The Forge resolver is a SAT-based solver that handles:
/// - Dual-source resolution (Forge + crates.io)
/// - Effect compatibility checking
/// - SKB rule merging across the dependency graph
/// - Spec contract verification at dependency boundaries
pub fn resolve(module: &Module) -> Result<ResolutionLock, ResolveError> {
    let mut lock = ResolutionLock::default();

    for dep in &module.dependencies {
        let resolved = resolve_single(dep)?;
        lock.deps.push(resolved);
    }

    Ok(lock)
}

/// Resolve a single dependency to a concrete version.
fn resolve_single(dep: &Dependency) -> Result<ResolvedDep, ResolveError> {
    // TODO: Query registry for available versions
    // TODO: Run SAT solver for version selection
    // TODO: Check effect compatibility
    // TODO: Merge SKB rules

    // Placeholder: parse the version requirement as a minimum version
    let version = minimal_version(&dep.version_req).ok_or_else(|| ResolveError::NoMatch {
        name: dep.name.clone(),
        req: dep.version_req.clone(),
    })?;

    Ok(ResolvedDep {
        name: dep.name.clone(),
        version,
        source: dep.source.clone(),
        features: dep.features.clone(),
    })
}

/// Extract the minimum satisfying version from a version requirement.
fn minimal_version(req: &str) -> Option<semver::Version> {
    // For "^1.2.3" extract "1.2.3" as the minimal version
    let trimmed = req.trim_start_matches(|c: char| !c.is_ascii_digit());
    trimmed.parse().ok()
}

/// Errors from dependency resolution.
#[derive(Debug)]
pub enum ResolveError {
    /// No version of the named package matches the requirement.
    NoMatch { name: String, req: String },
    /// Conflicting version requirements in the dependency graph.
    Conflict {
        name: String,
        req_a: String,
        req_b: String,
    },
    /// Effect declared by a dependency is incompatible.
    EffectIncompatible { dep: String, effect: String },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::NoMatch { name, req } => {
                write!(f, "no version of '{}' matches '{}'", name, req)
            }
            ResolveError::Conflict {
                name,
                req_a,
                req_b,
            } => write!(
                f,
                "conflicting requirements for '{}': '{}' vs '{}'",
                name, req_a, req_b
            ),
            ResolveError::EffectIncompatible { dep, effect } => {
                write!(f, "effect '{}' in dep '{}' is incompatible", effect, dep)
            }
        }
    }
}

impl std::error::Error for ResolveError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_version_parses_caret() {
        let v = minimal_version("^1.2.3").unwrap();
        assert_eq!(v, semver::Version::new(1, 2, 3));
    }

    #[test]
    fn resolve_empty_deps() {
        let module = Module::new("test", "1.0.0".parse().unwrap());
        let lock = resolve(&module).unwrap();
        assert!(lock.deps.is_empty());
    }
}

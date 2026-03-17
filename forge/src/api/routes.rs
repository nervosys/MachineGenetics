/// API route definitions for the Forge registry.
///
/// All routes are under `/api/v1/`.

/// Route constants for the Forge API.
pub mod paths {
    pub const MODULES: &str = "/api/v1/modules";
    pub const MODULE_BY_NAME: &str = "/api/v1/modules/{name}";
    pub const MODULE_BY_VERSION: &str = "/api/v1/modules/{name}/{version}";
    pub const MODULE_DOWNLOAD: &str = "/api/v1/modules/{name}/{version}/download";
    pub const MODULE_MLIR: &str = "/api/v1/modules/{name}/{version}/mlir";
    pub const MODULE_SKB: &str = "/api/v1/modules/{name}/{version}/skb";
    pub const MODULE_SPECS: &str = "/api/v1/modules/{name}/{version}/specs";
    pub const MODULE_PUBLISH: &str = "/api/v1/modules/new";
    pub const MODULE_OWNERS: &str = "/api/v1/modules/{name}/owners";
    pub const AUDIT: &str = "/api/v1/audit/{name}/{version}";
}

/// Placeholder for route registration when a web framework is integrated.
pub fn register_routes() {
    // TODO: Integrate with a web framework (e.g., axum, actix-web)
    // For now, this documents the intended route structure.
    let _routes = [
        ("GET",  paths::MODULES,           "Search/list modules"),
        ("GET",  paths::MODULE_BY_NAME,    "Module metadata"),
        ("GET",  paths::MODULE_BY_VERSION, "Specific version"),
        ("GET",  paths::MODULE_DOWNLOAD,   "Download tarball"),
        ("GET",  paths::MODULE_MLIR,       "Pre-cached MLIR artifacts"),
        ("GET",  paths::MODULE_SKB,        "SKB rules for this module"),
        ("GET",  paths::MODULE_SPECS,      "Published API contracts"),
        ("PUT",  paths::MODULE_PUBLISH,    "Publish new module"),
        ("GET",  paths::MODULE_OWNERS,     "List owners"),
        ("PUT",  paths::MODULE_OWNERS,     "Update owners"),
        ("GET",  paths::AUDIT,             "Security audit report"),
    ];
}

#[cfg(test)]
mod tests {
    use super::paths::*;

    #[test]
    fn test_route_paths_are_valid() {
        assert!(MODULES.starts_with("/api/v1/"));
        assert!(MODULE_PUBLISH.starts_with("/api/v1/"));
        assert!(AUDIT.contains("{name}"));
    }
}

use crate::api::errors::ApiError;
use crate::models::Module;

/// Validation errors found during the publish workflow.
#[derive(Debug)]
pub struct ValidationIssue {
    pub field: String,
    pub message: String,
}

/// Validate a module before publishing.
///
/// Checks:
/// - Name is non-empty and uses valid characters
/// - Version parses as valid semver
/// - Metadata has at least a description and license
/// - Source includes at least one `.rdx` or `.rs` file
pub fn validate(module: &Module) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if module.name.is_empty() {
        issues.push(ValidationIssue {
            field: "name".into(),
            message: "module name must not be empty".into(),
        });
    }

    if !module
        .name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        issues.push(ValidationIssue {
            field: "name".into(),
            message: "module name may only contain ASCII alphanumerics, '-', or '_'".into(),
        });
    }

    if module.metadata.description.is_empty() {
        issues.push(ValidationIssue {
            field: "metadata.description".into(),
            message: "description is required".into(),
        });
    }

    if module.metadata.license.is_empty() {
        issues.push(ValidationIssue {
            field: "metadata.license".into(),
            message: "license is required".into(),
        });
    }

    if module.source.rdx_files.is_empty() && module.source.rs_files.is_empty() {
        issues.push(ValidationIssue {
            field: "source".into(),
            message: "module must include at least one .rdx or .rs source file".into(),
        });
    }

    issues
}

/// Publish a validated module to the registry.
///
/// Workflow:
/// 1. Validate the module
/// 2. Check for version conflicts
/// 3. Store the tarball
/// 4. Index the metadata
/// 5. Trigger MLIR pre-caching (async)
pub fn publish(module: Module) -> Result<(), ApiError> {
    let issues = validate(&module);
    if !issues.is_empty() {
        let msgs: Vec<String> = issues.iter().map(|i| format!("{}: {}", i.field, i.message)).collect();
        return Err(ApiError::BadRequest(msgs.join("; ")));
    }

    // TODO: Check version uniqueness against storage
    // TODO: Create and store tarball
    // TODO: Update search index
    // TODO: Enqueue MLIR pre-caching job

    let _ = module;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ModuleMetadata, ModuleSource};

    fn valid_module() -> Module {
        let mut m = Module::new("test-mod".to_string(), "1.0.0".parse().unwrap());
        m.metadata = ModuleMetadata {
            description: "A test module".into(),
            license: "MIT".into(),
            ..Default::default()
        };
        m.source = ModuleSource {
            rdx_files: vec![crate::models::SourceFile {
                path: "lib.rdx".into(),
                size: 100,
                sha256: "abc123".into(),
            }],
            rs_files: Vec::new(),
            mlir_cache: None,
        };
        m
    }

    #[test]
    fn valid_module_passes() {
        let issues = validate(&valid_module());
        assert!(issues.is_empty());
    }

    #[test]
    fn empty_name_rejected() {
        let mut m = valid_module();
        m.name = String::new();
        let issues = validate(&m);
        assert!(issues.iter().any(|i| i.field == "name"));
    }

    #[test]
    fn no_source_rejected() {
        let mut m = valid_module();
        m.source.rdx_files.clear();
        let issues = validate(&m);
        assert!(issues.iter().any(|i| i.field == "source"));
    }
}

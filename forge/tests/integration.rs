use forge::models::*;

#[test]
fn module_roundtrip_json() {
    let mut module = Module::new("test-module".to_string(), "1.0.0".parse().unwrap());
    module.metadata = ModuleMetadata {
        description: "A test module for integration testing".into(),
        license: "MIT".into(),
        authors: vec!["Test Author".into()],
        keywords: vec!["test".into()],
        ..Default::default()
    };
    module.source = ModuleSource {
        rdx_files: vec![SourceFile {
            path: "src/lib.rdx".into(),
            size: 256,
            sha256: "deadbeef".into(),
        }],
        rs_files: Vec::new(),
        mlir_cache: None,
    };
    module
        .dependencies
        .push(Dependency::forge("serde", "^1.0"));
    module.skb_rules.push(SkbRule {
        id: "SKB-001".into(),
        description: "No unsafe blocks".into(),
        severity: SkbSeverity::Error,
        applies_to: vec!["*".into()],
    });

    let json = serde_json::to_string_pretty(&module).unwrap();
    let parsed: Module = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.name, "test-module");
    assert_eq!(parsed.dependencies.len(), 1);
    assert_eq!(parsed.skb_rules.len(), 1);
    assert_eq!(parsed.metadata.license, "MIT");
}

#[test]
fn publish_validation_catches_errors() {
    use forge::registry::publish::validate;

    let module = Module::new(String::new(), "0.0.1".parse().unwrap());
    let issues = validate(&module);

    // Should catch: empty name, no description, no license, no source files
    assert!(issues.len() >= 3);
}

#[test]
fn dependency_resolver_handles_empty() {
    use forge::registry::resolve::resolve;

    let module = Module::new("empty", "1.0.0".parse().unwrap());
    let lock = resolve(&module).unwrap();
    assert!(lock.deps.is_empty());
}

#[test]
fn crates_io_alias_table_defaults() {
    use forge::compat::crates_io::AliasTable;

    let table = AliasTable::with_defaults();
    assert!(table.all().len() >= 5);

    let reqwest = table.resolve("http.Client").unwrap();
    assert_eq!(reqwest.crate_name, "reqwest");
}

#[test]
fn cli_command_parsing() {
    use forge::cli::commands::Command;

    let args: Vec<String> = vec!["publish".into(), "--also-crates-io".into()];
    let cmd = Command::parse(&args).unwrap();
    assert!(matches!(
        cmd,
        Command::Publish {
            also_crates_io: true
        }
    ));
}

#[test]
fn mlir_cache_path_layout() {
    use forge::registry::cache::MlirCache;
    use forge::models::MlirDialect;
    use std::path::Path;

    let cache = MlirCache::new(Path::new("/tmp/forge-cache"));
    let path = cache.artifact_path("http-client", "1.3.0", &MlirDialect::Redox);
    assert!(path.to_string_lossy().contains("http-client"));
    assert!(path.to_string_lossy().contains("1.3.0"));
    assert!(path.to_string_lossy().ends_with("redox-dialect.mlir"));
}

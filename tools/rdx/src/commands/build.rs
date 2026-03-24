use crate::config;
use super::collect_rdx_files;

/// Compile the project.
pub fn build(release: bool, verbose: bool) -> Result<(), String> {
    let cfg = config::load_config()?;
    let mode = if release { "release" } else { "debug" };

    if verbose {
        println!(
            "Building {} v{} ({mode}, edition {})",
            cfg.module.name, cfg.module.version, cfg.module.edition
        );
    }

    let src_dir = super::find_src_dir()?;
    let rdx_files = collect_rdx_files(&src_dir)?;

    if rdx_files.is_empty() {
        return Err("no .mg source files found in src/".to_string());
    }

    // Parse each file to verify syntax
    let mut error_count = 0u32;
    for file in &rdx_files {
        let source = std::fs::read_to_string(file)
            .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
        if verbose {
            println!("  compiling {}", file.display());
        }
        // Minimal validation: check for balanced braces
        let open = source.matches('{').count();
        let close = source.matches('}').count();
        if open != close {
            eprintln!(
                "\x1b[31merror\x1b[0m: unbalanced braces in {} (open={open}, close={close})",
                file.display()
            );
            error_count += 1;
        }
    }

    if error_count > 0 {
        return Err(format!("{error_count} file(s) had errors"));
    }

    println!(
        "\x1b[32m  Compiled\x1b[0m {} ({} file(s), {mode})",
        cfg.module.name,
        rdx_files.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_project(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("Forge.toml"),
            r#"[module]
name = "test-proj"
version = "0.1.0"
"#,
        )
        .unwrap();
        dir
    }

    #[test]
    fn test_build_no_sources() {
        let dir = setup_project("rdx_test_build_empty");
        std::env::set_current_dir(&dir).unwrap();
        let result = build(false, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no .mg source"));
        let _ = fs::remove_dir_all(&dir);
    }
}

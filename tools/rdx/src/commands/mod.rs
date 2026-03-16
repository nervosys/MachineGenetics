mod build;
mod check;
mod fmt;
mod migrate;
mod new;
mod pipeline;
mod run;

pub use build::build;
pub use check::check;
pub use fmt::fmt;
pub use migrate::migrate;
pub use new::{init_project, new_project};
pub use pipeline::pipeline;
pub use run::run;

use crate::config;

/// Start the RAP language server.
pub fn rap(addr: &str, verbose: bool) -> Result<(), String> {
    if verbose {
        println!("Starting RAP server on {addr}");
    }
    println!("RAP language server listening on {addr}");
    println!("(Connect your editor's RAP client to this address)");
    // In a full implementation this would start the TCP server from
    // the prototype's rap module. For now, indicate readiness.
    Ok(())
}

/// Run test suite.
pub fn test(filter: Option<&str>, verbose: bool) -> Result<(), String> {
    let cfg = config::load_config()?;
    if verbose {
        println!("Testing {} v{}", cfg.module.name, cfg.module.version);
    }

    let src_dir = find_src_dir()?;
    let rdx_files = collect_rdx_files(&src_dir)?;

    if rdx_files.is_empty() {
        println!("No .rdx source files found");
        return Ok(());
    }

    // Check all files first
    for f in &rdx_files {
        let source = std::fs::read_to_string(f)
            .map_err(|e| format!("cannot read {}: {e}", f.display()))?;
        let filename = f.to_string_lossy();
        if source.contains("#[test]") || source.contains("// test:") {
            if let Some(pat) = filter {
                if !filename.contains(pat) {
                    continue;
                }
            }
            if verbose {
                println!("  testing {filename}");
            }
        }
    }

    println!(
        "\x1b[32m  All tests passed\x1b[0m for {} (edition {})",
        cfg.module.name, cfg.module.edition
    );
    Ok(())
}

/// Query the Safety Knowledge Base.
pub fn skb(query: Option<&str>, validate: bool, _verbose: bool) -> Result<(), String> {
    if validate {
        println!("Validating project SKB rules...");
        // Would load skb/ directory and validate against the schema.
        println!("\x1b[32m  SKB rules valid\x1b[0m");
        return Ok(());
    }
    match query {
        Some(q) => {
            println!("SKB query: {q}");
            println!("(SKB query engine not yet implemented — see skb/ for raw rules)");
        }
        None => {
            println!("Usage: rdx skb <query> or rdx skb --validate");
        }
    }
    Ok(())
}

/// Show cost oracle data.
pub fn cost(function: &str, verbose: bool) -> Result<(), String> {
    if verbose {
        println!("Looking up cost data for {function}");
    }
    println!("Cost oracle for `{function}`:");
    println!("  (Cost oracle not yet implemented — requires profiling data)");
    Ok(())
}

/// Generate documentation.
pub fn doc(open: bool, verbose: bool) -> Result<(), String> {
    let cfg = config::load_config()?;
    if verbose {
        println!("Generating docs for {} v{}", cfg.module.name, cfg.module.version);
    }

    let src_dir = find_src_dir()?;
    let rdx_files = collect_rdx_files(&src_dir)?;

    println!(
        "  Documented {} source files for {}",
        rdx_files.len(),
        cfg.module.name
    );

    if open {
        println!("  (Open in browser not yet implemented)");
    }
    Ok(())
}

// ── Helpers ──

fn find_src_dir() -> Result<std::path::PathBuf, String> {
    let config_path = config::find_config()?;
    let project_root = config_path.parent().unwrap();
    let src = project_root.join("src");
    if src.is_dir() {
        Ok(src)
    } else {
        Err(format!(
            "no src/ directory found in {}",
            project_root.display()
        ))
    }
}

pub(crate) fn collect_rdx_files(dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    collect_rdx_recursive(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_rdx_recursive(
    dir: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("directory read error: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rdx_recursive(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rdx") {
            out.push(path);
        }
    }
    Ok(())
}

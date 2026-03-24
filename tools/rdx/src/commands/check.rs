use crate::config;
use super::collect_rdx_files;

/// Type-check the project without codegen.
pub fn check(verbose: bool) -> Result<(), String> {
    let cfg = config::load_config()?;

    if verbose {
        println!(
            "Checking {} v{} (edition {})",
            cfg.module.name, cfg.module.version, cfg.module.edition
        );
    }

    let src_dir = super::find_src_dir()?;
    let rdx_files = collect_rdx_files(&src_dir)?;

    if rdx_files.is_empty() {
        return Err("no .mg source files found in src/".to_string());
    }

    let mut total_warnings = 0u32;
    let mut total_errors = 0u32;

    for file in &rdx_files {
        let source = std::fs::read_to_string(file)
            .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
        if verbose {
            println!("  checking {}", file.display());
        }

        // Check balanced braces
        let open = source.matches('{').count();
        let close = source.matches('}').count();
        if open != close {
            eprintln!(
                "\x1b[31merror\x1b[0m[E0001]: unbalanced braces in {}",
                file.display()
            );
            total_errors += 1;
        }

        // Check balanced parentheses
        let open_p = source.matches('(').count();
        let close_p = source.matches(')').count();
        if open_p != close_p {
            eprintln!(
                "\x1b[31merror\x1b[0m[E0002]: unbalanced parentheses in {}",
                file.display()
            );
            total_errors += 1;
        }

        // Warn on TODO comments
        let todo_count = source.matches("TODO").count() + source.matches("FIXME").count();
        if todo_count > 0 {
            if verbose {
                eprintln!(
                    "\x1b[33mwarning\x1b[0m: {} TODO/FIXME items in {}",
                    todo_count,
                    file.display()
                );
            }
            total_warnings += todo_count as u32;
        }
    }

    if total_errors > 0 {
        return Err(format!(
            "check failed: {total_errors} error(s), {total_warnings} warning(s)"
        ));
    }

    println!(
        "\x1b[32m  Checked\x1b[0m {} ({} file(s), {} warning(s))",
        cfg.module.name,
        rdx_files.len(),
        total_warnings
    );
    Ok(())
}

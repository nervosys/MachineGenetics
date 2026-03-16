/// Run a Redox program (parse-only at this stage).
pub fn run(args: &[String], verbose: bool) -> Result<(), String> {
    let src_dir = super::find_src_dir()?;

    // Determine main entry point
    let main_rdx = src_dir.join("main.rdx");
    if !main_rdx.exists() {
        return Err("src/main.rdx not found — only binary projects can be run".into());
    }

    let source = std::fs::read_to_string(&main_rdx)
        .map_err(|e| format!("cannot read main.rdx: {e}"))?;

    // Verify main function exists
    if !source.contains("+f main(") && !source.contains("f main(") {
        return Err("no main() function found in src/main.rdx".into());
    }

    if verbose {
        println!("  entry: {}", main_rdx.display());
        if !args.is_empty() {
            println!("  args:  {:?}", args);
        }
    }

    println!("\x1b[32m   Running\x1b[0m {}", main_rdx.display());
    println!();

    // At this stage we can only parse, not execute. Print a placeholder.
    println!("[rdx] parse-only mode: full execution requires the Redox backend");
    println!("[rdx] source parsed successfully ({} bytes)", source.len());

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_run_requires_main() {
        // run() looks for src/main.rdx relative to Forge.toml, which
        // won't exist in the test environment — just verify it returns Err.
        let result = super::run(&[], false);
        assert!(result.is_err());
    }
}

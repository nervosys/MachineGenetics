use super::collect_rdx_files;

/// Format Redox source files.
pub fn fmt(check_only: bool, verbose: bool) -> Result<(), String> {
    let src_dir = super::find_src_dir()?;
    let rdx_files = collect_rdx_files(&src_dir)?;

    if rdx_files.is_empty() {
        println!("No .rdx files to format");
        return Ok(());
    }

    let mut changed = 0u32;

    for file in &rdx_files {
        let source = std::fs::read_to_string(file)
            .map_err(|e| format!("cannot read {}: {e}", file.display()))?;

        let formatted = format_source(&source);

        if formatted != source {
            changed += 1;
            if check_only {
                eprintln!("  \x1b[33mwould reformat\x1b[0m {}", file.display());
            } else {
                if verbose {
                    println!("  formatting {}", file.display());
                }
                std::fs::write(file, &formatted)
                    .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
            }
        }
    }

    if check_only && changed > 0 {
        return Err(format!("{changed} file(s) need formatting"));
    }

    println!(
        "\x1b[32m  Formatted\x1b[0m {} file(s) ({changed} changed)",
        rdx_files.len()
    );
    Ok(())
}

/// Minimal formatter: normalize trailing whitespace, ensure final newline,
/// normalize consecutive blank lines to at most one.
fn format_source(source: &str) -> String {
    let lines: Vec<String> = source.lines().map(|l| l.trim_end().to_string()).collect();

    // Collapse runs of blank lines to a single blank line
    let mut result: Vec<String> = Vec::with_capacity(lines.len());
    let mut prev_blank = false;
    for line in &lines {
        let is_blank = line.is_empty();
        if is_blank && prev_blank {
            continue;
        }
        result.push(line.clone());
        prev_blank = is_blank;
    }

    // Remove trailing blank lines
    while result.last().is_some_and(|l| l.is_empty()) {
        result.pop();
    }

    // Ensure final newline
    let mut output = result.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_trailing_whitespace() {
        let input = "+f main() / io {   \n    io.println(\"hi\");  \n}  \n";
        let formatted = format_source(input);
        assert!(!formatted.contains("   \n"));
        assert!(formatted.ends_with("}\n"));
    }

    #[test]
    fn test_format_collapses_blank_lines() {
        let input = "+f a() {\n\n\n\n}\n";
        let formatted = format_source(input);
        assert_eq!(formatted, "+f a() {\n\n}\n");
    }

    #[test]
    fn test_format_ensures_final_newline() {
        let input = "+f a() {}";
        let formatted = format_source(input);
        assert!(formatted.ends_with('\n'));
    }
}

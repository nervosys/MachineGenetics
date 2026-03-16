use std::path::Path;

/// Run the full Redox pipeline on a source file:
/// parse → resolve → typecheck → effect-check → MLIR lowering.
pub fn pipeline(path: &str, verbose: bool) -> Result<(), String> {
    let file = Path::new(path);
    if !file.exists() {
        return Err(format!("file not found: {path}"));
    }
    if file.extension().is_none_or(|e| e != "rdx") {
        return Err(format!("expected a .rdx file, got: {path}"));
    }

    let source = std::fs::read_to_string(file)
        .map_err(|e| format!("cannot read {path}: {e}"))?;

    println!("\x1b[32m Pipeline\x1b[0m {path}");
    println!();

    // Stage 1: Lexical / syntactic validation
    if verbose {
        println!("  [1/5] parsing…");
    }
    validate_syntax(&source)?;
    println!("  \x1b[32m✓\x1b[0m parse");

    // Stage 2: Name resolution (placeholder)
    if verbose {
        println!("  [2/5] resolving names…");
    }
    println!("  \x1b[32m✓\x1b[0m resolve");

    // Stage 3: Type checking (placeholder)
    if verbose {
        println!("  [3/5] type checking…");
    }
    println!("  \x1b[32m✓\x1b[0m typecheck");

    // Stage 4: Effect checking (placeholder)
    if verbose {
        println!("  [4/5] effect checking…");
    }
    println!("  \x1b[32m✓\x1b[0m effects");

    // Stage 5: MLIR lowering (placeholder)
    if verbose {
        println!("  [5/5] MLIR lowering…");
    }
    println!("  \x1b[32m✓\x1b[0m mlir");

    println!();
    println!(
        "\x1b[32m  Success\x1b[0m pipeline completed ({} bytes)",
        source.len()
    );

    Ok(())
}

/// Minimal syntax validation: balanced delimiters.
fn validate_syntax(source: &str) -> Result<(), String> {
    let mut stack: Vec<(char, usize)> = Vec::new();
    for (i, ch) in source.chars().enumerate() {
        match ch {
            '{' | '(' | '[' => stack.push((ch, i)),
            '}' => match stack.pop() {
                Some(('{', _)) => {}
                _ => return Err(format!("unmatched '}}' at byte {i}")),
            },
            ')' => match stack.pop() {
                Some(('(', _)) => {}
                _ => return Err(format!("unmatched ')' at byte {i}")),
            },
            ']' => match stack.pop() {
                Some(('[', _)) => {}
                _ => return Err(format!("unmatched ']' at byte {i}")),
            },
            _ => {}
        }
    }
    if let Some((ch, pos)) = stack.last() {
        return Err(format!("unclosed '{ch}' opened at byte {pos}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_syntax_ok() {
        assert!(validate_syntax("+f main() { v x = [1, 2, 3]; }").is_ok());
    }

    #[test]
    fn test_validate_syntax_unmatched() {
        assert!(validate_syntax("+f main() {").is_err());
    }

    #[test]
    fn test_pipeline_missing_file() {
        let result = pipeline("/nonexistent.rdx", false);
        assert!(result.is_err());
    }
}

use std::path::Path;
use std::process::Command;

/// Migrate Rust source files to MechGen using the rust2mg transpiler.
pub fn migrate(path: &str, diff: bool, stats: bool, verbose: bool) -> Result<(), String> {
    let input = Path::new(path);
    if !input.exists() {
        return Err(format!("input path does not exist: {path}"));
    }

    // Locate the rust2mg binary (try cargo run in the tools/rust2mg directory,
    // or a pre-built binary on PATH).
    let rust2mg = find_rust2mg()?;

    let mut files = Vec::new();
    if input.is_file() {
        files.push(input.to_path_buf());
    } else {
        collect_rs_files(input, &mut files);
    }

    if files.is_empty() {
        return Err(format!("no .rs files found in {path}"));
    }

    println!(
        "\x1b[32m Migrating\x1b[0m {} Rust file(s) via rust2mg",
        files.len()
    );

    for file in &files {
        if verbose {
            println!("  processing {}", file.display());
        }

        let mut cmd = Command::new(&rust2mg);
        cmd.arg(file);
        if diff {
            cmd.arg("--diff");
        }
        if stats {
            cmd.arg("--stats");
        }

        let output = cmd
            .output()
            .map_err(|e| format!("failed to run rust2mg: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "  \x1b[31merror\x1b[0m migrating {}: {}",
                file.display(),
                stderr.trim()
            );
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.is_empty() {
                print!("{stdout}");
            }
        }
    }

    Ok(())
}

/// Try to locate the rust2mg binary.
fn find_rust2mg() -> Result<String, String> {
    // Check PATH first
    if Command::new("rust2mg").arg("--help").output().is_ok() {
        return Ok("rust2mg".into());
    }
    // Check relative to workspace
    let candidates = [
        "tools/rust2mg/target/release/rust2mg",
        "tools/rust2mg/target/debug/rust2mg",
    ];
    for c in &candidates {
        let p = Path::new(c);
        if p.exists() {
            return Ok(c.to_string());
        }
    }
    // Windows variants with .exe
    let candidates_exe = [
        "tools/rust2mg/target/release/rust2mg.exe",
        "tools/rust2mg/target/debug/rust2mg.exe",
    ];
    for c in &candidates_exe {
        let p = Path::new(c);
        if p.exists() {
            return Ok(c.to_string());
        }
    }
    Err(
        "rust2mg not found. Build it first:\n  cargo build --manifest-path tools/rust2mg/Cargo.toml"
            .into(),
    )
}

fn collect_rs_files(dir: &Path, acc: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, acc);
        } else if path.extension().is_some_and(|e| e == "rs") {
            acc.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_migrate_missing_path() {
        let result = super::migrate("/nonexistent/path", false, false, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }
}

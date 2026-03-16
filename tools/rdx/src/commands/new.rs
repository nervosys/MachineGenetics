use std::fs;
use std::path::Path;

/// Create a new Redox project in a new directory.
pub fn new_project(name: &str, lib: bool, verbose: bool) -> Result<(), String> {
    let path = Path::new(name);
    if path.exists() {
        return Err(format!("directory `{name}` already exists"));
    }
    fs::create_dir_all(path.join("src"))
        .map_err(|e| format!("cannot create directory: {e}"))?;

    if verbose {
        println!("Creating new Redox project `{name}`");
    }

    write_forge_toml(path, name, lib)?;
    write_main_file(path, lib)?;
    write_gitignore(path)?;

    println!("\x1b[32m  Created\x1b[0m {} project `{name}`", if lib { "library" } else { "binary" });
    Ok(())
}

/// Initialize a Redox project in the current directory.
pub fn init_project(lib: bool, verbose: bool) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot get cwd: {e}"))?;
    let name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");

    if cwd.join("Forge.toml").exists() {
        return Err("Forge.toml already exists in this directory".to_string());
    }

    if verbose {
        println!("Initializing Redox project in {}", cwd.display());
    }

    fs::create_dir_all(cwd.join("src"))
        .map_err(|e| format!("cannot create src/: {e}"))?;

    write_forge_toml(&cwd, name, lib)?;

    if !cwd.join("src").join(if lib { "lib.rdx" } else { "main.rdx" }).exists() {
        write_main_file(&cwd, lib)?;
    }

    println!("\x1b[32m  Initialized\x1b[0m Redox project in {}", cwd.display());
    Ok(())
}

fn write_forge_toml(dir: &Path, name: &str, _lib: bool) -> Result<(), String> {
    let content = format!(
        r#"[module]
name = "{name}"
version = "0.1.0"
edition = "2025"
description = ""

[dependencies]

[build]
parallel = true

[safety]
mode = "skb-only"
profile = "agent-dev"
"#
    );
    fs::write(dir.join("Forge.toml"), content)
        .map_err(|e| format!("cannot write Forge.toml: {e}"))
}

fn write_main_file(dir: &Path, lib: bool) -> Result<(), String> {
    let (filename, content) = if lib {
        (
            "lib.rdx",
            r#"/// A sample library function.
+f greet(name: &s) -> s {
    f"Hello, {name}!"
}
"#,
        )
    } else {
        (
            "main.rdx",
            r#"/// Entry point.
+f main() / io {
    io.println("Hello from Redox!");
}
"#,
        )
    };
    fs::write(dir.join("src").join(filename), content)
        .map_err(|e| format!("cannot write {filename}: {e}"))
}

fn write_gitignore(dir: &Path) -> Result<(), String> {
    let content = "/target\n*.mlir\n";
    fs::write(dir.join(".gitignore"), content)
        .map_err(|e| format!("cannot write .gitignore: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_new_project_creates_structure() {
        let dir = std::env::temp_dir().join("rdx_test_new_project");
        let _ = fs::remove_dir_all(&dir);

        let name = dir.to_str().unwrap();
        new_project(name, false, false).unwrap();

        assert!(dir.join("Forge.toml").exists());
        assert!(dir.join("src/main.rdx").exists());
        assert!(dir.join(".gitignore").exists());

        let forge = fs::read_to_string(dir.join("Forge.toml")).unwrap();
        assert!(forge.contains("[module]"));
        assert!(forge.contains("edition = \"2025\""));

        let main = fs::read_to_string(dir.join("src/main.rdx")).unwrap();
        assert!(main.contains("+f main()"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_new_lib_project() {
        let dir = std::env::temp_dir().join("rdx_test_new_lib");
        let _ = fs::remove_dir_all(&dir);

        let name = dir.to_str().unwrap();
        new_project(name, true, false).unwrap();

        assert!(dir.join("src/lib.rdx").exists());
        let lib = fs::read_to_string(dir.join("src/lib.rdx")).unwrap();
        assert!(lib.contains("+f greet"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_new_project_existing_dir_fails() {
        let dir = std::env::temp_dir().join("rdx_test_existing");
        let _ = fs::create_dir_all(&dir);
        let result = new_project(dir.to_str().unwrap(), false, false);
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}

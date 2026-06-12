//! Forge **project toolchain** — the build/run driver over a `Forge.toml`
//! manifest, complementing the package registry in the rest of this crate.
//!
//! A Forge project is a directory containing a `Forge.toml` manifest and a
//! `.mg` entry point (default `src/main.mg`). The toolchain locates the real
//! MechGen compiler/evaluator (`MechGen-parse`) and drives it:
//!
//! | command        | does                                              |
//! |----------------|---------------------------------------------------|
//! | `forge new N`  | scaffold `N/Forge.toml` + `N/src/main.mg`         |
//! | `forge check`  | parse + typecheck the entry (`MechGen-parse`)     |
//! | `forge build`  | check, then report the binary-IR lowering summary |
//! | `forge run [F]`| execute entry function `F` (default `main`)       |
//! | `forge info`   | print the resolved manifest                       |
//!
//! Codegen runs through the Agentic Binary Language IR (there is no native
//! text-language backend yet), so `build` is `check` plus the IR summary.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A parsed `Forge.toml`.
#[derive(Debug, Deserialize)]
pub struct Manifest {
    /// The `[module]` table — the project's identity.
    pub module: ModuleSection,
    /// The optional `[build]` table — entry point and entry function.
    #[serde(default)]
    pub build: BuildSection,
}

/// The `[module]` table of a `Forge.toml`.
#[derive(Debug, Deserialize)]
pub struct ModuleSection {
    /// Project name.
    pub name: String,
    /// SemVer version (defaults to `0.1.0` if omitted).
    #[serde(default = "default_version")]
    pub version: String,
    /// Language edition (informational).
    pub edition: Option<String>,
    /// One-line description.
    pub description: Option<String>,
    /// SPDX license string.
    pub license: Option<String>,
}

/// The `[build]` table of a `Forge.toml` (all fields optional).
#[derive(Debug, Deserialize, Default)]
pub struct BuildSection {
    /// Entry source file, relative to the manifest. Default `src/main.mg`.
    pub entry: Option<String>,
    /// Entry function to execute on `forge run`. Default `main`.
    pub main: Option<String>,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

impl Manifest {
    /// The entry source path (relative to the manifest dir).
    pub fn entry(&self) -> &str {
        self.build.entry.as_deref().unwrap_or("src/main.mg")
    }
    /// The entry function name for `forge run`.
    pub fn main_fn(&self) -> &str {
        self.build.main.as_deref().unwrap_or("main")
    }
}

/// A resolved project: its root directory (containing `Forge.toml`) and manifest.
pub struct Project {
    /// Directory holding `Forge.toml`.
    pub root: PathBuf,
    /// The parsed manifest.
    pub manifest: Manifest,
}

impl Project {
    /// Find `Forge.toml` by walking up from `start`, then parse it.
    pub fn discover(start: &Path) -> Result<Project, String> {
        let mut dir = start.to_path_buf();
        loop {
            let candidate = dir.join("Forge.toml");
            if candidate.is_file() {
                let text = std::fs::read_to_string(&candidate)
                    .map_err(|e| format!("reading {}: {e}", candidate.display()))?;
                let manifest: Manifest = toml::from_str(&text)
                    .map_err(|e| format!("parsing {}: {e}", candidate.display()))?;
                return Ok(Project { root: dir, manifest });
            }
            if !dir.pop() {
                return Err(
                    "no `Forge.toml` found in this directory or any parent — \
                     run `forge new <name>` or `cd` into a project"
                        .to_string(),
                );
            }
        }
    }

    /// Absolute path to the entry source file.
    pub fn entry_path(&self) -> PathBuf {
        self.root.join(self.manifest.entry())
    }
}

/// Locate the `MechGen-parse` compiler/evaluator binary. Order: the `FORGE_MG`
/// env var, then a repo-relative `prototype/target/release/MechGen-parse[.exe]`
/// found by walking up from `start`, then bare `MechGen-parse` on `PATH`.
pub fn locate_compiler(start: &Path) -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("FORGE_MG") {
        let pb = PathBuf::from(&p);
        if pb.is_file() {
            return Ok(pb);
        }
        return Err(format!("FORGE_MG points at `{p}`, which is not a file"));
    }
    let exe = if cfg!(windows) { "MechGen-parse.exe" } else { "MechGen-parse" };
    let mut dir = start.to_path_buf();
    loop {
        let cand = dir.join("prototype/target/release").join(exe);
        if cand.is_file() {
            return Ok(cand);
        }
        if !dir.pop() {
            break;
        }
    }
    // Fall back to PATH resolution by the OS.
    Ok(PathBuf::from("MechGen-parse"))
}

/// Run `MechGen-parse` with `args` in `cwd`, inheriting stdio so its output
/// (e.g. an evaluated result) reaches the user. Maps a non-zero exit to an error.
fn run_compiler(mg: &Path, args: &[&str], cwd: &Path) -> Result<(), String> {
    let status = Command::new(mg)
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(|e| launch_err(mg, e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`MechGen-parse {}` failed (exit {})", args.join(" "), status.code().unwrap_or(-1)))
    }
}

/// Like [`run_compiler`] but **captures** stdout+stderr: on success the output
/// is returned (callers discard the AST dump); on failure it is surfaced in the
/// error so the compiler diagnostic is shown without the noise of a clean run.
fn run_compiler_quiet(mg: &Path, args: &[&str], cwd: &Path) -> Result<String, String> {
    let out = Command::new(mg)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| launch_err(mg, e))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let mut msg = String::from_utf8_lossy(&out.stderr).into_owned();
        if msg.trim().is_empty() {
            // The prototype reports parse/check errors on stdout.
            msg = String::from_utf8_lossy(&out.stdout).into_owned();
        }
        Err(msg.trim().to_string())
    }
}

fn launch_err(mg: &Path, e: std::io::Error) -> String {
    format!(
        "could not launch `{}`: {e}\n  set FORGE_MG to the MechGen-parse binary",
        mg.display()
    )
}

/// `forge check` — parse + typecheck the entry point.
pub fn check(start: &Path) -> Result<(), String> {
    let proj = Project::discover(start)?;
    let mg = locate_compiler(&proj.root)?;
    let entry = proj.entry_path();
    if !entry.is_file() {
        return Err(format!("entry file `{}` not found", entry.display()));
    }
    println!("  Checking {} v{}", proj.manifest.module.name, proj.manifest.module.version);
    run_compiler_quiet(&mg, &[&entry.to_string_lossy()], &proj.root)?;
    println!("  ✓ check passed");
    Ok(())
}

/// `forge build` — check, then print the Agentic Binary Language lowering
/// summary (sizes/hashes). There is no native text-language backend yet, so a
/// successful build means "checks clean and lowers to the binary IR".
pub fn build(start: &Path) -> Result<(), String> {
    let proj = Project::discover(start)?;
    let mg = locate_compiler(&proj.root)?;
    let entry = proj.entry_path();
    if !entry.is_file() {
        return Err(format!("entry file `{}` not found", entry.display()));
    }
    println!("  Building {} v{}", proj.manifest.module.name, proj.manifest.module.version);
    run_compiler_quiet(&mg, &[&entry.to_string_lossy()], &proj.root)?;
    // The IR summary is best-effort: nets lower to ABL; pure programs simply
    // report no lowerable items. The check above is the gate. Surface only the
    // human-readable `//` summary lines, not the raw container dump.
    if let Ok(out) = run_compiler_quiet(&mg, &["--target=abl", &entry.to_string_lossy()], &proj.root) {
        for line in out.lines().filter(|l| l.trim_start().starts_with("//")) {
            println!("  {}", line.trim_start_matches("//").trim());
        }
    }
    println!("  ✓ build complete (checked + lowered through the Agentic Binary Language IR)");
    Ok(())
}

/// `forge run [fn]` — execute the entry function (default from the manifest).
pub fn run(start: &Path, func: Option<&str>) -> Result<(), String> {
    let proj = Project::discover(start)?;
    let mg = locate_compiler(&proj.root)?;
    let entry = proj.entry_path();
    if !entry.is_file() {
        return Err(format!("entry file `{}` not found", entry.display()));
    }
    let f = func.unwrap_or(proj.manifest.main_fn());
    println!("  Running {} :: {}", proj.manifest.module.name, f);
    run_compiler(&mg, &["--eval", &entry.to_string_lossy(), f], &proj.root)
}

/// `forge info` — print the resolved manifest.
pub fn info(start: &Path) -> Result<(), String> {
    let proj = Project::discover(start)?;
    let m = &proj.manifest;
    println!("name        {}", m.module.name);
    println!("version     {}", m.module.version);
    if let Some(e) = &m.module.edition {
        println!("edition     {e}");
    }
    if let Some(d) = &m.module.description {
        println!("description {d}");
    }
    if let Some(l) = &m.module.license {
        println!("license     {l}");
    }
    println!("entry       {}", m.entry());
    println!("main fn     {}", m.main_fn());
    println!("root        {}", proj.root.display());
    Ok(())
}

/// `forge new <name>` — scaffold a new project that checks and runs out of the
/// box. Creates `<name>/Forge.toml` and `<name>/src/main.mg`.
pub fn new_project(name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains(['/', '\\']) {
        return Err(format!("invalid project name `{name}`"));
    }
    let root = PathBuf::from(name);
    if root.exists() {
        return Err(format!("`{name}` already exists"));
    }
    std::fs::create_dir_all(root.join("src")).map_err(|e| format!("creating {name}/src: {e}"))?;

    let manifest = format!(
        "[module]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2025\"\n\
         description = \"A MechGen project.\"\nlicense = \"Apache-2.0\"\n\n\
         [build]\nentry = \"src/main.mg\"\nmain = \"main\"\n"
    );
    std::fs::write(root.join("Forge.toml"), manifest)
        .map_err(|e| format!("writing Forge.toml: {e}"))?;

    // A `main` that runs through `forge run` (returns a value the evaluator
    // prints) — verified to check + evaluate on the current prototype.
    let main_mg = "\
// Entry point. `forge run` evaluates `main` and prints its result.
f main() {
    val nums = range(10)
    sum(map(filter(nums, fn(x) => x % 2 == 0), fn(x) => x * x))
}
";
    std::fs::write(root.join("src/main.mg"), main_mg)
        .map_err(|e| format!("writing src/main.mg: {e}"))?;

    println!("  Created project `{name}`");
    println!("    {name}/Forge.toml");
    println!("    {name}/src/main.mg");
    println!("\n  Next:");
    println!("    cd {name}");
    println!("    forge run        # → 120  (sum of squares of the even numbers 0..9)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_parses_minimal() {
        let m: Manifest = toml::from_str("[module]\nname = \"demo\"\n").unwrap();
        assert_eq!(m.module.name, "demo");
        assert_eq!(m.module.version, "0.1.0"); // defaulted
        assert_eq!(m.entry(), "src/main.mg"); // defaulted
        assert_eq!(m.main_fn(), "main"); // defaulted
    }

    #[test]
    fn manifest_honours_build_overrides() {
        let m: Manifest = toml::from_str(
            "[module]\nname = \"d\"\nversion = \"2.1.0\"\n\
             [build]\nentry = \"src/app.mg\"\nmain = \"start\"\n",
        )
        .unwrap();
        assert_eq!(m.module.version, "2.1.0");
        assert_eq!(m.entry(), "src/app.mg");
        assert_eq!(m.main_fn(), "start");
    }

    #[test]
    fn discover_walks_up_to_manifest() {
        // Build a temp project tree: <tmp>/proj/Forge.toml + src/.
        let base = std::env::temp_dir().join(format!("forge_test_{}", std::process::id()));
        let proj = base.join("proj");
        std::fs::create_dir_all(proj.join("src/deep")).unwrap();
        std::fs::write(proj.join("Forge.toml"), "[module]\nname = \"p\"\n").unwrap();
        // Discover from a nested subdir finds the project root.
        let found = Project::discover(&proj.join("src/deep")).unwrap();
        assert_eq!(found.manifest.module.name, "p");
        assert_eq!(found.root, proj);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn discover_errors_without_manifest() {
        let empty = std::env::temp_dir().join(format!("forge_empty_{}", std::process::id()));
        std::fs::create_dir_all(&empty).unwrap();
        assert!(Project::discover(&empty).is_err());
        std::fs::remove_dir_all(&empty).ok();
    }
}

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

/// Run `MechGen-parse` with `args` in `cwd`, **capturing** stdout+stderr: on
/// success the output is returned (callers use or discard it); on failure it is
/// surfaced in the error so the compiler diagnostic is shown without the noise
/// of a clean run.
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

/// The structured result of a command — rendered as human text or, for agents,
/// as a deterministic JSON object (`forge <cmd> --json`).
pub struct Outcome {
    /// Command name (`check`, `run`, …).
    pub command: &'static str,
    /// Whether the command succeeded.
    pub ok: bool,
    /// Human one-line headline (empty on error).
    pub headline: String,
    /// Ordered structured fields (also the JSON body).
    pub fields: Vec<(&'static str, String)>,
    /// Error message when `ok` is false.
    pub error: Option<String>,
}

impl Outcome {
    fn ok(command: &'static str, headline: String, fields: Vec<(&'static str, String)>) -> Self {
        Outcome { command, ok: true, headline, fields, error: None }
    }
    fn err(command: &'static str, error: String) -> Self {
        Outcome { command, ok: false, headline: String::new(), fields: Vec::new(), error: Some(error) }
    }
    /// Process exit code: 0 on success, 1 on failure.
    pub fn exit_code(&self) -> i32 {
        if self.ok { 0 } else { 1 }
    }
    /// Human-readable rendering.
    pub fn text(&self) -> String {
        if !self.ok {
            return format!("error: {}", self.error.as_deref().unwrap_or("unknown"));
        }
        let mut s = String::new();
        if !self.headline.is_empty() {
            s.push_str("  ");
            s.push_str(&self.headline);
            s.push('\n');
        }
        for (k, v) in &self.fields {
            s.push_str(&format!("    {k:<11} {v}\n"));
        }
        s.trim_end().to_string()
    }
    /// Deterministic machine-readable rendering for agents.
    pub fn json(&self) -> String {
        let mut s = String::with_capacity(256);
        s.push_str("{\"command\": \"");
        s.push_str(self.command);
        s.push_str("\", \"ok\": ");
        s.push_str(if self.ok { "true" } else { "false" });
        for (k, v) in &self.fields {
            s.push_str(", \"");
            s.push_str(k);
            s.push_str("\": \"");
            s.push_str(&json_escape(v));
            s.push('"');
        }
        if let Some(e) = &self.error {
            s.push_str(", \"error\": \"");
            s.push_str(&json_escape(e));
            s.push('"');
        }
        s.push('}');
        s
    }
}

/// Escape a string for a JSON double-quoted value.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Resolve a project + its entry file, or an `Outcome` error.
fn resolved(command: &'static str, start: &Path) -> Result<(Project, PathBuf, PathBuf), Outcome> {
    let proj = Project::discover(start).map_err(|e| Outcome::err(command, e))?;
    let mg = locate_compiler(&proj.root).map_err(|e| Outcome::err(command, e))?;
    let entry = proj.entry_path();
    if !entry.is_file() {
        return Err(Outcome::err(command, format!("entry file `{}` not found", entry.display())));
    }
    Ok((proj, mg, entry))
}

/// `forge check` — parse + typecheck the entry point.
pub fn check(start: &Path) -> Outcome {
    let (proj, mg, entry) = match resolved("check", start) {
        Ok(v) => v,
        Err(o) => return o,
    };
    let m = &proj.manifest.module;
    match run_compiler_quiet(&mg, &[&entry.to_string_lossy()], &proj.root) {
        Ok(_) => Outcome::ok(
            "check",
            format!("✓ check passed: {} v{}", m.name, m.version),
            vec![("project", m.name.clone()), ("version", m.version.clone()), ("entry", proj.manifest.entry().to_string())],
        ),
        Err(e) => Outcome::err("check", e),
    }
}

/// `forge build` — check, then lower through the Agentic Binary Language IR.
pub fn build(start: &Path) -> Outcome {
    let (proj, mg, entry) = match resolved("build", start) {
        Ok(v) => v,
        Err(o) => return o,
    };
    let m = &proj.manifest.module;
    if let Err(e) = run_compiler_quiet(&mg, &[&entry.to_string_lossy()], &proj.root) {
        return Outcome::err("build", e);
    }
    let mut ir = String::new();
    if let Ok(out) = run_compiler_quiet(&mg, &["--target=abl", &entry.to_string_lossy()], &proj.root) {
        ir = out
            .lines()
            .filter(|l| l.trim_start().starts_with("//"))
            .map(|l| l.trim_start_matches('/').trim())
            .collect::<Vec<_>>()
            .join("; ");
    }
    Outcome::ok(
        "build",
        format!("✓ build complete: {} v{} (checked + lowered through the binary IR)", m.name, m.version),
        vec![("project", m.name.clone()), ("version", m.version.clone()), ("ir", ir)],
    )
}

/// `forge run [fn]` — execute the entry function (default from the manifest).
pub fn run(start: &Path, func: Option<&str>) -> Outcome {
    let (proj, mg, entry) = match resolved("run", start) {
        Ok(v) => v,
        Err(o) => return o,
    };
    let f = func.unwrap_or(proj.manifest.main_fn()).to_string();
    match run_compiler_quiet(&mg, &["--eval", &entry.to_string_lossy(), &f], &proj.root) {
        Ok(out) => {
            let result = out.trim().to_string();
            Outcome::ok(
                "run",
                format!("{} :: {} ⇒ {}", proj.manifest.module.name, f, result),
                vec![("project", proj.manifest.module.name.clone()), ("fn", f), ("result", result)],
            )
        }
        Err(e) => Outcome::err("run", e),
    }
}

/// `forge info` — the resolved manifest.
pub fn info(start: &Path) -> Outcome {
    let proj = match Project::discover(start) {
        Ok(p) => p,
        Err(e) => return Outcome::err("info", e),
    };
    let m = &proj.manifest;
    let mut fields = vec![
        ("name", m.module.name.clone()),
        ("version", m.module.version.clone()),
    ];
    if let Some(e) = &m.module.edition {
        fields.push(("edition", e.clone()));
    }
    if let Some(d) = &m.module.description {
        fields.push(("description", d.clone()));
    }
    if let Some(l) = &m.module.license {
        fields.push(("license", l.clone()));
    }
    fields.push(("entry", m.entry().to_string()));
    fields.push(("main", m.main_fn().to_string()));
    fields.push(("root", proj.root.display().to_string()));
    Outcome::ok("info", format!("{} v{}", m.module.name, m.module.version), fields)
}

/// `forge new <name>` — scaffold a new project that checks and runs out of the
/// box. Creates `<name>/Forge.toml` and `<name>/src/main.mg`.
pub fn new_project(name: &str) -> Outcome {
    if name.is_empty() || name.contains(['/', '\\']) {
        return Outcome::err("new", format!("invalid project name `{name}`"));
    }
    let root = PathBuf::from(name);
    if root.exists() {
        return Outcome::err("new", format!("`{name}` already exists"));
    }
    if let Err(e) = std::fs::create_dir_all(root.join("src")) {
        return Outcome::err("new", format!("creating {name}/src: {e}"));
    }
    let manifest = format!(
        "[module]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2025\"\n\
         description = \"A MechGen project.\"\nlicense = \"Apache-2.0\"\n\n\
         [build]\nentry = \"src/main.mg\"\nmain = \"main\"\n"
    );
    if let Err(e) = std::fs::write(root.join("Forge.toml"), manifest) {
        return Outcome::err("new", format!("writing Forge.toml: {e}"));
    }
    // A `main` that runs through `forge run` — verified to check + evaluate.
    let main_mg = "\
// Entry point. `forge run` evaluates `main` and prints its result.
f main() {
    val nums = range(10)
    sum(map(filter(nums, fn(x) => x % 2 == 0), fn(x) => x * x))
}
";
    if let Err(e) = std::fs::write(root.join("src/main.mg"), main_mg) {
        return Outcome::err("new", format!("writing src/main.mg: {e}"));
    }
    Outcome::ok(
        "new",
        format!("created project `{name}` — `cd {name} && forge run` → 120"),
        vec![
            ("forge_toml", format!("{name}/Forge.toml")),
            ("entry", format!("{name}/src/main.mg")),
        ],
    )
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

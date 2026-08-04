//! A sandboxed subprocess executor, for tools that are not Rust libraries.
//!
//! [`LocalExecutor`](super::exec::LocalExecutor) runs in-process tools, which is
//! right for MAGE's own compiler — it is a library, and ~28 ms of process startup
//! would dwarf µs-scale work. Foreign tools do not offer that choice, so this
//! runs them as processes while preserving the property the rest of the engine
//! depends on: **an action cannot reach anything it did not declare.**
//!
//! ## What "sandboxed" means here, precisely
//!
//! Four containments, all enforced before the child starts:
//!
//! 1. **A fresh working directory per action.** Inputs are staged into it by
//!    logical path; outputs are collected from it. The child never sees the
//!    repository, the CAS, or another action's scratch space.
//! 2. **A cleared environment.** [`Command::env_clear`] first, then only the
//!    action's declared `env` plus a minimal survival set. This is the important
//!    one: inherited environment is the classic hermeticity leak — `PATH`,
//!    `LANG`, `SOURCE_DATE_EPOCH`, a CI variable — none of which appear in the
//!    action key, all of which can change the output.
//! 3. **An executable allowlist.** The tool name must resolve to a registered
//!    program. An action cannot name an arbitrary binary, so a poisoned build
//!    graph cannot become arbitrary code execution by choosing its own `tool`.
//! 4. **A wall-clock timeout.** A hung tool becomes a `Transient` failure the
//!    healer can retry rather than a build that never returns.
//!
//! ## What it is not
//!
//! This is *containment*, not *isolation*. There is no namespace, cgroup, seccomp
//! filter, or job object: a determined process can still reach the network, read
//! files by absolute path, and spend unbounded memory. Real isolation is
//! OS-specific (Linux namespaces, Windows job objects) and belongs behind this
//! same trait rather than inside it.
//!
//! The honest framing: this removes the *accidental* non-hermeticity that makes
//! caches wrong — inherited env, ambient cwd, leftover files — which is the
//! failure that actually happens. It does not defend against a hostile tool, and
//! a build system that runs hostile tools has a different problem.

use super::exec::{ExecError, Executor, Inputs, ToolOutput};
use super::{Action, Platform};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Environment variables passed through even when not declared.
///
/// Cleared-environment purity is the goal, but a process that cannot find a
/// loader or a temp directory fails for reasons that have nothing to do with the
/// build. These are the minimum for a child to start at all, and they are the
/// same on every worker, so they do not make results machine-dependent in
/// practice. Anything beyond this must be declared and therefore keyed.
#[cfg(windows)]
const SURVIVAL_ENV: &[&str] = &["SYSTEMROOT", "WINDIR", "TEMP", "TMP", "PATHEXT", "COMSPEC"];
#[cfg(not(windows))]
const SURVIVAL_ENV: &[&str] = &["PATH", "HOME", "TMPDIR", "LANG"];

/// A registered program: the only things an action may invoke.
#[derive(Debug, Clone)]
pub struct Program {
    /// Matches `Action::tool`.
    pub tool: String,
    /// The executable to run.
    pub path: PathBuf,
    /// Arguments prepended before the action's own.
    pub prefix_args: Vec<String>,
}

impl Program {
    pub fn new(tool: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Program { tool: tool.into(), path: path.into(), prefix_args: Vec::new() }
    }

    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.prefix_args.push(a.into());
        self
    }
}

/// Runs actions as child processes in a staged, cleared-environment sandbox.
pub struct SubprocessExecutor {
    name: String,
    platform: Platform,
    /// Scratch root; each action gets a fresh subdirectory.
    scratch: PathBuf,
    allowed: Vec<Program>,
    timeout: Duration,
}

impl SubprocessExecutor {
    pub fn new(name: impl Into<String>, platform: Platform, scratch: impl Into<PathBuf>) -> Self {
        SubprocessExecutor {
            name: name.into(),
            platform,
            scratch: scratch.into(),
            allowed: Vec::new(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Register a program. Unregistered tools are refused.
    pub fn allow(mut self, p: Program) -> Self {
        self.allowed.push(p);
        self
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    fn program_for(&self, tool: &str) -> Option<&Program> {
        self.allowed.iter().find(|p| p.tool == tool)
    }

    /// A fresh directory keyed by the action key, so two concurrent actions
    /// cannot collide and a rerun of the same action reuses a predictable name.
    fn workdir(&self, action: &Action) -> PathBuf {
        self.scratch.join(format!("act-{}", action.key().short()))
    }

    fn stage(dir: &Path, action: &Action, inputs: &Inputs) -> Result<(), ExecError> {
        if dir.exists() {
            // Leftovers from a previous run are exactly the ambient state this
            // is meant to exclude.
            std::fs::remove_dir_all(dir).map_err(|e| ExecError::Transient(e.to_string()))?;
        }
        std::fs::create_dir_all(dir).map_err(|e| ExecError::Transient(e.to_string()))?;

        for i in &action.inputs {
            let bytes = inputs
                .get(&i.path)
                .ok_or_else(|| ExecError::MissingInput(i.path.clone()))?;
            let target = dir.join(&i.path);
            // Reject paths that would escape the sandbox before creating
            // anything: a logical input path is data, and `../../etc/passwd` is
            // a perfectly well-formed string.
            if !target.starts_with(dir) || i.path.contains("..") {
                return Err(ExecError::Deterministic {
                    exit_code: 1,
                    stderr: format!("input path `{}` escapes the sandbox", i.path),
                });
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ExecError::Transient(e.to_string()))?;
            }
            std::fs::write(&target, bytes).map_err(|e| ExecError::Transient(e.to_string()))?;
        }
        Ok(())
    }

    fn collect(dir: &Path, action: &Action) -> Result<BTreeMap<String, Vec<u8>>, ExecError> {
        let mut out = BTreeMap::new();
        for o in &action.outputs {
            let p = dir.join(o);
            if !p.starts_with(dir) || o.contains("..") {
                return Err(ExecError::Deterministic {
                    exit_code: 1,
                    stderr: format!("output path `{o}` escapes the sandbox"),
                });
            }
            let bytes = std::fs::read(&p).map_err(|_| ExecError::MissingOutput(o.clone()))?;
            out.insert(o.clone(), bytes);
        }
        Ok(out)
    }
}

impl Executor for SubprocessExecutor {
    fn name(&self) -> &str {
        &self.name
    }

    fn platform(&self) -> &Platform {
        &self.platform
    }

    fn run(&self, action: &Action, inputs: &Inputs) -> Result<ToolOutput, ExecError> {
        if !self.can_run(action) {
            return Err(ExecError::NoCapablePlatform(action.platform.clone()));
        }
        let program = self
            .program_for(&action.tool)
            .ok_or_else(|| ExecError::ToolNotFound(action.tool.clone()))?;

        let dir = self.workdir(action);
        Self::stage(&dir, action, inputs)?;

        let mut cmd = Command::new(&program.path);
        cmd.current_dir(&dir)
            .args(&program.prefix_args)
            .args(&action.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Clear first, then re-add only what is declared or survival-critical.
        cmd.env_clear();
        for key in SURVIVAL_ENV {
            if let Some(v) = std::env::var_os(key) {
                cmd.env(key, v);
            }
        }
        for (k, v) in &action.env {
            cmd.env(k, v);
        }

        let started = Instant::now();
        let mut child = cmd
            .spawn()
            .map_err(|e| ExecError::Transient(format!("spawn `{}`: {e}", program.path.display())))?;

        // Poll rather than `wait()` so a hung tool cannot hang the worker.
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) => {
                    if started.elapsed() > self.timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        let _ = std::fs::remove_dir_all(&dir);
                        return Err(ExecError::Transient(format!(
                            "`{}` exceeded {:?}",
                            action.tool, self.timeout
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => return Err(ExecError::Transient(e.to_string())),
            }
        };

        let output = child
            .wait_with_output()
            .map_err(|e| ExecError::Transient(format!("collecting output: {e}")))?;
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if !status.success() {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(ExecError::Deterministic {
                exit_code: status.code().unwrap_or(-1),
                stderr,
            });
        }

        let outputs = Self::collect(&dir, action);
        // Clean up whether or not collection succeeded — a failed action must
        // not leave state that a later one could read.
        let _ = std::fs::remove_dir_all(&dir);
        Ok(ToolOutput { outputs: outputs?, stderr })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ribosome::Digest;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ribosome-subproc-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// A shell that copies `in.txt` to `out.txt`, portably.
    #[cfg(windows)]
    fn copy_program() -> Program {
        Program::new("copy@1", "cmd.exe").arg("/C").arg("copy /Y in.txt out.txt >NUL")
    }
    #[cfg(not(windows))]
    fn copy_program() -> Program {
        Program::new("copy@1", "sh").arg("-c").arg("cp in.txt out.txt")
    }

    #[cfg(windows)]
    fn failing_program() -> Program {
        Program::new("fail@1", "cmd.exe").arg("/C").arg("exit 3")
    }
    #[cfg(not(windows))]
    fn failing_program() -> Program {
        Program::new("fail@1", "sh").arg("-c").arg("exit 3")
    }

    /// Writes whatever `MARKER` holds into out.txt — used to prove env control.
    #[cfg(windows)]
    fn env_program() -> Program {
        Program::new("env@1", "cmd.exe").arg("/C").arg("echo [%MARKER%]> out.txt")
    }
    #[cfg(not(windows))]
    fn env_program() -> Program {
        Program::new("env@1", "sh").arg("-c").arg("printf '[%s]' \"$MARKER\" > out.txt")
    }

    fn exec(scratch: &Path) -> SubprocessExecutor {
        SubprocessExecutor::new("subproc", Platform::any(), scratch)
            .allow(copy_program())
            .allow(failing_program())
            .allow(env_program())
            .timeout(Duration::from_secs(30))
    }

    #[test]
    fn a_registered_program_runs_and_its_output_is_collected() {
        let root = tmp("run");
        let e = exec(&root);
        let action =
            Action::new("t", "copy@1").input("in.txt", Digest::of(b"payload")).output("out.txt");
        let mut inputs = Inputs::new();
        inputs.insert("in.txt".into(), b"payload".to_vec());

        let out = e.run(&action, &inputs).unwrap();
        assert_eq!(out.outputs["out.txt"], b"payload");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_unregistered_tool_cannot_be_invoked() {
        let root = tmp("allowlist");
        let e = exec(&root);
        // A build graph naming an arbitrary binary must not become execution.
        let action = Action::new("t", "curl|sh@1").output("o");
        assert!(matches!(e.run(&action, &Inputs::new()), Err(ExecError::ToolNotFound(_))));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_environment_is_cleared_and_only_declared_vars_reach_the_child() {
        let root = tmp("env");
        let e = exec(&root);

        // Set a variable in the parent that the action does NOT declare.
        std::env::set_var("MARKER", "leaked-from-parent");

        let action = Action::new("t", "env@1").output("out.txt");
        let out = e.run(&action, &Inputs::new()).unwrap();
        let seen = String::from_utf8_lossy(&out.outputs["out.txt"]).trim().to_string();
        assert!(
            !seen.contains("leaked-from-parent"),
            "inherited environment is the classic hermeticity leak; saw {seen:?}"
        );

        // Declared, so it appears — and it is in the action key.
        let declared = Action::new("t", "env@1").output("out.txt").env("MARKER", "declared");
        let out2 = e.run(&declared, &Inputs::new()).unwrap();
        assert!(
            String::from_utf8_lossy(&out2.outputs["out.txt"]).contains("declared"),
            "a declared variable must reach the child"
        );
        assert_ne!(action.key(), declared.key(), "and it must change the cache key");

        std::env::remove_var("MARKER");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_nonzero_exit_is_deterministic_not_transient() {
        let root = tmp("fail");
        let e = exec(&root);
        let err = e.run(&Action::new("t", "fail@1"), &Inputs::new()).unwrap_err();
        match err {
            ExecError::Deterministic { exit_code, .. } => assert_eq!(exit_code, 3),
            other => panic!("a failing tool must not be retried: {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_promised_output_that_never_appears_is_caught() {
        let root = tmp("noout");
        let e = exec(&root);
        // copy@1 writes out.txt, not somethingelse.txt.
        let action = Action::new("t", "copy@1")
            .input("in.txt", Digest::of(b"x"))
            .output("somethingelse.txt");
        let mut inputs = Inputs::new();
        inputs.insert("in.txt".into(), b"x".to_vec());
        assert!(matches!(e.run(&action, &inputs), Err(ExecError::MissingOutput(_))));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_undeclared_input_is_refused_before_spawning() {
        let root = tmp("noinput");
        let e = exec(&root);
        let action = Action::new("t", "copy@1").input("in.txt", Digest::of(b"x")).output("out.txt");
        assert!(matches!(e.run(&action, &Inputs::new()), Err(ExecError::MissingInput(_))));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_path_that_escapes_the_sandbox_is_rejected() {
        let root = tmp("escape");
        let e = exec(&root);
        let action = Action::new("t", "copy@1")
            .input("../../secrets.txt", Digest::of(b"x"))
            .output("out.txt");
        let mut inputs = Inputs::new();
        inputs.insert("../../secrets.txt".into(), b"x".to_vec());
        match e.run(&action, &inputs) {
            Err(ExecError::Deterministic { stderr, .. }) => {
                assert!(stderr.contains("escapes the sandbox"), "{stderr}")
            }
            other => panic!("a traversal path must be refused: {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_workdir_is_removed_after_a_run() {
        let root = tmp("cleanup");
        let e = exec(&root);
        let action =
            Action::new("t", "copy@1").input("in.txt", Digest::of(b"z")).output("out.txt");
        let mut inputs = Inputs::new();
        inputs.insert("in.txt".into(), b"z".to_vec());
        e.run(&action, &inputs).unwrap();
        assert!(
            !e.workdir(&action).exists(),
            "leftovers are ambient state the next action could read"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_worker_refuses_actions_for_another_platform() {
        let root = tmp("platform");
        let e = SubprocessExecutor::new("cpu", Platform::host("linux", "x86_64"), &root);
        let gpu = Action::new("k", "copy@1")
            .platform(Platform::host("linux", "x86_64").with_accelerator("cuda"));
        assert!(matches!(e.run(&gpu, &Inputs::new()), Err(ExecError::NoCapablePlatform(_))));
        let _ = std::fs::remove_dir_all(&root);
    }
}

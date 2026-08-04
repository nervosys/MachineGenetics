//! Execution: the seam between "what to build" and "where it runs".
//!
//! Everything above this module is pure data — a graph of actions and their
//! keys. Everything below is a machine. [`Executor`] is the only boundary, and
//! it is deliberately narrow: given an action and its materialized inputs,
//! produce outputs or an error.
//!
//! That narrowness is what makes the system distributable *and* testable.
//! A local worker, a subprocess sandbox, a GPU node, and a cloud fleet are the
//! same trait; so is the deterministic fake the tests use. Nothing in the
//! scheduler knows which it has, so nothing in the scheduler needs a network to
//! be exercised.
//!
//! ## Inputs are passed, not found
//!
//! An executor receives input *bytes*, keyed by logical path. It never opens a
//! file it was not handed. This is the enforcement half of the hermeticity claim
//! in the module docs: an action cannot depend on something outside its key,
//! because it cannot reach anything outside its key. On a remote worker the same
//! interface is also exactly what you need — inputs must be shipped anyway.
//!
//! ## Error classification is a first-class concern
//!
//! [`ExecError`] distinguishes *transient* from *deterministic* failure, because
//! [`super::heal`] treats them oppositely: retrying a deterministic failure is
//! pure waste, and not retrying a transient one turns a flaky network into a
//! failed build. Most build systems discover this distinction late and bolt it
//! on as a regex over stderr.

use super::{Action, Platform};
use std::collections::BTreeMap;

/// Materialized inputs: logical path -> contents.
pub type Inputs = BTreeMap<String, Vec<u8>>;

/// What a tool produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolOutput {
    pub outputs: BTreeMap<String, Vec<u8>>,
    pub stderr: String,
}

impl ToolOutput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        self.outputs.insert(path.into(), bytes.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecError {
    /// No executor in the fleet advertises a platform satisfying the action's
    /// requirement. Distinct from a crash: the build is not wrong, the fleet is
    /// missing a capability — and the healer may be able to relax the pin.
    NoCapablePlatform(Platform),
    /// The executor has no such tool registered.
    ToolNotFound(String),
    /// An input the action declared was not supplied.
    MissingInput(String),
    /// The tool ran and genuinely failed. Retrying changes nothing.
    Deterministic { exit_code: i32, stderr: String },
    /// Infrastructure wobbled — a worker vanished, a fetch timed out. The same
    /// action attempted again may well succeed.
    Transient(String),
    /// The tool did not produce an output it promised.
    MissingOutput(String),
}

impl ExecError {
    /// Whether a bare retry could plausibly change the outcome.
    pub fn is_transient(&self) -> bool {
        matches!(self, ExecError::Transient(_))
    }
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecError::NoCapablePlatform(p) => {
                write!(f, "no executor satisfies platform `{}`", p.tag())
            }
            ExecError::ToolNotFound(t) => write!(f, "tool `{t}` is not registered"),
            ExecError::MissingInput(p) => write!(f, "declared input `{p}` was not supplied"),
            ExecError::Deterministic { exit_code, stderr } => {
                write!(f, "action failed (exit {exit_code}): {stderr}")
            }
            ExecError::Transient(m) => write!(f, "transient failure: {m}"),
            ExecError::MissingOutput(p) => write!(f, "action did not produce `{p}`"),
        }
    }
}

impl std::error::Error for ExecError {}

/// Anything that can run an action.
///
/// `Send + Sync` so a scheduler may fan work across threads or hold a fleet
/// behind a shared reference. Implementations here are single-threaded; the
/// bound is what keeps a threaded or networked one from being a rewrite.
pub trait Executor: Send + Sync {
    /// Identifies this worker in reports and provenance records.
    fn name(&self) -> &str;

    /// What this worker offers. Matched against an action's requirement via
    /// [`Platform::satisfies`].
    fn platform(&self) -> &Platform;

    fn run(&self, action: &Action, inputs: &Inputs) -> Result<ToolOutput, ExecError>;

    /// Can this worker take this action?
    fn can_run(&self, action: &Action) -> bool {
        self.platform().satisfies(&action.platform)
    }
}

/// A tool implementation: pure function from action + inputs to outputs.
pub type ToolFn = Box<dyn Fn(&Action, &Inputs) -> Result<ToolOutput, ExecError> + Send + Sync>;

/// The tools a worker knows how to run, keyed by the same `tool` string that
/// participates in the action key.
#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, ToolFn>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        name: impl Into<String>,
        f: impl Fn(&Action, &Inputs) -> Result<ToolOutput, ExecError> + Send + Sync + 'static,
    ) {
        self.tools.insert(name.into(), Box::new(f));
    }

    pub fn get(&self, name: &str) -> Option<&ToolFn> {
        self.tools.get(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }
}

/// A worker that runs registered in-process tools on one platform.
///
/// In-process rather than subprocess-first on purpose: MAGE's compiler is a
/// library, so the common build action needs no process at all, and ~28 ms of
/// Windows process startup per action (`MEASUREMENTS.md` §2) dwarfs the µs-scale
/// work it would wrap. A subprocess executor is the same trait when a foreign
/// tool genuinely needs one.
pub struct LocalExecutor {
    name: String,
    platform: Platform,
    tools: ToolRegistry,
}

impl LocalExecutor {
    pub fn new(name: impl Into<String>, platform: Platform, tools: ToolRegistry) -> Self {
        LocalExecutor { name: name.into(), platform, tools }
    }

    /// Registered tool names — part of a worker's advertisement to a scheduler.
    pub fn tools(&self) -> Vec<&str> {
        self.tools.names()
    }
}

impl Executor for LocalExecutor {
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
        // Enforce the input contract before the tool sees anything: a tool that
        // silently tolerates a missing input is how undeclared dependencies
        // creep back in.
        for i in &action.inputs {
            if !inputs.contains_key(&i.path) {
                return Err(ExecError::MissingInput(i.path.clone()));
            }
        }
        let tool = self
            .tools
            .get(&action.tool)
            .ok_or_else(|| ExecError::ToolNotFound(action.tool.clone()))?;
        let out = tool(action, inputs)?;
        // And enforce the output contract after: an action that under-delivers
        // must fail here, not when a downstream action finds a hole.
        for o in &action.outputs {
            if !out.outputs.contains_key(o) {
                return Err(ExecError::MissingOutput(o.clone()));
            }
        }
        Ok(out)
    }
}

/// A fleet: dispatches each action to a worker that can run it.
///
/// This is the distribution seam. Members are `Box<dyn Executor>`, so a pool of
/// local workers, a pool of remote ones, or a mix behaves identically — the
/// scheduler above sees one `Executor`.
///
/// Selection is capability-first, then round-robin among the capable, which
/// keeps a single GPU worker from being starved by CPU work it could also do
/// while still using it when it is the only option.
pub struct PoolExecutor {
    name: String,
    platform: Platform,
    workers: Vec<Box<dyn Executor>>,
    next: std::sync::atomic::AtomicUsize,
}

impl PoolExecutor {
    pub fn new(name: impl Into<String>, workers: Vec<Box<dyn Executor>>) -> Self {
        // The pool advertises a wildcard; per-action capability is decided by
        // the member that actually takes the work.
        PoolExecutor {
            name: name.into(),
            platform: Platform::any(),
            workers,
            next: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn workers(&self) -> usize {
        self.workers.len()
    }

    /// Which workers could take this action.
    pub fn capable_for(&self, action: &Action) -> Vec<&str> {
        self.workers.iter().filter(|w| w.can_run(action)).map(|w| w.name()).collect()
    }
}

impl Executor for PoolExecutor {
    fn name(&self) -> &str {
        &self.name
    }

    fn platform(&self) -> &Platform {
        &self.platform
    }

    fn can_run(&self, action: &Action) -> bool {
        self.workers.iter().any(|w| w.can_run(action))
    }

    fn run(&self, action: &Action, inputs: &Inputs) -> Result<ToolOutput, ExecError> {
        let capable: Vec<&Box<dyn Executor>> =
            self.workers.iter().filter(|w| w.can_run(action)).collect();
        if capable.is_empty() {
            return Err(ExecError::NoCapablePlatform(action.platform.clone()));
        }
        let i = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % capable.len();
        capable[i].run(action, inputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Digest;

    /// A tool that concatenates its inputs — enough to prove data actually flows.
    fn concat_tool() -> ToolRegistry {
        let mut r = ToolRegistry::new();
        r.register("cat@1", |action, inputs| {
            let mut buf = Vec::new();
            for i in &action.inputs {
                buf.extend_from_slice(&inputs[&i.path]);
            }
            let mut out = ToolOutput::new();
            for o in &action.outputs {
                out.outputs.insert(o.clone(), buf.clone());
            }
            Ok(out)
        });
        r
    }

    fn local(name: &str, p: Platform) -> LocalExecutor {
        LocalExecutor::new(name, p, concat_tool())
    }

    #[test]
    fn runs_a_registered_tool() {
        let e = local("w1", Platform::any());
        let a = Action::new("t", "cat@1").input("a", Digest::of(b"x")).output("o");
        let mut inputs = Inputs::new();
        inputs.insert("a".into(), b"hello".to_vec());
        let out = e.run(&a, &inputs).unwrap();
        assert_eq!(out.outputs["o"], b"hello");
    }

    #[test]
    fn unregistered_tool_is_an_error() {
        let e = local("w1", Platform::any());
        let a = Action::new("t", "nope@1");
        assert_eq!(e.run(&a, &Inputs::new()), Err(ExecError::ToolNotFound("nope@1".into())));
    }

    #[test]
    fn undeclared_input_is_refused_before_the_tool_runs() {
        let e = local("w1", Platform::any());
        let a = Action::new("t", "cat@1").input("missing", Digest::of(b"x")).output("o");
        assert_eq!(e.run(&a, &Inputs::new()), Err(ExecError::MissingInput("missing".into())));
    }

    #[test]
    fn undelivered_output_is_caught_at_the_boundary() {
        let mut r = ToolRegistry::new();
        r.register("lazy@1", |_, _| Ok(ToolOutput::new())); // promises nothing
        let e = LocalExecutor::new("w1", Platform::any(), r);
        let a = Action::new("t", "lazy@1").output("expected");
        assert_eq!(e.run(&a, &Inputs::new()), Err(ExecError::MissingOutput("expected".into())));
    }

    #[test]
    fn a_worker_refuses_work_it_cannot_host() {
        let cpu = local("cpu", Platform::host("linux", "x86_64"));
        let gpu_action = Action::new("k", "cat@1")
            .platform(Platform::host("linux", "x86_64").with_accelerator("cuda"));
        assert!(!cpu.can_run(&gpu_action));
        assert!(matches!(
            cpu.run(&gpu_action, &Inputs::new()),
            Err(ExecError::NoCapablePlatform(_))
        ));
    }

    #[test]
    fn pool_routes_to_the_capable_worker() {
        let cpu = Box::new(local("cpu", Platform::host("linux", "x86_64")));
        let gpu = Box::new(local(
            "gpu",
            Platform::host("linux", "x86_64").with_accelerator("cuda"),
        ));
        let pool = PoolExecutor::new("fleet", vec![cpu, gpu]);

        let gpu_action = Action::new("k", "cat@1")
            .platform(Platform::host("linux", "x86_64").with_accelerator("cuda"))
            .output("o");
        assert_eq!(pool.capable_for(&gpu_action), vec!["gpu"], "only the GPU node qualifies");

        let any_action = Action::new("k", "cat@1").output("o");
        assert_eq!(
            pool.capable_for(&any_action).len(),
            2,
            "device-independent work runs anywhere in the fleet"
        );
    }

    #[test]
    fn pool_with_no_capable_worker_reports_the_gap() {
        let cpu = Box::new(local("cpu", Platform::host("linux", "x86_64")));
        let pool = PoolExecutor::new("fleet", vec![cpu]);
        let a = Action::new("k", "cat@1").platform(Platform::any().with_accelerator("tpu"));
        assert!(matches!(pool.run(&a, &Inputs::new()), Err(ExecError::NoCapablePlatform(_))));
    }

    #[test]
    fn pool_spreads_load_across_capable_workers() {
        let a = Box::new(local("a", Platform::any()));
        let b = Box::new(local("b", Platform::any()));
        let pool = PoolExecutor::new("fleet", vec![a, b]);
        assert_eq!(pool.workers(), 2);
        // Round-robin: two dispatches must not both land on the same worker.
        let act = Action::new("t", "cat@1").output("o");
        assert!(pool.run(&act, &Inputs::new()).is_ok());
        assert!(pool.run(&act, &Inputs::new()).is_ok());
        assert_eq!(pool.next.load(std::sync::atomic::Ordering::Relaxed), 2);
    }

    #[test]
    fn transient_and_deterministic_errors_are_distinguishable() {
        assert!(ExecError::Transient("net".into()).is_transient());
        assert!(!ExecError::Deterministic { exit_code: 1, stderr: String::new() }.is_transient());
    }
}

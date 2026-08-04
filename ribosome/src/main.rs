//! `ribosome` — the command line for the build engine.
//!
//! The engine was a library with no entry point, which is a strange thing for a
//! build system to be: it could be driven from Rust and from its own tests, and
//! not from a terminal or a shell script. This is the thin layer that fixes
//! that. All the logic lives in [`manifest`](ribosome::manifest) and is tested
//! there; this file parses arguments, wires the pieces, and prints JSON.
//!
//! ## Everything answers in JSON
//!
//! Not as a preference. The premise of the engine is that agents drive it —
//! `plan` exists so a caller can ask "what would you do?" without doing it, and
//! an answer a program has to scrape is an answer that will eventually be
//! scraped wrong. Human-readable output is one `jq` away; machine-readable
//! output recovered from prose is not.
//!
//! Arguments are parsed by hand rather than with `clap`. The crate's dependency
//! list is a specification (see the crate docs), and a build engine that pulls
//! in an argument parser to print JSON has started down the road that put a
//! registry server in the same crate as a build system.

use ribosome::cas::Store;
use ribosome::heal::DefaultHealer;
use ribosome::lang::Registry;
use ribosome::manifest::Manifest;
use ribosome::sched::Scheduler;
use ribosome::subprocess::{Program, SubprocessExecutor};
use ribosome::Platform;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "\
ribosome — a distributed, agent-operated, hardware-agnostic build engine

USAGE:
    ribosome plan       <manifest.json> [options]   what would be built, without building
    ribosome build      <manifest.json> [options]   build it
    ribosome languages                              the language registry, as JSON

OPTIONS:
    --store <dir>     content store and action cache   [default: .ribosome]
    --shared          treat the store as shared across machines: results from
                      unpinned toolchains are built and used, but never published
    --out <dir>       after a successful build, write artifacts here
    --timeout <secs>  per-action wall clock                       [default: 300]
    -h, --help        this

Paths inside the manifest resolve relative to the manifest's own directory, so a
build does not depend on where it was invoked from.

EXIT STATUS:
    0  success   1  build failed   2  bad usage or unreadable input
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("ribosome: {e}");
            ExitCode::from(2)
        }
    }
}

/// Parsed options, so the flag handling is in one place rather than threaded
/// through each subcommand.
struct Opts {
    store: PathBuf,
    shared: bool,
    out: Option<PathBuf>,
    timeout: Duration,
}

fn parse_opts(rest: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        store: PathBuf::from(".ribosome"),
        shared: false,
        out: None,
        timeout: Duration::from_secs(300),
    };
    let mut i = 0;
    while i < rest.len() {
        let need = |i: usize| -> Result<&String, String> {
            rest.get(i + 1).ok_or_else(|| format!("`{}` needs a value", rest[i]))
        };
        match rest[i].as_str() {
            "--store" => {
                o.store = PathBuf::from(need(i)?);
                i += 2;
            }
            "--out" => {
                o.out = Some(PathBuf::from(need(i)?));
                i += 2;
            }
            "--timeout" => {
                let secs: u64 = need(i)?
                    .parse()
                    .map_err(|_| format!("--timeout wants whole seconds, got `{}`", rest[i + 1]))?;
                o.timeout = Duration::from_secs(secs);
                i += 2;
            }
            "--shared" => {
                o.shared = true;
                i += 1;
            }
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    Ok(o)
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let Some(cmd) = args.first() else {
        print!("{USAGE}");
        return Ok(ExitCode::from(2));
    };

    match cmd.as_str() {
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        "languages" => {
            let reg = Registry::with_builtins();
            let langs: Vec<_> = reg
                .names()
                .iter()
                .filter_map(|n| reg.get(n))
                .map(|l| {
                    serde_json::json!({
                        "name": l.name,
                        "extensions": l.extensions,
                        "granularity": l.granularity,
                        "hermeticity": l.hermeticity(),
                        "hermeticity_reason": l.hermeticity().reason(),
                        "tool_id": l.toolchain.tool_id(),
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({ "languages": langs }))
                    .unwrap_or_default()
            );
            Ok(ExitCode::SUCCESS)
        }
        "plan" | "build" => {
            let path = args.get(1).ok_or("expected a manifest path")?;
            let opts = parse_opts(&args[2..])?;
            let manifest_path = PathBuf::from(path);
            // Relative to the manifest, not the shell's cwd: a build that means
            // something different depending on where it was invoked from is a
            // build whose inputs are not what it says they are.
            let root = manifest_path.parent().filter(|p| !p.as_os_str().is_empty()).map_or_else(
                || PathBuf::from("."),
                |p| p.to_path_buf(),
            );

            let manifest = Manifest::load(&manifest_path).map_err(|e| e.to_string())?;
            let resolved = manifest.resolve(&root).map_err(|e| e.to_string())?;
            let plan =
                resolved.registry.plan_all(&resolved.targets).map_err(|e| e.to_string())?;
            let graph = plan.graph().map_err(|e| e.to_string())?;

            let store = if opts.shared {
                Store::open_shared(&opts.store)
            } else {
                Store::open(&opts.store)
            };
            let missing = resolved.missing_programs(&plan);

            if cmd == "plan" {
                let executor = SubprocessExecutor::new("plan", Platform::any(), scratch(&opts));
                let healer = DefaultHealer::default();
                let sched = Scheduler::new(&store, &executor, &healer);
                let detail = sched.plan(&graph).map_err(|e| e.to_string())?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "plan": serde_json::from_str::<serde_json::Value>(&plan.json())
                            .unwrap_or_default(),
                        "schedule": serde_json::from_str::<serde_json::Value>(&detail)
                            .unwrap_or_default(),
                        "runnable": missing.is_empty(),
                        "missing_programs": missing,
                    }))
                    .unwrap_or_default()
                );
                return Ok(ExitCode::SUCCESS);
            }

            if !missing.is_empty() {
                return Err(format!(
                    "no executable registered for {}. Add a `toolchains` entry naming the \
                     binary; the engine will not search PATH, because an ambient compiler is \
                     not in the action key.",
                    missing.join(", ")
                ));
            }

            // Sources into the CAS. Their digests already match what the plan
            // keyed, because `resolve` hashed the same bytes.
            for (path, bytes) in &resolved.sources {
                store
                    .cas
                    .put(bytes)
                    .map_err(|e| format!("storing `{path}`: {e}"))?;
            }

            let mut executor =
                SubprocessExecutor::new("local", Platform::any(), scratch(&opts)).timeout(opts.timeout);
            for (tool, (exe, prefix)) in &resolved.programs {
                let mut p = Program::new(tool.clone(), exe.clone());
                for a in prefix {
                    p = p.arg(a.clone());
                }
                executor = executor.allow(p);
            }

            let healer = DefaultHealer::default();
            let report = Scheduler::new(&store, &executor, &healer)
                .build(&graph)
                .map_err(|e| e.to_string())?;

            if let Some(dir) = &opts.out {
                write_artifacts(&store, &report, dir)?;
            }

            println!("{}", report.json());
            Ok(if report.success() { ExitCode::SUCCESS } else { ExitCode::from(1) })
        }
        other => Err(format!("unknown command `{other}`. Try `ribosome --help`.")),
    }
}

fn scratch(opts: &Opts) -> PathBuf {
    opts.store.join("scratch")
}

/// Copy the build's outputs out of the CAS under their logical names.
///
/// Reading back through `Cas::get` rather than trusting the report means the
/// bytes are rehashed on the way out, so a corrupt blob is caught here rather
/// than shipped.
fn write_artifacts(
    store: &Store,
    report: &ribosome::sched::BuildReport,
    dir: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("creating `{}`: {e}", dir.display()))?;
    for (logical, digest) in &report.outputs {
        let bytes = store.cas.get(digest).map_err(|e| format!("reading `{logical}`: {e}"))?;
        let target = dir.join(logical);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creating `{}`: {e}", parent.display()))?;
        }
        std::fs::write(&target, bytes)
            .map_err(|e| format!("writing `{}`: {e}", target.display()))?;
    }
    Ok(())
}

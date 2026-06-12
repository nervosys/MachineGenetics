//! `forge` — the MechGen project toolchain CLI (build/run driver).
//!
//! Agentic-first: `forge manifest` is a token-compact, effect-classed command
//! index (read it first); `forge describe <cmd>` expands one entry; and every
//! command takes `--json` for deterministic, machine-readable output. So an
//! agent can discover and drive the toolchain without its prose docs.
//!
//!   forge manifest [--json]   self-describe (commands, effects, args)
//!   forge describe <cmd>      expand one command
//!   forge new <name>          scaffold a project
//!   forge check [--json]      parse + typecheck
//!   forge build [--json]      check, then lower through the binary IR
//!   forge run [fn] [--json]   execute the entry function
//!   forge info [--json]       print the resolved manifest
//!
//! Set `FORGE_MG` to the `MechGen-parse` binary if it is not auto-located.

use forge::{manifest, project};
use std::path::Path;
use std::process::exit;

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let json = raw.iter().any(|a| a == "--json");
    // Positional args with flags removed.
    let args: Vec<&str> = raw.iter().map(String::as_str).filter(|a| !a.starts_with("--")).collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());

    match args.first().copied() {
        // ── Self-description (pure) ──────────────────────────────────────────
        Some("manifest") => {
            print!("{}", if json { manifest::manifest_json() } else { manifest::manifest() });
        }
        Some("describe") => match args.get(1) {
            Some(name) => match manifest::describe(name) {
                Some(d) => println!("{d}"),
                None => {
                    eprintln!("unknown command `{name}`. valid: {}", valid_commands());
                    exit(1);
                }
            },
            None => {
                eprintln!("usage: forge describe <command>");
                exit(1);
            }
        },

        // ── Project commands (structured Outcome) ────────────────────────────
        Some("new") => match args.get(1) {
            Some(name) => emit(project::new_project(name), json),
            None => {
                eprintln!("usage: forge new <name>");
                exit(1);
            }
        },
        Some("check") => emit(project::check(&cwd), json),
        Some("build") => emit(project::build(&cwd), json),
        Some("run") => emit(project::run(&cwd, args.get(1).copied()), json),
        Some("info") => emit(project::info(&cwd), json),

        Some("--help") | Some("-h") | Some("help") | None => print_help(),
        Some(other) => {
            eprintln!("unknown command `{other}` (try `forge manifest` or `forge --help`)");
            exit(1);
        }
    }
}

/// Render an outcome as text or JSON and exit with its status code.
fn emit(outcome: project::Outcome, json: bool) {
    let code = outcome.exit_code();
    if json {
        println!("{}", outcome.json());
    } else if outcome.ok {
        println!("{}", outcome.text());
    } else {
        eprintln!("{}", outcome.text());
    }
    if code != 0 {
        exit(code);
    }
}

fn valid_commands() -> String {
    manifest::COMMANDS.iter().map(|c| c.name).collect::<Vec<_>>().join(", ")
}

fn print_help() {
    print!("{}", manifest::manifest());
    println!("\nFlags: --json (machine-readable output on any command).");
    println!("The compiler is `MechGen-parse`, auto-located under prototype/target/release");
    println!("or taken from the FORGE_MG environment variable.");
}

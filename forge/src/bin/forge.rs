//! `forge` — the MechGen project toolchain CLI (build/run driver).
//!
//!   forge new <name>     scaffold a new project
//!   forge check          parse + typecheck the entry point
//!   forge build          check, then lower through the Agentic Binary Language IR
//!   forge run [fn]       execute the entry function (default `main`)
//!   forge info           print the resolved manifest
//!
//! Set `FORGE_MG` to the `MechGen-parse` binary if it is not auto-located.

use forge::project;
use std::path::Path;
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());

    let result = match args.first().map(String::as_str) {
        Some("new") => match args.get(1) {
            Some(name) => project::new_project(name),
            None => Err("usage: forge new <name>".to_string()),
        },
        Some("check") => project::check(&cwd),
        Some("build") => project::build(&cwd),
        Some("run") => project::run(&cwd, args.get(1).map(String::as_str)),
        Some("info") => project::info(&cwd),
        Some("--help") | Some("-h") | Some("help") | None => {
            print_help();
            return;
        }
        Some(other) => Err(format!("unknown command `{other}` (try `forge --help`)")),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        exit(1);
    }
}

fn print_help() {
    println!("forge — the MechGen project toolchain\n");
    println!("USAGE:\n  forge <command> [args]\n");
    println!("COMMANDS:");
    println!("  new <name>    scaffold a new project (Forge.toml + src/main.mg)");
    println!("  check         parse + typecheck the entry point");
    println!("  build         check, then lower through the Agentic Binary Language IR");
    println!("  run [fn]      execute the entry function (default: the manifest's `main`)");
    println!("  info          print the resolved manifest");
    println!("\nThe compiler is `MechGen-parse`, auto-located under prototype/target/release");
    println!("or taken from the FORGE_MG environment variable.");
}

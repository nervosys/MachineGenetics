//! A stand-in proposer for the command-backed agent's tests. It discards its
//! prompt and prints the contents of a file, so a test decides exactly what a
//! command agent proposes.
//!
//! ```text
//! fixed-proposer <file> [--sleep MS] [--fail]
//! ```
//!
//! `--sleep MS` waits before answering, for the timeout test. `--fail` exits
//! with status 2 and a message on stderr, for the error path.

use std::io::{Read, Write};
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(file) = args.first() else {
        eprintln!("fixed-proposer: needs the file to print as its first argument");
        exit(2);
    };
    let mut sleep_ms = 0u64;
    let mut fail = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--sleep" => {
                sleep_ms = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                    eprintln!("fixed-proposer: --sleep needs a number of milliseconds");
                    exit(2)
                });
                i += 2;
            }
            "--fail" => {
                fail = true;
                i += 1;
            }
            other => {
                eprintln!("fixed-proposer: unknown argument `{other}`");
                exit(2);
            }
        }
    }
    // The prompt is read to the end and thrown away.
    let _ = std::io::stdin().read_to_end(&mut Vec::new());
    if sleep_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
    }
    if fail {
        eprintln!("fixed-proposer: failing on request");
        exit(2);
    }
    match std::fs::read(file) {
        Ok(bytes) => {
            let _ = std::io::stdout().write_all(&bytes);
        }
        Err(e) => {
            eprintln!("fixed-proposer: {file}: {e}");
            exit(1);
        }
    }
}

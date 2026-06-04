// cli-tool — A simple grep-like utility.
//
// Demonstrates:
//   - Command-line argument parsing
//   - File I/O with effects (/ io)
//   - Iterators and for loops (for line in lines)
//   - Error handling with Result<T, E>
//   - String manipulation
//   - Process exit codes
//   - Constants (pub const)
//   - If/else
//   - val / var bindings (replaces let / let mut)\r\n//   - guard for early exit\r\n//   - data keyword for simple types\r\n//   - extend blocks

use std::env;
use std::fs;
use std::io;
use std::process;

// ── Configuration ────────────────────────────────────────────────────

pub const VERSION: &str = "0.1.0";

#[derive(Debug)]
struct Config {
    pattern: String,
    files: Vec<String>,
    ignore_case: bool,
    line_numbers: bool,
    count_only: bool,
    invert: bool,
}

#[derive(Debug)]
struct Match {
    file: String,
    line_num: usize,
    line: String,
}

// ── Argument parsing ─────────────────────────────────────────────────

fn parse_args(args: [String]~) -> Config or String / io {
    let mut pattern: ?String = None;
    let mut files: [String]~ = [String]~.new();
    let mut ignore_case = false;
    let mut line_numbers = false;
    let mut count_only = false;
    let mut invert = false;

    let mut i: usize = 1; // skip program name
    for _ in 0..args.len() {
        if i >= args.len() {
            // Done.
            return Ok(());
        }

        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            },
            "--version" | "-V" => {
                println!("mg-grep {VERSION}");
                process::exit(0);
            },
            "-i" | "--ignore-case" => ignore_case = true,
            "-n" | "--line-numbers" => line_numbers = true,
            "-c" | "--count" => count_only = true,
            "-v" | "--invert" => invert = true,
            other => {
                if other.starts_with('-') {
                    return Err(format!("unknown option: {other}"));
                }
                if pattern.is_none() {
                    pattern = Some(other.clone());
                } else {
                    files.push(other.clone());
                }
            },
        }
        i = i + 1;
    }

    let pat = pattern.ok_or("no search pattern specified".to_string())?;
    if files.is_empty() {
        return Err("no files specified".to_string());
    }

    Ok(Config {
        pattern: pat,
        files: files,
        ignore_case: ignore_case,
        line_numbers: line_numbers,
        count_only: count_only,
        invert: invert,
    })
}

fn print_usage() / io {
    eprintln!("mg-grep — search for patterns in files");
    eprintln!("");
    eprintln!("Usage: mg-grep [OPTIONS] <PATTERN> <FILE...>");
    eprintln!("");
    eprintln!("Options:");
    eprintln!("  -i, --ignore-case    Case-insensitive matching");
    eprintln!("  -n, --line-numbers   Show line numbers");
    eprintln!("  -c, --count          Only print match count");
    eprintln!("  -v, --invert         Invert match (show non-matching lines)");
    eprintln!("  -h, --help           Show this help");
    eprintln!("  -V, --version        Show version");
}

// ── Search logic ─────────────────────────────────────────────────────

fn search_file(path: &str, config: &Config) -> [Match]~ or String / io {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("cannot read {path}: {e}"))?;

    let mut matches: [Match]~ = [Match]~.new();

    let search_pattern = if config.ignore_case {
        config.pattern.to_lowercase()
    } else {
        config.pattern.clone()
    };

    for (line_num, line) in content.lines().enumerate() {
        let hay = if config.ignore_case {
            line.to_lowercase()
        } else {
            line.to_string()
        };

        let found = hay.contains(&search_pattern);
        let include = if config.invert { !found } else { found };

        if include {
            matches.push(Match {
                file: path.to_string(),
                line_num: line_num + 1,
                line: line.to_string(),
            });
        }
    }

    Ok(matches)
}

// ── Output ───────────────────────────────────────────────────────────

fn print_matches(matches: &[Match]~, config: &Config, multi_file: bool) / io {
    if config.count_only {
        if multi_file {
            // Group by file.
            let mut current_file: String = "".to_string();
            let mut count: usize = 0;
            for m in matches {
                if m.file != current_file {
                    if !current_file.is_empty() {
                        println!("{current_file}:{count}");
                    }
                    current_file = m.file.clone();
                    count = 0;
                }
                count = count + 1;
            }
            if !current_file.is_empty() {
                println!("{current_file}:{count}");
            }
        } else {
            println!("{matches.len()}");
        }
        return;
    }

    for m in matches {
        let prefix = if multi_file {
            if config.line_numbers {
                format!("{m.file}:{m.line_num}:")
            } else {
                format!("{m.file}:")
            }
        } else {
            if config.line_numbers {
                format!("{m.line_num}:")
            } else {
                "".to_string()
            }
        };
        println!("{prefix}{m.line}");
    }
}

// ── Entry point ──────────────────────────────────────────────────────

pub fn main() / io {
    let args: Vec<String> = env::args().collect();

    let config = match parse_args(args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            print_usage();
            process::exit(1);
        },
    };

    let multi_file = config.files.len() > 1;
    let mut all_matches: [Match]~ = Vec::new();
    let mut had_error = false;

    for file in &config.files {
        match search_file(file, &config) {
            Ok(matches) => {
                for m in matches {
                    all_matches.push(m);
                }
            },
            Err(e) => {
                eprintln!("{e}");
                had_error = true;
            },
        }
    }

    print_matches(&all_matches, &config, multi_file);

    if had_error {
        process::exit(2);
    }
    if all_matches.is_empty() {
        process::exit(1);
    }
}

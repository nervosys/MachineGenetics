/// MechGen `rust2mg` — Rust-to-MechGen source-level transpiler.
///
/// Applies the translation rules from REDOX_ECOSYSTEM.md §2.1.2 to convert
/// Rust source files into MechGen canonical syntax.
mod translate;

use std::path::{Path, PathBuf};
use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();
    let opts = match parse_args(&args[1..]) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!();
            print_usage();
            process::exit(1);
        }
    };

    if opts.help {
        print_usage();
        return;
    }

    let input = match &opts.input {
        Some(p) => p.clone(),
        None => {
            eprintln!("error: no input file specified");
            print_usage();
            process::exit(1);
        }
    };

    if input.is_dir() {
        process_directory(&input, &opts);
    } else {
        process_file(&input, &opts);
    }
}

// ── Options ──────────────────────────────────────────────────────────

struct Opts {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    diff: bool,
    stats: bool,
    dry_run: bool,
    help: bool,
}

fn parse_args(args: &[String]) -> Result<Opts, String> {
    let mut opts =
        Opts { input: None, output: None, diff: false, stats: false, dry_run: false, help: false };

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => opts.help = true,
            "--output" | "-o" => {
                i += 1;
                if i >= args.len() {
                    return Err("--output requires a path".into());
                }
                opts.output = Some(PathBuf::from(&args[i]));
            }
            "--diff" => opts.diff = true,
            "--stats" => opts.stats = true,
            "--dry-run" => opts.dry_run = true,
            other if !other.starts_with('-') => {
                opts.input = Some(PathBuf::from(other));
            }
            other => return Err(format!("unknown option: {other}")),
        }
        i += 1;
    }
    Ok(opts)
}

fn print_usage() {
    eprintln!("rust2mg — Rust to MechGen transpiler");
    eprintln!();
    eprintln!("Usage: rust2mg [OPTIONS] <INPUT>");
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  <INPUT>         Rust source file or directory");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --output, -o    Output directory (default: ./rdx/)");
    eprintln!("  --diff          Show side-by-side diff instead of writing files");
    eprintln!("  --stats         Print token count comparison");
    eprintln!("  --dry-run       Show what would change without writing");
    eprintln!("  --help, -h      Show this help message");
}

// ── Processing ───────────────────────────────────────────────────────

fn process_file(path: &Path, opts: &Opts) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {e}", path.display());
            process::exit(1);
        }
    };

    let result = translate::translate(&source);

    if opts.stats {
        print_stats(path, &source, &result);
    }

    if opts.diff {
        print_diff(path, &source, &result);
        return;
    }

    if opts.dry_run {
        eprintln!("would write: {}", output_path(path, opts).display());
        return;
    }

    let out_path = output_path(path, opts);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    match fs::write(&out_path, &result) {
        Ok(()) => eprintln!("wrote: {}", out_path.display()),
        Err(e) => eprintln!("error writing {}: {e}", out_path.display()),
    }
}

fn process_directory(dir: &Path, opts: &Opts) {
    let mut count = 0;
    let mut total_rust_bytes = 0usize;
    let mut total_rdx_bytes = 0usize;

    visit_dir(dir, &mut |path| {
        if path.extension().is_some_and(|ext| ext == "rs") {
            let source = match fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("skip {}: {e}", path.display());
                    return;
                }
            };

            let result = translate::translate(&source);
            total_rust_bytes += source.len();
            total_rdx_bytes += result.len();
            count += 1;

            if opts.diff {
                print_diff(path, &source, &result);
            } else if !opts.dry_run {
                let out_path = output_path(path, opts);
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent).ok();
                }
                match fs::write(&out_path, &result) {
                    Ok(()) => eprintln!("  wrote: {}", out_path.display()),
                    Err(e) => eprintln!("  error: {}: {e}", out_path.display()),
                }
            } else {
                eprintln!("  would write: {}", output_path(path, opts).display());
            }
        }
    });

    if opts.stats || count > 0 {
        eprintln!();
        eprintln!("=== Migration Summary ===");
        eprintln!("  Files processed: {count}");
        eprintln!("  Rust bytes:  {total_rust_bytes}");
        eprintln!("  MechGen bytes: {total_rdx_bytes}");
        if total_rust_bytes > 0 {
            let pct = (total_rdx_bytes as f64 / total_rust_bytes as f64) * 100.0;
            eprintln!("  Ratio: {pct:.1}%");
        }
    }
}

fn visit_dir(dir: &Path, cb: &mut dyn FnMut(&Path)) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip hidden dirs, target, etc.
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if !name.starts_with('.') && name != "target" {
                    visit_dir(&path, cb);
                }
            } else {
                cb(&path);
            }
        }
    }
}

fn output_path(input: &Path, opts: &Opts) -> PathBuf {
    let stem = input.file_stem().unwrap_or_default();
    let out_dir = opts.output.clone().unwrap_or_else(|| PathBuf::from("rdx"));
    out_dir.join(format!("{}.mg", stem.to_string_lossy()))
}

fn print_stats(path: &Path, rust_src: &str, rdx_src: &str) {
    let rust_lines = rust_src.lines().count();
    let rdx_lines = rdx_src.lines().count();
    let rust_tokens = rust_src.split_whitespace().count();
    let rdx_tokens = rdx_src.split_whitespace().count();

    eprintln!("--- Stats: {} ---", path.display());
    eprintln!("  Rust:  {rust_lines} lines, {rust_tokens} tokens, {} bytes", rust_src.len());
    eprintln!("  MechGen: {rdx_lines} lines, {rdx_tokens} tokens, {} bytes", rdx_src.len());
    if rust_tokens > 0 {
        let pct = (rdx_tokens as f64 / rust_tokens as f64) * 100.0;
        eprintln!("  Token reduction: {:.1}%", 100.0 - pct);
    }
}

fn print_diff(path: &Path, rust_src: &str, rdx_src: &str) {
    println!("=== {} ===", path.display());
    let rust_lines: Vec<&str> = rust_src.lines().collect();
    let rdx_lines: Vec<&str> = rdx_src.lines().collect();
    let max = rust_lines.len().max(rdx_lines.len());

    // Header.
    println!("{:<50} │ {}", "Rust", "MechGen");
    println!("{:─<50}─┼─{:─<50}", "", "");

    for i in 0..max {
        let left = rust_lines.get(i).unwrap_or(&"");
        let right = rdx_lines.get(i).unwrap_or(&"");
        if left != right {
            println!("{:<50} │ {}", left, right);
        }
    }
    println!();
}

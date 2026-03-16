mod ast;
mod effects;
mod hir;
mod lexer;
mod parser;
mod rap;
mod resolve;
mod types;

use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("--rap") => {
            let addr = args.get(2).map(|s| s.as_str()).unwrap_or("127.0.0.1:9876");
            rap::serve(addr);
        }
        Some("--check") => {
            // Full analysis pipeline: parse → resolve → typecheck → effects.
            let path = args.get(2).unwrap_or_else(|| {
                eprintln!("Usage: redox-parse --check <file>");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            run_check(&source, path);
        }
        Some(path) => {
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            run_parse(&source, path);
        }
        None => {
            let mut source = String::new();
            std::io::stdin().read_to_string(&mut source).unwrap();
            run_parse(&source, "<stdin>");
        }
    }
}

fn run_parse(source: &str, filename: &str) {
    let tokens = lexer::lex(source);

    let mut error_count = 0;
    for tok in &tokens {
        if tok.kind == lexer::TokenKind::Error {
            eprintln!(
                "{filename}:{}:{}: lexer error: unexpected character",
                tok.span.line, tok.span.col
            );
            error_count += 1;
        }
    }

    match parser::parse(&tokens) {
        Ok(module) => {
            println!("{}", serde_json::to_string_pretty(&module).unwrap());
        }
        Err(e) => {
            eprintln!("{filename}:{}:{}: parse error: {}", e.line, e.col, e.message);
            std::process::exit(1);
        }
    }

    if error_count > 0 {
        std::process::exit(1);
    }
}

fn run_check(source: &str, filename: &str) {
    // Phase 1: Lex.
    let tokens = lexer::lex(source);
    let mut total_errors = 0;

    for tok in &tokens {
        if tok.kind == lexer::TokenKind::Error {
            eprintln!(
                "{filename}:{}:{}: lexer error: unexpected character",
                tok.span.line, tok.span.col
            );
            total_errors += 1;
        }
    }

    // Phase 2: Parse.
    let module = match parser::parse(&tokens) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{filename}:{}:{}: parse error: {}", e.line, e.col, e.message);
            std::process::exit(1);
        }
    };

    // Phase 3: Name resolution.
    let resolver = resolve::resolve(&module);
    for diag in &resolver.diagnostics {
        eprintln!("{filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }

    // Phase 4: Type checking.
    let checker = types::check(&module);
    for diag in &checker.diagnostics {
        eprintln!("{filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }

    // Phase 5: Effect inference.
    let effect_infer = effects::infer_effects(&module);
    for diag in &effect_infer.diagnostics {
        eprintln!("{filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }

    // Report.
    let sym_count = resolver.symbols.len();
    let fn_count = effect_infer.inferred.len();

    eprintln!();
    eprintln!("=== Analysis Summary ===");
    eprintln!("  Symbols resolved: {sym_count}");
    eprintln!("  Functions analyzed: {fn_count}");

    // Print effect annotations.
    for (name, effects) in &effect_infer.inferred {
        if effects.is_empty() {
            eprintln!("  f {name}: pure");
        } else {
            let fx: Vec<String> = effects.iter().map(|e| e.to_string()).collect();
            eprintln!("  f {name}: {{ {} }}", fx.join(", "));
        }
    }

    eprintln!("  Errors: {total_errors}");

    if total_errors > 0 {
        std::process::exit(1);
    } else {
        eprintln!("  Status: OK");
    }
}

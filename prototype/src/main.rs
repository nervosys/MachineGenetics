mod ast;
mod effects;
mod hir;
mod lexer;
mod mlir;
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
        Some("--pipeline") => {
            // End-to-end demo pipeline: lex → parse → resolve → typecheck → effects → MLIR.
            let path = args.get(2).unwrap_or_else(|| {
                eprintln!("Usage: redox-parse --pipeline <file>");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            run_pipeline(&source, path);
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

fn run_pipeline(source: &str, filename: &str) {
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║  Redox End-to-End Pipeline                                  ║");
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
    eprintln!();

    let mut total_errors = 0;

    // ── Phase 1: Lex ─────────────────────────────────────────────────
    eprintln!("▸ Phase 1/6: Lexical analysis");
    let tokens = lexer::lex(source);
    let mut lex_errors = 0;
    for tok in &tokens {
        if tok.kind == lexer::TokenKind::Error {
            eprintln!("  {filename}:{}:{}: lexer error", tok.span.line, tok.span.col);
            lex_errors += 1;
        }
    }
    let token_count = tokens.len();
    eprintln!("  ✓ {token_count} tokens, {lex_errors} errors");
    total_errors += lex_errors;

    // ── Phase 2: Parse ───────────────────────────────────────────────
    eprintln!("▸ Phase 2/6: Parsing");
    let module = match parser::parse(&tokens) {
        Ok(m) => {
            eprintln!("  ✓ {} top-level items", m.items.len());
            m
        }
        Err(e) => {
            eprintln!("  ✗ parse error at {}:{}: {}", e.line, e.col, e.message);
            std::process::exit(1);
        }
    };

    // ── Phase 3: Name resolution ─────────────────────────────────────
    eprintln!("▸ Phase 3/6: Name resolution");
    let resolver = resolve::resolve(&module);
    for diag in &resolver.diagnostics {
        eprintln!("  {filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }
    eprintln!("  ✓ {} symbols resolved", resolver.symbols.len());

    // ── Phase 4: Type checking ───────────────────────────────────────
    eprintln!("▸ Phase 4/6: Type checking");
    let checker = types::check(&module);
    for diag in &checker.diagnostics {
        eprintln!("  {filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }
    eprintln!("  ✓ {} type diagnostics", checker.diagnostics.len());

    // ── Phase 5: Effect inference ────────────────────────────────────
    eprintln!("▸ Phase 5/6: Effect inference");
    let effect_infer = effects::infer_effects(&module);
    for diag in &effect_infer.diagnostics {
        eprintln!("  {filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }
    for (name, fx) in &effect_infer.inferred {
        if fx.is_empty() {
            eprintln!("  f {name}: pure");
        } else {
            let effects: Vec<String> = fx.iter().map(|e| e.to_string()).collect();
            eprintln!("  f {name}: {{ {} }}", effects.join(", "));
        }
    }

    // ── Phase 6: MLIR lowering ───────────────────────────────────────
    eprintln!("▸ Phase 6/6: MLIR lowering");
    let mlir_output = mlir::emit(&module, &effect_infer);
    let mlir_lines = mlir_output.lines().count();
    eprintln!("  ✓ {mlir_lines} lines of MLIR generated");

    // ── Summary ──────────────────────────────────────────────────────
    eprintln!();
    eprintln!("═══ Pipeline Summary ═══════════════════════════════════════════");
    eprintln!("  Source:          {filename}");
    eprintln!("  Tokens:          {token_count}");
    eprintln!("  Items:           {}", module.items.len());
    eprintln!("  Symbols:         {}", resolver.symbols.len());
    eprintln!("  Functions:       {}", effect_infer.inferred.len());
    eprintln!("  MLIR lines:      {mlir_lines}");
    eprintln!("  Errors:          {total_errors}");

    if total_errors > 0 {
        eprintln!("  Status:          FAIL");
        eprintln!("════════════════════════════════════════════════════════════════");
        std::process::exit(1);
    } else {
        eprintln!("  Status:          OK");
        eprintln!("════════════════════════════════════════════════════════════════");
    }

    // Print MLIR to stdout.
    println!("{mlir_output}");
}

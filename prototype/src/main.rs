mod ast;
mod cost;
mod effects;
mod elision;
mod heal;
mod hir;
mod legacy;
mod lexer;
mod mlir;
mod parser;
mod rap;
mod resolve;
mod skb;
mod types;
mod verify;

use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let no_elision = args.iter().any(|a| a == "--no-elision");
    let syntax_legacy = args.iter().any(|a| a == "--syntax=legacy");
    // Collect positional-ish args (skip flag-style args)
    let filtered: Vec<&str> = args
        .iter()
        .skip(1)
        .filter(|a| {
            !matches!(a.as_str(), "--no-elision" | "--syntax=legacy" | "--syntax=canonical")
        })
        .map(|s| s.as_str())
        .collect();

    match filtered.first().copied() {
        Some("--rap") => {
            let addr = filtered.get(1).copied().unwrap_or("127.0.0.1:9876");
            rap::serve(addr);
        }
        Some("--check") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: redox-parse --check <file> [--no-elision]");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            run_check(&source, path, !no_elision, syntax_legacy);
        }
        Some("--pipeline") => {
            let path = filtered.get(1).unwrap_or_else(|| {
                eprintln!("Usage: redox-parse --pipeline <file> [--no-elision] [--syntax=legacy]");
                std::process::exit(1);
            });
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            run_pipeline(&source, path, !no_elision, syntax_legacy);
        }
        Some(path) => {
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Error reading {path}: {e}");
                std::process::exit(1);
            });
            run_parse(&source, path, !no_elision, syntax_legacy);
        }
        None => {
            let mut source = String::new();
            std::io::stdin().read_to_string(&mut source).unwrap();
            run_parse(&source, "<stdin>", !no_elision, syntax_legacy);
        }
    }
}

fn run_parse(source: &str, filename: &str, do_elision: bool, legacy: bool) {
    let source = if legacy { legacy::translate(source) } else { source.to_string() };
    let tokens = lexer::lex(&source);

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
            let module = if do_elision { elision::elide(&module) } else { module };
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

fn run_check(source: &str, filename: &str, do_elision: bool, legacy: bool) {
    // Phase 0: Legacy syntax translation (if active).
    let source = if legacy { legacy::translate(source) } else { source.to_string() };

    // Phase 1: Lex.
    let tokens = lexer::lex(&source);
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

    // Phase 2.5: Safety elision (agentic mode default).
    let module = if do_elision { elision::elide(&module) } else { module };

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

    // Phase 6: Self-healing — generate fix candidates for all diagnostics.
    let mut all_diagnostics: Vec<hir::Diagnostic> = Vec::new();
    all_diagnostics.extend(resolver.diagnostics.iter().cloned());
    all_diagnostics.extend(checker.diagnostics.iter().cloned());
    all_diagnostics.extend(effect_infer.diagnostics.iter().cloned());

    let healed = heal::heal(&all_diagnostics);
    let fix_count: usize = healed.iter().map(|h| h.fixes.len()).sum();

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

    if fix_count > 0 {
        eprintln!("  Fix candidates: {fix_count}");
        for h in &healed {
            if !h.fixes.is_empty() {
                eprintln!("    ▸ {}: {} fix(es)", h.diagnostic.message, h.fixes.len());
                for fix in &h.fixes {
                    eprintln!("      - [conf={:.0}%] {}", fix.confidence * 100.0, fix.description);
                }
            }
        }
    }

    if total_errors > 0 {
        std::process::exit(1);
    } else {
        eprintln!("  Status: OK");
    }
}

fn run_pipeline(source: &str, filename: &str, do_elision: bool, legacy: bool) {
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║  Redox End-to-End Pipeline                                  ║");
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
    eprintln!();

    let mut total_errors = 0;

    // ── Phase 0: Legacy syntax translation ───────────────────────────
    let source = if legacy {
        eprintln!("▸ Phase 0: Legacy syntax translation (Rust → Redox)");
        let translated = legacy::translate(source);
        eprintln!("  ✓ translated to canonical syntax");
        translated
    } else {
        source.to_string()
    };

    // ── Phase 1: Lex ─────────────────────────────────────────────────
    eprintln!("▸ Phase 1/7: Lexical analysis");
    let tokens = lexer::lex(&source);
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
    eprintln!("▸ Phase 2/7: Parsing");
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

    // ── Phase 2.5: Safety elision ────────────────────────────────────
    let module = if do_elision {
        eprintln!("▸ Phase 2.5: Safety elision (agentic mode)");
        let elided = elision::elide(&module);
        eprintln!("  ✓ safety annotations stripped");
        elided
    } else {
        eprintln!("▸ Phase 2.5: Safety elision — SKIPPED (--no-elision)");
        module
    };

    // ── Phase 3: Name resolution ─────────────────────────────────────
    eprintln!("▸ Phase 3/7: Name resolution");
    let resolver = resolve::resolve(&module);
    for diag in &resolver.diagnostics {
        eprintln!("  {filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }
    eprintln!("  ✓ {} symbols resolved", resolver.symbols.len());

    // ── Phase 4: Type checking ───────────────────────────────────────
    eprintln!("▸ Phase 4/7: Type checking");
    let checker = types::check(&module);
    for diag in &checker.diagnostics {
        eprintln!("  {filename}: {diag}");
        if diag.severity == hir::Severity::Error {
            total_errors += 1;
        }
    }
    eprintln!("  ✓ {} type diagnostics", checker.diagnostics.len());

    // ── Phase 5: Effect inference ────────────────────────────────────
    eprintln!("▸ Phase 5/7: Effect inference");
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
    eprintln!("▸ Phase 6/7: MLIR lowering");
    let mlir_output = mlir::emit(&module, &effect_infer);
    let mlir_lines = mlir_output.lines().count();
    eprintln!("  ✓ {mlir_lines} lines of MLIR generated");

    // ── Phase 7: Self-healing ─────────────────────────────────────────
    eprintln!("▸ Phase 7/7: Self-healing analysis");
    let mut all_diags: Vec<hir::Diagnostic> = Vec::new();
    all_diags.extend(resolver.diagnostics.iter().cloned());
    all_diags.extend(checker.diagnostics.iter().cloned());
    all_diags.extend(effect_infer.diagnostics.iter().cloned());

    let healed = heal::heal(&all_diags);
    let fix_count: usize = healed.iter().map(|h| h.fixes.len()).sum();
    eprintln!("  ✓ {} diagnostics analyzed, {} fix candidates", all_diags.len(), fix_count);

    if fix_count > 0 {
        for h in &healed {
            for fix in &h.fixes {
                eprintln!("    ▸ [conf={:.0}%] {}", fix.confidence * 100.0, fix.description);
            }
        }
    }

    // ── Summary ──────────────────────────────────────────────────────
    eprintln!();
    eprintln!("═══ Pipeline Summary ═══════════════════════════════════════════");
    eprintln!("  Source:          {filename}");
    eprintln!("  Tokens:          {token_count}");
    eprintln!("  Items:           {}", module.items.len());
    eprintln!("  Symbols:         {}", resolver.symbols.len());
    eprintln!("  Functions:       {}", effect_infer.inferred.len());
    eprintln!("  MLIR lines:      {mlir_lines}");
    eprintln!("  Fix candidates:  {fix_count}");
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

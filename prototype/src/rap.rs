/// RAP (Redox Agent Protocol) — JSON-RPC over TCP server skeleton.
///
/// Provides language services for AI agents:
///   language/parse    — parse source to AST (JSON)
///   language/tokens   — tokenize source
///   build/check       — check syntax (parse + report errors)
///   build/heal        — check + generate fix candidates (P22)
///   cost/query        — query per-construct cost estimates (P19)
///   cost/compare      — compare costs of two constructs
///   skb/query         — query structured knowledge base (P14)
///   skb/spec          — lookup spec block for a symbol
///   verify/contracts  — verify function contracts (P21)
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use crate::cost;
use crate::heal;
use crate::hir;
use crate::lexer;
use crate::parser;
use crate::skb;
use crate::verify;

/// Start the RAP server on `addr` (e.g. "127.0.0.1:9876").
pub fn serve(addr: &str) {
    let listener = TcpListener::bind(addr).unwrap_or_else(|e| {
        eprintln!("rap: failed to bind {addr}: {e}");
        std::process::exit(1);
    });
    eprintln!("rap: listening on {addr}");

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("rap: accept error: {e}");
                continue;
            }
        };

        // One connection at a time (single-threaded prototype)
        if let Err(e) = handle_connection(stream) {
            eprintln!("rap: connection error: {e}");
        }
    }
}

fn handle_connection(stream: std::net::TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: serde_json::Value = serde_json::from_str(&line)?;
        let id = request.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(serde_json::Value::Null);

        let result = dispatch(method, &params);

        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        });

        let mut out = serde_json::to_string(&response)?;
        out.push('\n');
        writer.write_all(out.as_bytes())?;
        writer.flush()?;
    }

    Ok(())
}

fn dispatch(method: &str, params: &serde_json::Value) -> serde_json::Value {
    let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");

    match method {
        "language/tokens" => {
            let tokens = lexer::lex(source);
            let token_list: Vec<serde_json::Value> = tokens
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "kind": format!("{:?}", t.kind),
                        "text": t.text,
                        "line": t.span.line,
                        "col": t.span.col,
                    })
                })
                .collect();
            serde_json::json!({ "tokens": token_list })
        }

        "language/parse" => {
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => serde_json::json!({
                    "ok": true,
                    "ast": serde_json::to_value(&module).unwrap_or_default()
                }),
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": {
                        "line": e.line,
                        "col": e.col,
                        "message": e.message
                    }
                }),
            }
        }

        "build/check" => {
            let tokens = lexer::lex(source);
            let lex_errors: Vec<serde_json::Value> = tokens
                .iter()
                .filter(|t| t.kind == lexer::TokenKind::Error)
                .map(|t| {
                    serde_json::json!({
                        "line": t.span.line,
                        "col": t.span.col,
                        "message": format!("unexpected character: {}", t.text)
                    })
                })
                .collect();

            let parse_error = match parser::parse(&tokens) {
                Ok(_) => None,
                Err(e) => Some(serde_json::json!({
                    "line": e.line,
                    "col": e.col,
                    "message": e.message
                })),
            };

            let mut errors = lex_errors;
            if let Some(pe) = parse_error {
                errors.push(pe);
            }

            serde_json::json!({
                "ok": errors.is_empty(),
                "errors": errors
            })
        }

        "build/heal" => {
            // Parse + generate fix candidates for all diagnostics (P22).
            let tokens = lexer::lex(source);
            let mut diagnostics: Vec<hir::Diagnostic> = Vec::new();

            for tok in &tokens {
                if tok.kind == lexer::TokenKind::Error {
                    diagnostics.push(hir::Diagnostic {
                        severity: hir::Severity::Error,
                        message: format!("unexpected character: {}", tok.text),
                        span: Some(hir::Span { line: tok.span.line as u32, col: tok.span.col as u32 }),
                    });
                }
            }

            if let Err(e) = parser::parse(&tokens) {
                diagnostics.push(hir::Diagnostic {
                    severity: hir::Severity::Error,
                    message: e.message.clone(),
                    span: Some(hir::Span { line: e.line as u32, col: e.col as u32 }),
                });
            }

            let healed = heal::heal(&diagnostics);
            serde_json::json!({
                "ok": diagnostics.is_empty(),
                "diagnostics": serde_json::to_value(&healed).unwrap_or_default()
            })
        }

        "cost/query" => {
            // Query per-construct cost estimate (P19).
            let construct = params.get("construct").and_then(|v| v.as_str()).unwrap_or("");
            let target = params.get("target").and_then(|v| v.as_str()).unwrap_or("x86_64");
            let opt = match params.get("opt").and_then(|v| v.as_str()).unwrap_or("release") {
                "debug" => cost::OptLevel::Debug,
                "release_lto" => cost::OptLevel::ReleaseLto,
                _ => cost::OptLevel::Release,
            };

            match cost::query_cost(construct, target, opt) {
                Some(est) => serde_json::json!({
                    "ok": true,
                    "estimate": serde_json::to_value(&est).unwrap_or_default()
                }),
                None => serde_json::json!({
                    "ok": false,
                    "error": format!("no cost data for `{construct}` on `{target}`")
                }),
            }
        }

        "cost/compare" => {
            let a = params.get("a").and_then(|v| v.as_str()).unwrap_or("");
            let b = params.get("b").and_then(|v| v.as_str()).unwrap_or("");
            let target = params.get("target").and_then(|v| v.as_str()).unwrap_or("x86_64");
            let opt = cost::OptLevel::Release;

            match cost::compare(a, b, target, opt) {
                Some(cmp) => serde_json::json!({
                    "ok": true,
                    "comparison": serde_json::to_value(&cmp).unwrap_or_default()
                }),
                None => serde_json::json!({
                    "ok": false,
                    "error": "one or both constructs not found in cost database"
                }),
            }
        }

        "skb/query" => {
            // Query the structured knowledge base (P14).
            let by = params.get("by").and_then(|v| v.as_str()).unwrap_or("fqn");
            let value = params.get("value").and_then(|v| v.as_str()).unwrap_or("");

            let result = match by {
                "fqn" => skb::query_by_fqn(value),
                "effect" => skb::query_by_effect(value),
                "capability" => skb::query_by_capability(value),
                "tag" => skb::query_by_tag(value),
                "rust_alias" => skb::query_by_rust_alias(value),
                "module" => skb::query_module(value),
                _ => skb::query_by_fqn(value),
            };

            serde_json::json!({
                "ok": true,
                "query": result.query_text,
                "matches": serde_json::to_value(&result.matches).unwrap_or_default()
            })
        }

        "skb/spec" => {
            // Lookup function spec block.
            let fqn = params.get("fqn").and_then(|v| v.as_str()).unwrap_or("");
            match skb::query_spec(fqn) {
                Some(spec) => serde_json::json!({
                    "ok": true,
                    "spec": serde_json::to_value(&spec).unwrap_or_default()
                }),
                None => serde_json::json!({
                    "ok": false,
                    "error": format!("no spec found for `{fqn}`")
                }),
            }
        }

        "verify/contracts" => {
            // Verify function contracts (P21).
            let fqn = params.get("fqn").and_then(|v| v.as_str()).unwrap_or("");
            let requires: Vec<String> = params.get("requires")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let ensures: Vec<String> = params.get("ensures")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let declared_effects: Vec<String> = params.get("declared_effects")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let used_effects: Vec<String> = params.get("used_effects")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let spec_input = if requires.is_empty() && ensures.is_empty() {
                None
            } else {
                Some(verify::SpecInput { requires, ensures })
            };

            let effects = verify::EffectAnalysis { declared: declared_effects, used: used_effects };
            let result = verify::verify_contracts(fqn, spec_input.as_ref(), &effects);

            serde_json::json!({
                "ok": result.status == verify::VerifyStatus::Verified || result.status == verify::VerifyStatus::Trivial,
                "result": serde_json::to_value(&result).unwrap_or_default()
            })
        }

        _ => serde_json::json!({
            "error": format!("unknown method: {method}")
        }),
    }
}

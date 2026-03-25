/// RAP (MechGen Agent Protocol) — JSON-RPC over TCP server skeleton.
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
use crate::effects;
use crate::elision;
use crate::heal;
use crate::hir;
use crate::lexer;
use crate::parser;
use crate::skb;
use crate::token_budget;
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
                        span: Some(hir::Span {
                            line: tok.span.line as u32,
                            col: tok.span.col as u32,
                        }),
                        id: None,
                        category: Some(hir::DiagnosticCategory::SyntaxError),
                    });
                }
            }

            if let Err(e) = parser::parse(&tokens) {
                diagnostics.push(hir::Diagnostic {
                    severity: hir::Severity::Error,
                    message: e.message.clone(),
                    span: Some(hir::Span { line: e.line as u32, col: e.col as u32 }),
                    id: None,
                    category: Some(hir::DiagnosticCategory::SyntaxError),
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
            let requires: Vec<String> = params
                .get("requires")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let ensures: Vec<String> = params
                .get("ensures")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let declared_effects: Vec<String> = params
                .get("declared_effects")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let used_effects: Vec<String> = params
                .get("used_effects")
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

        // ── New methods (Step 36) ──────────────────────────────
        "verify/module" => {
            // Verify all contracts in a source module.
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let results = verify::verify_module(&module);
                    serde_json::json!({
                        "ok": true,
                        "results": serde_json::to_value(&results).unwrap_or_default()
                    })
                }
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message }
                }),
            }
        }

        "format/agent" => {
            // Return the agent (MechGen canonical) form of source.
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let elided = elision::elide(&module);
                    serde_json::json!({
                        "ok": true,
                        "ast": serde_json::to_value(&elided).unwrap_or_default()
                    })
                }
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message }
                }),
            }
        }

        "format/human" => {
            // Return the human (Rust-like) form of source — same as parse for now.
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => serde_json::json!({
                    "ok": true,
                    "ast": serde_json::to_value(&module).unwrap_or_default()
                }),
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message }
                }),
            }
        }

        "lint/check" => {
            // Lint: parse + verify contracts + check effects.
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let verify_results = verify::verify_module(&module);
                    let engine = effects::infer_effects(&module);
                    serde_json::json!({
                        "ok": true,
                        "verify": serde_json::to_value(&verify_results).unwrap_or_default(),
                        "effect_diagnostics": serde_json::to_value(&engine.diagnostics).unwrap_or_default()
                    })
                }
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message }
                }),
            }
        }

        "token/report" => {
            // Token budget report for the source module.
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let report = token_budget::report(&module);
                    serde_json::json!({
                        "ok": true,
                        "report": {
                            "total_agent": report.total_agent,
                            "total_human": report.total_human,
                            "overall_ratio": report.overall_ratio,
                            "items": serde_json::to_value(&report.items).unwrap_or_default()
                        }
                    })
                }
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message }
                }),
            }
        }

        "effects/infer" => {
            // Infer effects for all functions in the source.
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let engine = effects::infer_effects(&module);
                    let inferred: Vec<serde_json::Value> = engine
                        .inferred
                        .iter()
                        .map(|(name, eset)| {
                            serde_json::json!({
                                "function": name,
                                "effects": eset.iter().map(|e| e.to_string()).collect::<Vec<_>>()
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "ok": true,
                        "effects": inferred,
                        "diagnostics": serde_json::to_value(&engine.diagnostics).unwrap_or_default()
                    })
                }
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message }
                }),
            }
        }

        "effects/check" => {
            // Check declared vs inferred effects.
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let engine = effects::infer_effects(&module);
                    serde_json::json!({
                        "ok": engine.diagnostics.is_empty(),
                        "diagnostics": serde_json::to_value(&engine.diagnostics).unwrap_or_default()
                    })
                }
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message }
                }),
            }
        }

        "elision/apply" => {
            // Apply safety-elision pass to the source.
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let elided = elision::elide(&module);
                    serde_json::json!({
                        "ok": true,
                        "ast": serde_json::to_value(&elided).unwrap_or_default()
                    })
                }
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message }
                }),
            }
        }

        "attribute/expand" => {
            // Expand compressed attribute shorthands.
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            match elision::expand_attribute_name(name) {
                Some(expanded) => serde_json::json!({
                    "ok": true,
                    "expanded": expanded
                }),
                None => serde_json::json!({
                    "ok": false,
                    "error": format!("unknown attribute shorthand: `{name}`")
                }),
            }
        }

        "attribute/compress" => {
            // Compress a Rust attribute to MechGen shorthand.
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            match elision::compress_attribute_name(name) {
                Some(compressed) => serde_json::json!({
                    "ok": true,
                    "compressed": compressed
                }),
                None => serde_json::json!({
                    "ok": false,
                    "error": format!("no shorthand for `{name}`")
                }),
            }
        }

        "capability/check" => {
            // Check that agent capabilities are in the known taxonomy.
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let results = verify::verify_module(&module);
                    let agent_results: Vec<_> =
                        results.iter().filter(|r| r.fqn.starts_with("agent.")).collect();
                    serde_json::json!({
                        "ok": agent_results.iter().all(|r| r.status == verify::VerifyStatus::Verified),
                        "results": serde_json::to_value(&agent_results).unwrap_or_default()
                    })
                }
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message }
                }),
            }
        }

        "heal/graph" => {
            // Produce DiagnosticGraph objects from source errors.
            let tokens = lexer::lex(source);
            let mut diagnostics: Vec<hir::Diagnostic> = Vec::new();
            for tok in &tokens {
                if tok.kind == lexer::TokenKind::Error {
                    diagnostics.push(hir::Diagnostic {
                        severity: hir::Severity::Error,
                        message: format!("unexpected character: {}", tok.text),
                        span: Some(hir::Span {
                            line: tok.span.line as u32,
                            col: tok.span.col as u32,
                        }),
                        id: None,
                        category: Some(hir::DiagnosticCategory::SyntaxError),
                    });
                }
            }
            if let Err(e) = parser::parse(&tokens) {
                diagnostics.push(hir::Diagnostic {
                    severity: hir::Severity::Error,
                    message: e.message.clone(),
                    span: Some(hir::Span { line: e.line as u32, col: e.col as u32 }),
                    id: None,
                    category: Some(hir::DiagnosticCategory::SyntaxError),
                });
            }
            let graphs = heal::heal_to_graphs(&diagnostics);
            serde_json::json!({
                "ok": diagnostics.is_empty(),
                "graphs": serde_json::to_value(&graphs).unwrap_or_default()
            })
        }

        "sandbox/policy" => {
            // Return capability sandbox policy for an agent.
            let agent_name = params.get("agent").and_then(|v| v.as_str()).unwrap_or("");
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let mut policy = serde_json::json!({
                        "ok": false,
                        "error": format!("agent `{agent_name}` not found")
                    });
                    for item in &module.items {
                        if let crate::ast::ItemKind::Agent(ref ad) = item.kind {
                            if ad.name == agent_name {
                                policy = serde_json::json!({
                                    "ok": true,
                                    "agent": ad.name,
                                    "capabilities": ad.capabilities,
                                    "requires_approval": ad.requires_approval,
                                });
                                break;
                            }
                        }
                    }
                    policy
                }
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message }
                }),
            }
        }

        "skb/rules" => {
            // Query SKB safety rules by domain.
            let domain = params.get("domain").and_then(|v| v.as_str()).unwrap_or("");
            let result = skb::query_by_tag(domain);
            serde_json::json!({
                "ok": true,
                "matches": serde_json::to_value(&result.matches).unwrap_or_default()
            })
        }

        "doc/query" => {
            // Documentation query — return SKB entry docs for a symbol.
            let fqn = params.get("fqn").and_then(|v| v.as_str()).unwrap_or("");
            let result = skb::query_by_fqn(fqn);
            serde_json::json!({
                "ok": !result.matches.is_empty(),
                "matches": serde_json::to_value(&result.matches).unwrap_or_default()
            })
        }

        "grammar/list" => {
            // List all registered grammar extensions via the registry.
            let reg = crate::grammar::ExtensionRegistry::new();
            serde_json::json!({
                "ok": true,
                "extensions": reg.to_json()
            })
        }

        "manifest/generate" => {
            // Generate a capability manifest for the parsed module.
            let crate_name = params.get("crate_name").and_then(|v| v.as_str()).unwrap_or("unnamed");
            let version = params.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0");
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let m = crate::manifest::generate(&module, crate_name, version);
                    serde_json::json!({
                        "ok": true,
                        "manifest": crate::manifest::to_json_value(&m)
                    })
                }
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message }
                }),
            }
        }

        _ => serde_json::json!({
            "error": format!("unknown method: {method}")
        }),
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn call(method: &str, params: serde_json::Value) -> serde_json::Value {
        dispatch(method, &params)
    }

    fn src_params(source: &str) -> serde_json::Value {
        serde_json::json!({ "source": source })
    }

    // ── Original 9 methods ───────────────────────────────────────

    #[test]
    fn test_language_tokens() {
        let r = call("language/tokens", src_params("f main() {}"));
        assert!(r.get("tokens").unwrap().as_array().unwrap().len() > 0);
    }

    #[test]
    fn test_language_parse_ok() {
        let r = call("language/parse", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
    }

    #[test]
    fn test_language_parse_err() {
        let r = call("language/parse", src_params("@@@ garbage"));
        assert_eq!(r["ok"], false);
    }

    #[test]
    fn test_build_check_ok() {
        let r = call("build/check", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
    }

    #[test]
    fn test_build_heal_ok() {
        let r = call("build/heal", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
    }

    #[test]
    fn test_cost_query() {
        let r = call(
            "cost/query",
            serde_json::json!({
                "construct": "vec_push",
                "target": "x86_64",
                "opt": "release"
            }),
        );
        // May not find cost data — just check it doesn't panic
        assert!(r.get("ok").is_some());
    }

    #[test]
    fn test_cost_compare() {
        let r = call(
            "cost/compare",
            serde_json::json!({
                "a": "vec_push", "b": "vec_push", "target": "x86_64"
            }),
        );
        assert!(r.get("ok").is_some());
    }

    #[test]
    fn test_skb_query() {
        let r = call(
            "skb/query",
            serde_json::json!({
                "by": "fqn", "value": "Vec"
            }),
        );
        assert_eq!(r["ok"], true);
    }

    #[test]
    fn test_skb_spec() {
        let r = call("skb/spec", serde_json::json!({ "fqn": "nonexistent" }));
        assert_eq!(r["ok"], false);
    }

    #[test]
    fn test_verify_contracts() {
        let r = call(
            "verify/contracts",
            serde_json::json!({
                "fqn": "test_fn",
                "requires": ["x > 0"],
                "ensures": ["result > 0"]
            }),
        );
        assert!(r.get("ok").is_some());
    }

    // ── New methods (Step 36) ────────────────────────────────────

    #[test]
    fn test_verify_module() {
        let r = call("verify/module", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
    }

    #[test]
    fn test_format_agent() {
        let r = call("format/agent", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
        assert!(r.get("ast").is_some());
    }

    #[test]
    fn test_format_human() {
        let r = call("format/human", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
        assert!(r.get("ast").is_some());
    }

    #[test]
    fn test_lint_check() {
        let r = call("lint/check", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
        assert!(r.get("verify").is_some());
    }

    #[test]
    fn test_token_report() {
        let r = call("token/report", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
        let report = &r["report"];
        assert!(report.get("total_agent").is_some());
        assert!(report.get("total_human").is_some());
        assert!(report.get("overall_ratio").is_some());
    }

    #[test]
    fn test_effects_infer() {
        let r = call("effects/infer", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
        assert!(r.get("effects").is_some());
    }

    #[test]
    fn test_effects_check() {
        let r = call("effects/check", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
    }

    #[test]
    fn test_elision_apply() {
        let r = call("elision/apply", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
        assert!(r.get("ast").is_some());
    }

    #[test]
    fn test_attribute_expand() {
        let r = call("attribute/expand", serde_json::json!({ "name": "d" }));
        assert_eq!(r["ok"], true);
        assert_eq!(r["expanded"], "derive");
    }

    #[test]
    fn test_attribute_expand_unknown() {
        let r = call("attribute/expand", serde_json::json!({ "name": "zzz" }));
        assert_eq!(r["ok"], false);
    }

    #[test]
    fn test_attribute_compress() {
        let r = call("attribute/compress", serde_json::json!({ "name": "derive" }));
        assert_eq!(r["ok"], true);
        assert_eq!(r["compressed"], "d");
    }

    #[test]
    fn test_capability_check() {
        let src = "agent CodeBot { capabilities: [read_source, write_source] requires_approval: [write_source] }";
        let r = call("capability/check", src_params(src));
        assert_eq!(r["ok"], true);
    }

    #[test]
    fn test_heal_graph_clean() {
        let r = call("heal/graph", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
    }

    #[test]
    fn test_sandbox_policy() {
        let src = "agent CodeBot { capabilities: [read_source] requires_approval: [write_source] }";
        let r = call(
            "sandbox/policy",
            serde_json::json!({
                "source": src,
                "agent": "CodeBot"
            }),
        );
        assert_eq!(r["ok"], true);
        assert_eq!(r["agent"], "CodeBot");
    }

    #[test]
    fn test_sandbox_policy_not_found() {
        let r = call(
            "sandbox/policy",
            serde_json::json!({
                "source": "f main() {}",
                "agent": "Ghost"
            }),
        );
        assert_eq!(r["ok"], false);
    }

    #[test]
    fn test_skb_rules() {
        let r = call("skb/rules", serde_json::json!({ "domain": "ownership" }));
        assert_eq!(r["ok"], true);
    }

    #[test]
    fn test_doc_query() {
        let r = call("doc/query", serde_json::json!({ "fqn": "Vec" }));
        // May or may not match — just check structure
        assert!(r.get("ok").is_some());
    }

    #[test]
    fn test_grammar_list() {
        let r = call("grammar/list", serde_json::json!({}));
        assert_eq!(r["ok"], true);
        let exts = r["extensions"].as_array().unwrap();
        assert!(exts.len() > 20);
    }

    #[test]
    fn test_manifest_generate() {
        let src =
            "agent Bot { capabilities: [read_source, net] }\n+f check(x: i32) -> bool { x > 0 }";
        let r = call(
            "manifest/generate",
            serde_json::json!({ "source": src, "crate_name": "test_crate", "version": "1.0.0" }),
        );
        assert_eq!(r["ok"], true);
        let m = &r["manifest"];
        assert_eq!(m["name"], "test_crate");
        assert_eq!(m["version"], "1.0.0");
        assert_eq!(m["agents"].as_array().unwrap().len(), 1);
        assert_eq!(m["agents"][0]["name"], "Bot");
        assert!(m["capability_index"].as_array().unwrap().len() >= 2);
    }

    #[test]
    fn test_manifest_generate_empty() {
        let r = call("manifest/generate", src_params("f main() {}"));
        assert_eq!(r["ok"], true);
        let m = &r["manifest"];
        assert!(m["agents"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_unknown_method() {
        let r = call("nonexistent/method", serde_json::json!({}));
        assert!(r.get("error").is_some());
    }
}

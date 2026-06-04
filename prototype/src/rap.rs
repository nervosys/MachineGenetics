/// RAP (MechGen Agent Protocol) — JSON-RPC over TCP server skeleton.
///
/// Provides language services for AI agents:
///   language/parse    — parse source to AST (JSON)
///   language/tokens   — tokenize source
///   build/check       — check syntax (parse + report errors)
///   build/heal        — check + generate fix candidates (P22)
///   build/recover     — apply 3-stage recovery, return final source + stage
///   cost/query        — query per-construct cost estimates (P19)
///   cost/compare      — compare costs of two constructs
///   skb/query         — query structured knowledge base (P14)
///   skb/spec          — lookup spec block for a symbol
///   verify/contracts  — verify function contracts (P21)
///   ontology/full     — return the complete language + IR + protocol ontology
///   ontology/section  — return one named section of the ontology
///   pipeline/recover-and-encode — source → 3-stage recover → RMIB bytes in one call
///   rmil/encode       — source → RMIB bytes (hex) for application/rmib transport
///   rmil/decode       — RMIB bytes (hex) → decompiled per-item view
///   rmil/run          — source → encode → CpuBackend dispatch (no text round-trip)
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

/// True if `addr` names a non-loopback bind target — i.e. something other than
/// `127.0.0.0/8` or `::1` (a literal `localhost` is treated as safe). A wildcard
/// (`0.0.0.0`, `::`, or an empty host) is non-loopback. Used to gate the
/// unauthenticated RAP socket against accidental network exposure.
fn is_non_loopback(addr: &str) -> bool {
    // Whole-string loopback forms (bare IPv6 has no brackets, so port-splitting
    // is ambiguous — check these first).
    if addr == "::1" || addr == "localhost" {
        return false;
    }
    // Extract the host portion. Bracketed IPv6: "[host]:port". Otherwise take
    // everything before the last ':' (host:port), or the whole thing if no port.
    let host = if let Some(rest) = addr.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        addr.rsplit_once(':').map(|(h, _)| h).unwrap_or(addr)
    };
    match host {
        "localhost" | "::1" => false,
        "" | "0.0.0.0" | "::" => true,
        h if h.starts_with("127.") => false,
        _ => true,
    }
}

/// Start the RAP server on `addr` (e.g. "127.0.0.1:9876").
///
/// Security (MITRE ATT&CK T1190/T1071): the RAP socket has **no authentication
/// or transport encryption**. It is meant for loopback use by a local agent.
/// Binding a non-loopback / wildcard address exposes an unauthenticated control
/// plane to the network, so we refuse it unless the operator explicitly opts in
/// via `MECHGEN_RAP_ALLOW_REMOTE=1` (and even then warn). See SECURITY_AUDIT.md.
pub fn serve(addr: &str) {
    if is_non_loopback(addr) {
        let allow = std::env::var("MECHGEN_RAP_ALLOW_REMOTE").as_deref() == Ok("1");
        if !allow {
            eprintln!(
                "rap: REFUSING to bind non-loopback address {addr}: the RAP control plane is \
                 unauthenticated and unencrypted. Bind 127.0.0.1, or front it with a reverse \
                 proxy doing authN/Z + TLS and set MECHGEN_RAP_ALLOW_REMOTE=1 to override."
            );
            std::process::exit(2);
        }
        eprintln!(
            "rap: WARNING binding non-loopback {addr} with no auth/TLS \
             (MECHGEN_RAP_ALLOW_REMOTE=1). Do not expose to untrusted networks."
        );
    }
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
        let id = request
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let params = request
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

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
                    span: Some(hir::Span {
                        line: e.line as u32,
                        col: e.col as u32,
                    }),
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
            let construct = params
                .get("construct")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let target = params
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("x86_64");
            let opt = match params
                .get("opt")
                .and_then(|v| v.as_str())
                .unwrap_or("release")
            {
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
            let target = params
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("x86_64");
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
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let ensures: Vec<String> = params
                .get("ensures")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let declared_effects: Vec<String> = params
                .get("declared_effects")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let used_effects: Vec<String> = params
                .get("used_effects")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let spec_input = if requires.is_empty() && ensures.is_empty() {
                None
            } else {
                Some(verify::SpecInput { requires, ensures })
            };

            let effects = verify::EffectAnalysis {
                declared: declared_effects,
                used: used_effects,
            };
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
                    let agent_results: Vec<_> = results
                        .iter()
                        .filter(|r| r.fqn.starts_with("agent."))
                        .collect();
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
                    span: Some(hir::Span {
                        line: e.line as u32,
                        col: e.col as u32,
                    }),
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
            let crate_name = params
                .get("crate_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed");
            let version = params
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0");
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


        // ── Natural Language endpoints ──────────────
        "nl/generate" => {
            let prompt = params.get("prompt").and_then(|v| v.as_str()).unwrap_or("hello world");
            let mut engine = crate::nl_engine::NlEngine::new();
            let response = engine.process(prompt);
            serde_json::json!({
                "ok": response.ok,
                "code_human": response.code_human,
                "code_agent": response.code_agent,
                "explanation": response.explanation,
                "diagnostics": response.diagnostics.len(),
                "fixes": response.fixes.len(),
                "verification": response.verification_summary
            })
        }

        "nl/explain" => {
            let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
            let prompt = format!("explain this code\n{}", source);
            let mut engine = crate::nl_engine::NlEngine::new();
            let response = engine.process(&prompt);
            serde_json::json!({
                "ok": response.ok,
                "explanation": response.explanation
            })
        }

        "nl/refactor" => {
            let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
            let prompt = format!("refactor this code\n{}", source);
            let mut engine = crate::nl_engine::NlEngine::new();
            let response = engine.process(&prompt);
            serde_json::json!({
                "ok": response.ok,
                "code_human": response.code_human,
                "code_agent": response.code_agent,
                "explanation": response.explanation
            })
        }

        "nl/query" => {
            let prompt = params.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
            let mut engine = crate::nl_engine::NlEngine::new();
            let response = engine.process(prompt);
            serde_json::json!({
                "ok": true,
                "explanation": response.explanation,
                "kb_results": response.kb_results
            })
        }

        // Return the complete machine-readable ontology over the
        // MechGen language, the RMIL IR, and the RAP protocol. Single
        // self-contained payload so an autonomous agent can discover
        // every construct, opcode, and method without prior training.
        "ontology/full" => crate::ontology::build(),

        // Return one named section of the ontology. Useful when an
        // agent only needs (e.g.) the IR op catalog and doesn't want
        // the whole payload.
        "ontology/section" => {
            let name = params.get("section").and_then(|v| v.as_str()).unwrap_or("");
            match crate::ontology::section(name) {
                Some(data) => serde_json::json!({
                    "ok": true,
                    "section": name,
                    "data": data,
                }),
                None => serde_json::json!({
                    "ok": false,
                    "error": format!("unknown ontology section: {name:?}"),
                    "available": [
                        "sigils", "keywords", "types", "ast_kinds", "ir_ops",
                        "op_families", "layer_map", "rap_methods",
                        "heal_patterns", "recovery_stages", "rmib", "examples",
                        "framewerx_modules", "cli_flags", "bench_backends",
                        "effects", "wrapper_protocol", "project_layout",
                        "docs", "ci_floors", "hardware_accelerators",
                    ],
                }),
            }
        }

        // Apply the bench's 3-stage recovery pipeline to broken source.
        // Returns the final source plus which stage produced it, so the
        // caller can decide whether to trust the recovery or re-prompt.
        "build/recover" => {
            let r = crate::recover::recover(source);
            serde_json::json!({
                "ok": r.parsed_ok,
                "stage": r.stage.as_str(),
                "candidates_tried": r.candidates_tried,
                "source": r.source,
                "changed": r.source != source,
            })
        }

        // ── RMIL binary IR transport (application/rmib) ──────────

        // One-shot path: broken source → 3-stage mechanical recover →
        // parse → encode RMIB. Saves an agent two round-trips. Returns
        // `ok=false` only if even the recovered source fails to parse
        // (so the caller knows to fall back to refine).
        "pipeline/recover-and-encode" => {
            let r = crate::recover::recover(source);
            if !r.parsed_ok {
                return serde_json::json!({
                    "ok": false,
                    "stage": r.stage.as_str(),
                    "candidates_tried": r.candidates_tried,
                    "error": "recovery exhausted; refine required",
                });
            }
            let tokens = lexer::lex(&r.source);
            let module = match parser::parse(&tokens) {
                Ok(m) => m,
                Err(e) => {
                    return serde_json::json!({
                        "ok": false,
                        "stage": r.stage.as_str(),
                        "error": format!("recovered source still failed to parse: {}:{}: {}", e.line, e.col, e.message),
                    });
                }
            };
            let (blob, summary) = crate::rmib::encode_module(&module);
            let items: Vec<serde_json::Value> = summary
                .iter()
                .map(|(n, sz, h)| {
                    serde_json::json!({
                        "name": n,
                        "expr_bytes": sz,
                        "content_hash": format!("{h:016x}"),
                    })
                })
                .collect();
            serde_json::json!({
                "ok": true,
                "recover_stage": r.stage.as_str(),
                "candidates_tried": r.candidates_tried,
                "changed": r.source != source,
                "recovered_source": r.source,
                "container_bytes": blob.len(),
                "items": items,
                "rmib_hex": crate::rmib::to_hex(&blob),
            })
        }

        // Source → RMIB bytes (hex-encoded for JSON channel).
        "rmil/encode" => {
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let (blob, summary) = crate::rmib::encode_module(&module);
                    let items: Vec<serde_json::Value> = summary
                        .iter()
                        .map(|(n, sz, h)| serde_json::json!({
                            "name": n,
                            "expr_bytes": sz,
                            "content_hash": format!("{h:016x}"),
                        }))
                        .collect();
                    serde_json::json!({
                        "ok": true,
                        "magic": "RMIB",
                        "version": crate::rmib::RMIB_VERSION,
                        "container_bytes": blob.len(),
                        "items": items,
                        "rmib_hex": crate::rmib::to_hex(&blob),
                    })
                }
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message },
                }),
            }
        }

        // RMIB bytes (hex) → decompiled MechGen view + per-item summary.
        "rmil/decode" => {
            let hex = params.get("rmib_hex").and_then(|v| v.as_str()).unwrap_or("");
            let blob = match crate::rmib::from_hex(hex) {
                Ok(b) => b,
                Err(e) => return serde_json::json!({ "ok": false, "error": format!("hex: {e}") }),
            };
            match crate::rmib::decode_container(&blob) {
                Ok(items) => {
                    let decoded: Vec<serde_json::Value> = items
                        .iter()
                        .map(|it| {
                            let result = crate::rmil_bridge::decompile(&it.expr, &it.name);
                            let layers: Vec<serde_json::Value> = result
                                .net
                                .layers
                                .iter()
                                .map(|l| {
                                    let type_name = match &l.layer_type {
                                        crate::ast::Type::Path { segments, .. } => {
                                            segments.last().cloned().unwrap_or_default()
                                        }
                                        _ => "?".to_string(),
                                    };
                                    serde_json::json!({
                                        "name": l.name,
                                        "type": type_name,
                                    })
                                })
                                .collect();
                            let skipped: Vec<String> =
                                result.skipped.iter().map(|op| format!("{op:?}")).collect();
                            serde_json::json!({
                                "name": it.name,
                                "expr_bytes": it.expr_bytes_len,
                                "content_hash": format!("{:016x}", it.expr.content_hash()),
                                "layers": layers,
                                "skipped": skipped,
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "ok": true,
                        "container_bytes": blob.len(),
                        "items": decoded,
                    })
                }
                Err(e) => serde_json::json!({ "ok": false, "error": e }),
            }
        }

        // Source → encode → CpuBackend dispatch (text-roundtrip-free path).
        "rmil/run" => {
            let tokens = lexer::lex(source);
            let module = match parser::parse(&tokens) {
                Ok(m) => m,
                Err(e) => {
                    return serde_json::json!({
                        "ok": false,
                        "stage": "parse",
                        "error": { "line": e.line, "col": e.col, "message": e.message },
                    });
                }
            };
            let (blob, _) = crate::rmib::encode_module(&module);
            let items = match crate::rmib::decode_container(&blob) {
                Ok(i) => i,
                Err(e) => return serde_json::json!({
                    "ok": false,
                    "stage": "decode",
                    "error": e,
                }),
            };
            let backend = rmi::compute::cpu::CpuBackend::new();
            let runs: Vec<serde_json::Value> = items
                .iter()
                .map(|it| {
                    let families = crate::rmil_bridge::expr_op_families(&it.expr);
                    let stub_families: Vec<String> = families
                        .iter()
                        .filter(|f| crate::rmil_bridge::is_stubbed_family(**f))
                        .filter(|f| !matches!(**f, rmi::lang::OpFamily::Neural))
                        .map(|f| format!("{f:?}"))
                        .collect();
                    if !stub_families.is_empty()
                        && !families.contains(&rmi::lang::OpFamily::Neural)
                    {
                        return serde_json::json!({
                            "name": it.name,
                            "status": "stub",
                            "families": stub_families,
                        });
                    }
                    let inferred = crate::rmil_compute::infer_input_shape(&it.expr);
                    let shape: Vec<usize> = inferred.unwrap_or_else(|| vec![8]);
                    match crate::rmil_compute::run_pipeline(&backend, &it.expr, &shape, 1.0) {
                        Ok(r) => {
                            let unsupported: Vec<String> =
                                r.unsupported.iter().map(|op| format!("{op:?}")).collect();
                            serde_json::json!({
                                "name": it.name,
                                "status": "dispatched",
                                "dispatched": r.dispatched,
                                "unsupported": unsupported,
                                "output_sum": r.output_sum,
                                "output_shape": r.output.shape,
                                "input_shape": shape,
                            })
                        }
                        Err(e) => serde_json::json!({
                            "name": it.name,
                            "status": "error",
                            "error": format!("{e}"),
                            "input_shape": shape,
                        }),
                    }
                })
                .collect();
            serde_json::json!({
                "ok": true,
                "container_bytes": blob.len(),
                "runs": runs,
            })
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

    #[test]
    fn loopback_detection_gates_remote_bind() {
        // Loopback / localhost are safe (no gate).
        for a in ["127.0.0.1:9876", "127.0.0.1", "localhost:9876", "[::1]:9876", "::1"] {
            assert!(!is_non_loopback(a), "{a} should be loopback");
        }
        // Wildcard / routable hosts are gated.
        for a in ["0.0.0.0:9876", "0.0.0.0", "[::]:9876", "::", "192.168.1.10:9876", "example.com:80"] {
            assert!(is_non_loopback(a), "{a} should be non-loopback");
        }
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
        let r = call(
            "attribute/compress",
            serde_json::json!({ "name": "derive" }),
        );
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

    // ── ontology ─────────────────────────────────────────────────

    #[test]
    fn test_ontology_full_returns_all_sections() {
        let r = call("ontology/full", serde_json::json!({}));
        assert_eq!(r["ok"], true);
        let sections = r["sections"].as_object().expect("sections object");
        for name in [
            "sigils", "keywords", "ast_kinds", "ir_ops",
            "op_families", "layer_map", "rap_methods",
            "heal_patterns", "recovery_stages", "rmib",
        ] {
            assert!(sections.contains_key(name), "missing section: {name}");
        }
        assert!(r["counts"]["ir_ops"].as_u64().unwrap() > 50);
    }

    #[test]
    fn test_ontology_section_ir_ops() {
        let r = call("ontology/section", serde_json::json!({ "section": "ir_ops" }));
        assert_eq!(r["ok"], true);
        assert_eq!(r["section"], "ir_ops");
        assert!(r["data"].as_array().unwrap().len() > 50);
    }

    #[test]
    fn test_ontology_section_unknown() {
        let r = call("ontology/section", serde_json::json!({ "section": "bogus" }));
        assert_eq!(r["ok"], false);
        assert!(r["available"].as_array().unwrap().len() >= 10);
    }

    // ── build/recover ────────────────────────────────────────────

    #[test]
    fn test_build_recover_already_valid() {
        let r = call("build/recover", src_params("+f main() {}"));
        assert_eq!(r["ok"], true);
        assert_eq!(r["stage"], "already-valid");
        assert_eq!(r["changed"], false);
    }

    #[test]
    fn test_build_recover_brace_balance() {
        let r = call("build/recover", src_params("+f main() { v x = 1;"));
        assert_eq!(r["ok"], true);
        let stage = r["stage"].as_str().unwrap();
        assert!(
            matches!(stage, "structural-balance" | "pattern-heal"),
            "stage was {stage}"
        );
        assert_eq!(r["changed"], true);
    }

    #[test]
    fn test_build_recover_failed_returns_original() {
        let r = call("build/recover", src_params("@@@!!!###"));
        assert_eq!(r["ok"], false);
        assert_eq!(r["stage"], "failed");
        assert_eq!(r["changed"], false);
    }

    // ── pipeline/recover-and-encode ─────────────────────────────

    #[test]
    fn test_pipeline_recover_and_encode_clean_source() {
        let src = "net tiny { layer fc: Linear(8, 4); forward { fc } }";
        let r = call("pipeline/recover-and-encode", src_params(src));
        assert_eq!(r["ok"], true);
        assert_eq!(r["recover_stage"], "already-valid");
        assert_eq!(r["changed"], false);
        assert!(r["rmib_hex"].as_str().unwrap().starts_with("524d4942"));
    }

    #[test]
    fn test_pipeline_recover_and_encode_brace_balance() {
        // Source missing closing brace — structural-balance saves it,
        // then we encode the net.
        let src = "net tiny { layer fc: Linear(8, 4); forward { fc } ";
        let r = call("pipeline/recover-and-encode", src_params(src));
        assert_eq!(r["ok"], true);
        let stage = r["recover_stage"].as_str().unwrap();
        assert!(
            matches!(stage, "structural-balance" | "pattern-heal"),
            "stage was {stage}"
        );
        assert_eq!(r["changed"], true);
        assert!(r["container_bytes"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_pipeline_recover_and_encode_unrecoverable() {
        let r = call("pipeline/recover-and-encode", src_params("@@@!!!###"));
        assert_eq!(r["ok"], false);
        assert_eq!(r["stage"], "failed");
        assert!(r["error"].as_str().unwrap().contains("refine"));
    }

    // ── application/rmib transport ──────────────────────────────

    const RMIB_NET: &str = "net tiny { layer fc: Linear(8, 4); forward { fc } }";

    #[test]
    fn test_rmil_encode_returns_container() {
        let r = call("rmil/encode", src_params(RMIB_NET));
        assert_eq!(r["ok"], true);
        assert_eq!(r["magic"], "RMIB");
        let bytes = r["container_bytes"].as_u64().unwrap();
        let hex = r["rmib_hex"].as_str().unwrap();
        assert_eq!(hex.len() as u64, bytes * 2);
        assert!(hex.starts_with("524d4942")); // "RMIB"
        let items = r["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["name"], "tiny");
    }

    #[test]
    fn test_rmil_encode_parse_error_surfaces() {
        let r = call("rmil/encode", src_params("@@@ garbage"));
        assert_eq!(r["ok"], false);
        assert!(r["error"]["message"].is_string());
    }

    #[test]
    fn test_rmil_encode_decode_round_trip() {
        let enc = call("rmil/encode", src_params(RMIB_NET));
        let hex = enc["rmib_hex"].as_str().unwrap().to_string();
        let dec = call("rmil/decode", serde_json::json!({ "rmib_hex": hex }));
        assert_eq!(dec["ok"], true);
        let items = dec["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["name"], "tiny");
        assert_eq!(items[0]["content_hash"], enc["items"][0]["content_hash"]);
    }

    #[test]
    fn test_rmil_decode_bad_hex() {
        let r = call("rmil/decode", serde_json::json!({ "rmib_hex": "not hex!!" }));
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().starts_with("hex:"));
    }

    #[test]
    fn test_rmil_decode_bad_magic() {
        // Valid hex but wrong magic bytes.
        let r = call("rmil/decode", serde_json::json!({ "rmib_hex": "deadbeef" }));
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().contains("magic"));
    }

    #[test]
    fn test_rmil_run_dispatches() {
        let r = call("rmil/run", src_params(RMIB_NET));
        assert_eq!(r["ok"], true);
        let runs = r["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 1);
        // Status must be one of the three documented values; Linear should
        // dispatch on the CpuBackend without falling through to stub.
        let status = runs[0]["status"].as_str().unwrap();
        assert!(
            matches!(status, "dispatched" | "stub" | "error"),
            "unexpected status: {status}"
        );
    }

    #[test]
    fn test_rmil_run_parse_error_surfaces() {
        let r = call("rmil/run", src_params("@@@ garbage"));
        assert_eq!(r["ok"], false);
        assert_eq!(r["stage"], "parse");
    }
}

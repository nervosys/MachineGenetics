/// RAP (MAGE Agent Protocol) — JSON-RPC over TCP server skeleton.
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
///   pipeline/recover-and-encode — source → 3-stage recover → Agentic Binary Language bytes in one call
///   abl/encode       — source → Agentic Binary Language bytes (hex) for application/abl transport
///   abl/decode       — Agentic Binary Language bytes (hex) → decompiled per-item view
///   abl/run          — source → encode → CpuBackend dispatch (no text round-trip)
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
/// via `MAGE_RAP_ALLOW_REMOTE=1` (and even then warn). See SECURITY_AUDIT.md.
pub fn serve(addr: &str) {
    if is_non_loopback(addr) {
        let allow = std::env::var("MAGE_RAP_ALLOW_REMOTE").as_deref() == Ok("1");
        if !allow {
            eprintln!(
                "rap: REFUSING to bind non-loopback address {addr}: the RAP control plane is \
                 unauthenticated and unencrypted. Bind 127.0.0.1, or front it with a reverse \
                 proxy doing authN/Z + TLS and set MAGE_RAP_ALLOW_REMOTE=1 to override."
            );
            std::process::exit(2);
        }
        eprintln!(
            "rap: WARNING binding non-loopback {addr} with no auth/TLS \
             (MAGE_RAP_ALLOW_REMOTE=1). Do not expose to untrusted networks."
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

        // A malformed frame is answered, not fatal. `from_str(&line)?`
        // propagated out of `handle_connection`, which dropped the whole
        // connection: one bad line and every later request on that socket got
        // no response and no explanation. JSON-RPC 2.0 §4.2 says reply -32700.
        let response = match serde_json::from_str::<serde_json::Value>(&line) {
            Err(e) => {
                error_response(serde_json::Value::Null, RpcError::parse_error(&e.to_string()))
            }
            Ok(request) => {
                let id = request.get("id").cloned().unwrap_or(serde_json::Value::Null);
                let params = request
                    .get("params")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                match request.get("method").and_then(|v| v.as_str()) {
                    // `unwrap_or("")` turned a request with no `method` into a
                    // request for the method named "", which then reported
                    // `unknown method: ` — a confusing way to say "malformed".
                    None => error_response(id, RpcError::invalid_request()),
                    Some(method) => match dispatch_checked(method, &params) {
                        Ok(result) => serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": result
                        }),
                        Err(e) => error_response(id, e),
                    },
                }
            }
        };

        let mut out = serde_json::to_string(&response)?;
        out.push('\n');
        writer.write_all(out.as_bytes())?;
        writer.flush()?;
    }

    Ok(())
}

/// A JSON-RPC 2.0 error object (§5.1).
///
/// Every failure the *protocol* can have, as distinct from every failure a
/// MAGE program can have. The distinction is the whole point of this type:
/// `language/parse` answering `{"ok": false, "error": {...}}` is a **successful
/// call** whose answer is "your program does not parse", and it belongs in
/// `result`. A request naming a method this server does not implement is not a
/// call at all, and belongs in `error`.
///
/// Until 2026-08-19 both came back as `result`, so a conforming client — one
/// that checks for the `error` member, which is the only thing JSON-RPC
/// guarantees — saw a typo'd method name as a success.
#[derive(Debug, Clone)]
struct RpcError {
    code: i64,
    message: String,
    data: Option<serde_json::Value>,
}

impl RpcError {
    /// -32700: the frame was not JSON.
    fn parse_error(detail: &str) -> Self {
        Self {
            code: -32700,
            message: "Parse error".to_string(),
            data: Some(serde_json::json!({ "detail": detail })),
        }
    }

    /// -32600: the frame was JSON, but not a request.
    fn invalid_request() -> Self {
        Self {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: Some(serde_json::json!({
                "detail": "a request must carry a string `method`",
            })),
        }
    }

    /// -32601: no such method.
    ///
    /// Carries the discovery hint in `data` rather than dropping it: the
    /// previous shape put a `hint` beside the error, and that was the one
    /// genuinely useful thing about it.
    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {method}"),
            data: Some(serde_json::json!({
                "method": method,
                "hint": "call `rap/methods` for the list this server actually dispatches",
            })),
        }
    }
}

fn error_response(id: serde_json::Value, e: RpcError) -> serde_json::Value {
    let mut err = serde_json::json!({ "code": e.code, "message": e.message });
    if let Some(data) = e.data {
        err["data"] = data;
    }
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": err })
}

/// [`dispatch`], with the unknown-method case lifted out of `result`.
///
/// The gate reads [`METHODS`], not the match arms — which is only sound
/// because `methods_list_matches_the_dispatcher` and
/// `every_dispatched_method_is_advertised` pin the two together in both
/// directions. Those tests existed already; this makes them load-bearing for
/// the wire format, which is noted here so that deleting one is understood to
/// change what clients see.
fn dispatch_checked(
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    if !METHODS.contains(&method) {
        return Err(RpcError::method_not_found(method));
    }
    Ok(dispatch(method, params))
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
            // Return the agent (MAGE canonical) form of source.
            //
            // It returned the *AST* — `format/human` even said "same as parse
            // for now" — under a published contract of `{formatted, ok}`. Both
            // are named `format/*`, and an agent calling either to reformat
            // source got a syntax tree. `fmt::format_agent` is what
            // `--fmt-compact` uses and has been there all along.
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let elided = elision::elide(&module);
                    serde_json::json!({
                        "ok": true,
                        "formatted": crate::fmt::format_agent(&elided),
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
            // Return the human (Rust-like) form of source. See `format/agent`
            // above: this said "same as parse for now" and returned the AST.
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => serde_json::json!({
                    "ok": true,
                    "formatted": crate::fmt::format_human(&module),
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
            // Compress a Rust attribute to MAGE shorthand.
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
                        if let crate::ast::ItemKind::Agent(ref ad) = item.kind
                            && ad.name == agent_name {
                                policy = serde_json::json!({
                                    "ok": true,
                                    "agent": ad.name,
                                    "capabilities": ad.capabilities,
                                    "requires_approval": ad.requires_approval,
                                });
                                break;
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
            // Fenced, because `nl_engine::extract_code_block` reads source
            // *only* from a ``` block. Interpolated bare, the source was
            // invisible to the extractor and `intent.source` was always
            // `None`, so this method could never succeed — it answered
            // "No source code provided" for every input, including the
            // well-formed ones. Nothing tested it.
            let prompt = format!("explain this code\n```mg\n{source}\n```");
            let mut engine = crate::nl_engine::NlEngine::new();
            let response = engine.process(&prompt);
            serde_json::json!({
                "ok": response.ok,
                "explanation": response.explanation
            })
        }

        "nl/refactor" => {
            let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
            // Fenced, because `nl_engine::extract_code_block` reads source
            // *only* from a ``` block. Interpolated bare, the source was
            // invisible to the extractor and `intent.source` was always
            // `None`, so this method could never succeed — it answered
            // "No source code provided" for every input, including the
            // well-formed ones. Nothing tested it.
            let prompt = format!("refactor this code\n```mg\n{source}\n```");
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
        // MAGE language, the Agentic Binary Language IR, and the RAP protocol. Single
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
                        "heal_patterns", "recovery_stages", "abl", "examples",
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

        // ── Agentic Binary Language binary IR transport (application/abl) ──────────

        // One-shot path: broken source → 3-stage mechanical recover →
        // parse → encode Agentic Binary Language. Saves an agent two round-trips. Returns
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
            let (blob, summary) = crate::abl::encode_module(&module);
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
                "abl_hex": crate::abl::to_hex(&blob),
            })
        }

        // Source → Agentic Binary Language bytes (hex-encoded for JSON channel).
        "abl/encode" => {
            let tokens = lexer::lex(source);
            match parser::parse(&tokens) {
                Ok(module) => {
                    let (blob, summary) = crate::abl::encode_module(&module);
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
                        "magic": "ABL1",
                        "version": crate::abl::ABL_VERSION,
                        "container_bytes": blob.len(),
                        "items": items,
                        "abl_hex": crate::abl::to_hex(&blob),
                    })
                }
                Err(e) => serde_json::json!({
                    "ok": false,
                    "error": { "line": e.line, "col": e.col, "message": e.message },
                }),
            }
        }

        // Agentic Binary Language bytes (hex) → decompiled MAGE view + per-item summary.
        "abl/decode" => {
            let hex = params.get("abl_hex").and_then(|v| v.as_str()).unwrap_or("");
            let blob = match crate::abl::from_hex(hex) {
                Ok(b) => b,
                Err(e) => return serde_json::json!({ "ok": false, "error": format!("hex: {e}") }),
            };
            match crate::abl::decode_container(&blob) {
                Ok(items) => {
                    let decoded: Vec<serde_json::Value> = items
                        .iter()
                        .map(|it| {
                            let result = crate::abl_bridge::decompile(&it.expr, &it.name);
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
        "abl/run" => {
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
            let (blob, _) = crate::abl::encode_module(&module);
            let items = match crate::abl::decode_container(&blob) {
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
                    let families = crate::abl_bridge::expr_op_families(&it.expr);
                    let stub_families: Vec<String> = families
                        .iter()
                        .filter(|f| crate::abl_bridge::is_stubbed_family(**f))
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
                    let inferred = crate::abl_compute::infer_input_shape(&it.expr);
                    let shape: Vec<usize> = inferred.unwrap_or_else(|| vec![8]);
                    match crate::abl_compute::run_pipeline(&backend, &it.expr, &shape, 1.0) {
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

        // Method discovery. The rest of this toolchain is self-describing —
        // `--build=schema` exists so an agent need not read documentation — and
        // the RAP server was the one surface where the only way to learn the
        // method list was to read `rap.rs` or a document. That is how four
        // method names in ROADMAP step 36 (`format/compact`, `format/expand`,
        // `grammar/extensions`, `grammar/expand`) stayed listed as delivered
        // while dispatching nothing: no client could have noticed.
        //
        // Derived from `METHODS` below, which is asserted against the real
        // dispatch arms by a test, so this cannot drift from what the server
        // actually answers.
        "rap/methods" => serde_json::json!({
            "ok": true,
            "count": METHODS.len(),
            "methods": METHODS,
        }),

        // Not reachable from the wire: `dispatch_checked` rejects anything
        // outside `METHODS` before calling in here, so a client gets a
        // JSON-RPC -32601 instead. This arm survives as the invariant guard
        // for the other direction — a name in `METHODS` with no arm above —
        // which is exactly what `methods_list_matches_the_dispatcher` probes,
        // and it must stay a value rather than a panic for that test to read
        // it.
        _ => serde_json::json!({
            "error": format!("unknown method: {method}"),
            "hint": "call `rap/methods` for the list this server actually dispatches",
        }),
    }
}

/// Every method [`dispatch`] answers.
///
/// Kept beside the dispatcher and checked against it by
/// `methods_list_matches_the_dispatcher`, so the advertised surface and the
/// implemented one cannot disagree.
pub const METHODS: &[&str] = &[
    "abl/decode",
    "abl/encode",
    "abl/run",
    "attribute/compress",
    "attribute/expand",
    "build/check",
    "build/heal",
    "build/recover",
    "capability/check",
    "cost/compare",
    "cost/query",
    "doc/query",
    "effects/check",
    "effects/infer",
    "elision/apply",
    "format/agent",
    "format/human",
    "grammar/list",
    "heal/graph",
    "language/parse",
    "language/tokens",
    "lint/check",
    "manifest/generate",
    "nl/explain",
    "nl/generate",
    "nl/query",
    "nl/refactor",
    "ontology/full",
    "ontology/section",
    "pipeline/recover-and-encode",
    "rap/methods",
    "sandbox/policy",
    "skb/query",
    "skb/rules",
    "skb/spec",
    "token/report",
    "verify/contracts",
    "verify/module",
];

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
        assert!(!r.get("tokens").unwrap().as_array().unwrap().is_empty());
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

    /// Every method the ontology publishes actually dispatches.
    ///
    /// The published list was checked by grepping `rap.rs` for the method
    /// name — which proves the string is in the file, not that the server
    /// answers to it. Same shape as the ontology being pinned byte-for-byte
    /// against its own generator: agreement, not truth.
    ///
    /// Params are omitted deliberately. A method that needs them may fail on
    /// the missing argument; what must not happen is the *unknown method*
    /// response, which is the one an agent reading the ontology would get for
    /// a name that does not exist.
    #[test]
    fn every_published_rap_method_dispatches() {
        let published = crate::ontology::section("rap_methods").expect("rap_methods section");
        let rows = published.as_array().expect("array");
        assert!(rows.len() >= 37, "rap_methods shrank to {}", rows.len());

        for row in rows {
            let method = row["method"].as_str().expect("method name");
            let r = call(method, serde_json::json!({}));
            let unknown = r["error"]
                .as_str()
                .map(|e| e.contains("unknown method"))
                .unwrap_or(false);
            assert!(!unknown, "ontology publishes `{method}`, which does not dispatch: {r}");
        }
    }

    /// Every published parameter is read, and every published return key comes
    /// back from a successful call.
    ///
    /// `every_published_rap_method_dispatches` calls each method with `{}` and
    /// asserts only that it is not "unknown" — which a method that always
    /// errors still passes, and which said nothing about the *contract*. Eight
    /// methods read parameters the ontology did not publish and seventeen
    /// returned keys it did not name: an agent calling `skb/query {"query":…}`
    /// as published got `matches: []`, because the code reads `by`/`value` and
    /// defaults to the empty string. `build/check` publishes `diagnostics` and
    /// returns `errors`.
    ///
    /// This one calls each method with inputs that succeed and checks the
    /// answer's shape.
    #[test]
    fn every_published_rap_key_is_real() {
        const SRC: &str = "+f add(a: i32, b: i32) -> i32 { a + b }";
        // `abl/decode` is the only method whose input is another method's
        // output, so it is produced rather than written down — a hand-copied
        // hex string would go stale the next time the container format moves.
        let abl_hex = call("abl/encode", serde_json::json!({ "source": SRC }))["abl_hex"]
            .as_str()
            .expect("abl/encode returns abl_hex")
            .to_string();
        // Inputs chosen so each method takes its success path. A method that
        // cannot succeed with any input is a finding, not a reason to skip it.
        let calls: Vec<(&str, serde_json::Value)> = vec![
            ("language/parse", serde_json::json!({ "source": SRC })),
            ("language/tokens", serde_json::json!({ "source": SRC })),
            ("build/check", serde_json::json!({ "source": SRC })),
            ("build/heal", serde_json::json!({ "source": SRC })),
            ("build/recover", serde_json::json!({ "source": SRC })),
            ("abl/encode", serde_json::json!({ "source": SRC })),
            ("abl/run", serde_json::json!({ "source": SRC })),
            ("pipeline/recover-and-encode", serde_json::json!({ "source": SRC })),
            ("cost/query",
                serde_json::json!({ "construct": "Vec::push", "target": "x86_64", "opt": "release" })),
            ("cost/compare",
                serde_json::json!({ "a": "Vec::push", "b": "stack array", "target": "x86_64" })),
            ("skb/query", serde_json::json!({ "by": "fqn", "value": "std" })),
            ("skb/rules", serde_json::json!({ "domain": "" })),
            ("verify/module", serde_json::json!({ "source": SRC })),
            ("format/agent", serde_json::json!({ "source": SRC })),
            ("format/human", serde_json::json!({ "source": SRC })),
            ("lint/check", serde_json::json!({ "source": SRC })),
            ("token/report", serde_json::json!({ "source": SRC })),
            ("effects/infer", serde_json::json!({ "source": SRC })),
            ("effects/check", serde_json::json!({ "source": SRC })),
            ("elision/apply", serde_json::json!({ "source": SRC })),
            ("attribute/expand", serde_json::json!({ "name": "d" })),
            ("attribute/compress", serde_json::json!({ "name": "derive" })),
            ("capability/check",
                serde_json::json!({ "source": "agent A { capabilities: [io] }" })),
            ("heal/graph", serde_json::json!({ "source": "+f broken( { }" })),
            ("doc/query", serde_json::json!({ "fqn": "map" })),
            ("grammar/list", serde_json::json!({})),
            ("manifest/generate",
                serde_json::json!({ "source": SRC, "crate_name": "demo", "version": "0.1.0" })),
            ("nl/generate", serde_json::json!({ "prompt": "add two numbers" })),
            ("nl/explain", serde_json::json!({ "source": SRC })),
            ("nl/query", serde_json::json!({ "prompt": "add two numbers" })),
            ("ontology/section", serde_json::json!({ "section": "vocabulary" })),
            ("ontology/full", serde_json::json!({})),
            ("abl/decode", serde_json::json!({ "abl_hex": abl_hex })),
            // An fqn that carries a spec block. Most SKB entries do not, and
            // for those the arm returns `ok: false` — a working call has to
            // name one that does.
            ("skb/spec", serde_json::json!({ "fqn": "std.io.read_file" })),
            ("verify/contracts", serde_json::json!({
                "fqn": "add",
                "requires": [],
                "ensures": [],
                "declared_effects": [],
                "used_effects": [],
            })),
            ("sandbox/policy", serde_json::json!({
                "source": "agent Planner { capabilities: [io] }",
                "agent": "Planner",
            })),
            ("nl/refactor", serde_json::json!({ "source": SRC })),
        ];

        let published = crate::ontology::section("rap_methods").expect("rap_methods");
        let rows = published.as_array().expect("array");

        // Both directions. The list above is written by hand, so without this
        // the test's own doc comment ("every published parameter…") would be a
        // claim about whatever someone remembered to add: the first version of
        // this test exercised 31 of 37 methods and said nothing about the other
        // six. A new method must be given a working call here, or fail.
        for row in rows {
            let method = row["method"].as_str().expect("method");
            assert!(
                calls.iter().any(|(m, _)| *m == method),
                "`{method}` is published and has no working call in this test — \
                 its published contract is unchecked"
            );
        }

        for (method, params) in &calls {
            let row = rows
                .iter()
                .find(|r| r["method"].as_str() == Some(method))
                .unwrap_or_else(|| panic!("`{method}` is called here and not published"));

            // Every published parameter must be one the arm reads: the params
            // we pass are the ones the code looks for, so the published list
            // has to agree with them.
            let sent: Vec<&str> = params.as_object().unwrap().keys().map(|k| k.as_str()).collect();
            let pubbed: Vec<&str> =
                row["params"].as_array().unwrap().iter().map(|p| p.as_str().unwrap()).collect();
            for p in &pubbed {
                assert!(
                    sent.contains(p),
                    "`{method}` publishes parameter `{p}`, which the working \
                     call does not use — the published name is wrong"
                );
            }
            // And the other direction, which is the one that hides real gaps:
            // the calls above are minimal, so anything they pass is something
            // the method needs. `sandbox/policy` published only `agent` while
            // reading the module out of `source` — an agent following the
            // published list got `agent `X` not found` for every agent that
            // existed, because it never sent the program to look in.
            for p in &sent {
                assert!(
                    pubbed.contains(p),
                    "`{method}` needs parameter `{p}` and does not publish it — \
                     a caller following the ontology cannot make this call work"
                );
            }

            let r = call(method, params.clone());
            for key in row["returns"].as_array().unwrap() {
                let key = key.as_str().unwrap();
                // `error` only appears on the failure path.
                if key == "error" {
                    continue;
                }
                assert!(
                    r.get(key).is_some(),
                    "`{method}` publishes return key `{key}`, which is not in \
                     the response: {r}"
                );
            }
        }
    }

    /// The four `nl/*` methods had no test at all.
    ///
    /// They are the natural-language surface — the methods an agent reaches
    /// for first — and the only ones of the 37 that nothing exercised.
    #[test]
    fn nl_methods_answer() {
        let r = call("nl/generate", serde_json::json!({ "prompt": "add two numbers" }));
        assert_eq!(r["ok"], true, "nl/generate: {r}");
        assert!(r["code_agent"].is_string(), "nl/generate must return code_agent: {r}");

        let r = call("nl/explain", serde_json::json!({ "source": "+f a() -> i32 { 1 }" }));
        assert_eq!(r["ok"], true, "nl/explain: {r}");
        assert!(r["explanation"].is_string(), "nl/explain must explain: {r}");

        let r = call("nl/refactor", serde_json::json!({ "source": "+f a() -> i32 { 1 }" }));
        assert_eq!(r["ok"], true, "nl/refactor: {r}");

        let r = call("nl/query", serde_json::json!({ "prompt": "what effects exist" }));
        assert_eq!(r["ok"], true, "nl/query: {r}");
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
            "heal_patterns", "recovery_stages", "abl",
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
        assert!(r["abl_hex"].as_str().unwrap().starts_with("41424c31"));
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

    // ── application/abl transport ──────────────────────────────

    const ABL_NET: &str = "net tiny { layer fc: Linear(8, 4); forward { fc } }";

    #[test]
    fn test_abl_encode_returns_container() {
        let r = call("abl/encode", src_params(ABL_NET));
        assert_eq!(r["ok"], true);
        assert_eq!(r["magic"], "ABL1");
        let bytes = r["container_bytes"].as_u64().unwrap();
        let hex = r["abl_hex"].as_str().unwrap();
        assert_eq!(hex.len() as u64, bytes * 2);
        assert!(hex.starts_with("41424c31")); // "ABL1"
        let items = r["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["name"], "tiny");
    }

    #[test]
    fn test_abl_encode_parse_error_surfaces() {
        let r = call("abl/encode", src_params("@@@ garbage"));
        assert_eq!(r["ok"], false);
        assert!(r["error"]["message"].is_string());
    }

    #[test]
    fn test_abl_encode_decode_round_trip() {
        let enc = call("abl/encode", src_params(ABL_NET));
        let hex = enc["abl_hex"].as_str().unwrap().to_string();
        let dec = call("abl/decode", serde_json::json!({ "abl_hex": hex }));
        assert_eq!(dec["ok"], true);
        let items = dec["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["name"], "tiny");
        assert_eq!(items[0]["content_hash"], enc["items"][0]["content_hash"]);
    }

    #[test]
    fn test_abl_decode_bad_hex() {
        let r = call("abl/decode", serde_json::json!({ "abl_hex": "not hex!!" }));
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().starts_with("hex:"));
    }

    #[test]
    fn test_abl_decode_bad_magic() {
        // Valid hex but wrong magic bytes.
        let r = call("abl/decode", serde_json::json!({ "abl_hex": "deadbeef" }));
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().contains("magic"));
    }

    #[test]
    fn test_abl_run_dispatches() {
        let r = call("abl/run", src_params(ABL_NET));
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
    fn test_abl_run_parse_error_surfaces() {
        let r = call("abl/run", src_params("@@@ garbage"));
        assert_eq!(r["ok"], false);
        assert_eq!(r["stage"], "parse");
    }
}

#[cfg(test)]
mod method_surface_tests {
    use super::*;

    /// `METHODS` must list exactly what `dispatch` answers.
    ///
    /// The list is what `rap/methods` advertises, so a drift here would make the
    /// server lie about itself — which is the failure this endpoint was added to
    /// prevent, and it would be perverse for the fix to reintroduce it one level
    /// up. Detected by asking the dispatcher: an advertised method must not
    /// answer "unknown method".
    #[test]
    fn methods_list_matches_the_dispatcher() {
        for m in METHODS {
            let got = dispatch(m, &serde_json::Value::Null);
            let unknown = got
                .get("error")
                .and_then(|e| e.as_str())
                .map(|e| e.starts_with("unknown method"))
                .unwrap_or(false);
            assert!(!unknown, "`{m}` is advertised by rap/methods but not dispatched");
        }
    }

    /// The reverse direction: every method the dispatcher answers must be
    /// advertised.
    ///
    /// The first version of this test only checked advertised → dispatched, and
    /// passed while `METHODS` was missing `pipeline/recover-and-encode` — the
    /// list was built with a regex that did not allow hyphens. A one-directional
    /// check on a two-directional invariant is the same mistake as a checker
    /// that only ever passes, made one level up.
    ///
    /// Reads this file's own source because match arms cannot be enumerated at
    /// runtime. Brittle if the dispatcher is restructured, and that is
    /// acceptable: it fails loudly and points at the list to update.
    #[test]
    fn every_dispatched_method_is_advertised() {
        let src = include_str!("rap.rs");
        let mut missing = Vec::new();
        for line in src.lines() {
            let t = line.trim();
            // Match arms look like `"ns/name" => {` or `"a/b" | "c/d" => {`.
            if !t.starts_with('"') || !t.contains("=>") {
                continue;
            }
            for tok in t.split("=>").next().unwrap_or("").split('|') {
                let name = tok.trim().trim_matches('"').trim();
                if name.contains('/') && !name.contains(' ') && !METHODS.contains(&name) {
                    missing.push(name.to_string());
                }
            }
        }
        assert!(missing.is_empty(), "dispatched but not in METHODS: {missing:?}");
    }

    #[test]
    fn methods_are_sorted_and_unique() {
        let mut sorted = METHODS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.as_slice(), METHODS, "keep METHODS sorted and duplicate-free");
    }

    /// An unknown method is a JSON-RPC `error`, not a `result` that contains
    /// the word "error".
    ///
    /// The old shape was `{"result": {"error": "unknown method: ..."}}` — a
    /// success envelope carrying a failure. A client written against the spec
    /// checks for the `error` member and nothing else, so a typo'd method name
    /// read as a successful call returning an object, and the mistake surfaced
    /// wherever that object was later indexed. This asserts the full envelope,
    /// not just the code, because the defect was in the envelope.
    #[test]
    fn an_unknown_method_is_a_jsonrpc_error_not_a_result() {
        let e = dispatch_checked("no/such_method", &serde_json::Value::Null)
            .expect_err("an unimplemented method must not succeed");
        assert_eq!(e.code, -32601, "JSON-RPC 2.0 §5.1: Method not found");

        let envelope = error_response(serde_json::json!(7), e);
        assert!(
            envelope.get("result").is_none(),
            "a failed call must not carry a `result` member: {envelope}"
        );
        assert_eq!(envelope["id"], serde_json::json!(7), "the id must come back");
        assert_eq!(envelope["jsonrpc"], "2.0");
        assert_eq!(envelope["error"]["code"], -32601);
        assert!(
            envelope["error"]["data"]["hint"]
                .as_str()
                .unwrap_or("")
                .contains("rap/methods"),
            "a wrong method name is the moment to point at discovery: {envelope}"
        );
    }

    /// A malformed frame is answered rather than dropping the connection.
    ///
    /// `serde_json::from_str(&line)?` propagated out of `handle_connection`,
    /// which closed the socket. The client saw a hang and then EOF, with
    /// nothing to say which of its frames was bad.
    #[test]
    fn a_malformed_frame_gets_an_answer_with_a_null_id() {
        let e = serde_json::from_str::<serde_json::Value>("{not json")
            .expect_err("that is not JSON");
        let envelope = error_response(serde_json::Value::Null, RpcError::parse_error(&e.to_string()));
        assert_eq!(envelope["error"]["code"], -32700);
        assert!(envelope["result"].is_null() && envelope.get("result").is_none());
        assert_eq!(envelope["id"], serde_json::Value::Null, "no id is recoverable from a bad frame");
    }

    /// A frame with no `method` is Invalid Request, not method `""`.
    #[test]
    fn a_request_without_a_method_is_invalid_not_unknown() {
        let envelope = error_response(serde_json::json!(1), RpcError::invalid_request());
        assert_eq!(envelope["error"]["code"], -32600);
        assert!(
            !envelope["error"]["message"].as_str().unwrap_or("").contains("unknown"),
            "the old path reported `unknown method: ` for a missing method"
        );
    }

    /// Every advertised method survives the gate, and the gate is the only
    /// thing standing between a client and `dispatch`.
    ///
    /// Pins the coupling that `dispatch_checked`'s doc comment relies on: if
    /// `METHODS` and the match arms ever diverge, the wire format diverges
    /// with them.
    #[test]
    fn the_gate_admits_exactly_the_advertised_methods() {
        for m in METHODS {
            assert!(
                dispatch_checked(m, &serde_json::Value::Null).is_ok(),
                "`{m}` is advertised but the gate rejects it"
            );
        }
        for m in ["", "no/such_method", "language/Tokens", "rap/methods "] {
            assert!(
                dispatch_checked(m, &serde_json::Value::Null).is_err(),
                "`{m}` is not advertised and must not reach the dispatcher"
            );
        }
    }
}

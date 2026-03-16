/// RAP (Redox Agent Protocol) — JSON-RPC over TCP server skeleton.
///
/// Provides language services for AI agents:
///   language/parse    — parse source to AST (JSON)
///   language/tokens   — tokenize source
///   build/check       — check syntax (parse + report errors)
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use crate::lexer;
use crate::parser;

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

        _ => serde_json::json!({
            "error": format!("unknown method: {method}")
        }),
    }
}

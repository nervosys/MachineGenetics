mod ast;
mod lexer;
mod parser;
mod rap;

use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("--rap") => {
            let addr = args.get(2).map(|s| s.as_str()).unwrap_or("127.0.0.1:9876");
            rap::serve(addr);
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

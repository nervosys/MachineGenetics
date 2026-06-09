//! Canonical-form token measurement.
//!
//! The token-bench (`bin/token_bench.rs`) counts the corpus's `rdx_source`
//! *verbatim*. But the corpus was authored in a verbose, Rust-mirroring style
//! (`Option<String>`, `Result<T, E>`, explicit binding types) even though
//! MechGen's *canonical* surface is terser (`?String`, `T or E`, sigils). The
//! formatter defines "idiomatic", so the honest measure of typical agent-written
//! token cost is the **canonical-formatted** source, not the hand-authored one.
//!
//! This module re-tokenises every corpus solution after round-tripping it
//! through `lex → parse → format`, and reports the reduction. It runs as a
//! `--nocapture` test so it has access to the in-crate parser/formatter (there
//! is no lib target). Parse failures fall back to the raw count (conservative:
//! a source we can't canonicalize gets no credit), and the fallback rate is
//! reported so the figure can't silently overstate.

#[cfg(test)]
mod measure {
    use crate::{fmt, lexer, parser};
    use std::path::PathBuf;

    fn native_tokens(src: &str) -> usize {
        lexer::lex(src)
            .iter()
            .filter(|t| t.kind != lexer::TokenKind::Eof && t.kind != lexer::TokenKind::Error)
            .count()
    }

    fn tasks_dir() -> PathBuf {
        // CARGO_MANIFEST_DIR = .../MechGen/prototype ; corpus = ../benchmarks/tasks
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.pop();
        p.push("benchmarks");
        p.push("tasks");
        p
    }

    #[test]
    fn report_canonical_token_reduction() {
        let dir = tasks_dir();
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .expect("read tasks dir")
            .filter_map(|r| r.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();

        let mut raw_total = 0usize;
        let mut agent_total = 0usize; // format_agent: R[T,E]
        let mut human_total = 0usize; // format_human: T or E
        let mut n = 0usize;
        let mut parsed = 0usize;
        let mut parse_fail = 0usize;

        for f in &files {
            let content = std::fs::read_to_string(f).expect("read task file");
            let arr: serde_json::Value = serde_json::from_str(&content).expect("parse json");
            let Some(items) = arr.as_array() else { continue };
            for it in items {
                let Some(src) = it
                    .get("solution")
                    .and_then(|s| s.get("rdx_source"))
                    .and_then(|s| s.as_str())
                else {
                    continue;
                };
                n += 1;
                let raw = native_tokens(src);
                raw_total += raw;

                let toks = lexer::lex(src);
                match parser::parse(&toks) {
                    Ok(module) => {
                        parsed += 1;
                        agent_total += native_tokens(&fmt::format_agent(&module));
                        human_total += native_tokens(&fmt::format_human(&module));
                    }
                    Err(_) => {
                        // Conservative: a source we cannot canonicalize keeps
                        // its raw cost on both canonical sides.
                        parse_fail += 1;
                        agent_total += raw;
                        human_total += raw;
                    }
                }
            }
        }

        let pct = |c: usize| 100.0 * (1.0 - c as f64 / raw_total as f64);
        println!("\n=== Canonical-form token measurement ({n} solutions) ===");
        println!("parsed {parsed}/{n}  (parse-fail fallback to raw: {parse_fail})");
        println!("raw native tokens (as-authored):    {raw_total}");
        println!(
            "canonical (format_agent, R[T,E]):   {agent_total}   ({:.1}% reduction)",
            pct(agent_total)
        );
        println!(
            "canonical (format_human, T or E):   {human_total}   ({:.1}% reduction)",
            pct(human_total)
        );
        println!("(run with: cargo test report_canonical_token_reduction -- --nocapture)\n");
    }
}

//! Plan 9.3: one canonical form. Every `.mg` source in the repository, printed
//! by either surface (`--fmt-compact`, the sigil projection agents write in, or
//! `--fmt-expand`, the keyword projection), reparses to the identical AST, and
//! printing that AST again gives the same text. So the AST is the language and
//! both surfaces are lossless views of it.
//!
//! The canonical form is what the parser reads. When this test was first run,
//! 14 of 103 sources survived: the agent printer used sigils the parser has no
//! rule for (`Ψ` for a net, `⊢` for a kb fact), printed stubs (`forward { ...
//! }`), dropped the `@` that makes `@P { .. }` a struct literal, and printed
//! `String`, which the parser reads as a path rather than the string type `s`.
//! One print also crashed the lexer (an unknown multi-byte character).
//!
//! A failure names the file, the surface, and the first AST path that differs.

use mage_prototype::{fmt, lexer, parser};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn corpus(root: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(root).unwrap().flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if p.is_dir() {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            corpus(&p, out);
        } else if name.ends_with(".mg") {
            out.push(p);
        }
    }
}

fn parse(src: &str) -> Result<mage_prototype::ast::Module, String> {
    let src = src.to_string();
    match std::panic::catch_unwind(move || parser::parse(&lexer::lex(&src))) {
        Err(_) => Err("PANIC".into()),
        Ok(r) => r.map_err(|e| format!("{}:{}: {}", e.line, e.col, e.message)),
    }
}

fn first_diff(a: &Value, b: &Value, path: String) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, va) in x {
                match y.get(k) {
                    None => return Some(format!("{path}.{k} missing")),
                    Some(vb) => {
                        if let Some(d) = first_diff(va, vb, format!("{path}.{k}")) {
                            return Some(d);
                        }
                    }
                }
            }
            for k in y.keys() {
                if !x.contains_key(k) {
                    return Some(format!("{path}.{k} added"));
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            for (i, (va, vb)) in x.iter().zip(y).enumerate() {
                if let Some(d) = first_diff(va, vb, format!("{path}[{i}]")) {
                    return Some(d);
                }
            }
            if x.len() != y.len() {
                return Some(format!("{path} len {} -> {}", x.len(), y.len()));
            }
            None
        }
        _ if a == b => None,
        _ => {
            let s = |v: &Value| {
                let t = v.to_string();
                t.chars().take(160).collect::<String>()
            };
            Some(format!("{path}: {} -> {}", s(a), s(b)))
        }
    }
}

fn line_of(s: &str, err: &str) -> String {
    let n: usize = err.split(':').next().and_then(|x| x.parse().ok()).unwrap_or(0);
    s.lines().nth(n.saturating_sub(1)).unwrap_or("").chars().take(120).collect()
}

#[test]
fn every_source_round_trips_through_both_surfaces() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    corpus(root, &mut files);
    files.sort();
    let mut report = String::new();
    let (mut ok, mut skip, mut bad) = (0, 0, 0);
    for f in &files {
        let rel = f.strip_prefix(root).unwrap().display().to_string();
        let src = std::fs::read_to_string(f).unwrap();
        let m = match parse(&src) {
            Ok(m) => m,
            Err(e) => {
                skip += 1;
                report += &format!("{rel}: original does not parse: {e}\n");
                continue;
            }
        };
        let a0 = serde_json::to_value(&m).unwrap();
        let mut errs = Vec::new();
        for (mode, print) in [
            ("agent", fmt::format_agent as fn(&_) -> String),
            ("human", fmt::format_human),
        ] {
            let s1 = print(&m);
            match parse(&s1) {
                Err(e) => errs.push(format!(
                    "  {mode}: reparse failed {e}\n      | {}",
                    line_of(&s1, &e)
                )),
                Ok(m1) => {
                    let a1 = serde_json::to_value(&m1).unwrap();
                    if let Some(d) = first_diff(&a0, &a1, String::new()) {
                        errs.push(format!("  {mode}: AST {d}"));
                    }
                    if print(&m1) != s1 {
                        errs.push(format!("  {mode}: not a fixpoint"));
                    }
                }
            }
        }
        if errs.is_empty() {
            ok += 1
        } else {
            bad += 1;
            report += &format!("{rel}\n{}\n", errs.join("\n"));
        }
    }
    // A walk that finds nothing would pass vacuously.
    assert!(ok >= 100, "only {ok} sources found under {}", root.display());
    assert!(
        bad == 0 && skip == 0,
        "{report}{ok} round-trip, {bad} do not, {skip} do not parse"
    );
}

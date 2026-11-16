//! Source positions in the AST are invisible to serde, equality and the
//! canonical hash: the same program at different positions serialises, hashes
//! and compares identically.

use mage_prototype::{ast, canon, lexer, parser};

fn parse(src: &str) -> ast::Module {
    parser::parse(&lexer::lex(src)).expect("parses")
}

const SRC: &str = "f take(x: s) -> s { x }\nf host(a: s) {\n    v b = a\n    take(a)\n    take(a)\n}\n";

#[test]
fn json_and_canonical_hash_ignore_positions() {
    let plain = parse(SRC);
    let shifted = parse(&format!("// header\n\n\n{}", SRC.replace("    ", "          ")));

    // The positions exist...
    let ast::ItemKind::Function(h) = &plain.items[1].kind else { panic!() };
    assert!(h.body.pos.is_set() && h.body.stmt_pos.iter().all(|p| p.is_set()));
    assert_eq!(plain.starts, vec![1, 2]);
    assert_eq!(shifted.starts, vec![4, 5]);

    // ...but serde output has none of them and is byte-identical.
    let (a, b) = (serde_json::to_string(&plain).unwrap(), serde_json::to_string(&shifted).unwrap());
    assert_eq!(a, b);
    for key in ["\"pos\"", "stmt_pos", "tail_pos", "starts"] {
        assert!(!a.contains(key), "{key} leaked into the JSON: {a}");
    }
    // The pre-change shape: a block is exactly `stmts` + `tail_expr`.
    assert!(a.contains("\"body\":{\"stmts\":["), "{a}");

    // Canonical hashes of every function agree.
    for (x, y) in plain.items.iter().zip(&shifted.items) {
        if let (ast::ItemKind::Function(x), ast::ItemKind::Function(y)) = (&x.kind, &y.kind) {
            assert_eq!(canon::definition_hash(x), canon::definition_hash(y));
            assert_eq!(canon::shape_hash(x), canon::shape_hash(y));
        }
    }
}

#[test]
fn positions_are_relative_to_the_item_so_a_moved_item_debugs_the_same() {
    let a = parse(SRC);
    let b = parse(&format!("\n\n\n{SRC}"));
    for (x, y) in a.items.iter().zip(&b.items) {
        assert_eq!(format!("{x:?}"), format!("{y:?}"), "an item's Debug form must not depend on where it sits");
    }
}

#[test]
fn positions_round_trip_only_when_asked_for() {
    let m = parse(SRC);
    let with = ast::with_positions(|| serde_json::to_string(&m).unwrap());
    assert!(with.contains("stmt_pos"));
    let back: ast::Module = serde_json::from_str(&with).unwrap();
    assert_eq!(format!("{m:?}"), format!("{back:?}"));
    // Deserialising JSON that has no positions still works (they default).
    let plain: ast::Module = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
    assert!(plain.starts.is_empty());
}

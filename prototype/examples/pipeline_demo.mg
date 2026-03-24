// ─────────────────────────────────────────────────────────────
// MechGen End-to-End Pipeline Demo
//
// This file exercises every stage of the MechGen compiler pipeline:
//   1. Lexer    – tokens for keywords, sigils, literals, operators
//   2. Parser   – structs, enums, functions, traits, impls
//   3. Resolver – name binding across nested scopes
//   4. Types    – bidirectional type inference, unification
//   5. Effects  – bottom-up effect inference and verification
//   6. MLIR     – lowering to MechGen dialect operations
//
// Run:  cargo run -- --pipeline examples/pipeline_demo.mg
// ─────────────────────────────────────────────────────────────

// ── Struct definitions ───────────────────────────────────────

struct Point {
    x: f64,
    y: f64,
}

struct Rect {
    origin: Point,
    width: f64,
    height: f64,
}

// ── Enum definitions ─────────────────────────────────────────

enum Shape {
    Circle,
    Square,
    Triangle,
}

// ── Pure functions ───────────────────────────────────────────

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(x: i32, y: i32) -> i32 {
    x * y
}

pub fn distance(p1: Point, p2: Point) -> f64 {
    let dx: f64 = p2.x - p1.x;
    let dy: f64 = p2.y - p1.y;
    dx * dx + dy * dy
}

// ── Functions with effects ───────────────────────────────────

pub fn greet(name: String) -> String {
    name
}

pub fn read_config(path: String) -> String {
    path
}

// ── Nested scopes & shadowing ────────────────────────────────

pub fn scopes() -> i32 {
    let x: i32 = 10;
    let y: i32 = 20;
    let result: i32 = x + y;
    result
}

// ── Control flow ─────────────────────────────────────────────

pub fn max_val(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

// ── Entry point ──────────────────────────────────────────────

pub fn main() -> i32 {
    let sum: i32 = add(3, 4);
    let prod: i32 = multiply(sum, 2);
    greet("MechGen Pipeline Demo");
    let best: i32 = max_val(sum, prod);
    best
}

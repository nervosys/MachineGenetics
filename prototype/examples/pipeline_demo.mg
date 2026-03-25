// ─────────────────────────────────────────────────────────────
// MechGen End-to-End Pipeline Demo
//
// This file exercises every stage of the MechGen compiler pipeline:
//   1. Lexer    – tokens for keywords, sigils, literals, operators
//   2. Parser   – records, sums, functions, signatures, extensions
//   3. Resolver – name binding across nested scopes
//   4. Types    – bidirectional type inference, unification
//   5. Effects  – bottom-up effect inference and verification
//   6. MLIR     – lowering to MechGen dialect operations
//
// Run:  cargo run -- --pipeline examples/pipeline_demo.mg
// ─────────────────────────────────────────────────────────────

// ── Record definitions ───────────────────────────────────────

rec Point {
    x: f64,
    y: f64,
}

rec Rect {
    origin: Point,
    width: f64,
    height: f64,
}

// ── Sum type definitions ─────────────────────────────────────

sum Shape {
    Circle,
    Square,
    Triangle,
}

// ── Pure functions ───────────────────────────────────────────

exp def add(a: i32, b: i32) -> i32 {
    a + b
}

exp def multiply(x: i32, y: i32) -> i32 {
    x * y
}

exp def distance(p1: Point, p2: Point) -> f64 {
    val dx: f64 = p2.x - p1.x;
    val dy: f64 = p2.y - p1.y;
    dx * dx + dy * dy
}

// ── Functions with effects ───────────────────────────────────

exp def greet(name: String) -> String {
    name
}

exp def read_config(path: String) -> String {
    path
}

// ── Nested scopes & shadowing ────────────────────────────────

exp def scopes() -> i32 {
    val x: i32 = 10;
    val y: i32 = 20;
    val result: i32 = x + y;
    result
}

// ── Control flow ─────────────────────────────────────────────

exp def max_val(a: i32, b: i32) -> i32 {
    when a > b { a } or { b }
}

// ── Entry point ──────────────────────────────────────────────

exp def main() -> i32 {
    val sum: i32 = add(3, 4);
    val prod: i32 = multiply(sum, 2);
    greet("MechGen Pipeline Demo");
    val best: i32 = max_val(sum, prod);
    best
}

// ─────────────────────────────────────────────────────────────
// MechGen End-to-End Pipeline Demo
//
// This file exercises every stage of the MechGen compiler pipeline:
//   1. Lexer    – tokens for keywords, sigils, literals, operators
//   2. Parser   – data records, data sums, functions, extensions
//   3. Resolver – name binding across nested scopes
//   4. Types    – bidirectional type inference, unification
//   5. Effects  – bottom-up effect inference and verification
//   6. MLIR     – lowering to MechGen dialect operations
//
// Demonstrates:
//   - data keyword (record and sum forms)
//   - extend keyword (replaces impl)
//   - val / var (replaces let / let mut)
//   - guard ... else { } (early exit)
//   - defer expr (cleanup on scope exit)
//   - Pipeline operator |> (data transformation chains)
//   - is pattern (pattern test expression)
//   - T or E (error union type)
//   - Expression-body functions (-> T = expr)
//   - Default parameter values
//
// Run:  cargo run -- --pipeline examples/pipeline_demo.mg
// ─────────────────────────────────────────────────────────────

// ── Record definitions ───────────────────────────────────────

data Point(x: f64, y: f64)

data Rect(origin: Point, width: f64, height: f64)

// ── Sum type definitions ─────────────────────────────────────

data Shape = Circle | Square | Triangle

// ── Pure functions ───────────────────────────────────────────

// Expression-body functions: no block needed for single-expression returns.
pub fn add(a: i32, b: i32) -> i32 = a + b

pub fn multiply(x: i32, y: i32) -> i32 = x * y

pub fn distance(p1: Point, p2: Point) -> f64 {
    val dx: f64 = p2.x - p1.x;
    val dy: f64 = p2.y - p1.y;
    dx * dx + dy * dy
}

// ── Functions with error unions ──────────────────────────────

// T or E replaces Result<T, E>.
pub fn parse_config(path: String) -> String or Error {
    guard path.len() > 0 else { return Err(Error.new("empty path")); }
    path
}

// ── Default parameter values ─────────────────────────────────

pub fn greet(name: String, prefix: String = "Hello") -> String {
    format!("{prefix}, {name}!")
}

// ── Nested scopes & shadowing ────────────────────────────────

pub fn scopes() -> i32 {
    val x: i32 = 10;
    val y: i32 = 20;
    val result: i32 = x + y;
    result
}

// ── Control flow ─────────────────────────────────────────────

pub fn max_val(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

// ── Pipeline & pattern test ──────────────────────────────────

pub fn pipeline_demo() -> i32 {
    val result = 10 |> add(5) |> multiply(2);
    result
}

// ── Guard & defer ────────────────────────────────────────────

pub fn guarded_work(input: ?String) -> String or Error {
    // guard for early exit when precondition fails.
    guard input is Some(_) else { return Err(Error.new("no input")); }

    // defer runs cleanup when scope exits.
    defer io.println("work complete");

    "done"
}

// ── Entry point ──────────────────────────────────────────────

pub fn main() -> i32 {
    val sum: i32 = add(3, 4);
    val prod: i32 = multiply(sum, 2);
    greet("MechGen Pipeline Demo");
    val best: i32 = max_val(sum, prod);
    best
}

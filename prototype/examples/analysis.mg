// Comprehensive MechGen example — exercises name resolution, type checking, and effect inference.

// ── Pure computation ─────────────────────────────────────────────────

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn double(x: i32) -> i32 {
    x * 2
}

fn quadruple(x: i32) -> i32 {
    double(double(x))
}

// ── Record and field access ──────────────────────────────────────────

pub struct Point {
    pub x: f64,
    pub y: f64,
}

fn distance_sq(p: Point) -> f64 {
    p.x * p.x + p.y * p.y
}

// ── Sum type ─────────────────────────────────────────────────────────

enum Shape {
    Circle,
    Square,
    Triangle,
}

// ── Generics ─────────────────────────────────────────────────────────

fn identity<T>(val: T) -> T {
    val
}

// ── Option and Result types ──────────────────────────────────────────

fn safe_div(a: f64, b: f64) -> Option<f64> {
    a
}

fn fallible(x: i32) -> Result<i32, str> {
    x
}

// ── Control flow ─────────────────────────────────────────────────────

fn abs(x: i32) -> i32 {
    if x > 0 { x } else { 0 - x }
}

fn classify(n: i32) -> str {
    if n > 0 { "positive" } else { "non-positive" }
}

// ── Let bindings and closures ──────────────────────────────────────────

fn compute() -> i32 {
    let x: i32 = 10;
    let y: i32 = 20;
    let sum: i32 = add(x, y);
    let doubled: i32 = double(sum);
    doubled
}

fn apply_twice() -> i32 {
    let inc = |x: i32| x + 1;
    inc(inc(0))
}

// ── Effectful functions ──────────────────────────────────────────────

fn greet(name: str) -> () {
    println(name)
}

fn write_data() -> () {
    open();
    println("done")
}

fn main() -> () {
    greet("world");
    let result: i32 = compute();
    println("calculated")
}

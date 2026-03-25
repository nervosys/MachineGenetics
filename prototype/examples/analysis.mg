// Comprehensive MechGen example — exercises name resolution, type checking, and effect inference.

// ── Pure computation ─────────────────────────────────────────────────

exp def add(a: i32, b: i32) -> i32 {
    a + b
}

def double(x: i32) -> i32 {
    x * 2
}

def quadruple(x: i32) -> i32 {
    double(double(x))
}

// ── Record and field access ──────────────────────────────────────────

exp rec Point {
    exp x: f64,
    exp y: f64,
}

def distance_sq(p: Point) -> f64 {
    p.x * p.x + p.y * p.y
}

// ── Sum type ─────────────────────────────────────────────────────────

sum Shape {
    Circle,
    Square,
    Triangle,
}

// ── Generics ─────────────────────────────────────────────────────────

def identity<T>(val: T) -> T {
    val
}

// ── Option and Result types ──────────────────────────────────────────

def safe_div(a: f64, b: f64) -> Option<f64> {
    a
}

def fallible(x: i32) -> Result<i32, str> {
    x
}

// ── Control flow ─────────────────────────────────────────────────────

def abs(x: i32) -> i32 {
    when x > 0 { x } or { 0 - x }
}

def classify(n: i32) -> str {
    when n > 0 { "positive" } or { "non-positive" }
}

// ── Val bindings and closures ────────────────────────────────────────

def compute() -> i32 {
    val x: i32 = 10;
    val y: i32 = 20;
    val sum: i32 = add(x, y);
    val doubled: i32 = double(sum);
    doubled
}

def apply_twice() -> i32 {
    val inc = |x: i32| x + 1;
    inc(inc(0))
}

// ── Effectful functions ──────────────────────────────────────────────

def greet(name: str) -> () {
    println(name)
}

def write_data() -> () {
    open();
    println("done")
}

def main() -> () {
    greet("world");
    val result: i32 = compute();
    println("calculated")
}

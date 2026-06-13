// Comprehensive MAGE example — exercises name resolution, type checking, and effect inference.
//
// Demonstrates:
//   - data records and sums
//   - extend blocks
//   - val / var bindings
//   - guard early-exit
//   - defer cleanup
//   - Pipeline operator |>
//   - is pattern matching
//   - T or E error union
//   - Expression-body functions
//   - Default parameters

// ── Pure computation ─────────────────────────────────────────────────

pub fn add(a: i32, b: i32) -> i32 = a + b

fn double(x: i32) -> i32 = x * 2

fn quadruple(x: i32) -> i32 = double(double(x))

// ── Record and field access ──────────────────────────────────────────

data Point(pub x: f64, pub y: f64)

// Extension block: attach methods to a type.
extend Point {
    fn distance_sq(&self) -> f64 = self.x * self.x + self.y * self.y
}

// ── Sum type ─────────────────────────────────────────────────────────

data Shape = Circle | Square | Triangle

// ── Generics ─────────────────────────────────────────────────────────

fn identity[T](v: T) -> T = v

// ── Option and error union types ─────────────────────────────────────

fn safe_div(a: f64, b: f64) -> ?f64 {
    guard b != 0.0 else { return None; }
    Some(a / b)
}

// Error union: i32 or str replaces Result<i32, str>.
fn fallible(x: i32) -> i32 or str {
    guard x >= 0 else { return Err("negative"); }
    Ok(x)
}

// ── Control flow ─────────────────────────────────────────────────────

fn abs(x: i32) -> i32 {
    if x > 0 { x } else { 0 - x }
}

fn classify(n: i32) -> str {
    if n > 0 { "positive" } else { "non-positive" }
}

// ── Val/var bindings, pipeline, closures ─────────────────────────────

fn compute() -> i32 {
    val x: i32 = 10;
    val y: i32 = 20;
    // Pipeline: chain through add and double.
    x |> add(y) |> double()
}

fn apply_twice() -> i32 {
    val inc = |x: i32| x + 1;
    inc(inc(0))
}

// ── Guard, defer, is ─────────────────────────────────────────────────

fn process(input: ?String) -> String or str {
    // Guard: exit early when None.
    guard input is Some(_) else { return Err("missing input"); }

    // Defer: log when scope exits.
    defer io.println("process done");

    Ok("ok")
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
    val result: i32 = compute();
    println("calculated")
}

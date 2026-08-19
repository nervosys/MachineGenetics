// analysis.mg — a broad pass over name resolution, typing, and effect inference.
//
// Every construct here was verified with `--check` and `--eval`. The previous
// version had never compiled: it used `pub fn`, `let`, `&self`, expression-body
// functions (`-> T = expr`) and the `|>` pipeline operator. The last two do not
// exist and are documented nowhere.
//
// Demonstrates:
//   - `data` records and sums
//   - `extend` blocks
//   - `v` / `m` bindings
//   - `guard` early exit
//   - `defer` cleanup
//   - `is` pattern tests
//   - `?T` options and `T or E` unions
//   - generics
//   - closures, in both spellings
//   - default arguments

// ── Pure computation ─────────────────────────────────────────────────

+f add(a: i32, b: i32) -> i32 { a + b }

f double(x: i32) -> i32 { x * 2 }

f quadruple(x: i32) -> i32 { double(double(x)) }

// Default arguments. Only trailing defaults may be omitted, and a default may
// refer to an earlier parameter.
f scaled(a: i32, b: i32 = a * 2) -> i32 { a + b }

// ── Records and extensions ───────────────────────────────────────────

data Point(x: f64, y: f64)

// `extend` attaches methods. The receiver is `self`, with no sigil.
extend Point {
    +f distance_sq(self) -> f64 { self.x * self.x + self.y * self.y }
}

// ── Sums ─────────────────────────────────────────────────────────────

data Shape = Circle | Square | Triangle

f sides(s: Shape) -> i32 {
    ?= s {
        Circle => 0,
        Square => 4,
        Triangle => 3,
    }
}

// ── Generics ─────────────────────────────────────────────────────────
//
// Each call site instantiates its own copy of `T`, so one generic serves
// several types in one program.

f identity[T](v: T) -> T { v }

// ── Options and unions ───────────────────────────────────────────────

f safe_div(a: f64, b: f64) -> ?f64 {
    guard b != 0.0 else { ret None }
    Some(a / b)
}

// `i32 or str` is the union spelling of `R[i32, str]`.
f fallible(x: i32) -> i32 or str {
    guard x >= 0 else { ret Err("negative") }
    Ok(x)
}

// ── Control flow ─────────────────────────────────────────────────────

f abs(x: i32) -> i32 {
    ? x > 0 { x } : { 0 - x }
}

f classify(n: i32) -> str {
    ? n > 0 { "positive" } : { "non-positive" }
}

// ── Closures ─────────────────────────────────────────────────────────

f apply_twice() -> i32 {
    v inc = |x: i32| x + 1
    inc(inc(0))
}

f transformed(xs: [i32]~) -> i32 {
    // Both closure spellings are real; `|x| …` is the one the spec's grammar
    // defines, `f(x) => …` also parses.
    v evens = filter(xs, |x| x % 2 == 0)
    fold(evens, 0, f(a, x) => a + x)
}

// ── `is`, `guard`, `defer` ───────────────────────────────────────────

f describe(input: ?i32) -> str {
    // `is` tests a pattern without binding.
    guard input is Some(_) else { ret "missing" }
    "present"
}

f with_cleanup() -> i32 / io {
    // `defer` runs on scope exit.
    defer println("done")
    41
}

// ── Effects ──────────────────────────────────────────────────────────
//
// `println` is a recognised effectful builtin, so this function performs `io`
// and — being public — must say so.

+f greet(name: str) -> i32 / io {
    println(name)
    0
}

+f main() -> i32 / io {
    greet("world")

    v p = @Point { x: 3.0, y: 4.0 }
    v shape_sides = sides(Square) + sides(Triangle)

    // 25 + 7 + 12 + 2 + 15 + 41 + 1 = 103
    (p.distance_sq() as i32)
        + shape_sides
        + transformed([1, 2, 3, 4, 5, 6])
        + apply_twice()
        + scaled(5)
        + with_cleanup()
        + identity(1)
}

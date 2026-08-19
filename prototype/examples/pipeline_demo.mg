// ─────────────────────────────────────────────────────────────
// MAGE end-to-end pipeline demo
//
// Exercises every stage: lexer, parser, resolver, types, effects.
// Every construct here was verified with `--check` and `--eval`.
//
// The previous version had never compiled. It used `pub fn`, `let`, `String`
// method calls, `format!`, an `Error` type that does not exist, and
// expression-body functions (`-> T = expr`), which do not either. It also used
// `|>`, which does — see below.
//
// Demonstrates:
//   - `data` records and sums
//   - `extend` methods
//   - `v` / `m` bindings
//   - `guard … else` early exit
//   - `defer` cleanup
//   - `|>` pipelines
//   - `is` pattern tests
//   - `T or E` error unions
//   - default arguments
// ─────────────────────────────────────────────────────────────

// ── Records ──────────────────────────────────────────────────

data Point(x: f64, y: f64)

extend Point {
    +f dist_sq(self, o: Point) -> f64 {
        v dx = o.x - self.x
        v dy = o.y - self.y
        dx * dx + dy * dy
    }
}

// ── Sums ─────────────────────────────────────────────────────

data Shape = Circle | Square | Triangle

f sides(s: Shape) -> i32 {
    ?= s {
        Circle => 0,
        Square => 4,
        Triangle => 3,
    }
}

// ── Pure functions ───────────────────────────────────────────

f add(a: i32, b: i32) -> i32 { a + b }

f multiply(x: i32, y: i32) -> i32 { x * y }

// ── Pipelines ────────────────────────────────────────────────
//
// `x |> f(a)` is `f(x, a)`: the left operand becomes the first argument. It
// chains left to right, and a bare function reference works too (`x |> f`).
//
// This is the operator the file is named for, and until now every program
// using it *ran correctly and failed `--check`* — the checker inferred the two
// sides independently, so `10 |> add(5)` was checked as the standalone call
// `add(5)` and reported a missing argument.

f piped() -> i32 {
    10 |> add(5) |> multiply(2)
}

// ── Error unions ─────────────────────────────────────────────

f parse_port(raw: i32) -> i32 or str {
    guard raw > 0 else { ret Err("port must be positive") }
    Ok(raw)
}

// ── Default arguments ────────────────────────────────────────
//
// Only *trailing* defaults may be omitted.

f greet(name: str, prefix: str = "Hello") -> str {
    join([prefix, name], ", ")
}

// ── Control flow ─────────────────────────────────────────────

f max_val(a: i32, b: i32) -> i32 {
    ? a > b { a } : { b }
}

// ── `guard`, `is`, `defer` ───────────────────────────────────

f guarded(input: ?i32) -> i32 or str {
    guard input is Some(_) else { ret Err("no input") }
    Ok(1)
}

f with_cleanup() -> i32 / io {
    defer println("work complete")
    2
}

// ── Entry point ──────────────────────────────────────────────

+f main() -> i32 / io {
    v sum = add(3, 4)
    v prod = multiply(sum, 2)

    v a = @Point { x: 0.0, y: 0.0 }
    v b = @Point { x: 3.0, y: 4.0 }

    println(greet("MAGE"))

    v ok = ?= parse_port(8080) { Ok(n) => n, Err(_) => 0 }
    v got = ?= guarded(Some(1)) { Ok(n) => n, Err(_) => 0 }

    // 14 + 25 + 7 + 30 + 8080 + 1 + 2 = 8159
    max_val(sum, prod)
        + (a.dist_sq(b) as i32)
        + (sides(Square) + sides(Triangle))
        + piped()
        + ok
        + got
        + with_cleanup()
}

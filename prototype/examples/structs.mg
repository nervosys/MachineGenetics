// structs.mg — records, sums, extensions, generics
//
// Every construct here was verified by running `--check` and `--eval` on it.
// The previous version of this file listed four features in its header that do
// not exist: expression-body functions (`-> T = expr`), the pipeline operator
// (`|>`), `pub fn` with `&self` receivers, and `let`. It had never compiled.
//
// Demonstrates:
//   - `data Name(field: Type, ...)` — a record
//   - `data Name = A(T) | B(T, U)` — a sum
//   - `extend` — methods on a type
//   - `@Name { field: value }` — a struct literal (bare `Name { … }` is a MAP)
//   - `?=` — match

// ── Records ──────────────────────────────────────────────────────────

data Point(x: f64, y: f64)

// ── Sums ─────────────────────────────────────────────────────────────
//
// Variants construct either bare (`Rect(3.0, 4.0)`) or qualified
// (`Shape.Rect(3.0, 4.0)`). Qualify when two sums share a variant name —
// unqualified, that is an error naming both, not a silent pick.

data Shape = Circle(f64) | Rect(f64, f64)

// ── Extensions ───────────────────────────────────────────────────────
//
// `extend` attaches methods to a type. The receiver is `self`, with no
// sigil — `&self` is Rust.

extend Point {
    +f norm2(self) -> f64 { self.x * self.x + self.y * self.y }
}

// ── Using them ───────────────────────────────────────────────────────

+f area(s: Shape) -> f64 {
    ?= s {
        Circle(r) => 3.14159 * r * r,
        Rect(w, h) => w * h,
    }
}

+f main() -> f64 {
    v p = @Point { x: 3.0, y: 4.0 }
    // 25.0 from the method, plus 12.0 from the Rect area.
    p.norm2() + area(Rect(3.0, 4.0))
}

// structs.mg — records, sums, extensions, generics
//
// Demonstrates:
//   - data keyword for record types (replaces struct for simple types)
//   - data keyword for sum types (replaces enum)
//   - extend keyword (replaces impl)
//   - val bindings (replaces let)
//   - Expression-body functions (-> T = expr)
//   - Pipeline operator (|>)

// ── Record types ─────────────────────────────────────────────────────

// Concise record: data Name(field: Type, ...)
data Point[T](x: T, y: T)

// ── Sum types ────────────────────────────────────────────────────────

// Concise sum: data Name = Variant1 | Variant2(Type)
data Shape = Circle(f64) | Rect(f64, f64) | Poly([Point[f64]]~)

// ── Extensions ───────────────────────────────────────────────────────

// extend replaces impl — attach methods to a type.
extend Point[f64] {
    pub fn distance(&self, other: &Point[f64]) -> f64 {
        val dx = self.x - other.x;
        val dy = self.y - other.y;
        (dx * dx + dy * dy)
    }
}

// Expression-body function: no block needed.
pub fn make_origin() -> Point[f64] = Point { x: 0.0, y: 0.0 }

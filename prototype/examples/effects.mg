// effects.mg — effect declarations, handlers, closures
//
// Every construct here was verified with `--check` and `--eval`. The previous
// version of this file had never compiled: it used `pub fn`, `let`, an `Error`
// type that does not exist, and the `|>` pipeline operator, which does not
// either.
//
// Demonstrates:
//   - `effect` declarations and their operations
//   - `handle … with` — the elimination rule
//   - `guard … else` — early exit
//   - `R[T, E]` and the `T or E` union spelling
//   - closures, in both spellings

// ── Declaring an effect ──────────────────────────────────────────────
//
// An `effect` block names an effect and the operations that belong to it.
// Calling one performs the effect: `Audit.record(x)` puts `audit` in the
// calling function's inferred set. An operation the block does not declare is
// an error, so a misspelling cannot pass for a real one.

effect Audit {
    f record(entry: str) -> usize;
}

// ── Performing ───────────────────────────────────────────────────────
//
// `/ audit` is the annotation. A `pub` function must declare what it performs;
// a private one infers silently, and its effects still reach its public
// callers.

f note(entry: str) -> usize / audit {
    Audit.record(entry)
}

// ── Early exit ───────────────────────────────────────────────────────
//
// `guard cond else { … }` must diverge in the else branch — `ret`, `break`, or
// a panic. A `guard` whose else falls through is a check-time error.

+f safe_div(a: i32, b: i32) -> R[i32, str] {
    guard b != 0 else { ret Err("divide by zero") }
    Ok(a / b)
}

// `T or E` is the same thing spelled as a union.
+f halve(n: i32) -> i32 or str {
    Ok(n / 2)
}

// ── Closures ─────────────────────────────────────────────────────────
//
// Two spellings, both real: `|x| expr` (what the spec's grammar defines) and
// `f(x) => expr`. The vocabulary is built on higher-order functions, so this
// is how `map` / `filter` / `fold` are used.

+f doubled(xs: [i32]~) -> [i32]~ {
    map(xs, |x| x * 2)
}

+f total(xs: [i32]~) -> i32 {
    fold(xs, 0, |acc, x| acc + x)
}

// ── Handling ─────────────────────────────────────────────────────────
//
// `handle … with` removes the effect from the block it wraps, so `main` is
// pure despite calling `note`, which is not. The subtraction is per block: an
// unhandled call elsewhere would still report.

+f main() -> i32 {
    v audited = handle {
        note("startup")
    } with Audit {
        record(entry) => len(entry)
    }

    v quotient = ?= safe_div(10, 2) {
        Ok(n) => n,
        Err(_) => 0,
    }

    // 7 (len "startup") + 5 (10 / 2) + 12 (2+4+6) = 24
    (audited as i32) + quotient + total(doubled([1, 2, 3]))
}

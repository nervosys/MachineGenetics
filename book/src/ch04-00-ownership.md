# Ownership & Safety

MechGen inherits Rust's ownership model — every value has exactly one owner,
borrowing is checked, and data races are impossible. But the way safety is
*expressed* is fundamentally different.

In Rust, safety rules are encoded in **syntax**: lifetime annotations (`'a`),
borrow markers (`&mut`), `unsafe` blocks, `Pin<T>`, `PhantomData`. The compiler
enforces them at compile time.

In MechGen, safety rules live in the **Safety Knowledge Base (SKB)** — a
structured, versioned, queryable database. The compiler can optionally enforce
them, but agents query the SKB directly. This means:

- **No lifetime annotations** — the SKB tracks borrow lifetimes internally
- **No `unsafe` blocks** — capability-based regions replace them
- **No `PhantomData`** — variance is inferred
- **No `Pin<T>`** — self-referencing is handled by the SKB

The source code is dramatically simpler. The safety is the same.

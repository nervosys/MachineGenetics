# Effects & Handlers

One of MechGen's most important innovations is its **algebraic effect system**.
Effects make side effects *explicit, composable, and trackable* — every function
that touches the outside world declares what kind of side effects it performs.

This chapter covers:

- **Algebraic effects** — the theory and how MechGen uses it
- **Built-in effects** — `io`, `net`, `rng`, `async`, `agent`, and others
- **Writing handlers** — intercepting and controlling effects

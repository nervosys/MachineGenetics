# Syntax Reference

MechGen's syntax is built on a single principle: **every token must be
unambiguous**. The grammar is deterministic LL(1) — the parser never backtracks,
never guesses, and never needs more than one token of lookahead.

This chapter covers every syntactic construct in the language:

- **Keywords & Declarations** — the single-character declaration forms
- **Variables & Mutability** — `v` and `m` bindings
- **Functions** — `f`, `+f`, visibility, effects
- **Control Flow** — `?` (if/match), `@` (for), loops
- **Pattern Matching** — destructuring, guards, exhaustiveness
- **Modules & Imports** — `u` imports, dot-separated paths

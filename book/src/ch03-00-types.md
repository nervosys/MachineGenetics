# Type System

Redox has a rich type system built on Hindley-Milner inference with algebraic
data types, traits, and a collection of **type sugar** that compresses common
patterns into single sigils.

This chapter covers:

- **Primitive types** — integers, floats, booleans, strings, characters
- **Composite types** — structs, enums, tuples, arrays
- **Generics** — parameterized types and functions
- **Traits** — interfaces and polymorphism
- **Type sugar** — the sigils that make Redox concise (`?T`, `[T]~`, `{K:V}`,
  `^T`, `$T`, `@T`, `&!T`)

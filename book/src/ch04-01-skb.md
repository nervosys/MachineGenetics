# The SKB Model

The **Safety Knowledge Base (SKB)** is Redox's alternative to Rust's compile-time
safety enforcement. Instead of encoding safety rules in syntax, they are stored
in a structured database that both the compiler and agents can query.

## What the SKB contains

The SKB stores **rules** — formal safety invariants organized by category:

| Category      | Example Rules                                       |
| ------------- | --------------------------------------------------- |
| `ownership`   | "A value cannot be used after move"                 |
| `borrow`      | "Cannot hold `&!T` and `&T` simultaneously"         |
| `lifetime`    | "Reference cannot outlive its referent"             |
| `type`        | "Integer arithmetic must not overflow in safe code" |
| `concurrency` | "`@T` is required for cross-thread sharing"         |
| `ffi`         | "FFI calls require the `ffi` capability"            |

## Querying the SKB

From code, use the `std::skb` module:

```rdx
use std::skb::{query, Rule};

pub fn check_rules() {
    let rules = query().category("borrow").severity(Error).run();
    for rule in rules {
        println!("Rule {rule.id}: {rule.title}");
    }
}
```

From the CLI:

```sh
rdx skb query --category borrow
rdx skb validate src/
```

## How the SKB replaces lifetimes

In Rust, you write:

```rust
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() > b.len() { a } else { b }
}
```

In Redox, the same function:

```rdx
fn longest(a: &str, b: &str) -> &str {
    if a.len() > b.len() { a } else { b }
}
```

No lifetime annotations. The SKB rule `lifetime:return-borrow` ensures that
the returned reference does not outlive either input. The compiler can verify
this (with `rdx check --skb-enforce`), or an agent can query the rule directly.

## SKB safety profiles

Projects choose a safety profile in `Forge.toml`:

```toml
[safety]
mode = "skb-only"           # Agent dev: SKB available, no compile enforcement
# mode = "warnings"         # Human dev: show warnings but compile anyway
# mode = "full"             # CI/production: enforce all SKB rules at compile
```

| Profile    | Compile Enforcement | SKB Queryable | Use Case          |
| ---------- | :-----------------: | :-----------: | ----------------- |
| `none`     |         No          |      Yes      | Experimentation   |
| `skb-only` |         No          |      Yes      | Agent development |
| `warnings` |      Warn only      |      Yes      | Human development |
| `full`     |        Error        |      Yes      | CI / production   |

## Why this matters for agents

Traditional safety enforcement is designed for humans who make mistakes. Agents
don't make the *same* mistakes — they can internalize safety rules from the SKB
and produce correct code without waiting for the compiler to reject incorrect
attempts. This eliminates the `emit → error → fix → re-emit` cycle that wastes
tokens and latency.

When an agent queries `skb.query().category("borrow")` before emitting code, it
gets the exact rules it needs to follow — structured data, not error messages
parsed from compiler output.

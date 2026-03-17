# Redox Safety Knowledge Base (SKB)

The SKB is a corpus of **9,157 safety rules** that the Redox compiler queries
at compile time to verify code safety, suggest fixes, and provide agent-readable
diagnostics.

## Structure

```shell
skb/
├── manifest.json           # Version, database sizes, hashes
├── rule-schema.json        # JSON Schema for rule validation
├── README.md               # This file
└── rule s/
    ├──  ownership.json     # OWN-xxx  (2,847 rules in full corpus)
    ├──  borrow.json        # BR-xxx   (1,203 rules in full corpus)
    ├──  lifetime.json      # LT-xxx   (894 rules in full corpus)
    ├──  type_safety.json   # TS-xxx   (3,412 rules in full corpus)
    ├── concurrency.json    # CC-xxx   (567 rules in full corpus)
    └── ffi.json            # FFI-xxx  (234 rules in full corpus)
```

## Rule Categories

| Database    | Rules     | ID Prefix | Covers                                          |
| ----------- | --------- | --------- | ----------------------------------------------- |
| Ownership   | 2,847     | `OWN-`    | Move semantics, Copy, Clone, Drop               |
| Borrow      | 1,203     | `BR-`     | Aliasing XOR mutability, iterator invalidation  |
| Lifetime    | 894       | `LT-`     | Dangling references, elision, struct lifetimes  |
| Type Safety | 3,412     | `TS-`     | Type mismatch, overflow, Option/Result, effects |
| Concurrency | 567       | `CC-`     | Data races, deadlocks, Send/Sync, async         |
| FFI         | 234       | `FFI-`    | Null pointers, ABI, strings, repr(C)            |
| **Total**   | **9,157** |           |                                                 |

## Rule Lifecycle

1. **Proposed** — new rule submitted for review
2. **Staged** — under testing; may generate warnings only
3. **Active** — fully enforced by the compiler
4. **Deprecated** — superseded or found to be incorrect

## Seed Corpus

This directory contains **seed rules** — representative examples for each
category that define the schema, patterns, and fix templates. The full 9,157-rule
corpus is generated from these seeds and empirical data from the Rust ecosystem.

## Query Language (SKB-QL)

Rules are queried using SKB-QL, which supports both SQL-like and compact forms:

```
// SQL-like
SELECT * FROM borrow WHERE category = 'double-borrow' AND severity = 'error'

// Compact (agent-optimized)
?borrow MutBorrow(Vec<*>) @loop
```

## Integration

The SKB is part of the formal type judgment: **Γ; Σ; Δ ⊢ e : τ ⊣ ε**, where
**Σ** is the SKB context. Rules are queried via the MLIR `redox.skb.query` and
`redox.skb.validate` operations during compile-time evaluation.

The RAP server exposes SKB queries via `skb/query` JSON-RPC method.

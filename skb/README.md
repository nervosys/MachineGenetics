# MAGE Safety Knowledge Base (SKB)

> **Corrected and then regenerated, 2026-09-02.** This file described a 9,157-rule corpus that the
> compiler queries at compile time, a query language, and two MLIR operations.
> The compiler serves **255 rules**, from a table compiled into the binary, and
> **reads nothing in this directory**. The query language and both MLIR
> operations do not exist. What follows is what is here; the design that was
> here before is kept at the end, labelled, because the intent is worth keeping
> and the false impression is not.

## What the compiler actually serves

`prototype/src/skb.rs` holds `builtin_rules()`, a `Vec<Rule>` compiled into
`mage-parse`. Counts below are from `skb::rule_counts_by_db()` and
`skb::rule_count()`, and are pinned by `rule_counts_per_database` in
`skb.rs` — so they cannot drift from this table without a test failing.

| Database | Rules | ID prefix | Covers |
| --- | ---: | --- | --- |
| Ownership | 40 | `OWN-` | Move semantics, Copy, Clone, Drop |
| Borrow | 40 | `BOR-` | Aliasing XOR mutability, iterator invalidation |
| Lifetime | 35 | `LIF-` | Dangling references, elision, struct lifetimes |
| TypeSafety | 40 | `TYP-` | Type mismatch, overflow, Option/Result, effects |
| Concurrency | 35 | `CON-` | Data races, deadlocks, Send/Sync, async |
| FFI | 20 | `FFI-` | Null pointers, ABI, strings, repr(C) |
| AgentElision | 30 | `AEL-` | Safety constructs agent mode elides from syntax |
| SwarmSafety | 15 | `SWM-` | Consensus, topology, fault tolerance, deadlock |
| **Total** | **255** | | |

The last two are not a detail. `AgentElision` and `SwarmSafety` are the
databases that make this a *MAGE* safety knowledge base rather than a Rust one,
they are 45 of the 255 rules, and the previous version of this file did not
mention either.

## What is in this directory

**Generated. Do not edit these files.** `mage-parse --emit-skb skb` writes them
from `builtin_rules()`, and `scripts/check-skb-tree.sh` regenerates the tree in
CI and fails if the committed one differs. To change a rule, change `skb.rs`
and re-emit.

```
skb/
├── manifest.json        generated: totals and per-database counts
├── rule-schema.json     JSON Schema for a rule
├── README.md            this file
└── rules/
    ├── ownership.json     40    ├── concurrency.json     35
    ├── borrow.json        40    ├── ffi.json             20
    ├── lifetime.json      35    ├── agent_elision.json   30
    ├── type_safety.json   40    └── swarm_safety.json    15
```

Each rule carries the nine fields a `Rule` has: `id`, `database`, `category`,
`severity`, `description`, `rationale`, `fix_template`, `fix_confidence`,
`tags`.

### What it was until 2026-09-02

Six files holding **56 rules that nothing read**. Not the compiler, not any
tool — the `"skb/rules"` occurrences in `rap.rs` and `ontology.rs` are the
JSON-RPC *method name*, easy to mistake for evidence the directory is loaded.

It was not the source of the built-in rules either. The two did not share an
identifier scheme:

| | on disk | in the binary |
| --- | --- | --- |
| Borrow | `BR-` | `BOR-` |
| Lifetime | `LT-` | `LIF-` |
| TypeSafety | `TS-` | `TYP-` |
| Concurrency | `CC-` | `CON-` |
| — | `MEM-`, `TC-` (1 each) | no such database |
| AgentElision, SwarmSafety | *no file* | 30 and 15 rules |

So it was neither an input nor an export: a parallel corpus, free to drift from
the compiler with nothing to notice — the `stdlib/` shape this repository has
found before. `check-orphan-sources.sh` could not see it, because that finds
`.rs` files no `mod` reaches, not **data** nothing loads.

Open item 23 offered three answers: load it, generate it, or delete it.
**Generating won.** Loading would have made 56 rules authoritative and discarded
the 199 that actually run, needing a schema migration on the way. Deleting would
lose a machine-readable corpus worth having on disk. Generating makes the tree
true by construction — and only stays true because something regenerates and
compares, which is the check above.

The old files carried seventeen fields. Eight of them — `pattern`, `context`,
`alternatives`, `false_positive_rate`, `frequency`, `scope`, `source`,
`version` — were metadata about rules that were not the rules being enforced,
populated by nothing and read by nothing. They are not in the export.

## Querying the rules

Over RAP (`mage-parse --rap`), which is the only programmatic surface:

| Method | Serves |
| --- | --- |
| `skb/rules` | the 255 safety rules, optionally filtered by `domain` |
| `skb/query` | the **symbol** knowledge base — effects, capabilities, tags, Rust aliases — by `fqn`, `effect`, `capability`, `tag`, `rust_alias` or `module` |
| `skb/spec` | one function's `@req`/`@ens` spec block, by `fqn` |

**Only the first of those serves rules.** `skb/query` and `skb/spec` read
`builtin_skb()`, a table of symbol metadata that happens to live in the same
module; `skb/rules` reads `builtin_rules()`. Two knowledge bases share the
prefix, and asking `skb/query` for `UseAfterMove` correctly returns nothing.

Outside RAP, `skb::` is called from exactly two other places — `codegen_bridge`
and `rmi_ontology_adapter` — and from **no** stage of the compiler.
`types.rs`, `effects.rs`, `resolve.rs`, `verify.rs`, `heal.rs` and `mlir.rs`
contain zero references to it between them.

---

## Design, not implemented

Kept because the intent is worth keeping. Everything in this section describes
something that **does not exist today**, and `MAGE_SPEC.md` uses the same
convention for the five constructs it documents and does not implement.

**A 9,157-rule corpus** — Ownership 2,847, Borrow 1,203, Lifetime 894, Type
Safety 3,412, Concurrency 567, FFI 234 — generated from the seed rules in this
directory plus empirical data from the Rust ecosystem. There is no generator,
and the figure appears in no measurement.

**SKB-QL**, a query language in SQL-like and compact agent-optimised forms:

```
SELECT * FROM borrow WHERE category = 'double-borrow' AND severity = 'error'
?borrow MutBorrow(Vec<*>) @loop
```

Not implemented. The only occurrences of this syntax in the tree are three
example lines in `skb.rs`'s own header comment, which document a language the
file below them does not parse.

**A rule lifecycle** — Proposed → Staged → Active → Deprecated. `RuleSeverity`
exists; a lifecycle state does not.

**MLIR integration**, `MAGE.skb.query` and `MAGE.skb.validate` operations
consulted during compile-time evaluation, with the SKB as **Σ** in the judgment
Γ; Σ; Δ ⊢ e : τ ⊣ ε. Neither operation appears anywhere in the tree. The
compiler does not consult the SKB during checking at all — the rules are served
to agents over RAP, and that is their only consumer.

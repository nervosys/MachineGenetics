# Redox Migration Guide

A step-by-step guide for migrating Rust projects to Redox. Written for Rust
developers who know Cargo, ownership, traits, and async — and want to move
their codebase to Redox's compact syntax, effect system, and agent primitives.

## Who This Guide Is For

- **Rust developers** porting existing crates or applications
- **Team leads** evaluating Redox adoption for a Rust codebase
- **CI/CD engineers** wiring up Redox builds alongside existing Rust pipelines

## Chapters

| #   | Chapter                                      | What You'll Learn                                            |
| --- | -------------------------------------------- | ------------------------------------------------------------ |
| 1   | [Pre-Migration Assessment](01-assessment.md) | Evaluate readiness, estimate effort, plan phases             |
| 2   | [Project Setup & Tooling](02-setup.md)       | Initialize a Redox project, configure Forge.toml, dual-build |
| 3   | [Syntax Migration](03-syntax.md)             | Keyword-by-keyword translation with worked diffs             |
| 4   | [Type System Migration](04-types.md)         | Type sugar, generics, lifetimes, trait bounds                |
| 5   | [Effects & Safety](05-effects.md)            | Annotate effects, remove unsafe, adopt capabilities          |
| 6   | [Async & Concurrency](06-async.md)           | Migrate tokio/async-std, adopt Swarm, structured concurrency |
| 7   | [Testing & CI](07-testing.md)                | Port test suites, effect mocking, benchmark migration        |
| 8   | [Case Studies](08-case-studies.md)           | Real-world migration walkthroughs of complete programs       |

## Migration Workflow Overview

```
┌─────────────────┐
│  1. Assessment   │  Audit crate deps, unsafe blocks, async runtime
└────────┬────────┘
         │
┌────────▼────────┐
│  2. Setup        │  rdx new, Forge.toml, dual-build config
└────────┬────────┘
         │
┌────────▼────────┐
│  3. Syntax pass  │  rdx migrate --dry-run, then fix manually
└────────┬────────┘
         │
┌────────▼────────┐
│  4. Types pass   │  Replace Vec/Option/Result with sugar, drop lifetimes
└────────┬────────┘
         │
┌────────▼────────┐
│  5. Effects pass │  Annotate all impure functions, remove unsafe
└────────┬────────┘
         │
┌────────▼────────┐
│  6. Async pass   │  Replace tokio::spawn with Swarm, add / async
└────────┬────────┘
         │
┌────────▼────────┐
│  7. Test pass    │  Port tests, add effect handlers for mocking
└────────┬────────┘
         │
┌────────▼────────┐
│  8. Validate     │  rdx check, rdx test, rdx bench, CI green
└────────┴────────┘
```

## Quick Reference

- **Automated tool:** `rdx migrate path/to/rust/project` — runs rust2rdx on all `.rs` files
- **Dry run:** `rdx migrate --dry-run` — preview changes without writing
- **Partial migration:** Redox can depend on Rust crates via Forge.toml `[rust-dependencies]`
- **Escape hatch:** Keep individual `.rs` files and compile them as Rust within the Redox build

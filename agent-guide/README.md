# Redox Agent Guide

Structured instructions for AI agents generating, reviewing, and migrating
Redox code. This guide is the **primary reference** for AI models working with
the Redox programming language.

## Contents

| File                                       | Purpose                                   | Audience         |
| ------------------------------------------ | ----------------------------------------- | ---------------- |
| [system-prompt.md](system-prompt.md)       | Drop-in system prompt for AI agents       | All AI models    |
| [syntax-quick-ref.md](syntax-quick-ref.md) | Condensed syntax lookup table             | Fast reference   |
| [patterns.md](patterns.md)                 | Idiomatic Redox patterns and conventions  | Code generation  |
| [anti-patterns.md](anti-patterns.md)       | Common mistakes and how to avoid them     | Error prevention |
| [effects.md](effects.md)                   | Effect annotation rules and decision tree | Effect system    |
| [migration.md](migration.md)               | Rust → Redox translation rules for agents | Migration tasks  |
| [rap-agentic.md](rap-agentic.md)           | RAP agentic methods: heal, cost, SKB, verify | RAP clients |
| [examples/](examples/)                     | Worked prompt → response examples         | Training / eval  |

## Quick Start for Agent Developers

1. **Embed** [system-prompt.md](system-prompt.md) as the system prompt
2. **Reference** [syntax-quick-ref.md](syntax-quick-ref.md) for syntax lookups
3. **Follow** [patterns.md](patterns.md) for idiomatic code generation
4. **Avoid** everything in [anti-patterns.md](anti-patterns.md)
5. **Validate** effect annotations using [effects.md](effects.md)

## Project-Level Configuration

Place an `agent-instructions.yaml` in `.redox/` at the project root to give
agents project-specific context:

```yaml
language: redox
edition: "2025"
safety_profile: "full"
allowed_effects: ["io", "net", "async"]
project_conventions:
  - "All public functions must have effect annotations"
  - "Use agent.Swarm for parallel work, not raw spawn"
```

## Design Principles

The Agent Guide follows these principles:

1. **Precision over prose** — Tables and code over paragraphs
2. **Copy-paste ready** — Every example is syntactically valid
3. **Negative examples** — Show what NOT to do, not just what to do
4. **Effect-first** — Always annotate effects; pure is the default
5. **Diff-friendly** — Show Rust → Redox side-by-side for migration

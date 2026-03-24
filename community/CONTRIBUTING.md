# Contributing to MechGen

Thank you for your interest in contributing to MechGen! This guide covers how to
get involved, from reporting bugs to proposing new language features.

## Getting Started

1. **Read the docs** — Start with the [Quick Start Guide](quick-start/README.md),
   then the [MechGen Book](book/README.md) for a deep dive into the language.
2. **Set up your environment** — Follow [INSTALL.md](INSTALL.md) to build
   the compiler and tools from source.
3. **Join the conversation** — Open a
   [GitHub Discussion](https://github.com/nervosys/MechGen/discussions) to ask
   questions or share ideas.

## Ways to Contribute

| Area               | Description                                       | Good First? |
| ------------------ | ------------------------------------------------- | :---------: |
| Bug reports        | File issues with reproduction steps               |     Yes     |
| Documentation      | Fix typos, improve examples, add cookbook entries |     Yes     |
| Training data      | Add JSONL samples to `training/samples/`          |     Yes     |
| Standard library   | Implement stubs in `stdlib/`                      |     Yes     |
| Transpiler rules   | Add patterns to `rust2rdx` or `rdx2rs`            |   Medium    |
| SKB rules          | Propose new Safety Knowledge Base rules           |   Medium    |
| Editor support     | Improve VS Code, Neovim, Helix, or Zed configs    |   Medium    |
| Compiler internals | Work on parsing, lowering, or MLIR pipeline       |  Advanced   |
| Language design    | Propose RFCs for new syntax or semantics          |  Advanced   |

## Contribution Workflow

```
1. Discuss    → GitHub Discussions or Discord
2. Propose    → RFC (for language changes) or Issue (for bugs/features)
3. Branch     → Fork the repo and create a feature branch
4. Implement  → Write code, tests, and documentation
5. Test       → Run `mg test` and `mg fmt --check`
6. PR         → Open a pull request against `master`
7. Review     → Address feedback from maintainers
8. Merge      → After approval and CI passes
```

## Development Setup

```bash
# Clone the repository
git clone https://github.com/nervosys/MechGen.git
cd MechGen

# Build the transpiler
cd tools/rust2rdx && cargo build && cargo test && cd ../..

# Build the reverse transpiler
cd tools/rdx2rs && cargo build && cargo test && cd ../..

# Build the CLI
cd tools/mg && cargo build && cargo test && cd ../..

# Run all tests
cargo test --workspace
```

## Code Style

- **MechGen code** (`.mg` files): Follow the conventions in
  [training/agent-instructions.yaml](training/agent-instructions.yaml)
- **Rust code** (`.rs` files): Use `rustfmt` with the project's
  [rustfmt.toml](rustfmt.toml)
- **Documentation**: Use Markdown with line wrapping at 80 characters
- **Commit messages**: Use imperative mood, e.g., "Add support for X" not
  "Added support for X"

## Adding Training Data

When contributing to `training/samples/`:

1. Use the JSONL format (one JSON object per line)
2. Include both `rdx_source` and `rs_source` (Rust equivalent)
3. Verify token counts are accurate
4. Tag effects and SKB rules used
5. Assign a difficulty level: `simple`, `medium`, `hard`, or `very-hard`
6. Do NOT duplicate tasks from `benchmarks/tasks/`

## Adding Transpiler Rules

For `tools/rust2rdx/` or `tools/rdx2rs/`:

1. Add the translation rule to the appropriate function in `translate.rs`
2. Add a test case in the test module
3. Verify the rule doesn't conflict with existing patterns
4. Update the rule count in the tool's `README.md`

## Proposing Language Changes (RFC Process)

Significant language changes require an RFC:

1. Copy `community/rfc-template.md` to `community/rfcs/0000-your-proposal.md`
2. Fill in all sections (motivation, design, alternatives, etc.)
3. Open a PR titled `RFC: Your Proposal Title`
4. The core team will review and discuss in the PR comments
5. After consensus, the RFC is accepted or deferred

Minor changes (typos, doc fixes, small bug fixes) do not need an RFC.

## Reporting Bugs

Use the [Bug Report](https://github.com/nervosys/MechGen/issues/new?template=bug_report.yaml)
issue template. Include:

- MechGen version (`mg --version`)
- Minimal reproduction code (`.mg` source)
- Expected vs. actual behavior
- Error messages or backtraces

## Code of Conduct

All contributors must follow our [Code of Conduct](CODE_OF_CONDUCT.md). We are
committed to fostering a welcoming and inclusive community.

## License

By contributing, you agree that your contributions will be licensed under the
same terms as the project: [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE),
at your option.

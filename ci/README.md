# Redox CI/CD Pipeline

GitHub Actions workflows for the Redox language ecosystem.

## Workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| [redox-ci.yml](workflows/redox-ci.yml) | Push to `master`, PRs | Full CI pipeline |
| [redox-pr.yml](workflows/redox-pr.yml) | PRs only | Fast PR feedback |
| [redox-release.yml](workflows/redox-release.yml) | Version tags (`v*`) | Build & publish releases |

## CI Pipeline Stages

Per [REDOX_ECOSYSTEM.md](../REDOX_ECOSYSTEM.md) §8.3:

```
┌─────────┐   ┌─────────┐   ┌──────────┐   ┌───────────┐   ┌──────────┐
│  Lint   │──▶│  Build  │──▶│   Test   │──▶│ Transpile │──▶│ Validate │
│ & Fmt   │   │ (3 OS)  │   │ (4 crates│   │ round-trip│   │ecosystem │
└─────────┘   └─────────┘   │  143 tsts│   └───────────┘   └──────────┘
                             └──────────┘
```

### Stage Details

| Stage | Check | Tool | Duration Target |
|-------|-------|------|:-:|
| Lint | Formatting | `cargo fmt --check` | < 5s |
| Lint | Linting | `cargo clippy` | < 10s |
| Build | Compile tools | `cargo build --release` | < 60s |
| Test | rust2rdx | `cargo test` (28 rules) | < 10s |
| Test | rdx2rs | `cargo test` (39 rules) | < 10s |
| Test | rdx CLI | `cargo test` (12 commands) | < 10s |
| Test | prototype | `cargo test` (43 tests) | < 15s |
| Transpile | Round-trip | Rust → Redox → Rust | < 30s |
| Validate | Examples | Check Forge.toml + .rdx | < 5s |
| Validate | Stdlib | Count .rdx modules | < 2s |
| Validate | SKB | JSON parsing | < 2s |
| Validate | Benchmarks | JSON parsing | < 2s |

## Release Process

1. Tag a commit with a version: `git tag v0.1.0`
2. Push the tag: `git push origin v0.1.0`
3. The `redox-release` workflow automatically:
   - Runs all tests
   - Builds binaries for 5 platform targets
   - Creates a GitHub Release with tarballs and checksums

### Release Targets

| Target | OS |
|--------|----|
| `x86_64-unknown-linux-gnu` | Linux (x64) |
| `aarch64-unknown-linux-gnu` | Linux (ARM64) |
| `x86_64-apple-darwin` | macOS (Intel) |
| `aarch64-apple-darwin` | macOS (Apple Silicon) |
| `x86_64-pc-windows-msvc` | Windows (x64) |

## Included Binaries

| Binary | Description |
|--------|-------------|
| `rust2rdx` | Rust → Redox transpiler |
| `rdx2rs` | Redox → Rust back-transpiler |
| `rdx` | Redox CLI (build, test, run, fmt, migrate) |

# MechGen CI/CD Pipeline

GitHub Actions workflows for the MechGen language ecosystem.

## Workflows

| Workflow                                         | Trigger               | Purpose                  |
| ------------------------------------------------ | --------------------- | ------------------------ |
| [MechGen-ci.yml](workflows/MechGen-ci.yml)           | Push to `master`, PRs | Full CI pipeline         |
| [MechGen-pr.yml](workflows/MechGen-pr.yml)           | PRs only              | Fast PR feedback         |
| [MechGen-release.yml](workflows/MechGen-release.yml) | Version tags (`v*`)   | Build & publish releases |

## CI Pipeline Stages

Per [mechgen_ECOSYSTEM.md](../mechgen_ECOSYSTEM.md) §8.3:

```
┌─────────┐   ┌─────────┐   ┌──────────┐   ┌───────────┐   ┌──────────┐
│  Lint   │──▶│  Build  │──▶│   Test   │──▶│ Transpile │──▶│ Validate │
│ & Fmt   │   │ (3 OS)  │   │ (4 crates│   │ round-trip│   │ecosystem │
└─────────┘   └─────────┘   │  143 tsts│   └───────────┘   └──────────┘
                             └──────────┘
```

### Stage Details

| Stage     | Check         | Tool                       | Duration Target |
| --------- | ------------- | -------------------------- | :-------------: |
| Lint      | Formatting    | `cargo fmt --check`        |      < 5s       |
| Lint      | Linting       | `cargo clippy`             |      < 10s      |
| Build     | Compile tools | `cargo build --release`    |      < 60s      |
| Test      | rust2rdx      | `cargo test` (28 rules)    |      < 10s      |
| Test      | rdx2rs        | `cargo test` (39 rules)    |      < 10s      |
| Test      | mg CLI       | `cargo test` (12 commands) |      < 10s      |
| Test      | prototype     | `cargo test` (43 tests)    |      < 15s      |
| Transpile | Round-trip    | Rust → MechGen → Rust        |      < 30s      |
| Validate  | Examples      | Check Forge.toml + .mg    |      < 5s       |
| Validate  | Stdlib        | Count .mg modules         |      < 2s       |
| Validate  | SKB           | JSON parsing               |      < 2s       |
| Validate  | Benchmarks    | JSON parsing               |      < 2s       |

## Release Process

1. Tag a commit with a version: `git tag v0.1.0`
2. Push the tag: `git push origin v0.1.0`
3. The `MechGen-release` workflow automatically:
   - Runs all tests
   - Builds binaries for 5 platform targets
   - Creates a GitHub Release with tarballs and checksums

### Release Targets

| Target                      | OS                    |
| --------------------------- | --------------------- |
| `x86_64-unknown-linux-gnu`  | Linux (x64)           |
| `aarch64-unknown-linux-gnu` | Linux (ARM64)         |
| `x86_64-apple-darwin`       | macOS (Intel)         |
| `aarch64-apple-darwin`      | macOS (Apple Silicon) |
| `x86_64-pc-windows-msvc`    | Windows (x64)         |

## Included Binaries

| Binary     | Description                                |
| ---------- | ------------------------------------------ |
| `rust2rdx` | Rust → MechGen transpiler                    |
| `rdx2rs`   | MechGen → Rust back-transpiler               |
| `mg`      | MechGen CLI (build, test, run, fmt, migrate) |

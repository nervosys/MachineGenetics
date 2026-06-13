# Chapter 1: Pre-Migration Assessment

Before writing a single line of MAGE, audit the Rust project to understand
scope, estimate effort, and plan a phased migration.

---

## 1.1 Crate Inventory

Run a dependency audit to understand what you're working with:

```bash
# In the Rust project
cargo tree --depth 1 > deps.txt
cargo tree --depth 1 | wc -l   # count direct + transitive deps
```

Classify each dependency:

| Category          | Examples                       | Migration Impact                        |
| ----------------- | ------------------------------ | --------------------------------------- |
| **Pure logic**    | `serde`, `regex`, `itertools`  | Low — use via `[rust-dependencies]`     |
| **Async runtime** | `tokio`, `async-std`           | High — replace with MAGE async + Swarm |
| **FFI / unsafe**  | `libc`, `winapi`, `nix`        | High — wrap with Capability system      |
| **Build tools**   | `cc`, `bindgen`, `proc-macro2` | Medium — configure in Forge.toml        |
| **Framework**     | `actix-web`, `rocket`, `axum`  | High — port handler by handler          |
| **Data**          | `diesel`, `sqlx`, `rusqlite`   | Medium — wrap with effect annotations   |

### Decision Matrix

For each crate dependency, decide:

```
Can it be used as-is via [rust-dependencies]?
  ├── YES → Keep as Rust dependency, no migration needed
  └── NO  → Is there a MAGE equivalent in std?
              ├── YES → Replace with std module
              └── NO  → Write a thin MAGE wrapper with effects
```

## 1.2 Unsafe Audit

Count and categorize `unsafe` usage:

```bash
grep -rn "unsafe" src/ --include="*.rs" | wc -l
grep -rn "unsafe" src/ --include="*.rs"
```

| Unsafe Pattern          | MAGE Replacement                      |
| ----------------------- | -------------------------------------- |
| Raw pointer dereference | `Capability.request("mem.deref", ...)` |
| FFI function call       | `Capability.request("ffi.call", ...)`  |
| Mutable static          | Module-level state with `/ env` effect |
| Union access            | Enum with explicit variants            |
| `transmute`             | Type-safe conversion functions         |
| Inline assembly         | Platform-specific capability           |

**Rule of thumb:** Each `unsafe` block becomes a `Capability.request()` call
with the appropriate permission string. The MAGE runtime enforces these at
startup via capability grants.

## 1.3 Async Runtime Assessment

If the project uses async:

```bash
grep -rn "tokio\|async-std\|#\[tokio::main\]\|#\[async_std::main\]" src/ --include="*.rs"
grep -rn "\.await" src/ --include="*.rs" | wc -l
grep -rn "tokio::spawn\|task::spawn" src/ --include="*.rs" | wc -l
```

Map each async pattern:

| Rust Async Pattern        | MAGE Equivalent                      |
| ------------------------- | ------------------------------------- |
| `#[tokio::main]`          | `+af main() / async { }`              |
| `tokio::spawn(future)`    | `Swarm.spawn(agent)`                  |
| `tokio::select!`          | `async.select(...)`                   |
| `tokio::time::sleep`      | `time.sleep(duration)`                |
| `tokio::sync::Mutex`      | `std.sync.Mutex` (same semantics)     |
| `tokio::fs::read`         | `fs.read_to_string(path)` with `/ io` |
| `tokio::net::TcpListener` | `net.TcpListener` with `/ net`        |

## 1.4 Effort Estimation

Use this formula for rough sizing:

```
Lines of Rust code (excluding tests):  L
Unsafe blocks:                         U
Async spawn sites:                     A
Direct crate dependencies:             D

Estimated migration effort (person-hours):
  Syntax pass:      L × 0.02    (2 min per 100 lines, mostly automated)
  Unsafe removal:   U × 1.0     (1 hour per unsafe block)
  Async migration:  A × 0.5     (30 min per spawn site)
  Dep wiring:       D × 0.25    (15 min per dependency)
  Testing:          L × 0.01    (1 min per 100 lines)

  Total ≈ L×0.03 + U×1.0 + A×0.5 + D×0.25
```

**Example:** A 10,000-line project with 5 unsafe blocks, 20 spawn sites, and
30 dependencies:

```
10000 × 0.03 = 300 hours (syntax — but mg migrate automates ~80%)
5 × 1.0      =   5 hours
20 × 0.5     =  10 hours
30 × 0.25    =   7.5 hours
              ─────────
Total        ≈  ~80 hours (with automation), ~320 hours (manual)
```

## 1.5 Phase Planning

Recommended migration phases:

### Phase 1: Dual-Build (Week 1-2)
- Set up MAGE project alongside existing Rust
- Wire `[rust-dependencies]` for all existing crates
- Migrate one self-contained module as a proof of concept
- Verify `mg build` + `cargo build` both work

### Phase 2: Syntax Migration (Week 3-4)
- Run `mg migrate --dry-run` on all source files
- Review and accept automated translations
- Fix edge cases the tool can't handle
- Run `mg check` until clean

### Phase 3: Effect Annotation (Week 5-6)
- Add `/ io`, `/ net`, etc. to all impure functions
- Remove `unsafe` blocks, replace with Capabilities
- Run `mg check --effects` to validate effect propagation

### Phase 4: Agent Adoption (Week 7-8)
- Replace `tokio::spawn` with `Swarm.spawn`
- Convert eligible structs into `Agent` implementations
- Add capability grants to the application manifest

### Phase 5: Testing & Validation (Week 9-10)
- Port all `#[test]` functions to `@test`
- Add effect handler mocks for I/O-heavy tests
- Run benchmarks, compare with Rust baseline
- Set up CI with `mg test` and `mg bench`

## 1.6 Go/No-Go Checklist

Before starting migration, confirm:

- [ ] All direct dependencies are available or can be used via `[rust-dependencies]`
- [ ] Team has read the [MAGE Book](../book/) chapters 1-4
- [ ] `mg` CLI is installed and `mg new test-project` works
- [ ] CI infrastructure can run `mg build` and `mg test`
- [ ] A single module has been migrated as proof-of-concept
- [ ] Stakeholders approve the timeline estimate
- [ ] Rollback plan exists (keep Rust source in a `rust-src/` backup)

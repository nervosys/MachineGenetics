# Step 4: Build, Run, Test

## Project Structure

A standard MAGE project looks like this:

```
my-project/
├── mg.toml              # project manifest
├── src/
│   ├── main.mg          # binary entry point
│   └── lib.mg           # library root (optional)
├── tests/
│   └── integration.mg   # integration tests
└── benches/
    └── perf.mg          # benchmarks
```

## `mg.toml`

```toml
[package]
name = "my-project"
version = "0.1.0"
edition = "2026"

[dependencies]
# dependencies go here

[dev-dependencies]
# test-only dependencies
```

## Build

```bash
mg build                   # debug build
mg build --release         # optimized release build
mg build --target wasm32   # cross-compile to WASM
```

Output goes to `target/debug/` or `target/release/`.

## Run

```bash
mg run                     # build + run (debug)
mg run --release            # build + run (release)
mg run src/main.mg         # run a specific file
```

## Check (Type-Check Only)

```bash
mg check                   # fast — no codegen
```

Use this during development for quick feedback.

## Test

### Writing Tests

```MAGE
// src/math.mg

+f add(a: i32, b: i32) -> i32 {
    a + b
}

+f factorial(n: u64) -> u64 {
    ? n <= 1 { ret 1 }
    n * factorial(n - 1)
}

// Tests go in the same file or in tests/
@test
f test_add() {
    assert(add(2, 3) == 5)
    assert(add(-1, 1) == 0)
    assert(add(0, 0) == 0)
}

@test
f test_factorial() {
    assert(factorial(0) == 1)
    assert(factorial(1) == 1)
    assert(factorial(5) == 120)
    assert(factorial(10) == 3628800)
}
```

### Running Tests

```bash
mg test                     # run all tests
mg test test_add            # run tests matching a name
mg test --verbose           # show each test name
```

Output:

```
running 2 tests
test test_add ... ok
test test_factorial ... ok

test result: ok. 2 passed; 0 failed
```

## Format

```bash
mg fmt                      # format all .mg files
mg fmt --check              # check formatting (CI mode)
mg fmt src/main.mg         # format one file
```

## Lint

```bash
mg lint                     # run all lints
mg lint --fix               # auto-fix where possible
```

## Generate Documentation

```bash
mg doc                      # generate HTML docs
mg doc --open               # generate and open in browser
```

## Benchmarks

```MAGE
// benches/perf.mg

@bench
f bench_factorial(b: &!Bencher) {
    b.iter(|| factorial(20))
}
```

```bash
mg bench                    # run all benchmarks
mg bench bench_factorial    # run specific benchmark
```

## Common Workflows

### Development Loop

```bash
mg check     # fast type-check
mg test      # run tests
mg run       # run the program
```

### CI Pipeline

```bash
mg fmt --check     # verify formatting
mg lint            # verify lint rules
mg check           # type-check
mg test            # run tests
mg build --release # build release binary
```

### REPL Experimentation

```bash
mg repl            # interactive mode
mg eval '2 + 3'    # one-shot expression evaluation
```

---

**[Next: What's Next? →](05-whats-next.md)**

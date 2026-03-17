# Step 4: Build, Run, Test

## Project Structure

A standard Redox project looks like this:

```
my-project/
├── rdx.toml              # project manifest
├── src/
│   ├── main.rdx          # binary entry point
│   └── lib.rdx           # library root (optional)
├── tests/
│   └── integration.rdx   # integration tests
└── benches/
    └── perf.rdx          # benchmarks
```

## `rdx.toml`

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
rdx build                   # debug build
rdx build --release         # optimized release build
rdx build --target wasm32   # cross-compile to WASM
```

Output goes to `target/debug/` or `target/release/`.

## Run

```bash
rdx run                     # build + run (debug)
rdx run --release            # build + run (release)
rdx run src/main.rdx         # run a specific file
```

## Check (Type-Check Only)

```bash
rdx check                   # fast — no codegen
```

Use this during development for quick feedback.

## Test

### Writing Tests

```redox
// src/math.rdx

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
rdx test                     # run all tests
rdx test test_add            # run tests matching a name
rdx test --verbose           # show each test name
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
rdx fmt                      # format all .rdx files
rdx fmt --check              # check formatting (CI mode)
rdx fmt src/main.rdx         # format one file
```

## Lint

```bash
rdx lint                     # run all lints
rdx lint --fix               # auto-fix where possible
```

## Generate Documentation

```bash
rdx doc                      # generate HTML docs
rdx doc --open               # generate and open in browser
```

## Benchmarks

```redox
// benches/perf.rdx

@bench
f bench_factorial(b: &!Bencher) {
    b.iter(|| factorial(20))
}
```

```bash
rdx bench                    # run all benchmarks
rdx bench bench_factorial    # run specific benchmark
```

## Common Workflows

### Development Loop

```bash
rdx check     # fast type-check
rdx test      # run tests
rdx run       # run the program
```

### CI Pipeline

```bash
rdx fmt --check     # verify formatting
rdx lint            # verify lint rules
rdx check           # type-check
rdx test            # run tests
rdx build --release # build release binary
```

### REPL Experimentation

```bash
rdx repl            # interactive mode
rdx eval '2 + 3'    # one-shot expression evaluation
```

---

**[Next: What's Next? →](05-whats-next.md)**

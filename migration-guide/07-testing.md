# Chapter 7: Testing & Quality Migration

Migrate your Rust test suite to Redox: unit tests, integration tests, effect
mocking, benchmarks, and CI pipelines.

---

## 7.1 Unit Test Migration

### Basic Tests

```diff
  // Rust
- #[test]
- fn test_addition() {
-     assert_eq!(add(2, 3), 5);
- }
-
- #[test]
- #[should_panic(expected = "overflow")]
- fn test_overflow() {
-     add(u32::MAX, 1);
- }

  // Redox
+ @test
+ f test_addition() {
+     assert_eq!(add(2, 3), 5)
+ }
+
+ @test
+ @should_panic(expected: "overflow")
+ f test_overflow() {
+     add(u32.MAX, 1)
+ }
```

### Test Modules

```diff
  // Rust
- #[cfg(test)]
- mod tests {
-     use super::*;
-
-     #[test]
-     fn it_works() {
-         let result = compute(42);
-         assert!(result > 0);
-     }
- }

  // Redox
+ @cfg(test)
+ M tests {
+     u super.*
+
+     @test
+     f it_works() {
+         v result = compute(42)
+         assert!(result > 0)
+     }
+ }
```

### Assertion Macros

| Rust                      | Redox                     | Notes     |
| ------------------------- | ------------------------- | --------- |
| `assert!(cond)`           | `assert!(cond)`           | Identical |
| `assert_eq!(a, b)`        | `assert_eq!(a, b)`        | Identical |
| `assert_ne!(a, b)`        | `assert_ne!(a, b)`        | Identical |
| `assert!(cond, "msg")`    | `assert!(cond, "msg")`    | Identical |
| `debug_assert!(cond)`     | `debug_assert!(cond)`     | Identical |
| `assert_matches!(v, Pat)` | `assert_matches!(v, Pat)` | Identical |

Assertion macros are unchanged — they are part of the Redox prelude.

### Result-Returning Tests

```diff
  // Rust
- #[test]
- fn test_with_result() -> Result<(), Box<dyn std::error::Error>> {
-     let val = parse("42")?;
-     assert_eq!(val, 42);
-     Ok(())
- }

  // Redox
+ @test
+ f test_with_result() -> R[(), ^dyn Error] {
+     v val = parse("42")?
+     assert_eq!(val, 42)
+     Ok(())
+ }
```

## 7.2 Async Test Migration

```diff
  // Rust with tokio
- #[tokio::test]
- async fn test_fetch() {
-     let result = fetch_data("https://example.com").await.unwrap();
-     assert!(!result.is_empty());
- }

  // Redox — built-in async test support
+ @test
+ af test_fetch() / net {
+     v result = fetch_data("https://example.com").await.unwrap()
+     assert!(!result.is_empty())
+ }
```

No special test macro needed — `af` (async fn) in a `@test` runs on the
built-in async runtime automatically.

## 7.3 Effect Mocking

Redox's effect system enables test isolation without mock libraries.

### Basic Effect Mocking

```diff
  // Rust — requires trait objects or mockall
- use mockall::automock;
-
- #[automock]
- trait Storage {
-     fn read(&self, key: &str) -> Option<String>;
-     fn write(&mut self, key: &str, value: &str);
- }
-
- #[test]
- fn test_with_mock() {
-     let mut mock = MockStorage::new();
-     mock.expect_read()
-         .with(eq("key"))
-         .returning(|_| Some("value".to_string()));
-
-     let result = process(&mock, "key");
-     assert_eq!(result, "processed: value");
- }

  // Redox — handle block intercepts effects
+ // Production function that uses io effect
+ f read_config(path: &s) -> R[Config, Error] / io {
+     v text = fs.read_to_string(path)?
+     Config.parse(&text)
+ }
+
+ @test
+ f test_read_config() {
+     // handle block intercepts io effects
+     v result = handle / io {
+         read_config("app.toml")
+     } with {
+         fs.read_to_string(_) => Ok(s.from("port = 8080")),
+     }
+
+     v config = result.unwrap()
+     assert_eq!(config.port, 8080)
+ }
```

### Network Effect Mocking

```diff
  // Rust — requires wiremock or custom test server setup
- #[tokio::test]
- async fn test_api_call() {
-     let mock_server = MockServer::start().await;
-     Mock::given(method("GET"))
-         .and(path("/api/data"))
-         .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
-         .mount(&mock_server)
-         .await;
-
-     let result = fetch_api(&mock_server.uri()).await.unwrap();
-     assert!(result.ok);
- }

  // Redox — handle block replaces the HTTP calls
+ @test
+ af test_api_call() {
+     v result = handle / net {
+         fetch_api("https://api.example.com").await
+     } with {
+         http.get(_) => Ok(Response.json(@{ ok: 1b })),
+     }
+
+     assert!(result.unwrap().ok)
+ }
```

### File System Mocking

```diff
  // Rust — requires tempdir or custom abstractions
- use tempfile::TempDir;
-
- #[test]
- fn test_file_ops() {
-     let dir = TempDir::new().unwrap();
-     let path = dir.path().join("test.txt");
-     std::fs::write(&path, "hello").unwrap();
-
-     let result = read_and_process(&path).unwrap();
-     assert_eq!(result, "HELLO");
- }

  // Redox — mock io effects directly
+ @test
+ f test_file_ops() {
+     m files = {s: s}.new()
+     files.insert(s.from("/test.txt"), s.from("hello"))
+
+     v result = handle / io {
+         read_and_process("/test.txt")
+     } with {
+         fs.read_to_string(p) => {
+             ? files.get(p) {
+                 Some(content) => Ok(content.clone()),
+                 None => Err(Error.not_found(p)),
+             }
+         },
+     }
+
+     assert_eq!(result.unwrap(), "HELLO")
+ }
```

## 7.4 Integration Tests

```diff
  // Rust — tests/ directory
  // tests/integration_test.rs
- use my_crate::process;
-
- #[test]
- fn end_to_end() {
-     let input = std::fs::read_to_string("fixtures/input.json").unwrap();
-     let result = process(&input).unwrap();
-     let expected = std::fs::read_to_string("fixtures/expected.json").unwrap();
-     assert_eq!(result, expected);
- }

  // Redox — tests/ directory
  // tests/integration_test.rdx
+ u my_crate.process
+
+ @test
+ f end_to_end() / io {
+     v input = fs.read_to_string("fixtures/input.json")?
+     v result = process(&input)?
+     v expected = fs.read_to_string("fixtures/expected.json")?
+     assert_eq!(result, expected)
+ }
```

### Test Organization

```text
my-project/
├── src/
│   ├── main.rdx
│   └── lib.rdx
├── tests/                  # Integration tests
│   ├── api_test.rdx
│   └── cli_test.rdx
├── fixtures/               # Test data
│   ├── input.json
│   └── expected.json
└── Forge.toml
```

## 7.5 Benchmark Migration

```diff
  // Rust — criterion
- use criterion::{black_box, criterion_group, criterion_main, Criterion};
-
- fn bench_sort(c: &mut Criterion) {
-     c.bench_function("sort_1000", |b| {
-         b.iter(|| {
-             let mut data = black_box(generate_data(1000));
-             data.sort();
-         })
-     });
- }
-
- criterion_group!(benches, bench_sort);
- criterion_main!(benches);

  // Redox — built-in benchmarks
+ @bench
+ f bench_sort(b: &!Bencher) {
+     v data_template = generate_data(1000)
+     b.iter(|| {
+         m data = data_template.clone()
+         data.sort()
+     })
+ }
```

Run benchmarks:

```bash
rdx bench                # run all benchmarks
rdx bench sort           # filter by name
rdx bench --save base    # save as baseline
rdx bench --compare base # compare to baseline
```

## 7.6 Property Testing

```diff
  // Rust — proptest
- use proptest::prelude::*;
-
- proptest! {
-     #[test]
-     fn test_roundtrip(input in "\\PC*") {
-         let encoded = encode(&input);
-         let decoded = decode(&encoded).unwrap();
-         prop_assert_eq!(input, decoded);
-     }
- }

  // Redox — built-in property testing
+ u std.test.property
+
+ @test
+ @property
+ f test_roundtrip(input: s ~ any_string()) {
+     v encoded = encode(&input)
+     v decoded = decode(&encoded).unwrap()
+     assert_eq!(input, decoded)
+ }
```

## 7.7 Test Configuration

### Conditional Tests

```diff
  // Rust
- #[test]
- #[ignore]
- fn expensive_test() { /* ... */ }
-
- #[test]
- #[cfg(target_os = "linux")]
- fn linux_only() { /* ... */ }

  // Redox
+ @test
+ @ignore
+ f expensive_test() { /* ... */ }
+
+ @test
+ @cfg(target_os: "linux")
+ f linux_only() { /* ... */ }
```

### Test Helpers

```diff
  // Rust — helper in test module
- #[cfg(test)]
- mod tests {
-     fn setup() -> TestContext {
-         TestContext::new()
-     }
-
-     #[test]
-     fn test_a() {
-         let ctx = setup();
-         assert!(ctx.run().is_ok());
-     }
- }

  // Redox
+ @cfg(test)
+ M tests {
+     f setup() -> TestContext {
+         TestContext.new()
+     }
+
+     @test
+     f test_a() {
+         v ctx = setup()
+         assert!(ctx.run().is_ok())
+     }
+ }
```

## 7.8 CI Pipeline Migration

### GitHub Actions

```diff
  # Rust CI
- name: Rust CI
- on: [push, pull_request]
- jobs:
-   test:
-     runs-on: ubuntu-latest
-     steps:
-       - uses: actions/checkout@v4
-       - uses: dtolnay/rust-toolchain@stable
-       - run: cargo test
-       - run: cargo clippy -- -D warnings
-       - run: cargo fmt -- --check

  # Redox CI
+ name: Redox CI
+ on: [push, pull_request]
+ jobs:
+   test:
+     runs-on: ubuntu-latest
+     steps:
+       - uses: actions/checkout@v4
+       - uses: nervosys/setup-rdx@v1
+       - run: rdx test
+       - run: rdx lint
+       - run: rdx fmt --check
```

### Dual CI (Migration Period)

```yaml
name: Dual CI
on: [push, pull_request]
jobs:
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace
      - run: cargo clippy -- -D warnings

  redox:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: nervosys/setup-rdx@v1
      - run: rdx test
      - run: rdx lint
```

## 7.9 Coverage

```bash
# Rust
cargo tarpaulin --out Html

# Redox
rdx test --coverage
rdx test --coverage --format html --output coverage/
```

### Coverage in CI

```yaml
  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: nervosys/setup-rdx@v1
      - run: rdx test --coverage --format lcov --output coverage.lcov
      - uses: codecov/codecov-action@v4
        with:
          files: coverage.lcov
```

## 7.10 Testing Migration Checklist

- [ ] Convert all `#[test]` to `@test`
- [ ] Convert all `#[ignore]` to `@ignore`
- [ ] Convert `#[should_panic]` to `@should_panic`
- [ ] Update assertion macro syntax (mostly unchanged)
- [ ] Convert `#[cfg(test)]` modules to `@cfg(test)`
- [ ] Replace mock libraries with `handle` blocks for effect mocking
- [ ] Convert `#[tokio::test]` to `@test af` without runtime annotation
- [ ] Migrate criterion benchmarks to `@bench` functions
- [ ] Replace tempfile/tempdir with io effect mocking
- [ ] Update CI pipeline: `cargo test` → `rdx test`
- [ ] Update CI pipeline: `cargo clippy` → `rdx lint`
- [ ] Update CI pipeline: `cargo fmt` → `rdx fmt`
- [ ] Set up coverage: `rdx test --coverage`
- [ ] Run full suite: `rdx test && rdx lint && rdx fmt --check`

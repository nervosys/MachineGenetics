# Testing

> Recipes for tests, mocks and benchmarks. Agent-mode syntax; every block was
> verified with `mage-parse --check`, and the tests below were run.

The previous version of this page tested against a harness that does not
exist: `u std.test.*`, `assert_eq(a, b, msg)`, `TestFixture.setup()`,
`handle(|| greet("Alice"), |effect| effect.resume(8))`, `#[should_panic]`,
property generators. What MAGE actually gives you is smaller and stronger:

- **`@test`** marks a function; its **return value** is the result.
- **`assert(cond)`** / `assert(cond, "message")` is the only assertion.
- **`handle { … } with E { … }`** is the mocking mechanism, and it is part of
  the type system rather than a library — a handler can only substitute
  operations of a *declared* effect, and it discharges that effect for exactly
  the block it wraps.

---

### Table-driven tests

**Problem**: Test a function against many input/output pairs.

**Solution**:

```MAGE
f add(a: i32, b: i32) -> i32 { a + b }

// A table is a list of records. Tuple destructuring in a binding does not
// parse (`v (a, b) = pair`), so name the columns.
+S Case { a: i32, b: i32, want: i32 }

@test
f test_add() -> bool {
    v cases = [
        @Case { a: 0, b: 0, want: 0 },
        @Case { a: 1, b: 2, want: 3 },
        @Case { a: -1, b: 1, want: 0 },
        @Case { a: 100, b: -100, want: 0 },
    ]
    all(cases, |c| add(c.a, c.b) == c.want)
}
```

**Discussion**: A test is a function marked `@test` whose **value** the runner checks — there is no `assert_eq`, and `assert(cond, "msg")` is the only assertion. Tuple destructuring in a binding does not parse (`v (a, b) = pair`), so a table is a list of records.

---

### Setup and teardown

**Problem**: Create shared state before a test and clean it up after.

**Solution**:

```MAGE
// There is no fixture protocol and no automatic teardown. `defer` runs at
// scope exit, which is the whole mechanism.
f setup(dir: s) -> s / fs {
    fs.mkdir(dir)
    fs.write(f"{dir}/data.txt", "test data")
    dir
}

@test
+f test_read_data() -> bool / fs {
    v dir = setup("build/test_tmp")
    defer fs.remove(dir)
    fs.read_to_string(f"{dir}/data.txt") == "test data"
}
```

**Discussion**: There is no fixture protocol, no `Drop`, and no automatic teardown. `defer` runs its expression at scope exit, and that is the entire mechanism.

---

### Mock an effect

**Problem**: Test a function that performs I/O, without performing any.

**Solution**:

```MAGE
// Declaring an effect is what makes a function mockable: a handler can only
// substitute operations that belong to a declared effect.
effect Clock {
    f hour() -> i32;
}

+f greet(name: s) -> s / clock {
    v hour = Clock.hour()
    ? hour < 12 { f"Good morning, {name}!" } : { f"Good afternoon, {name}!" }
}

// `handle … with` removes the effect for the block it wraps, so the test is
// pure — no clock, no I/O, and no unhandled call elsewhere silenced with it.
@test
f test_morning() -> bool {
    handle {
        greet("Alice")
    } with Clock {
        hour() => 8,
    } == "Good morning, Alice!"
}

@test
f test_afternoon() -> bool {
    handle {
        greet("Bob")
    } with Clock {
        hour() => 14,
    } == "Good afternoon, Bob!"
}
```

**Discussion**: **This is what the effect system is for.** Put the operation behind a declared `effect`, and `handle … with` substitutes it for one block — the test is pure, and an unhandled call elsewhere in the program still reports. A handler arm names the operation bare and binds its parameters by name; `_` is not a parameter name.

---

### Property-based testing

**Problem**: Check a property over many generated inputs.

**Solution**:

```MAGE
f reverse_str(text: s) -> s { join(reverse(chars(text)), "") }

// There is no property-testing harness and no generator. Generate the inputs
// yourself — `rng` is a capability, so a test that uses it declares it, and a
// deterministic table is usually the better answer.
@test
+f test_reverse_twice_is_identity() -> bool / rng {
    v inputs = map(range(20), |n| f"case {n}")
    all(inputs, |text| reverse_str(reverse_str(text)) == text)
}
```

**Discussion**: There is no property-testing harness and no shrinking. Generate the inputs yourself and use `all`. `rng` is a capability, so a test that reaches for randomness declares it — which is usually the signal that a fixed table would be the better test.

---

### Benchmark a function

**Problem**: Measure how long a function takes.

**Solution**:

```MAGE
f factorial(n: i32) -> i32 {
    ? n <= 1 { 1 } : { n * factorial(n - 1) }
}

// `@bench` marks a benchmark. There is no `Bencher` and no `b.iter(…)` —
// the harness times the call.
@bench
f bench_factorial() -> i32 {
    factorial(20)
}
```

**Discussion**: `@bench` marks a benchmark. There is no `Bencher` type and no `b.iter(…)` closure; the harness times the call.

---

### Test expected errors

**Problem**: Assert that a function fails the way it should.

**Solution**:

```MAGE
+f divide(a: i32, b: i32) -> R[i32, s] {
    guard b != 0 else { ret Err("divide by zero") }
    Ok(a / b)
}

// Test the error case by matching it. There is no `#[should_panic]`, and a
// panicking test would abort the run rather than pass.
@test
f test_divide_by_zero() -> bool {
    ?= divide(1, 0) {
        Ok(_) => 0b,
        Err(msg) => msg == "divide by zero",
    }
}
```

**Discussion**: Match the error. There is no `#[should_panic]`, and a panicking test aborts the run rather than passing — which is the right default for a language where `panic` is a declared capability.

---

### Snapshot testing

**Problem**: Compare output against a recorded snapshot.

**Solution**:

```MAGE
f render(name: s, rows: i32) -> s {
    join([f"report for {name}", f"{rows} rows"], "\n")
}

// A snapshot is a string comparison against a file you keep in the repo.
// There is no snapshot library and no `--update` flag; the effect is `fs`,
// declared like any other.
@test
+f test_render_matches_snapshot() -> bool / fs {
    render("alpha", 3) == fs.read_to_string("tests/snapshots/report.txt")
}
```

**Discussion**: A snapshot is a file you keep in the repo and a string comparison. Reading it performs `fs`, declared like anything else. There is no snapshot library and no `--update` flag: the recorded answer changes when you change it.

---

### Test async code

**Problem**: Test a function that would otherwise make a network call.

**Solution**:

```MAGE
+af fetch(url: s) -> s / net {
    net.connect(url)
}

// An async function is tested like any other — there is no runtime to start
// and no `.await` to write at the call site. Mock the capability by putting
// the work behind a declared effect instead.
effect Http {
    f get(url: s) -> s;
}

f fetch_via(url: s) -> s / http { Http.get(url) }

@test
f test_fetch() -> bool {
    handle {
        fetch_via("https://example.com")
    } with Http {
        get(url) => "hello",
    } == "hello"
}
```

**Discussion**: An async function needs no runtime and no `.await` at the call site. The mockable seam is the same one as everywhere else: put the call behind a declared effect and handle it.

---

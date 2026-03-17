# Testing

---

### Table-driven tests

**Problem**: Test a function against many input/output pairs without
duplicating test code.

**Solution**:

```rdx
u std.test.*

f add(a: i32, b: i32) -> i32 { a + b }

@test
f test_add() {
    v cases = [
        (0, 0, 0),
        (1, 2, 3),
        (-1, 1, 0),
        (i32.MAX, 0, i32.MAX),
        (100, -100, 0),
    ]~

    @ (a, b, expected) : &cases {
        assert_eq(add(*a, *b), *expected,
            f"add({a}, {b}) should be {expected}")
    }
}
```

---

### Test with setup and teardown

**Problem**: Create shared state before tests and clean up afterward.

**Solution**:

```rdx
u std.test.*
u std.fs

S TestFixture {
    dir: s,
}

I ~ TestFixture {
    +f setup() -> Self / io {
        v dir = f"test_tmp_{env.pid()}"
        fs.create_dir(&dir).unwrap()
        fs.write(f"{dir}/data.txt", "test data").unwrap()
        TestFixture @{ dir }
    }

    +f teardown(&self) / io {
        fs.remove_dir_all(&self.dir).unwrap()
    }
}

@test
f test_read_data() / io {
    v fixture = TestFixture.setup()

    v content = fs.read(f"{fixture.dir}/data.txt")?
    assert_eq(content, "test data")

    fixture.teardown()
}
```

---

### Mock an effect

**Problem**: Test a function that performs I/O without actual I/O.

**Solution**:

```rdx
u std.test.*
u std.effect.handle

// Function under test
+f greet(name: &s) -> s / io {
    v time = get_time_of_day()
    ? time < 12 { f"Good morning, {name}!" }
    : { f"Good afternoon, {name}!" }
}

@test
f test_morning_greeting() {
    // Mock the io effect to return a fixed time
    v result = handle(|| greet("Alice"), |effect| {
        ? effect.is("get_time_of_day") {
            effect.resume(8)  // 8 AM
        }
    })

    assert_eq(result, "Good morning, Alice!")
}

@test
f test_afternoon_greeting() {
    v result = handle(|| greet("Bob"), |effect| {
        ? effect.is("get_time_of_day") {
            effect.resume(14)  // 2 PM
        }
    })

    assert_eq(result, "Good afternoon, Bob!")
}
```

---

### Property-based testing

**Problem**: Verify a property holds for any input, not just hand-picked
examples.

**Solution**:

```rdx
u std.test.{prop, assert_true}

f reverse[T: Clone](items: &[T]~) -> [T]~ {
    m r = items.clone()
    r.reverse()
    r
}

@test
f test_reverse_is_involution() {
    // Reversing twice gives back the original
    prop(|items: [i32]~| {
        assert_eq(reverse(&reverse(&items)), items)
    })
}

@test
f test_reverse_preserves_length() {
    prop(|items: [i32]~| {
        assert_eq(reverse(&items).len(), items.len())
    })
}

@test
f test_sort_is_idempotent() {
    prop(|items: [i32]~| {
        m a = items.clone()
        a.sort()
        m b = a.clone()
        b.sort()
        assert_eq(a, b)
    })
}
```

---

### Benchmark a function

**Problem**: Measure the performance of a function.

**Solution**:

```rdx
u std.test.{Bencher, black_box}

f fibonacci(n: u64) -> u64 {
    ? n <= 1 { ret n }
    m a: u64 = 0
    m b: u64 = 1
    @ _ : 2..=n {
        v tmp = a + b
        a = b
        b = tmp
    }
    b
}

@bench
f bench_fib_20(b: &!Bencher) {
    b.iter(|| black_box(fibonacci(black_box(20))))
}

@bench
f bench_fib_40(b: &!Bencher) {
    b.iter(|| black_box(fibonacci(black_box(40))))
}
```

Run with `rdx bench`.

---

### Test expected errors

**Problem**: Verify a function returns the correct error variant.

**Solution**:

```rdx
u std.test.*

+f divide(a: f64, b: f64) -> R[f64, s] {
    ? b == 0.0 {
        Err("division by zero".into())
    } : {
        Ok(a / b)
    }
}

@test
f test_divide_by_zero() {
    v result = divide(1.0, 0.0)
    assert_err(&result)

    ? result => Err(msg) {
        assert_eq(msg, "division by zero")
    }
}

@test
f test_divide_ok() {
    v result = divide(10.0, 2.0)
    assert_ok(&result)
    assert_eq(result.unwrap(), 5.0)
}
```

---

### Snapshot testing

**Problem**: Compare output against a saved "golden" file.

**Solution**:

```rdx
u std.test.*
u std.fs

f assert_snapshot(name: &s, actual: &s) / io {
    v path = f"tests/snapshots/{name}.snap"

    ? fs.exists(&path) {
        v expected = fs.read(&path)?
        assert_eq(actual, &expected,
            f"Snapshot mismatch for '{name}'. Run with UPDATE_SNAPSHOTS=1 to update.")
    } : {
        // First run — create the snapshot
        fs.create_dir_all("tests/snapshots")?
        fs.write(&path, actual)?
        p"Created snapshot: {path}"
    }
}

@test
f test_report_output() / io {
    v report = generate_report(&sample_data())
    assert_snapshot("report_output", &report)
}
```

**Discussion**: To update snapshots, delete the `.snap` file and re-run
the test, or add a `UPDATE_SNAPSHOTS` environment check.

---

### Test async code

**Problem**: Test an async function.

**Solution**:

```rdx
u std.test.*

+af fetch_name(id: u64) -> R[s, Error] / net {
    v resp = Request.get(f"https://api.example.com/users/{id}").send().await?
    v user: User = from_str(&resp.text().await?)?
    Ok(user.name)
}

@test
af test_fetch_name() / net {
    // With a mock handler for the net effect
    v name = handle(|| fetch_name(1).await, |effect| {
        ? effect.is("http_get") {
            effect.resume(r#"{"name": "Alice", "id": 1}"#)
        }
    }).await

    assert_eq(name.unwrap(), "Alice")
}
```

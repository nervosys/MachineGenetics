# Testing

Redox's test module includes assertions, benchmarking, and property-based
testing — all in the standard library.

## Basic assertions

```rdx
u std.test.*

@test
f test_addition() {
    assert_eq(2 + 2, 4)
    assert_ne(2 + 2, 5)
    assert_true(10 > 5)
}

@test
f test_option() {
    v x: ?i32 = Some(42)
    assert_some(x)

    v y: ?i32 = None
    assert_none(y)
}

@test
f test_result() {
    v ok: R[i32, s] = Ok(42)
    assert_ok(ok)

    v err: R[i32, s] = Err("oops".into())
    assert_err(err)
}
```

## Running tests

```sh
rdx test                    # Run all tests
rdx test --filter "sort"    # Run tests containing "sort"
rdx test -- --nocapture     # Show print output
```

## Approximate equality

For floating-point comparisons:

```rdx
@test
f test_pi() {
    v computed_pi = 4.0 * (1.0 - 1.0/3.0 + 1.0/5.0 - 1.0/7.0)
    assert_approx(computed_pi, 3.14159, 0.01)
}
```

## Benchmarking

```rdx
u std.test.{Bencher, black_box}

@bench
f bench_sort(b: &!Bencher) {
    v data = [5, 3, 1, 4, 2]~

    b.iter(|| {
        m d = data.clone()
        d.sort()
        black_box(d)
    })
}
```

Run benchmarks:

```sh
rdx bench
```

## Property-based testing

Generate random inputs and verify properties:

```rdx
u std.test.{prop, Arbitrary}

@test
f test_sort_is_sorted() {
    prop(|items: [i32]~| {
        m sorted = items.clone()
        sorted.sort()

        // Property: every adjacent pair is in order
        @ i : 0..sorted.len().saturating_sub(1) {
            assert_true(sorted[i] <= sorted[i + 1])
        }
    })
}

@test
f test_reverse_reverse_is_identity() {
    prop(|items: [i32]~| {
        m reversed = items.clone()
        reversed.reverse()
        reversed.reverse()
        assert_eq(reversed, items)
    })
}
```

### Custom Arbitrary implementations

```rdx
u std.test.Arbitrary

S Point { x: f64, y: f64 }

I Arbitrary ~ Point {
    +f arbitrary(rng: &!Rng) -> Self {
        Point @{
            x: f64.arbitrary(rng),
            y: f64.arbitrary(rng),
        }
    }

    +f shrink(&self) -> [Self]~ {
        [
            Point @{ x: 0.0, y: self.y },
            Point @{ x: self.x, y: 0.0 },
            Point @{ x: self.x / 2.0, y: self.y / 2.0 },
        ]~
    }
}
```

Property testing with custom config:

```rdx
u std.test.PropConfig

@test
f test_with_many_cases() {
    prop_with_config(
        PropConfig @{ num_tests: 10_000, max_shrink_steps: 200 },
        |x: i32| {
            assert_eq(x + 0, x)    // additive identity
        },
    )
}
```

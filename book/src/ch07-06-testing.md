# Testing

Redox's test module includes assertions, benchmarking, and property-based
testing — all in the standard library.

## Basic assertions

```rdx
use std::test::*;

#[test]
fn test_addition() {
    assert_eq!(2 + 2, 4);
    assert_ne!(2 + 2, 5);
    assert!(10 > 5);
}

#[test]
fn test_option() {
    let x: Option<i32> = Some(42);
    assert_some!(x);

    let y: Option<i32> = None;
    assert_none!(y);
}

#[test]
fn test_result() {
    let ok: Result<i32, String> = Ok(42);
    assert_ok!(ok);

    let err: Result<i32, String> = Err("oops".into());
    assert_err!(err);
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
#[test]
fn test_pi() {
    let computed_pi = 4.0 * (1.0 - 1.0/3.0 + 1.0/5.0 - 1.0/7.0);
    assert_approx!(computed_pi, 3.14159, 0.01);
}
```

## Benchmarking

```rdx
use std::test::{Bencher, black_box};

#[bench]
fn bench_sort(b: &mut Bencher) {
    let data = vec![5, 3, 1, 4, 2];

    b.iter(|| {
        let mut d = data.clone();
        d.sort();
        black_box(d)
    });
}
```

Run benchmarks:

```sh
rdx bench
```

## Property-based testing

Generate random inputs and verify properties:

```rdx
use std::test::{prop, Arbitrary};

#[test]
fn test_sort_is_sorted() {
    prop(|items: Vec<i32>| {
        let mut sorted = items.clone();
        sorted.sort();

        // Property: every adjacent pair is in order
        for i in 0..sorted.len().saturating_sub(1) {
            assert!(sorted[i] <= sorted[i + 1]);
        }
    });
}

#[test]
fn test_reverse_reverse_is_identity() {
    prop(|items: Vec<i32>| {
        let mut reversed = items.clone();
        reversed.reverse();
        reversed.reverse();
        assert_eq!(reversed, items);
    });
}
```

### Custom Arbitrary implementations

```rdx
use std::test::Arbitrary;

struct Point { x: f64, y: f64 }

impl Arbitrary for Point {
    pub fn arbitrary(rng: &mut Rng) -> Self {
        Point {
            x: f64::arbitrary(rng),
            y: f64::arbitrary(rng),
        }
    }

    pub fn shrink(&self) -> Vec<Self> {
        vec![
            Point { x: 0.0, y: self.y },
            Point { x: self.x, y: 0.0 },
            Point { x: self.x / 2.0, y: self.y / 2.0 },
        ]
    }
}
```

Property testing with custom config:

```rdx
use std::test::PropConfig;

#[test]
fn test_with_many_cases() {
    prop_with_config(
        PropConfig { num_tests: 10_000, max_shrink_steps: 200 },
        |x: i32| {
            assert_eq!(x + 0, x);    // additive identity
        },
    );
}
```

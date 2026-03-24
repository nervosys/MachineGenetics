//! # std::test — Testing & Benchmarking
//!
//! Assertions, test runners, benchmarking, and property-based testing.

// ---------------------------------------------------------------------------
// Assertions
// ---------------------------------------------------------------------------

/// Assert that a condition is true. Panics with `msg` if false.
pub fn assert(cond: bool, msg: &String);

/// Assert two values are equal.
pub fn assert_eq<T: PartialEq + std::fmt::Debug>(left: &T, right: &T);

/// Assert two values are not equal.
pub fn assert_ne<T: PartialEq + std::fmt::Debug>(left: &T, right: &T);

/// Assert that a float is within `epsilon` of an expected value.
pub fn assert_approx(actual: f64, expected: f64, epsilon: f64);

/// Assert that a `Result` is `Ok`.
pub fn assert_ok<T, E: std::fmt::Debug>(result: &Result<T, E>);

/// Assert that a `Result` is `Err`.
pub fn assert_err<T: std::fmt::Debug, E>(result: &Result<T, E>);

/// Assert that an `Option` is `Some`.
pub fn assert_some<T>(opt: &Option<T>);

/// Assert that an `Option` is `None`.
pub fn assert_none<T: std::fmt::Debug>(opt: &Option<T>);

/// Assert that a string contains a substring.
pub fn assert_contains(haystack: &String, needle: &String);

/// Assert the closure panics.
pub fn assert_panics(f: fn());

// ---------------------------------------------------------------------------
// Test attributes (by convention)
// ---------------------------------------------------------------------------

// Use `#[test]` attribute to mark a function as a test.
// Use `#[test] #[ignore]` to mark a test as ignored.
// Use `#[bench]` to mark a function as a benchmark.

// ---------------------------------------------------------------------------
// Benchmarking
// ---------------------------------------------------------------------------

/// Handle passed to benchmark functions.
pub struct Bencher {
    _iters: u64,
}

impl Bencher {
    /// Run the given closure in a benchmarking loop.
    pub fn iter(&mut self, f: fn());

    /// Control the number of iterations manually.
    pub fn iter_custom(&mut self, f: fn(u64));
}

/// Result of a benchmark run.
pub struct BenchResult {
    pub name: String,
    pub iterations: u64,
    pub total_ns: u64,
    pub ns_per_iter: f64,
}

impl BenchResult {
    /// Format the result as a human-readable string.
    pub fn display(&self) -> String {
        std::fmt::format(
            "{}: {} iterations, {:.2} ns/iter",
            &self.name,
            self.iterations,
            self.ns_per_iter,
        )
    }
}

// ---------------------------------------------------------------------------
// Property-based testing
// ---------------------------------------------------------------------------

/// Trait for types that can generate arbitrary values.
pub trait Arbitrary {
    /// Generate a random instance.
    pub fn arbitrary(rng: &mut std::math::Rng) -> Self;

    /// Optionally shrink a failing case to a smaller counterexample.
    pub fn shrink(&self) -> Vec<Self> { Vec::new() }
}

// Standard implementations
impl Arbitrary for i32 {
    pub fn arbitrary(rng: &mut std::math::Rng) -> i32 {
        rng.range_int(-1000, 1000) as i32
    }
}

impl Arbitrary for i64 {
    pub fn arbitrary(rng: &mut std::math::Rng) -> i64 {
        rng.range_int(-100000, 100000)
    }
}

impl Arbitrary for f64 {
    pub fn arbitrary(rng: &mut std::math::Rng) -> f64 {
        rng.range_f64(-1e6, 1e6)
    }
}

impl Arbitrary for bool {
    pub fn arbitrary(rng: &mut std::math::Rng) -> bool {
        rng.next_u64() % 2 == 0
    }
}

impl Arbitrary for String {
    pub fn arbitrary(rng: &mut std::math::Rng) -> String {
        let len = rng.range_int(0, 64) as usize;
        let mut s = String::new();
        for _i in 0..len {
            let c = (rng.range_int(32, 127) as u8) as char;
            s.push(c);
        }
        s
    }
}

impl Arbitrary for Vec<T: Arbitrary> {
    pub fn arbitrary(rng: &mut std::math::Rng) -> Vec<T> {
        let len = rng.range_int(0, 20) as usize;
        let mut v = Vec::new();
        for _i in 0..len {
            v.push(T::arbitrary(rng));
        }
        v
    }
}

/// Run a property test with `num_cases` random inputs.
pub fn prop_test<A: Arbitrary>(
    name: &String,
    num_cases: usize,
    property: fn(A) -> bool,
) -> PropTestResult / rng {
    let mut rng = std::math::Rng::new();
    for i in 0..num_cases {
        let input = A::arbitrary(&mut rng);
        if !property(input) {
            return PropTestResult {
                name: name.to_owned(),
                passed: false,
                cases_run: i + 1,
                counterexample: Some(std::fmt::format("{:?}", &input)),
            };
        }
    }
    PropTestResult {
        name: name.to_owned(),
        passed: true,
        cases_run: num_cases,
        counterexample: None,
    }
}

/// Property test with two arguments.
pub fn prop_test2<A: Arbitrary, B: Arbitrary>(
    name: &String,
    num_cases: usize,
    property: fn(A, B) -> bool,
) -> PropTestResult / rng;

pub struct PropTestResult {
    pub name: String,
    pub passed: bool,
    pub cases_run: usize,
    pub counterexample: Option<String>,
}

// ---------------------------------------------------------------------------
// Test suite runner
// ---------------------------------------------------------------------------

/// Represents a single test case.
pub struct TestCase {
    name: String,
    run: fn(),
    ignored: bool,
}

/// A collection of tests.
pub struct TestSuite {
    cases: Vec<TestCase>,
}

impl TestSuite {
    pub fn new() -> TestSuite { TestSuite { cases: Vec::new() } }

    pub fn add(&mut self, name: &String, run: fn()) {
        self.cases.push(TestCase { name: name.to_owned(), run, ignored: false });
    }

    pub fn add_ignored(&mut self, name: &String, run: fn()) {
        self.cases.push(TestCase { name: name.to_owned(), run, ignored: true });
    }

    /// Run all tests and return results.
    pub fn run(&self) -> TestSuiteResult {
        let mut passed = 0usize;
        let mut failed = 0usize;
        let mut ignored = 0usize;
        let mut failures = Vec::new();

        for tc in &self.cases {
            if tc.ignored {
                ignored += 1;
                continue;
            }
            // In a real runtime, we'd catch panics here.
            (tc.run)();
            passed += 1;
        }

        TestSuiteResult { total: self.cases.len(), passed, failed, ignored, failures }
    }
}

pub struct TestSuiteResult {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
    pub failures: Vec<String>,
}

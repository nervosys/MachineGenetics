//! # std::math — Mathematics
//!
//! Trigonometric, exponential, logarithmic functions, RNG, and SIMD.
//! All functions are pure (no effects).

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const PI:      f64 = 3.14159265358979323846;
pub const TAU:     f64 = 6.28318530717958647692;
pub const E:       f64 = 2.71828182845904523536;
pub const SQRT_2:  f64 = 1.41421356237309504880;
pub const LN_2:    f64 = 0.69314718055994530941;
pub const LN_10:   f64 = 2.30258509299404568401;
pub const INFINITY:     f64 = f64::INFINITY;
pub const NEG_INFINITY: f64 = f64::NEG_INFINITY;
pub const NAN:     f64 = f64::NAN;

// ---------------------------------------------------------------------------
// Trigonometric
// ---------------------------------------------------------------------------

pub fn sin(x: f64) -> f64;
pub fn cos(x: f64) -> f64;
pub fn tan(x: f64) -> f64;
pub fn asin(x: f64) -> f64;
pub fn acos(x: f64) -> f64;
pub fn atan(x: f64) -> f64;
pub fn atan2(y: f64, x: f64) -> f64;
pub fn sinh(x: f64) -> f64;
pub fn cosh(x: f64) -> f64;
pub fn tanh(x: f64) -> f64;

// ---------------------------------------------------------------------------
// Exponential / Logarithmic
// ---------------------------------------------------------------------------

pub fn exp(x: f64) -> f64;
pub fn exp2(x: f64) -> f64;
pub fn ln(x: f64) -> f64;
pub fn log2(x: f64) -> f64;
pub fn log10(x: f64) -> f64;
pub fn log(x: f64, base: f64) -> f64;
pub fn pow(base: f64, exp: f64) -> f64;
pub fn sqrt(x: f64) -> f64;
pub fn cbrt(x: f64) -> f64;
pub fn hypot(x: f64, y: f64) -> f64;

// ---------------------------------------------------------------------------
// Rounding / Absolute
// ---------------------------------------------------------------------------

pub fn abs(x: f64) -> f64;
pub fn floor(x: f64) -> f64;
pub fn ceil(x: f64) -> f64;
pub fn round(x: f64) -> f64;
pub fn trunc(x: f64) -> f64;
pub fn fract(x: f64) -> f64;

/// Clamp a value between a minimum and maximum.
pub fn clamp(x: f64, min: f64, max: f64) -> f64 {
    if x < min { return min; }
    if x > max { return max; }
    x
}

/// The minimum of two values.
pub fn min(a: f64, b: f64) -> f64 { if a < b { a } else { b } }

/// The maximum of two values.
pub fn max(a: f64, b: f64) -> f64 { if a > b { a } else { b } }

// ---------------------------------------------------------------------------
// Integer math
// ---------------------------------------------------------------------------

/// Greatest common divisor.
pub fn gcd(a: i64, b: i64) -> i64 {
    let mut a = abs_i64(a);
    let mut b = abs_i64(b);
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Least common multiple.
pub fn lcm(a: i64, b: i64) -> i64 {
    if a == 0 || b == 0 { return 0; }
    abs_i64(a / gcd(a, b) * b)
}

fn abs_i64(x: i64) -> i64 { if x < 0 { -x } else { x } }

// ---------------------------------------------------------------------------
// Random — declares `rng` effect
// ---------------------------------------------------------------------------

/// A pseudo-random number generator.
pub struct Rng {
    _state: u64,
}

impl Rng {
    /// Create an RNG seeded from the system entropy source.
    pub fn new() -> Rng / rng;

    /// Create an RNG from a specific seed (deterministic).
    pub fn from_seed(seed: u64) -> Rng;

    /// Generate a random u64.
    pub fn next_u64(&mut self) -> u64;

    /// Generate a random f64 in [0.0, 1.0).
    pub fn next_f64(&mut self) -> f64;

    /// Generate a random integer in [low, high).
    pub fn range_int(&mut self, low: i64, high: i64) -> i64;

    /// Generate a random float in [low, high).
    pub fn range_f64(&mut self, low: f64, high: f64) -> f64;

    /// Shuffle a slice in place.
    pub fn shuffle<T>(&mut self, data: &mut [T]);

    /// Pick a random element from a slice.
    pub fn choose<T>(&mut self, data: &[T]) -> Option<&T>;
}

/// Convenience: generate a random u64 with a thread-local RNG.
pub fn random_u64() -> u64 / rng;

/// Convenience: generate a random f64 in [0.0, 1.0).
pub fn random_f64() -> f64 / rng;

// ---------------------------------------------------------------------------
// SIMD (placeholder)
// ---------------------------------------------------------------------------

/// A 128-bit SIMD vector of 4 floats.
pub struct f32x4 { _data: [f32; 4] }

impl f32x4 {
    pub fn splat(v: f32) -> f32x4;
    pub fn from_array(a: [f32; 4]) -> f32x4;
    pub fn to_array(&self) -> [f32; 4];
    pub fn add(&self, other: &f32x4) -> f32x4;
    pub fn sub(&self, other: &f32x4) -> f32x4;
    pub fn mul(&self, other: &f32x4) -> f32x4;
    pub fn div(&self, other: &f32x4) -> f32x4;
    pub fn sum(&self) -> f32;
}

/// A 256-bit SIMD vector of 4 doubles.
pub struct f64x4 { _data: [f64; 4] }

impl f64x4 {
    pub fn splat(v: f64) -> f64x4;
    pub fn from_array(a: [f64; 4]) -> f64x4;
    pub fn to_array(&self) -> [f64; 4];
    pub fn add(&self, other: &f64x4) -> f64x4;
    pub fn mul(&self, other: &f64x4) -> f64x4;
    pub fn sum(&self) -> f64;
}

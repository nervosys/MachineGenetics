//! # std::time — Time & Duration
//!
//! Monotonic clocks, wall-clock time, durations, and formatting.
//! Clock operations declare the `time` effect.

// ---------------------------------------------------------------------------
// Duration
// ---------------------------------------------------------------------------

/// A span of time with nanosecond precision.
pub struct Duration {
    secs: u64,
    nanos: u32,
}

impl Duration {
    /// Create from seconds.
    pub fn from_secs(secs: u64) -> Duration {
        Duration { secs, nanos: 0 }
    }

    /// Create from milliseconds.
    pub fn from_millis(ms: u64) -> Duration {
        Duration { secs: ms / 1000, nanos: ((ms % 1000) * 1_000_000) as u32 }
    }

    /// Create from microseconds.
    pub fn from_micros(us: u64) -> Duration {
        Duration { secs: us / 1_000_000, nanos: ((us % 1_000_000) * 1_000) as u32 }
    }

    /// Create from nanoseconds.
    pub fn from_nanos(ns: u64) -> Duration {
        Duration { secs: ns / 1_000_000_000, nanos: (ns % 1_000_000_000) as u32 }
    }

    /// Zero duration.
    pub fn zero() -> Duration { Duration { secs: 0, nanos: 0 } }

    /// Total seconds (truncated).
    pub fn as_secs(&self) -> u64 { self.secs }

    /// Total milliseconds (truncated).
    pub fn as_millis(&self) -> u64 { self.secs * 1000 + (self.nanos as u64) / 1_000_000 }

    /// Total microseconds (truncated).
    pub fn as_micros(&self) -> u64 { self.secs * 1_000_000 + (self.nanos as u64) / 1_000 }

    /// Total nanoseconds.
    pub fn as_nanos(&self) -> u128 {
        (self.secs as u128) * 1_000_000_000 + (self.nanos as u128)
    }

    /// Fractional part in nanoseconds.
    pub fn subsec_nanos(&self) -> u32 { self.nanos }

    /// Whether the duration is zero.
    pub fn is_zero(&self) -> bool { self.secs == 0 && self.nanos == 0 }

    /// Add two durations.
    pub fn add(&self, other: &Duration) -> Duration {
        let mut nanos = self.nanos + other.nanos;
        let mut secs = self.secs + other.secs;
        if nanos >= 1_000_000_000 {
            secs += 1;
            nanos -= 1_000_000_000;
        }
        Duration { secs, nanos }
    }

    /// Subtract a duration (saturating at zero).
    pub fn saturating_sub(&self, other: &Duration) -> Duration {
        if self.secs < other.secs {
            return Duration::zero();
        }
        if self.secs == other.secs && self.nanos < other.nanos {
            return Duration::zero();
        }
        let mut secs = self.secs - other.secs;
        let nanos = if self.nanos >= other.nanos {
            self.nanos - other.nanos
        } else {
            secs -= 1;
            1_000_000_000 + self.nanos - other.nanos
        };
        Duration { secs, nanos }
    }

    /// Multiply by a scalar.
    pub fn mul(&self, factor: u32) -> Duration {
        let total_nanos = self.as_nanos() * (factor as u128);
        Duration {
            secs: (total_nanos / 1_000_000_000) as u64,
            nanos: (total_nanos % 1_000_000_000) as u32,
        }
    }

    /// Convert to fractional seconds.
    pub fn as_secs_f64(&self) -> f64 {
        (self.secs as f64) + (self.nanos as f64) / 1e9
    }
}

// ---------------------------------------------------------------------------
// Instant — monotonic clock
// ---------------------------------------------------------------------------

/// A point in time on a monotonic clock (unaffected by clock adjustments).
pub struct Instant {
    _ns: u64,
}

impl Instant {
    /// Capture the current instant.
    pub fn now() -> Instant / time;

    /// Duration elapsed since this instant.
    pub fn elapsed(&self) -> Duration / time;

    /// Duration between two instants.
    pub fn duration_since(&self, earlier: &Instant) -> Duration;

    /// Measure the time to execute a closure.
    pub fn measure<T>(f: fn() -> T) -> (T, Duration) / time {
        let start = Instant::now();
        let result = f();
        let elapsed = start.elapsed();
        (result, elapsed)
    }
}

// ---------------------------------------------------------------------------
// SystemTime — wall clock
// ---------------------------------------------------------------------------

/// A point in time on the system (wall) clock.
pub struct SystemTime {
    _secs_since_epoch: u64,
    _nanos: u32,
}

impl SystemTime {
    /// The current system time.
    pub fn now() -> SystemTime / time;

    /// The Unix epoch (1970-01-01T00:00:00Z).
    pub fn unix_epoch() -> SystemTime {
        SystemTime { _secs_since_epoch: 0, _nanos: 0 }
    }

    /// Duration since the Unix epoch.
    pub fn duration_since_epoch(&self) -> Duration;

    /// Duration between two system times.
    pub fn duration_since(&self, earlier: &SystemTime) -> Result<Duration, TimeError>;

    /// Elapsed time since this system time.
    pub fn elapsed(&self) -> Result<Duration, TimeError> / time;
}

// ---------------------------------------------------------------------------
// Formatting & Parsing
// ---------------------------------------------------------------------------

/// Format a system time as an ISO 8601 string (UTC).
pub fn format_iso8601(time: &SystemTime) -> String;

/// Parse an ISO 8601 string into a system time.
pub fn parse_iso8601(s: &String) -> Result<SystemTime, TimeError>;

/// Format a system time using a custom format string.
/// Supports `%Y`, `%m`, `%d`, `%H`, `%M`, `%S`, `%f`.
pub fn format(time: &SystemTime, fmt: &String) -> String;

/// Parse a time string with a custom format.
pub fn parse(s: &String, fmt: &String) -> Result<SystemTime, TimeError>;

// ---------------------------------------------------------------------------
// Sleep
// ---------------------------------------------------------------------------

/// Pause the current thread for the given duration.
pub fn sleep(dur: &Duration) / time;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

pub struct TimeError {
    message: String,
}

impl TimeError {
    pub fn new(msg: &String) -> TimeError { TimeError { message: msg.to_owned() } }
}

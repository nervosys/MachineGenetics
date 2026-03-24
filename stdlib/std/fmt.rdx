//! # std::fmt — Formatting
//!
//! Display and debug formatting, print macros, and format engine.

// ---------------------------------------------------------------------------
// Core formatting traits
// ---------------------------------------------------------------------------

/// Human-readable formatting (the default for `format!("...")` interpolation).
pub trait Display {
    pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>;
}

/// Debug formatting (used by `{:?}` in format strings).
pub trait Debug {
    pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>;
}

/// Binary formatting (`{:b}`).
pub trait Binary {
    pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>;
}

/// Hex formatting, lowercase (`{:x}`).
pub trait LowerHex {
    pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>;
}

/// Hex formatting, uppercase (`{:X}`).
pub trait UpperHex {
    pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>;
}

// ---------------------------------------------------------------------------
// Formatter
// ---------------------------------------------------------------------------

/// Controls how values are formatted into strings.
pub struct Formatter {
    _buf: &mut String,
    width: Option<usize>,
    precision: Option<usize>,
    fill: char,
    align: Alignment,
}

pub enum Alignment {
    Left,
    Right,
    Center,
}

impl Formatter {
    /// Write a string slice to the output buffer.
    pub fn write_str(&mut self, s: &String) -> Result<(), FmtError>;

    /// Write a single character.
    pub fn write_char(&mut self, c: char) -> Result<(), FmtError>;
}

pub struct FmtError { _priv: () }

// ---------------------------------------------------------------------------
// Format function
// ---------------------------------------------------------------------------

/// Format a value using its `Display` implementation.
/// This is the runtime backing for `format!("...")` format strings.
pub fn format<T: Display>(value: &T) -> String;

/// Format a value using its `Debug` implementation.
pub fn debug<T: Debug>(value: &T) -> String;

// ---------------------------------------------------------------------------
// Print functions — declare `io` effect
// ---------------------------------------------------------------------------

/// Print a formatted string to stdout (no newline).
pub fn print(msg: &String) / io;

/// Print a formatted string to stdout with a newline.
pub fn println(msg: &String) / io;

/// Print a formatted string to stderr (no newline).
pub fn eprint(msg: &String) / io;

/// Print a formatted string to stderr with a newline.
pub fn eprintln(msg: &String) / io;

// ---------------------------------------------------------------------------
// Standard Display implementations
// ---------------------------------------------------------------------------

impl Display for String { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Display for i32    { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Display for i64    { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Display for u32    { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Display for u64    { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Display for usize  { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Display for f32    { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Display for f64    { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Display for bool   { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Display for char   { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }

impl Debug for String   { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Debug for i32      { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Debug for i64      { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Debug for u32      { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Debug for u64      { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Debug for usize    { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Debug for f64      { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }
impl Debug for bool     { pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }

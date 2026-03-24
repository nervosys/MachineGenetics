//! # std::str — Strings & Regular Expressions
//!
//! String manipulation, searching, splitting, encoding, and regex.

// ---------------------------------------------------------------------------
// Core string methods (impl String)
// ---------------------------------------------------------------------------

impl String {
    /// Length in bytes.
    pub fn len(&self) -> usize;

    /// Length in Unicode scalar values (chars).
    pub fn char_count(&self) -> usize;

    /// Whether the string is empty.
    pub fn is_empty(&self) -> bool { self.len() == 0 }

    /// Returns a byte slice.
    pub fn as_bytes(&self) -> &[u8];

    /// Create from UTF-8 bytes.
    pub fn from_utf8(bytes: &[u8]) -> Result<String, StrError>;

    // -- Searching --

    /// Whether the string contains a substring.
    pub fn contains(&self, needle: &String) -> bool;

    /// Whether the string starts with a prefix.
    pub fn starts_with(&self, prefix: &String) -> bool;

    /// Whether the string ends with a suffix.
    pub fn ends_with(&self, suffix: &String) -> bool;

    /// Find the byte index of the first occurrence.
    pub fn find(&self, needle: &String) -> Option<usize>;

    /// Find the byte index of the last occurrence.
    pub fn rfind(&self, needle: &String) -> Option<usize>;

    // -- Slicing / Splitting --

    /// Substring by byte range.
    pub fn slice(&self, start: usize, end: usize) -> String;

    /// Split by a separator, returning all parts.
    pub fn split(&self, sep: &String) -> Vec<String>;

    /// Split into at most `n` parts.
    pub fn splitn(&self, n: usize, sep: &String) -> Vec<String>;

    /// Split into lines.
    pub fn lines(&self) -> Vec<String>;

    // -- Trimming --

    /// Trim whitespace from both ends.
    pub fn trim(&self) -> String;

    /// Trim whitespace from the start.
    pub fn trim_start(&self) -> String;

    /// Trim whitespace from the end.
    pub fn trim_end(&self) -> String;

    // -- Case --

    pub fn to_lowercase(&self) -> String;
    pub fn to_uppercase(&self) -> String;

    // -- Modification --

    /// Replace all occurrences of `from` with `to`.
    pub fn replace(&self, from: &String, to: &String) -> String;

    /// Replace the first `n` occurrences.
    pub fn replacen(&self, from: &String, to: &String, n: usize) -> String;

    /// Repeat the string `n` times.
    pub fn repeat(&self, n: usize) -> String;

    /// Reverse the string (by chars, not bytes).
    pub fn reverse(&self) -> String;

    // -- Conversion --

    /// Parse the string into a numeric type.
    pub fn parse_int(&self) -> Result<i64, StrError>;
    pub fn parse_float(&self) -> Result<f64, StrError>;

    /// Convert to owned (clone).
    pub fn to_owned(&self) -> String;

    // -- Joining --

    /// Join a list of strings with this string as separator.
    pub fn join(sep: &String, parts: &[String]) -> String;

    // -- Chars iterator --

    /// Iterate over Unicode scalar values.
    pub fn chars(&self) -> Chars;
}

/// Iterator over characters in a string.
pub struct Chars {
    _inner: String,
    _pos: usize,
}

impl Iterator for Chars {
    type Item = char;
    pub fn next(&mut self) -> Option<char>;
}

// ---------------------------------------------------------------------------
// Regular Expressions
// ---------------------------------------------------------------------------

/// A compiled regular expression.
pub struct Regex {
    _pattern: String,
}

impl Regex {
    /// Compile a regex pattern. Returns an error for invalid patterns.
    pub fn new(pattern: &String) -> Result<Regex, StrError>;

    /// Test whether the pattern matches anywhere in the text.
    pub fn is_match(&self, text: &String) -> bool;

    /// Find the first match.
    pub fn find(&self, text: &String) -> Option<Match>;

    /// Find all non-overlapping matches.
    pub fn find_all(&self, text: &String) -> Vec<Match>;

    /// Capture groups for the first match.
    pub fn captures(&self, text: &String) -> Option<Captures>;

    /// Replace the first match.
    pub fn replace(&self, text: &String, replacement: &String) -> String;

    /// Replace all matches.
    pub fn replace_all(&self, text: &String, replacement: &String) -> String;

    /// Split text by the pattern.
    pub fn split(&self, text: &String) -> Vec<String>;
}

/// A single regex match.
pub struct Match {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// Capture groups from a regex match.
pub struct Captures {
    groups: Vec<Option<Match>>,
}

impl Captures {
    /// Get a capture group by index (0 = full match).
    pub fn get(&self, idx: usize) -> Option<&Match> {
        match self.groups.get(idx) {
            Some(m) => m.as_ref(),
            None => None,
        }
    }

    /// Number of capture groups (including group 0).
    pub fn len(&self) -> usize { self.groups.len() }
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

/// Base64 encode a byte slice to a string.
pub fn base64_encode(data: &[u8]) -> String;

/// Base64 decode a string to bytes.
pub fn base64_decode(s: &String) -> Result<Vec<u8>, StrError>;

/// Hex-encode a byte slice.
pub fn hex_encode(data: &[u8]) -> String;

/// Hex-decode a string to bytes.
pub fn hex_decode(s: &String) -> Result<Vec<u8>, StrError>;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

pub struct StrError {
    message: String,
}

impl StrError {
    pub fn new(msg: &String) -> StrError { StrError { message: msg.to_owned() } }
}

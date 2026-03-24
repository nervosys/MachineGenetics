//! # std::io — Input/Output
//!
//! Core I/O traits, buffered wrappers, and standard streams.
//! All I/O functions declare the `io` effect.

// ---------------------------------------------------------------------------
// Core traits
// ---------------------------------------------------------------------------

/// A type that can be read from.
pub trait Read {
    /// Read bytes into a buffer. Returns number of bytes read.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> / io;

    /// Read all remaining bytes into a growable buffer.
    pub fn read_all(&mut self, buf: &mut Vec<u8>) -> Result<usize, IoError> / io {
        let total = 0usize;
        let chunk: [u8; 4096];
        loop {
            let n = self.read(&mut chunk)?;
            if n == 0 { return total; }
            buf.extend(&chunk[..n]);
            total += n;
        }
    }

    /// Read exactly `buf.len()` bytes or error.
    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), IoError> / io;
}

/// A type that can be written to.
pub trait Write {
    /// Write bytes from a buffer. Returns number of bytes written.
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> / io;

    /// Write all bytes from a buffer.
    pub fn write_all(&mut self, buf: &[u8]) -> Result<(), IoError> / io {
        let remaining = buf;
        while remaining.len() > 0 {
            let n = self.write(remaining)?;
            remaining = &remaining[n..];
        }
        Result::ok(())
    }

    /// Flush buffered data.
    pub fn flush(&mut self) -> Result<(), IoError> / io;
}

/// A type that supports seeking.
pub trait Seek {
    /// Seek to a position. Returns the new position.
    pub fn seek(&mut self, pos: SeekFrom) -> Result<u64, IoError> / io;
}

// ---------------------------------------------------------------------------
// Seek positions
// ---------------------------------------------------------------------------

/// Position argument for `Seek::seek()`.
pub enum SeekFrom {
    /// Seek from the beginning of the stream.
    Start(u64),
    /// Seek from the end of the stream.
    End(i64),
    /// Seek from the current position.
    Current(i64),
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// I/O error type.
pub struct IoError {
    kind: IoErrorKind,
    message: String,
}

pub enum IoErrorKind {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    BrokenPipe,
    TimedOut,
    Interrupted,
    UnexpectedEof,
    Other,
}

impl IoError {
    pub fn new(kind: IoErrorKind, msg: String) -> IoError {
        IoError { kind, message: msg }
    }
}

// ---------------------------------------------------------------------------
// Buffered wrappers
// ---------------------------------------------------------------------------

/// Wraps a `Read` with an internal buffer.
pub struct BufReader<R: Read> {
    inner: R,
    buf: Vec<u8>,
    pos: usize,
    cap: usize,
}

impl BufReader<R: Read> {
    pub fn new(inner: R) -> BufReader<R> {
        BufReader::with_capacity(8192, inner)
    }

    pub fn with_capacity(cap: usize, inner: R) -> BufReader<R> {
        BufReader {
            inner,
            buf: Vec::<u8>::with_capacity(cap),
            pos: 0,
            cap: 0,
        }
    }

    /// Read a line into the given string. Returns bytes read.
    pub fn read_line(&mut self, line: &mut String) -> Result<usize, IoError> / io;

    /// Return an iterator over lines.
    pub fn lines(&mut self) -> Lines<R> / io;
}

/// Wraps a `Write` with an internal buffer.
pub struct BufWriter<W: Write> {
    inner: W,
    buf: Vec<u8>,
}

impl BufWriter<W: Write> {
    pub fn new(inner: W) -> BufWriter<W> {
        BufWriter::with_capacity(8192, inner)
    }

    pub fn with_capacity(cap: usize, inner: W) -> BufWriter<W> {
        BufWriter {
            inner,
            buf: Vec::<u8>::with_capacity(cap),
        }
    }
}

// ---------------------------------------------------------------------------
// File
// ---------------------------------------------------------------------------

/// A handle to an open file on the filesystem.
pub struct File {
    _fd: u64,
}

impl File {
    /// Open a file in read-only mode.
    pub fn open(path: &String) -> Result<File, IoError> / io;

    /// Create a new file or truncate an existing one.
    pub fn create(path: &String) -> Result<File, IoError> / io;

    /// Open a file with custom options.
    pub fn options() -> OpenOptions;
}

impl Read for File {
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> / io;
    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), IoError> / io;
}

impl Write for File {
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> / io;
    pub fn flush(&mut self) -> Result<(), IoError> / io;
}

impl Seek for File {
    pub fn seek(&mut self, pos: SeekFrom) -> Result<u64, IoError> / io;
}

/// Builder for opening files with specific options.
pub struct OpenOptions {
    read: bool,
    write: bool,
    append: bool,
    create: bool,
    truncate: bool,
}

impl OpenOptions {
    pub fn new() -> OpenOptions {
        OpenOptions {
            read: false,
            write: false,
            append: false,
            create: false,
            truncate: false,
        }
    }

    pub fn read(&mut self, yes: bool) -> &mut OpenOptions   { self.read = yes; self }
    pub fn write(&mut self, yes: bool) -> &mut OpenOptions   { self.write = yes; self }
    pub fn append(&mut self, yes: bool) -> &mut OpenOptions  { self.append = yes; self }
    pub fn create(&mut self, yes: bool) -> &mut OpenOptions  { self.create = yes; self }
    pub fn truncate(&mut self, yes: bool) -> &mut OpenOptions { self.truncate = yes; self }

    pub fn open(&self, path: &String) -> Result<File, IoError> / io;
}

// ---------------------------------------------------------------------------
// Standard streams
// ---------------------------------------------------------------------------

/// Returns a handle to standard input.
pub fn stdin() -> Stdin / io;

/// Returns a handle to standard output.
pub fn stdout() -> Stdout / io;

/// Returns a handle to standard error.
pub fn stderr() -> Stderr / io;

pub struct Stdin { _priv: () }
pub struct Stdout { _priv: () }
pub struct Stderr { _priv: () }

impl Read for Stdin {
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> / io;
    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), IoError> / io;
}

impl Write for Stdout {
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> / io;
    pub fn flush(&mut self) -> Result<(), IoError> / io;
}

impl Write for Stderr {
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> / io;
    pub fn flush(&mut self) -> Result<(), IoError> / io;
}

// ---------------------------------------------------------------------------
// Iterator adapters
// ---------------------------------------------------------------------------

/// Iterator over lines from a buffered reader.
pub struct Lines<R: Read> {
    reader: BufReader<R>,
}

impl Iterator for Lines<R: Read> {
    type Item = Result<String, IoError>;
    pub fn next(&mut self) -> Option<Result<String, IoError>> / io;
}

//! # std::fs — File System
//!
//! File system operations: read, write, create, remove, metadata, walk.
//! All functions declare the `io` effect.

use std::io::{IoError, IoErrorKind};
use std::time::SystemTime;

// ---------------------------------------------------------------------------
// Convenience functions
// ---------------------------------------------------------------------------

/// Read the entire contents of a file into a string.
pub fn read(path: &String) -> Result<String, IoError> / io;

/// Read the entire contents of a file into a byte vector.
pub fn read_bytes(path: &String) -> Result<Vec<u8>, IoError> / io;

/// Write a string to a file, creating it if needed, truncating if exists.
pub fn write(path: &String, contents: &String) -> Result<(), IoError> / io;

/// Write bytes to a file.
pub fn write_bytes(path: &String, contents: &[u8]) -> Result<(), IoError> / io;

/// Append a string to a file.
pub fn append(path: &String, contents: &String) -> Result<(), IoError> / io;

// ---------------------------------------------------------------------------
// Directory operations
// ---------------------------------------------------------------------------

/// Create a directory. Fails if it already exists.
pub fn create_dir(path: &String) -> Result<(), IoError> / io;

/// Create a directory and all parent directories.
pub fn create_dir_all(path: &String) -> Result<(), IoError> / io;

/// Remove an empty directory.
pub fn remove_dir(path: &String) -> Result<(), IoError> / io;

/// Remove a directory and all its contents.
pub fn remove_dir_all(path: &String) -> Result<(), IoError> / io;

/// Read directory entries.
pub fn read_dir(path: &String) -> Result<Vec<DirEntry>, IoError> / io;

// ---------------------------------------------------------------------------
// File operations
// ---------------------------------------------------------------------------

/// Remove a file.
pub fn remove(path: &String) -> Result<(), IoError> / io;

/// Rename / move a file or directory.
pub fn rename(from: &String, to: &String) -> Result<(), IoError> / io;

/// Copy a file. Returns the number of bytes copied.
pub fn copy(from: &String, to: &String) -> Result<u64, IoError> / io;

/// Test whether a path exists.
pub fn exists(path: &String) -> bool / io;

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

/// Query metadata about a file (follows symlinks).
pub fn metadata(path: &String) -> Result<Metadata, IoError> / io;

/// Query metadata about a file (does not follow symlinks).
pub fn symlink_metadata(path: &String) -> Result<Metadata, IoError> / io;

/// File metadata.
pub struct Metadata {
    file_type: FileType,
    len: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    permissions: Permissions,
}

impl Metadata {
    pub fn is_file(&self) -> bool { self.file_type == FileType::File }
    pub fn is_dir(&self) -> bool  { self.file_type == FileType::Dir }
    pub fn is_symlink(&self) -> bool { self.file_type == FileType::Symlink }
}

pub enum FileType {
    File,
    Dir,
    Symlink,
}

pub struct Permissions {
    readonly: bool,
    mode: Option<u32>,   // Unix mode bits
}

// ---------------------------------------------------------------------------
// Directory entries
// ---------------------------------------------------------------------------

/// A single entry inside a directory.
pub struct DirEntry {
    name: String,
    path: String,
    file_type: FileType,
}

impl DirEntry {
    pub fn name(&self) -> &String { &self.name }
    pub fn path(&self) -> &String { &self.path }
    pub fn metadata(&self) -> Result<Metadata, IoError> / io;
}

// ---------------------------------------------------------------------------
// Directory walking
// ---------------------------------------------------------------------------

/// Recursively walk a directory tree.
pub fn walk(path: &String) -> Result<Walker, IoError> / io;

/// Recursive directory walker.
pub struct Walker {
    stack: Vec<String>,
}

impl Iterator for Walker {
    type Item = Result<DirEntry, IoError>;
    pub fn next(&mut self) -> Option<Result<DirEntry, IoError>> / io;
}

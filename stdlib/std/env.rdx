//! # std::env — Environment
//!
//! Access to command-line arguments, environment variables, and directories.
//! All functions declare the `env` effect since they read process state.

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

/// Get command-line arguments as a vector of strings.
pub fn args() -> Vec<String> / env;

/// Get the number of command-line arguments.
pub fn args_count() -> usize / env;

// ---------------------------------------------------------------------------
// Environment variables
// ---------------------------------------------------------------------------

/// Get the value of an environment variable.
pub fn var(key: &String) -> Result<String, EnvError> / env;

/// Get the value of an environment variable, or `None` if not set.
pub fn var_opt(key: &String) -> Option<String> / env;

/// Set an environment variable.
pub fn set_var(key: &String, value: &String) / env;

/// Remove an environment variable.
pub fn remove_var(key: &String) / env;

/// Iterator over all environment variables as `(key, value)` pairs.
pub fn vars() -> Vec<(String, String)> / env;

// ---------------------------------------------------------------------------
// Directories
// ---------------------------------------------------------------------------

/// Get the current working directory.
pub fn current_dir() -> Result<String, EnvError> / env;

/// Change the current working directory.
pub fn set_current_dir(path: &String) -> Result<(), EnvError> / env;

/// Get the user's home directory.
pub fn home_dir() -> Option<String> / env;

/// Get the system temporary directory.
pub fn temp_dir() -> String / env;

/// Get the path of the current executable.
pub fn current_exe() -> Result<String, EnvError> / env;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

pub struct EnvError {
    kind: EnvErrorKind,
    message: String,
}

pub enum EnvErrorKind {
    NotPresent,
    NotUnicode,
    PermissionDenied,
    Other,
}

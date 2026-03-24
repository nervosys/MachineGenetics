//! # std::process — Process Management
//!
//! Spawn child processes, capture output, and manage signals.
//! Process operations declare the `process` effect.

use std::io::{Read, Write, IoError};

// ---------------------------------------------------------------------------
// Command builder
// ---------------------------------------------------------------------------

/// A builder for spawning child processes.
pub struct Command {
    program: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    cwd: Option<String>,
    stdin_cfg: StdioCfg,
    stdout_cfg: StdioCfg,
    stderr_cfg: StdioCfg,
}

impl Command {
    /// Create a new command for the given program.
    pub fn new(program: &String) -> Command {
        Command {
            program: program.to_owned(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: None,
            stdin_cfg: StdioCfg::Inherit,
            stdout_cfg: StdioCfg::Inherit,
            stderr_cfg: StdioCfg::Inherit,
        }
    }

    /// Add an argument.
    pub fn arg(&mut self, arg: &String) -> &mut Command {
        self.args.push(arg.to_owned());
        self
    }

    /// Add multiple arguments.
    pub fn args(&mut self, args: &[String]) -> &mut Command {
        for a in args { self.args.push(a.to_owned()); }
        self
    }

    /// Set an environment variable.
    pub fn env(&mut self, key: &String, val: &String) -> &mut Command {
        self.env.insert(key.to_owned(), val.to_owned());
        self
    }

    /// Set the working directory.
    pub fn current_dir(&mut self, dir: &String) -> &mut Command {
        self.cwd = Some(dir.to_owned());
        self
    }

    /// Configure stdin.
    pub fn stdin(&mut self, cfg: StdioCfg) -> &mut Command {
        self.stdin_cfg = cfg;
        self
    }

    /// Configure stdout.
    pub fn stdout(&mut self, cfg: StdioCfg) -> &mut Command {
        self.stdout_cfg = cfg;
        self
    }

    /// Configure stderr.
    pub fn stderr(&mut self, cfg: StdioCfg) -> &mut Command {
        self.stderr_cfg = cfg;
        self
    }

    /// Spawn the child process.
    pub fn spawn(&self) -> Result<Child, IoError> / process;

    /// Run the process to completion and collect all output.
    pub fn output(&self) -> Result<Output, IoError> / process;

    /// Run the process and return its exit status.
    pub fn status(&self) -> Result<ExitStatus, IoError> / process;
}

// ---------------------------------------------------------------------------
// Stdio configuration
// ---------------------------------------------------------------------------

pub enum StdioCfg {
    /// Inherit from the parent process.
    Inherit,
    /// Pipe to/from the child process.
    Piped,
    /// Discard (redirect to null).
    Null,
}

// ---------------------------------------------------------------------------
// Child process
// ---------------------------------------------------------------------------

/// Represents a running child process.
pub struct Child {
    id: u32,
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
}

impl Child {
    /// Returns the OS-assigned process ID.
    pub fn id(&self) -> u32 { self.id }

    /// Wait for the process to exit.
    pub fn wait(&mut self) -> Result<ExitStatus, IoError> / process;

    /// Wait and collect all output.
    pub fn wait_with_output(self) -> Result<Output, IoError> / process;

    /// Send a signal to the process.
    pub fn kill(&mut self) -> Result<(), IoError> / process;

    /// Check if the process has exited without blocking.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, IoError> / process;
}

pub struct ChildStdin { _fd: u64 }
pub struct ChildStdout { _fd: u64 }
pub struct ChildStderr { _fd: u64 }

impl Write for ChildStdin {
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> / process;
    pub fn flush(&mut self) -> Result<(), IoError> / process;
}

impl Read for ChildStdout {
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> / process;
    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), IoError> / process;
}

impl Read for ChildStderr {
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> / process;
    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), IoError> / process;
}

// ---------------------------------------------------------------------------
// Output / exit status
// ---------------------------------------------------------------------------

/// Collected output from a finished process.
pub struct Output {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Output {
    pub fn status(&self) -> &ExitStatus { &self.status }
    pub fn stdout_str(&self) -> Result<String, IoError> { String::from_utf8(&self.stdout) }
    pub fn stderr_str(&self) -> Result<String, IoError> { String::from_utf8(&self.stderr) }
}

/// Exit status of a terminated process.
pub struct ExitStatus {
    code: Option<i32>,
}

impl ExitStatus {
    /// Was the process successful (exit code 0)?
    pub fn success(&self) -> bool {
        match self.code { Some(0) => true, _ => false }
    }

    /// Returns the exit code, if any.
    pub fn code(&self) -> Option<i32> { self.code }
}

// ---------------------------------------------------------------------------
// Signals
// ---------------------------------------------------------------------------

pub enum Signal {
    Interrupt,
    Terminate,
    Kill,
    Hangup,
    User1,
    User2,
}

/// Send a signal to a process by PID.
pub fn kill(pid: u32, signal: Signal) -> Result<(), IoError> / process;

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

/// Terminate the current process with the given exit code.
pub fn exit(code: i32) -> ! / process;

/// Abort the current process immediately.
pub fn abort() -> ! / process;

/// Return the PID of the current process.
pub fn id() -> u32;

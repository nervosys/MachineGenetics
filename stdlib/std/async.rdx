//! # std::async — Async Runtime
//!
//! Async task spawning, joining, selection, and streaming.
//! Async functions declare the `async` effect.

use std::time::Duration;

// ---------------------------------------------------------------------------
// Core async operations
// ---------------------------------------------------------------------------

/// Spawn an async task. Returns a handle that can be awaited.
pub fn spawn<T: Send>(task: async fn() -> T) -> JoinHandle<T> / async;

/// Join multiple handles, waiting for all to complete.
pub fn join<T>(handles: Vec<JoinHandle<T>>) -> Vec<Result<T, JoinError>> / async;

/// Wait for the first of several futures to complete.
pub fn select<T>(futures: Vec<Future<T>>) -> (usize, T) / async;

/// Race multiple futures; return the first to resolve, cancel the rest.
pub fn race<T>(futures: Vec<Future<T>>) -> T / async;

/// Sleep for the given duration.
pub fn sleep(dur: Duration) / async;

/// Run a future with a timeout. Returns `Err` if the timeout expires.
pub fn timeout<T>(dur: Duration, fut: Future<T>) -> Result<T, TimeoutError> / async;

/// Run an async closure on the current thread, blocking until complete.
pub fn block_on<T>(fut: Future<T>) -> T;

// ---------------------------------------------------------------------------
// JoinHandle
// ---------------------------------------------------------------------------

/// A handle to a spawned async task.
pub struct JoinHandle<T> {
    _id: u64,
}

impl JoinHandle<T> {
    /// Wait for the task to complete and get its result.
    pub fn await(&self) -> Result<T, JoinError> / async;

    /// Cancel the task.
    pub fn cancel(&self);

    /// Check if the task has completed.
    pub fn is_finished(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Future — the core async trait
// ---------------------------------------------------------------------------

/// A value that will be available in the future.
pub trait Future<T> {
    /// Poll the future. Returns `Ready(T)` or `Pending`.
    pub fn poll(&mut self, cx: &mut Context) -> Poll<T>;
}

pub enum Poll<T> {
    Ready(T),
    Pending,
}

pub struct Context {
    _waker: Waker,
}

pub struct Waker {
    _data: Arc<_WakerVtable>,
}

impl Waker {
    pub fn wake(&self);
    pub fn clone(&self) -> Waker;
}

// ---------------------------------------------------------------------------
// Stream — async iterator
// ---------------------------------------------------------------------------

/// An asynchronous iterator that yields values over time.
pub trait Stream<T> {
    /// Poll for the next item. Returns `Ready(Some(T))`, `Ready(None)`, or `Pending`.
    pub fn poll_next(&mut self, cx: &mut Context) -> Poll<Option<T>>;
}

/// Extension methods for streams.
pub trait StreamExt<T>: Stream<T> {
    /// Collect all items into a vector.
    pub fn collect(&mut self) -> Vec<T> / async;

    /// Map each item to a new value.
    pub fn map<U>(&self, f: fn(T) -> U) -> MapStream<T, U>;

    /// Filter items by a predicate.
    pub fn filter(&self, pred: fn(&T) -> bool) -> FilterStream<T>;

    /// Take at most `n` items.
    pub fn take(&self, n: usize) -> TakeStream<T>;

    /// Apply an async function to each item.
    pub fn for_each(&mut self, f: async fn(T)) / async;
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

pub struct JoinError    { message: String }
pub struct TimeoutError { _priv: () }

// ---------------------------------------------------------------------------
// Placeholder stream adapter types
// ---------------------------------------------------------------------------

pub struct MapStream<T, U>    { _phantom: () }
pub struct FilterStream<T>    { _phantom: () }
pub struct TakeStream<T>      { _phantom: () }

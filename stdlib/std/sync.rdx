//! # std::sync — Synchronization Primitives
//!
//! Mutexes, read-write locks, channels, barriers, semaphores, and atomics.
//! All blocking operations declare the `sync` effect.

// ---------------------------------------------------------------------------
// Mutex
// ---------------------------------------------------------------------------

/// A mutual exclusion lock.
pub struct Mutex<T> {
    _data: T,
}

impl Mutex<T> {
    /// Create a new unlocked mutex.
    pub fn new(value: T) -> Mutex<T>;

    /// Acquire the lock, blocking until available.
    pub fn lock(&self) -> Result<MutexGuard<T>, SyncError> / sync;

    /// Try to acquire the lock without blocking.
    pub fn try_lock(&self) -> Result<MutexGuard<T>, SyncError>;

    /// Check if the lock is poisoned.
    pub fn is_poisoned(&self) -> bool;
}

/// RAII guard for `Mutex`.
pub struct MutexGuard<T> {
    _lock: &Mutex<T>,
}

impl MutexGuard<T> {
    /// Access the protected data.
    pub fn get(&self) -> &T;

    /// Mutably access the protected data.
    pub fn get_mut(&mut self) -> &mut T;
}

// ---------------------------------------------------------------------------
// Read-Write Lock
// ---------------------------------------------------------------------------

/// A reader-writer lock: multiple readers OR one writer.
pub struct RwLock<T> {
    _data: T,
}

impl RwLock<T> {
    pub fn new(value: T) -> RwLock<T>;

    /// Acquire a read lock.
    pub fn read(&self) -> Result<RwLockReadGuard<T>, SyncError> / sync;

    /// Acquire a write lock.
    pub fn write(&self) -> Result<RwLockWriteGuard<T>, SyncError> / sync;

    /// Try to acquire a read lock without blocking.
    pub fn try_read(&self) -> Result<RwLockReadGuard<T>, SyncError>;

    /// Try to acquire a write lock without blocking.
    pub fn try_write(&self) -> Result<RwLockWriteGuard<T>, SyncError>;
}

pub struct RwLockReadGuard<T> { _lock: &RwLock<T> }
pub struct RwLockWriteGuard<T> { _lock: &RwLock<T> }

impl RwLockReadGuard<T> {
    pub fn get(&self) -> &T;
}

impl RwLockWriteGuard<T> {
    pub fn get(&self) -> &T;
    pub fn get_mut(&mut self) -> &mut T;
}

// ---------------------------------------------------------------------------
// Channel
// ---------------------------------------------------------------------------

/// Create a bounded channel with the given capacity.
pub fn channel<T>(capacity: usize) -> (Sender<T>, Receiver<T>);

/// Create an unbounded channel.
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>);

/// The sending half of a channel.
pub struct Sender<T> {
    _inner: (),
}

impl Sender<T> {
    /// Send a value. Blocks if the channel is full (bounded).
    pub fn send(&self, value: T) -> Result<(), SyncError> / sync;

    /// Try to send without blocking.
    pub fn try_send(&self, value: T) -> Result<(), SyncError>;

    /// Clone the sender (multiple-producer support).
    pub fn clone(&self) -> Sender<T>;
}

/// The receiving half of a channel.
pub struct Receiver<T> {
    _inner: (),
}

impl Receiver<T> {
    /// Receive a value. Blocks if empty.
    pub fn recv(&self) -> Result<T, SyncError> / sync;

    /// Try to receive without blocking.
    pub fn try_recv(&self) -> Result<T, SyncError>;

    /// Receive with a timeout.
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Result<T, SyncError> / sync;

    /// Iterate over received values until the channel closes.
    pub fn iter(&self) -> RecvIter<T>;
}

/// Iterator over values from a `Receiver`.
pub struct RecvIter<T> {
    _rx: &Receiver<T>,
}

impl Iterator for RecvIter<T> {
    type Item = T;
    pub fn next(&mut self) -> Option<T> / sync;
}

// ---------------------------------------------------------------------------
// Barrier
// ---------------------------------------------------------------------------

/// A synchronization barrier for N threads.
pub struct Barrier {
    _count: usize,
}

impl Barrier {
    pub fn new(count: usize) -> Barrier;

    /// Wait until all threads reach the barrier.
    pub fn wait(&self) -> BarrierWaitResult / sync;
}

pub struct BarrierWaitResult {
    is_leader: bool,
}

impl BarrierWaitResult {
    /// Returns true for exactly one thread (the "leader").
    pub fn is_leader(&self) -> bool { self.is_leader }
}

// ---------------------------------------------------------------------------
// Semaphore
// ---------------------------------------------------------------------------

/// A counting semaphore.
pub struct Semaphore {
    _permits: usize,
}

impl Semaphore {
    pub fn new(permits: usize) -> Semaphore;

    /// Acquire a permit, blocking until one is available.
    pub fn acquire(&self) -> Result<SemaphorePermit, SyncError> / sync;

    /// Try to acquire without blocking.
    pub fn try_acquire(&self) -> Result<SemaphorePermit, SyncError>;

    /// How many permits are currently available.
    pub fn available(&self) -> usize;
}

pub struct SemaphorePermit {
    _sem: &Semaphore,
}

// ---------------------------------------------------------------------------
// Atomics
// ---------------------------------------------------------------------------

pub enum Ordering {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

/// An atomic boolean.
pub struct AtomicBool { _val: bool }

impl AtomicBool {
    pub fn new(v: bool) -> AtomicBool;
    pub fn load(&self, order: Ordering) -> bool;
    pub fn store(&self, val: bool, order: Ordering);
    pub fn swap(&self, val: bool, order: Ordering) -> bool;
    pub fn compare_exchange(
        &self,
        current: bool,
        new: bool,
        success: Ordering,
        failure: Ordering,
    ) -> Result<bool, bool>;
}

/// An atomic 64-bit integer.
pub struct AtomicI64 { _val: i64 }

impl AtomicI64 {
    pub fn new(v: i64) -> AtomicI64;
    pub fn load(&self, order: Ordering) -> i64;
    pub fn store(&self, val: i64, order: Ordering);
    pub fn fetch_add(&self, val: i64, order: Ordering) -> i64;
    pub fn fetch_sub(&self, val: i64, order: Ordering) -> i64;
    pub fn swap(&self, val: i64, order: Ordering) -> i64;
    pub fn compare_exchange(
        &self,
        current: i64,
        new: i64,
        success: Ordering,
        failure: Ordering,
    ) -> Result<i64, i64>;
}

/// An atomic unsigned 64-bit integer.
pub struct AtomicU64 { _val: u64 }

impl AtomicU64 {
    pub fn new(v: u64) -> AtomicU64;
    pub fn load(&self, order: Ordering) -> u64;
    pub fn store(&self, val: u64, order: Ordering);
    pub fn fetch_add(&self, val: u64, order: Ordering) -> u64;
    pub fn fetch_sub(&self, val: u64, order: Ordering) -> u64;
    pub fn swap(&self, val: u64, order: Ordering) -> u64;
    pub fn compare_exchange(
        &self,
        current: u64,
        new: u64,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u64, u64>;
}

// ---------------------------------------------------------------------------
// Once
// ---------------------------------------------------------------------------

/// A synchronization primitive for one-time initialization.
pub struct Once {
    _done: AtomicBool,
}

impl Once {
    pub fn new() -> Once;

    /// Call the given function exactly once, blocking other callers.
    pub fn call_once(&self, f: fn()) / sync;

    /// Whether `call_once` has completed.
    pub fn is_completed(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

pub struct SyncError {
    message: String,
}

impl SyncError {
    pub fn new(msg: &String) -> SyncError { SyncError { message: msg.to_owned() } }
}

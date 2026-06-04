//! Memory Pool & Zero-Copy Tensor Operations
//!
//! High-performance memory management for tensor computations:
//!
//! - **MemoryPool**: Arena-based allocation with slab management
//! - **TensorBuffer**: Zero-copy tensor views with reference counting
//! - **PoolStats**: Memory pool telemetry

use std::alloc::{self, Layout};
use std::collections::HashMap;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::error::{Result, RmiError};

// ============================================================================
// Memory Pool
// ============================================================================

/// Size class for slab allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SizeClass {
    /// Allocation size in bytes
    pub size: usize,
    /// Alignment requirement
    pub align: usize,
}

impl SizeClass {
    /// Round up to the nearest power of two.
    pub fn power_of_two(size: usize) -> Self {
        let size = size.next_power_of_two().max(64);
        Self { size, align: 64 }
    }

    /// Create from a tensor shape and element size.
    pub fn for_tensor(elements: usize, element_bytes: usize) -> Self {
        let raw = elements * element_bytes;
        Self::power_of_two(raw)
    }
}

/// A slab of pre-allocated memory blocks of a uniform size.
struct Slab {
    /// Size class of this slab
    size_class: SizeClass,
    /// Free block indices
    free_list: Vec<usize>,
    /// Backing memory
    base: NonNull<u8>,
    /// Layout for deallocation
    layout: Layout,
    /// Total capacity (number of blocks)
    capacity: usize,
    /// Currently allocated blocks
    allocated: usize,
}

impl Slab {
    /// Create a new slab with the given capacity.
    fn new(size_class: SizeClass, capacity: usize) -> Result<Self> {
        let total_bytes = size_class.size * capacity;
        let layout = Layout::from_size_align(total_bytes, size_class.align)
            .map_err(|e| RmiError::Compute(format!("Invalid layout: {}", e)))?;

        // SAFETY: `layout` was validated by `Layout::from_size_align` above;
        // the returned pointer is checked for null immediately after.
        let ptr = unsafe { alloc::alloc_zeroed(layout) };
        let base = NonNull::new(ptr)
            .ok_or_else(|| RmiError::ResourceExhausted("Failed to allocate slab".to_string()))?;

        let free_list = (0..capacity).rev().collect();

        Ok(Self {
            size_class,
            free_list,
            base,
            layout,
            capacity,
            allocated: 0,
        })
    }

    /// Allocate a block from this slab.
    fn alloc(&mut self) -> Option<NonNull<u8>> {
        let idx = self.free_list.pop()?;
        self.allocated += 1;
        let offset = idx * self.size_class.size;
        // SAFETY: `idx` came from `free_list` which only contains indices in [0, capacity),
        // so `offset` is within the slab's allocation.
        let ptr = unsafe { self.base.as_ptr().add(offset) };
        NonNull::new(ptr)
    }

    /// Free a block back to this slab.
    fn free(&mut self, ptr: NonNull<u8>) -> bool {
        // SAFETY: `ptr` is checked against slab bounds below; `offset_from` is
        // valid because both pointers come from the same allocation.
        let offset = unsafe { ptr.as_ptr().offset_from(self.base.as_ptr()) };
        if offset < 0 {
            return false;
        }
        let offset = offset as usize;
        if offset % self.size_class.size != 0 {
            return false;
        }
        let idx = offset / self.size_class.size;
        if idx >= self.capacity {
            return false;
        }
        self.free_list.push(idx);
        self.allocated -= 1;
        true
    }

    /// Check if slab contains the given pointer.
    fn contains(&self, ptr: NonNull<u8>) -> bool {
        let base = self.base.as_ptr() as usize;
        let p = ptr.as_ptr() as usize;
        p >= base && p < base + self.layout.size()
    }
}

impl Drop for Slab {
    fn drop(&mut self) {
        // SAFETY: `self.base` was allocated with `alloc::alloc_zeroed` using `self.layout`
        // in `Slab::new`, and is only deallocated once here in `Drop`.
        unsafe {
            alloc::dealloc(self.base.as_ptr(), self.layout);
        }
    }
}

// SAFETY: Slab manages raw memory with proper allocation/deallocation.
// Access is synchronized by the MemoryPool RwLock.
unsafe impl Send for Slab {}
unsafe impl Sync for Slab {}

/// Configuration for the memory pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Initial slab capacity per size class
    pub initial_capacity: usize,
    /// Maximum total memory in bytes
    pub max_total_bytes: u64,
    /// Size classes to pre-allocate (in bytes)
    pub size_classes: Vec<usize>,
    /// Enable auto-growth when a slab is exhausted
    pub auto_grow: bool,
    /// Growth factor for auto-growth
    pub growth_factor: f64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            initial_capacity: 64,
            max_total_bytes: 1024 * 1024 * 1024, // 1 GB
            size_classes: vec![64, 256, 1024, 4096, 16384, 65536, 262144, 1048576],
            auto_grow: true,
            growth_factor: 2.0,
        }
    }
}

/// Memory pool statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PoolStats {
    /// Total bytes allocated
    pub total_allocated: u64,
    /// Total bytes in free lists
    pub total_free: u64,
    /// Number of allocations
    pub alloc_count: u64,
    /// Number of deallocations
    pub dealloc_count: u64,
    /// Cache hit count (reused block)
    pub cache_hits: u64,
    /// Cache miss count (new slab needed)
    pub cache_misses: u64,
    /// Per-size-class utilization
    pub class_utilization: HashMap<usize, f64>,
}

/// Arena-based memory pool with slab allocation.
pub struct MemoryPool {
    /// Slabs organized by size class
    slabs: RwLock<HashMap<usize, Vec<Slab>>>,
    /// Configuration
    config: PoolConfig,
    /// Stats counters
    alloc_count: AtomicU64,
    dealloc_count: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    total_managed: AtomicU64,
}

impl MemoryPool {
    /// Create a new memory pool with default configuration.
    pub fn new() -> Result<Self> {
        Self::with_config(PoolConfig::default())
    }

    /// Create with specific configuration.
    pub fn with_config(config: PoolConfig) -> Result<Self> {
        let mut slabs_map = HashMap::new();

        for &size in &config.size_classes {
            let sc = SizeClass::power_of_two(size);
            let slab = Slab::new(sc, config.initial_capacity)?;
            slabs_map.insert(sc.size, vec![slab]);
        }

        let total = config
            .size_classes
            .iter()
            .map(|&s| SizeClass::power_of_two(s).size as u64 * config.initial_capacity as u64)
            .sum::<u64>();

        Ok(Self {
            slabs: RwLock::new(slabs_map),
            config,
            alloc_count: AtomicU64::new(0),
            dealloc_count: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            total_managed: AtomicU64::new(total),
        })
    }

    /// Allocate memory for a given size.
    pub fn alloc(&self, size: usize) -> Result<NonNull<u8>> {
        let sc = SizeClass::power_of_two(size);
        self.alloc_count.fetch_add(1, Ordering::Relaxed);

        let mut slabs = self.slabs.write().unwrap();
        let slab_list = slabs.entry(sc.size).or_default();

        // Try existing slabs
        for slab in slab_list.iter_mut() {
            if let Some(ptr) = slab.alloc() {
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(ptr);
            }
        }

        // Need a new slab
        self.cache_misses.fetch_add(1, Ordering::Relaxed);

        if !self.config.auto_grow {
            return Err(RmiError::ResourceExhausted(format!(
                "Memory pool exhausted for size class {}",
                sc.size
            )));
        }

        let new_capacity =
            (self.config.initial_capacity as f64 * self.config.growth_factor) as usize;
        let new_slab_bytes = sc.size as u64 * new_capacity as u64;

        if self.total_managed.load(Ordering::Relaxed) + new_slab_bytes > self.config.max_total_bytes
        {
            return Err(RmiError::ResourceExhausted(
                "Memory pool maximum exceeded".to_string(),
            ));
        }

        let mut new_slab = Slab::new(sc, new_capacity)?;
        let ptr = new_slab
            .alloc()
            .ok_or_else(|| RmiError::Compute("Fresh slab alloc failed".to_string()))?;

        self.total_managed
            .fetch_add(new_slab_bytes, Ordering::Relaxed);
        slab_list.push(new_slab);

        Ok(ptr)
    }

    /// Deallocate memory.
    pub fn dealloc(&self, ptr: NonNull<u8>, size: usize) {
        let sc = SizeClass::power_of_two(size);
        self.dealloc_count.fetch_add(1, Ordering::Relaxed);

        let mut slabs = self.slabs.write().unwrap();
        if let Some(slab_list) = slabs.get_mut(&sc.size) {
            for slab in slab_list.iter_mut() {
                if slab.contains(ptr) {
                    slab.free(ptr);
                    return;
                }
            }
        }
    }

    /// Get pool statistics.
    pub fn stats(&self) -> PoolStats {
        let slabs = self.slabs.read().unwrap();
        let mut total_allocated = 0u64;
        let mut total_free = 0u64;
        let mut class_util = HashMap::new();

        for (&size, slab_list) in slabs.iter() {
            let mut class_alloc = 0usize;
            let mut class_cap = 0usize;
            for slab in slab_list {
                class_alloc += slab.allocated;
                class_cap += slab.capacity;
                total_allocated += (slab.allocated * size) as u64;
                total_free += (slab.free_list.len() * size) as u64;
            }
            if class_cap > 0 {
                class_util.insert(size, class_alloc as f64 / class_cap as f64);
            }
        }

        PoolStats {
            total_allocated,
            total_free,
            alloc_count: self.alloc_count.load(Ordering::Relaxed),
            dealloc_count: self.dealloc_count.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            class_utilization: class_util,
        }
    }

    /// Reset all slabs (free all allocations).
    pub fn reset(&self) {
        let mut slabs = self.slabs.write().unwrap();
        for slab_list in slabs.values_mut() {
            for slab in slab_list.iter_mut() {
                slab.free_list.clear();
                for i in (0..slab.capacity).rev() {
                    slab.free_list.push(i);
                }
                slab.allocated = 0;
            }
        }
    }
}

impl Default for MemoryPool {
    fn default() -> Self {
        Self::new().expect("Failed to create default memory pool")
    }
}

// ============================================================================
// Tensor Buffer
// ============================================================================

/// Zero-copy tensor buffer with reference counting.
pub struct TensorBuffer {
    /// Pointer to data
    data: NonNull<u8>,
    /// Length in bytes
    len: usize,
    /// Reference count
    refcount: Arc<AtomicUsize>,
    /// Whether we own the memory (vs. borrowed view)
    owned: bool,
}

impl TensorBuffer {
    /// Create a new owned buffer from a Vec.
    pub fn from_vec(mut data: Vec<u8>) -> Self {
        let len = data.len();
        let ptr = NonNull::new(data.as_mut_ptr()).expect("Vec should not be null");
        std::mem::forget(data); // Transfer ownership

        Self {
            data: ptr,
            len,
            refcount: Arc::new(AtomicUsize::new(1)),
            owned: true,
        }
    }

    /// Create a read-only view (zero-copy slice).
    pub fn slice(&self, offset: usize, len: usize) -> Result<TensorBuffer> {
        if offset + len > self.len {
            return Err(RmiError::Compute(format!(
                "Slice [{}, {}) out of bounds for buffer of length {}",
                offset,
                offset + len,
                self.len
            )));
        }

        self.refcount.fetch_add(1, Ordering::Relaxed);

        // SAFETY: `offset + len <= self.len` was checked above, so the pointer
        // arithmetic stays within the original allocation.
        let data = unsafe { NonNull::new_unchecked(self.data.as_ptr().add(offset)) };

        Ok(TensorBuffer {
            data,
            len,
            refcount: Arc::clone(&self.refcount),
            owned: false,
        })
    }

    /// Get a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: `self.data` points to `self.len` initialized bytes, and the
        // borrow is tied to `&self` so no mutable alias exists.
        unsafe { std::slice::from_raw_parts(self.data.as_ptr(), self.len) }
    }

    /// Get a mutable byte slice (only if uniquely owned).
    pub fn as_bytes_mut(&mut self) -> Result<&mut [u8]> {
        if self.refcount.load(Ordering::Relaxed) != 1 {
            return Err(RmiError::Compute(
                "Cannot mutate shared tensor buffer".to_string(),
            ));
        }
        // SAFETY: refcount == 1 was verified above, guaranteeing exclusive access.
        // `self.data` points to `self.len` initialized bytes.
        Ok(unsafe { std::slice::from_raw_parts_mut(self.data.as_ptr(), self.len) })
    }

    /// Get a typed slice.
    pub fn as_f32_slice(&self) -> &[f32] {
        let ptr = self.data.as_ptr() as *const f32;
        let count = self.len / std::mem::size_of::<f32>();
        // SAFETY: `self.data` is aligned by construction (TensorBuffer alignment ≥ 4),
        // and `count * size_of::<f32>() <= self.len`.
        unsafe { std::slice::from_raw_parts(ptr, count) }
    }

    /// Length in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Reference count.
    #[inline]
    pub fn refcount(&self) -> usize {
        self.refcount.load(Ordering::Relaxed)
    }
}

impl Drop for TensorBuffer {
    fn drop(&mut self) {
        let prev = self.refcount.fetch_sub(1, Ordering::Release);
        if prev == 1 && self.owned {
            // Last reference and we own the data
            std::sync::atomic::fence(Ordering::Acquire);
            // SAFETY: this is the last reference (prev == 1) and we own the data,
            // so reconstructing the Vec for deallocation is safe. The pointer, len,
            // and capacity match what was passed to `mem::forget` in `from_vec`.
            unsafe {
                let _ = Vec::from_raw_parts(self.data.as_ptr(), self.len, self.len);
            }
        }
    }
}

impl Clone for TensorBuffer {
    fn clone(&self) -> Self {
        self.refcount.fetch_add(1, Ordering::Relaxed);
        Self {
            data: self.data,
            len: self.len,
            refcount: Arc::clone(&self.refcount),
            owned: false, // Clones are not owners
        }
    }
}

// SAFETY: TensorBuffer manages raw memory with Arc<AtomicUsize> reference counting.
unsafe impl Send for TensorBuffer {}
unsafe impl Sync for TensorBuffer {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_class() {
        let sc = SizeClass::power_of_two(100);
        assert_eq!(sc.size, 128); // Rounded to power of 2
        assert_eq!(sc.align, 64);
    }

    #[test]
    fn test_size_class_tensor() {
        let sc = SizeClass::for_tensor(1024, 4); // 1024 f32s = 4096 bytes
        assert_eq!(sc.size, 4096);
    }

    #[test]
    fn test_slab_alloc_dealloc() {
        let sc = SizeClass::power_of_two(256);
        let mut slab = Slab::new(sc, 4).unwrap();

        let p1 = slab.alloc().unwrap();
        let p2 = slab.alloc().unwrap();
        assert_ne!(p1.as_ptr(), p2.as_ptr());
        assert_eq!(slab.allocated, 2);

        slab.free(p1);
        assert_eq!(slab.allocated, 1);

        // Re-use freed block
        let _p3 = slab.alloc().unwrap();
        assert_eq!(slab.allocated, 2);
    }

    #[test]
    fn test_slab_exhaustion() {
        let sc = SizeClass::power_of_two(64);
        let mut slab = Slab::new(sc, 2).unwrap();

        let _ = slab.alloc().unwrap();
        let _ = slab.alloc().unwrap();
        assert!(slab.alloc().is_none());
    }

    #[test]
    fn test_memory_pool_basic() {
        let pool = MemoryPool::new().unwrap();
        let ptr = pool.alloc(100).unwrap();
        pool.dealloc(ptr, 100);

        let stats = pool.stats();
        assert_eq!(stats.alloc_count, 1);
        assert_eq!(stats.dealloc_count, 1);
    }

    #[test]
    fn test_memory_pool_auto_grow() {
        let config = PoolConfig {
            initial_capacity: 2,
            auto_grow: true,
            ..Default::default()
        };
        let pool = MemoryPool::with_config(config).unwrap();

        // Allocate more than initial capacity
        let mut ptrs = Vec::new();
        for _ in 0..5 {
            ptrs.push(pool.alloc(64).unwrap());
        }

        let stats = pool.stats();
        assert!(stats.cache_misses > 0);

        for ptr in ptrs {
            pool.dealloc(ptr, 64);
        }
    }

    #[test]
    fn test_memory_pool_reset() {
        let pool = MemoryPool::new().unwrap();
        let _ = pool.alloc(256).unwrap();
        let _ = pool.alloc(256).unwrap();

        pool.reset();
        let stats = pool.stats();
        assert_eq!(stats.total_allocated, 0);
    }

    #[test]
    fn test_tensor_buffer_from_vec() {
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let buf = TensorBuffer::from_vec(data);

        assert_eq!(buf.len(), 8);
        assert_eq!(buf.as_bytes(), &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(buf.refcount(), 1);
    }

    #[test]
    fn test_tensor_buffer_slice() {
        let data = vec![0u8; 100];
        let buf = TensorBuffer::from_vec(data);

        let slice = buf.slice(10, 30).unwrap();
        assert_eq!(slice.len(), 30);
        assert_eq!(buf.refcount(), 2);
    }

    #[test]
    fn test_tensor_buffer_slice_oob() {
        let data = vec![0u8; 10];
        let buf = TensorBuffer::from_vec(data);

        assert!(buf.slice(5, 10).is_err());
    }

    #[test]
    fn test_tensor_buffer_clone() {
        let data = vec![42u8; 16];
        let buf = TensorBuffer::from_vec(data);
        let clone = buf.clone();

        assert_eq!(buf.refcount(), 2);
        assert_eq!(clone.as_bytes(), buf.as_bytes());
    }

    #[test]
    fn test_tensor_buffer_mut_shared() {
        let data = vec![0u8; 16];
        let buf = TensorBuffer::from_vec(data);
        let _clone = buf.clone();

        // Can't get mutable access while shared
        let mut buf2 = buf;
        assert!(buf2.as_bytes_mut().is_err());
    }

    #[test]
    fn test_tensor_buffer_f32_slice() {
        let floats: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        let bytes: Vec<u8> = floats.iter().flat_map(|f| f.to_ne_bytes()).collect();
        let buf = TensorBuffer::from_vec(bytes);

        let f32_slice = buf.as_f32_slice();
        assert_eq!(f32_slice.len(), 4);
        assert_eq!(f32_slice[0], 1.0);
        assert_eq!(f32_slice[3], 4.0);
    }
}

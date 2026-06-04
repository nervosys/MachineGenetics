//! Incremental compilation cache for RMIL expressions.
//!
//! Uses content hashing to detect unchanged sub-expressions and skip
//! redundant work (encoding, optimization, evaluation). The cache maps
//! `Expr::content_hash()` → compiled artefact, so structurally identical
//! trees always share their compiled form.
//!
//! # Design
//!
//! An [`IncrementalCache`] stores compiled bytes (binary encoded) and
//! optimization results for previously-seen expressions. When an agent
//! resubmits a slightly modified architecture, only the changed
//! sub-expressions need reprocessing.
//!
//! # Examples
//!
//! ```
//! use rmi::lang::incremental::IncrementalCache;
//! use rmi::lang::{Expr, Op};
//!
//! let mut cache = IncrementalCache::new();
//!
//! let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
//! let hash = expr.content_hash();
//!
//! // First compilation: miss
//! assert!(!cache.contains(hash));
//!
//! // Store compiled artefact
//! cache.insert(hash, vec![0xDE, 0xAD], None);
//! assert!(cache.contains(hash));
//!
//! // Second time: hit
//! let entry = cache.get(hash).unwrap();
//! assert_eq!(entry.encoded, vec![0xDE, 0xAD]);
//! ```

use crate::lang::codec;
use crate::lang::expr::Expr;
use std::collections::HashMap;

// ── Cache entry ──────────────────────────────────────────────────────────────

/// A compiled artefact stored in the incremental cache.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Binary-encoded form of the expression.
    pub encoded: Vec<u8>,
    /// Wire-format size.
    pub wire_size: usize,
    /// Node count of the original expression.
    pub node_count: usize,
    /// Content hash.
    pub hash: u64,
    /// Optimized expression (if optimization was applied).
    pub optimized: Option<Expr>,
}

// ── Incremental cache ────────────────────────────────────────────────────────

/// Hash-based incremental compilation cache.
///
/// Maps `content_hash → CacheEntry` to avoid redundant work on unchanged
/// sub-expressions.
#[derive(Debug, Clone)]
pub struct IncrementalCache {
    entries: HashMap<u64, CacheEntry>,
    /// Total cache hits.
    pub hits: u64,
    /// Total cache misses.
    pub misses: u64,
    /// Maximum entries (0 = unlimited).
    pub max_entries: usize,
}

impl IncrementalCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            hits: 0,
            misses: 0,
            max_entries: 0,
        }
    }

    /// Create a cache with a maximum number of entries.
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_entries.min(1024)),
            hits: 0,
            misses: 0,
            max_entries,
        }
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Check if a hash is in the cache.
    pub fn contains(&self, hash: u64) -> bool {
        self.entries.contains_key(&hash)
    }

    /// Get a cache entry by hash.
    pub fn get(&mut self, hash: u64) -> Option<&CacheEntry> {
        if self.entries.contains_key(&hash) {
            self.hits += 1;
            self.entries.get(&hash)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Insert a pre-built cache entry.
    pub fn insert(&mut self, hash: u64, encoded: Vec<u8>, optimized: Option<Expr>) {
        // Evict oldest if over capacity (simple strategy: just don't add)
        if self.max_entries > 0 && self.entries.len() >= self.max_entries {
            // Remove an arbitrary entry (HashMap iteration order)
            if let Some(&key) = self.entries.keys().next() {
                self.entries.remove(&key);
            }
        }

        let wire_size = encoded.len();
        self.entries.insert(
            hash,
            CacheEntry {
                encoded,
                wire_size,
                node_count: 0,
                hash,
                optimized,
            },
        );
    }

    /// Compile an expression (encode to binary), caching the result.
    ///
    /// Returns the cache entry. On a cache hit, returns the previously
    /// compiled artefact without re-encoding.
    pub fn compile(&mut self, expr: &Expr) -> &CacheEntry {
        let hash = expr.content_hash();
        if self.entries.contains_key(&hash) {
            self.hits += 1;
            return self.entries.get(&hash).expect("cache entry exists after contains_key");
        }

        self.misses += 1;
        let encoded = codec::Encoder::encode_expr_only(expr);
        let wire_size = codec::wire_size(expr);
        let node_count = expr.node_count();

        if self.max_entries > 0 && self.entries.len() >= self.max_entries {
            if let Some(&key) = self.entries.keys().next() {
                self.entries.remove(&key);
            }
        }

        self.entries.insert(
            hash,
            CacheEntry {
                encoded,
                wire_size,
                node_count,
                hash,
                optimized: None,
            },
        );
        self.entries.get(&hash).expect("cache entry exists after insert")
    }

    /// Compile with optimization. If the expression was already compiled and
    /// optimized, returns the cached result.
    pub fn compile_optimized<F>(&mut self, expr: &Expr, optimize: F) -> &CacheEntry
    where
        F: FnOnce(&Expr) -> Expr,
    {
        let hash = expr.content_hash();
        if self.entries.contains_key(&hash) {
            self.hits += 1;
            return self.entries.get(&hash).expect("cache entry exists after contains_key");
        }

        self.misses += 1;
        let optimized = optimize(expr);
        let encoded = codec::Encoder::encode_expr_only(&optimized);
        let wire_size = codec::wire_size(&optimized);
        let node_count = optimized.node_count();

        if self.max_entries > 0 && self.entries.len() >= self.max_entries {
            if let Some(&key) = self.entries.keys().next() {
                self.entries.remove(&key);
            }
        }

        self.entries.insert(
            hash,
            CacheEntry {
                encoded,
                wire_size,
                node_count,
                hash,
                optimized: Some(optimized),
            },
        );
        self.entries.get(&hash).expect("cache entry exists after insert")
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// Hit ratio (0.0 to 1.0).
    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Summary statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.entries.len(),
            hits: self.hits,
            misses: self.misses,
            hit_ratio: self.hit_ratio(),
            total_bytes: self.entries.values().map(|e| e.wire_size).sum(),
        }
    }
}

impl Default for IncrementalCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics for the incremental cache.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of cache entries.
    pub entries: usize,
    /// Number of cache hits.
    pub hits: u64,
    /// Number of cache misses.
    pub misses: u64,
    /// Cache hit ratio (0.0–1.0).
    pub hit_ratio: f64,
    /// Total cached bytes.
    pub total_bytes: usize,
}

impl std::fmt::Display for CacheStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "IncrementalCache: {} entries, {} hits, {} misses ({:.1}% ratio), {} bytes",
            self.entries,
            self.hits,
            self.misses,
            self.hit_ratio * 100.0,
            self.total_bytes,
        )
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Op;

    #[test]
    fn cache_new_empty() {
        let cache = IncrementalCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn cache_insert_and_get() {
        let mut cache = IncrementalCache::new();
        cache.insert(42, vec![1, 2, 3], None);
        assert!(cache.contains(42));
        let entry = cache.get(42).unwrap();
        assert_eq!(entry.encoded, vec![1, 2, 3]);
    }

    #[test]
    fn cache_miss() {
        let mut cache = IncrementalCache::new();
        assert!(cache.get(99).is_none());
        assert_eq!(cache.misses, 1);
    }

    #[test]
    fn cache_hit_increments() {
        let mut cache = IncrementalCache::new();
        cache.insert(42, vec![1], None);
        let _ = cache.get(42);
        let _ = cache.get(42);
        assert_eq!(cache.hits, 2);
    }

    #[test]
    fn compile_caches_binary() {
        let mut cache = IncrementalCache::new();
        let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
        let hash = expr.content_hash();

        let entry_len = {
            let entry = cache.compile(&expr);
            assert!(!entry.encoded.is_empty());
            assert_eq!(entry.hash, hash);
            entry.encoded.len()
        };
        assert_eq!(cache.misses, 1);

        // Second compile: cache hit
        let entry2 = cache.compile(&expr);
        assert_eq!(entry2.encoded.len(), entry_len);
        assert_eq!(cache.hits, 1);
    }

    #[test]
    fn compile_different_exprs() {
        let mut cache = IncrementalCache::new();
        let e1 = Expr::op1(Op::RELU);
        let e2 = Expr::op1(Op::GELU);

        cache.compile(&e1);
        cache.compile(&e2);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn compile_optimized() {
        let mut cache = IncrementalCache::new();
        let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);

        {
            let entry = cache.compile_optimized(&expr, |e| e.clone());
            assert!(entry.optimized.is_some());
        }
        assert_eq!(cache.misses, 1);

        // Hit on second call
        let _ = cache.compile_optimized(&expr, |e| e.clone());
        assert_eq!(cache.hits, 1);
    }

    #[test]
    fn cache_clear() {
        let mut cache = IncrementalCache::new();
        cache.insert(1, vec![1], None);
        cache.insert(2, vec![2], None);
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.hits, 0);
        assert_eq!(cache.misses, 0);
    }

    #[test]
    fn hit_ratio() {
        let mut cache = IncrementalCache::new();
        assert_eq!(cache.hit_ratio(), 0.0);

        cache.insert(1, vec![1], None);
        let _ = cache.get(1); // hit
        let _ = cache.get(2); // miss
        assert!((cache.hit_ratio() - 0.5).abs() < 0.01);
    }

    #[test]
    fn cache_stats() {
        let mut cache = IncrementalCache::new();
        cache.insert(1, vec![0xAA, 0xBB], None);
        let _ = cache.get(1);
        let _ = cache.get(99);
        let stats = cache.stats();
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.total_bytes, 2);
    }

    #[test]
    fn cache_with_capacity_evicts() {
        let mut cache = IncrementalCache::with_capacity(2);
        cache.insert(1, vec![1], None);
        cache.insert(2, vec![2], None);
        cache.insert(3, vec![3], None); // should evict one
        assert_eq!(cache.len(), 2);
        assert!(cache.contains(3));
    }

    #[test]
    fn cache_default() {
        let cache = IncrementalCache::default();
        assert!(cache.is_empty());
    }

    #[test]
    fn structural_sharing() {
        let mut cache = IncrementalCache::new();
        // Two structurally identical expressions should share the cache entry
        let e1 = Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
        let e2 = Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
        assert_eq!(e1.content_hash(), e2.content_hash());

        cache.compile(&e1);
        cache.compile(&e2); // should be a hit
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn stats_display() {
        let mut cache = IncrementalCache::new();
        cache.insert(1, vec![0; 100], None);
        let stats = cache.stats();
        let s = format!("{stats}");
        assert!(s.contains("1 entries"));
        assert!(s.contains("100 bytes"));
    }

    #[test]
    fn insert_with_optimized() {
        let mut cache = IncrementalCache::new();
        let optimized = Expr::op1(Op::RELU);
        cache.insert(42, vec![1, 2], Some(optimized.clone()));
        let entry = cache.get(42).unwrap();
        assert!(entry.optimized.is_some());
        assert_eq!(entry.optimized.as_ref().unwrap(), &optimized);
    }
}

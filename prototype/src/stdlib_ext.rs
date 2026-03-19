// ── Redox Standard Library Extensions ──────────────────────────────
//
// Extended stdlib types for the Redox agentic runtime:
//
//   - BatchApi       — batch operation helpers (map, filter, reduce, partition)
//   - StreamBuf      — streaming I/O buffer with chunked reads/writes
//   - SwarmVec       — vector with per-agent ownership tracking
//   - ArenaVec       — arena-backed vector for allocation-free growth
//   - SwarmChannel   — typed multi-producer multi-consumer channel
//   - AgentArena     — per-agent arena allocator with stats

use std::collections::{BTreeMap, VecDeque};

// ── Batch API ──────────────────────────────────────────────────────

/// Batch operations on collections with result aggregation.
pub struct BatchApi;

impl BatchApi {
    /// Apply a function to every element, collecting results.
    pub fn map<T, U, F: Fn(&T) -> U>(items: &[T], f: F) -> Vec<U> {
        items.iter().map(f).collect()
    }

    /// Filter elements by predicate.
    pub fn filter<T, F: Fn(&T) -> bool>(items: &[T], f: F) -> Vec<&T> {
        items.iter().filter(|x| f(x)).collect()
    }

    /// Reduce a collection to a single value.
    pub fn reduce<T: Clone, F: Fn(&T, &T) -> T>(items: &[T], f: F) -> Option<T> {
        if items.is_empty() {
            return None;
        }
        let mut acc = items[0].clone();
        for item in &items[1..] {
            acc = f(&acc, item);
        }
        Some(acc)
    }

    /// Partition elements into (matching, not-matching).
    pub fn partition<T, F: Fn(&T) -> bool>(items: &[T], f: F) -> (Vec<&T>, Vec<&T>) {
        let mut yes = Vec::new();
        let mut no = Vec::new();
        for item in items {
            if f(item) {
                yes.push(item);
            } else {
                no.push(item);
            }
        }
        (yes, no)
    }

    /// Chunk a slice into groups of `size`.
    pub fn chunk<T>(items: &[T], size: usize) -> Vec<&[T]> {
        if size == 0 {
            return vec![];
        }
        items.chunks(size).collect()
    }
}

// ── Streaming I/O Buffer ───────────────────────────────────────────

/// Chunked streaming buffer for I/O operations.
pub struct StreamBuf {
    buffer: VecDeque<u8>,
    chunk_size: usize,
    bytes_read: u64,
    bytes_written: u64,
}

impl StreamBuf {
    pub fn new(chunk_size: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            chunk_size: if chunk_size == 0 { 4096 } else { chunk_size },
            bytes_read: 0,
            bytes_written: 0,
        }
    }

    /// Write bytes into the buffer.
    pub fn write(&mut self, data: &[u8]) {
        self.buffer.extend(data);
        self.bytes_written += data.len() as u64;
    }

    /// Read up to `chunk_size` bytes from the buffer.
    pub fn read_chunk(&mut self) -> Vec<u8> {
        let n = self.buffer.len().min(self.chunk_size);
        let chunk: Vec<u8> = self.buffer.drain(..n).collect();
        self.bytes_read += chunk.len() as u64;
        chunk
    }

    /// Read all available bytes.
    pub fn read_all(&mut self) -> Vec<u8> {
        let all: Vec<u8> = self.buffer.drain(..).collect();
        self.bytes_read += all.len() as u64;
        all
    }

    pub fn available(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn stats(&self) -> (u64, u64) {
        (self.bytes_read, self.bytes_written)
    }
}

// ── SwarmVec ───────────────────────────────────────────────────────

/// A vector that tracks which agent owns each element.
pub struct SwarmVec<T> {
    elements: Vec<(String, T)>, // (agent_id, value)
}

impl<T> SwarmVec<T> {
    pub fn new() -> Self {
        Self { elements: Vec::new() }
    }

    pub fn push(&mut self, agent_id: &str, value: T) {
        self.elements.push((agent_id.into(), value));
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<(&str, &T)> {
        self.elements.get(index).map(|(a, v)| (a.as_str(), v))
    }

    /// Get all elements owned by a specific agent.
    pub fn by_agent(&self, agent_id: &str) -> Vec<&T> {
        self.elements
            .iter()
            .filter(|(a, _)| a == agent_id)
            .map(|(_, v)| v)
            .collect()
    }

    /// Remove all elements owned by a specific agent.
    pub fn remove_by_agent(&mut self, agent_id: &str) -> Vec<T> {
        let mut removed = Vec::new();
        let mut i = 0;
        while i < self.elements.len() {
            if self.elements[i].0 == agent_id {
                removed.push(self.elements.remove(i).1);
            } else {
                i += 1;
            }
        }
        removed
    }

    /// Count elements per agent.
    pub fn agent_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for (agent, _) in &self.elements {
            *counts.entry(agent.clone()).or_insert(0) += 1;
        }
        counts
    }
}

// ── ArenaVec ───────────────────────────────────────────────────────

/// Arena-backed vector: pre-allocated, no per-element allocation.
pub struct ArenaVec<T> {
    storage: Vec<T>,
    capacity: usize,
}

impl<T> ArenaVec<T> {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            storage: Vec::with_capacity(cap),
            capacity: cap,
        }
    }

    /// Push a value. Returns Err if arena is full.
    pub fn push(&mut self, value: T) -> Result<usize, &'static str> {
        if self.storage.len() >= self.capacity {
            return Err("Arena full");
        }
        let idx = self.storage.len();
        self.storage.push(value);
        Ok(idx)
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.storage.get(index)
    }

    pub fn len(&self) -> usize {
        self.storage.len()
    }

    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    pub fn remaining(&self) -> usize {
        self.capacity - self.storage.len()
    }

    pub fn is_full(&self) -> bool {
        self.storage.len() >= self.capacity
    }

    /// Reset the arena, dropping all elements.
    pub fn clear(&mut self) {
        self.storage.clear();
    }
}

// ── SwarmChannel ───────────────────────────────────────────────────

/// Typed multi-producer multi-consumer channel for swarm agents.
pub struct SwarmChannel<T> {
    queue: VecDeque<(String, T)>, // (sender_agent_id, message)
    capacity: usize,
    total_sent: u64,
    total_received: u64,
}

impl<T> SwarmChannel<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            capacity: if capacity == 0 { usize::MAX } else { capacity },
            total_sent: 0,
            total_received: 0,
        }
    }

    /// Send a message. Returns Err if channel is full.
    pub fn send(&mut self, sender: &str, msg: T) -> Result<(), &'static str> {
        if self.queue.len() >= self.capacity {
            return Err("Channel full");
        }
        self.queue.push_back((sender.into(), msg));
        self.total_sent += 1;
        Ok(())
    }

    /// Receive the next message.
    pub fn recv(&mut self) -> Option<(String, T)> {
        let item = self.queue.pop_front();
        if item.is_some() {
            self.total_received += 1;
        }
        item
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn stats(&self) -> (u64, u64) {
        (self.total_sent, self.total_received)
    }
}

// ── Agent Arena Allocator ──────────────────────────────────────────

/// Per-agent arena allocator tracking.
pub struct AgentArena {
    /// agent_id → (allocated_bytes, peak_bytes, allocation_count)
    agents: BTreeMap<String, (u64, u64, u64)>,
}

impl AgentArena {
    pub fn new() -> Self {
        Self { agents: BTreeMap::new() }
    }

    /// Record an allocation for an agent.
    pub fn allocate(&mut self, agent_id: &str, bytes: u64) {
        let entry = self.agents.entry(agent_id.into()).or_insert((0, 0, 0));
        entry.0 += bytes;
        if entry.0 > entry.1 {
            entry.1 = entry.0;
        }
        entry.2 += 1;
    }

    /// Record a deallocation for an agent.
    pub fn deallocate(&mut self, agent_id: &str, bytes: u64) {
        if let Some(entry) = self.agents.get_mut(agent_id) {
            entry.0 = entry.0.saturating_sub(bytes);
        }
    }

    /// Get current usage for an agent: (current_bytes, peak_bytes, alloc_count).
    pub fn usage(&self, agent_id: &str) -> Option<(u64, u64, u64)> {
        self.agents.get(agent_id).copied()
    }

    /// Reset an agent's arena (free all).
    pub fn reset(&mut self, agent_id: &str) {
        if let Some(entry) = self.agents.get_mut(agent_id) {
            entry.0 = 0;
        }
    }

    /// Total memory across all agents.
    pub fn total_allocated(&self) -> u64 {
        self.agents.values().map(|(cur, _, _)| cur).sum()
    }

    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub fn stats(&self) -> String {
        let total = self.total_allocated();
        let agents = self.agents.len();
        let peak: u64 = self.agents.values().map(|(_, p, _)| p).sum();
        format!(
            "{{\"agents\":{},\"total_bytes\":{},\"peak_bytes\":{}}}",
            agents, total, peak
        )
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── BatchApi ──────────────────────────────────────────────────

    #[test]
    fn batch_map() {
        let items = vec![1, 2, 3];
        let doubled = BatchApi::map(&items, |x| x * 2);
        assert_eq!(doubled, vec![2, 4, 6]);
    }

    #[test]
    fn batch_filter() {
        let items = vec![1, 2, 3, 4, 5];
        let evens = BatchApi::filter(&items, |x| x % 2 == 0);
        assert_eq!(evens, vec![&2, &4]);
    }

    #[test]
    fn batch_reduce() {
        let items = vec![1, 2, 3, 4];
        let sum = BatchApi::reduce(&items, |a, b| a + b);
        assert_eq!(sum, Some(10));
    }

    #[test]
    fn batch_reduce_empty() {
        let items: Vec<i32> = vec![];
        assert_eq!(BatchApi::reduce(&items, |a, b| a + b), None);
    }

    #[test]
    fn batch_partition() {
        let items = vec![1, 2, 3, 4, 5];
        let (evens, odds) = BatchApi::partition(&items, |x| x % 2 == 0);
        assert_eq!(evens, vec![&2, &4]);
        assert_eq!(odds, vec![&1, &3, &5]);
    }

    #[test]
    fn batch_chunk() {
        let items = vec![1, 2, 3, 4, 5];
        let chunks = BatchApi::chunk(&items, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    // ── StreamBuf ─────────────────────────────────────────────────

    #[test]
    fn stream_buf_write_read() {
        let mut sb = StreamBuf::new(4);
        sb.write(b"hello world");
        assert_eq!(sb.available(), 11);
        let chunk = sb.read_chunk();
        assert_eq!(chunk, b"hell");
        assert_eq!(sb.available(), 7);
    }

    #[test]
    fn stream_buf_read_all() {
        let mut sb = StreamBuf::new(1024);
        sb.write(b"abc");
        let all = sb.read_all();
        assert_eq!(all, b"abc");
        assert!(sb.is_empty());
    }

    #[test]
    fn stream_buf_stats() {
        let mut sb = StreamBuf::new(10);
        sb.write(b"12345");
        sb.read_chunk();
        let (read, written) = sb.stats();
        assert_eq!(written, 5);
        assert_eq!(read, 5);
    }

    // ── SwarmVec ──────────────────────────────────────────────────

    #[test]
    fn swarm_vec_by_agent() {
        let mut sv: SwarmVec<i32> = SwarmVec::new();
        sv.push("a", 1);
        sv.push("b", 2);
        sv.push("a", 3);
        assert_eq!(sv.by_agent("a"), vec![&1, &3]);
        assert_eq!(sv.by_agent("b"), vec![&2]);
    }

    #[test]
    fn swarm_vec_remove_by_agent() {
        let mut sv: SwarmVec<i32> = SwarmVec::new();
        sv.push("a", 10);
        sv.push("b", 20);
        sv.push("a", 30);
        let removed = sv.remove_by_agent("a");
        assert_eq!(removed, vec![10, 30]);
        assert_eq!(sv.len(), 1);
    }

    #[test]
    fn swarm_vec_agent_counts() {
        let mut sv: SwarmVec<&str> = SwarmVec::new();
        sv.push("x", "a");
        sv.push("y", "b");
        sv.push("x", "c");
        let counts = sv.agent_counts();
        assert_eq!(counts["x"], 2);
        assert_eq!(counts["y"], 1);
    }

    // ── ArenaVec ──────────────────────────────────────────────────

    #[test]
    fn arena_vec_push_and_get() {
        let mut av = ArenaVec::with_capacity(3);
        assert_eq!(av.push(10), Ok(0));
        assert_eq!(av.push(20), Ok(1));
        assert_eq!(av.get(0), Some(&10));
        assert_eq!(av.remaining(), 1);
    }

    #[test]
    fn arena_vec_full() {
        let mut av = ArenaVec::with_capacity(1);
        assert!(av.push(1).is_ok());
        assert!(av.push(2).is_err());
        assert!(av.is_full());
    }

    #[test]
    fn arena_vec_clear() {
        let mut av = ArenaVec::with_capacity(5);
        av.push(1).unwrap();
        av.push(2).unwrap();
        av.clear();
        assert!(av.is_empty());
        assert_eq!(av.remaining(), 5);
    }

    // ── SwarmChannel ──────────────────────────────────────────────

    #[test]
    fn channel_send_recv() {
        let mut ch: SwarmChannel<String> = SwarmChannel::new(10);
        ch.send("agent-a", "hello".into()).unwrap();
        let (sender, msg) = ch.recv().unwrap();
        assert_eq!(sender, "agent-a");
        assert_eq!(msg, "hello");
    }

    #[test]
    fn channel_full() {
        let mut ch: SwarmChannel<i32> = SwarmChannel::new(1);
        assert!(ch.send("a", 1).is_ok());
        assert!(ch.send("a", 2).is_err());
    }

    #[test]
    fn channel_stats() {
        let mut ch: SwarmChannel<i32> = SwarmChannel::new(100);
        ch.send("a", 1).unwrap();
        ch.send("b", 2).unwrap();
        ch.recv();
        let (sent, received) = ch.stats();
        assert_eq!(sent, 2);
        assert_eq!(received, 1);
    }

    // ── AgentArena ────────────────────────────────────────────────

    #[test]
    fn agent_arena_allocate_deallocate() {
        let mut aa = AgentArena::new();
        aa.allocate("agent-a", 1024);
        aa.allocate("agent-a", 512);
        assert_eq!(aa.usage("agent-a"), Some((1536, 1536, 2)));
        aa.deallocate("agent-a", 512);
        assert_eq!(aa.usage("agent-a"), Some((1024, 1536, 2)));
    }

    #[test]
    fn agent_arena_reset() {
        let mut aa = AgentArena::new();
        aa.allocate("x", 4096);
        aa.reset("x");
        assert_eq!(aa.usage("x").unwrap().0, 0);
        assert_eq!(aa.usage("x").unwrap().1, 4096); // peak preserved
    }

    #[test]
    fn agent_arena_total() {
        let mut aa = AgentArena::new();
        aa.allocate("a", 100);
        aa.allocate("b", 200);
        assert_eq!(aa.total_allocated(), 300);
    }

    #[test]
    fn agent_arena_stats() {
        let mut aa = AgentArena::new();
        aa.allocate("a", 1000);
        let s = aa.stats();
        assert!(s.contains("\"agents\":1"));
        assert!(s.contains("\"total_bytes\":1000"));
    }
}

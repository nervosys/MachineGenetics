//! # Agentic Standard Library
//!
//! Primitives for swarm-based concurrent agent programming:
//! - `SwarmVec` — concurrent growable vector for agent swarms
//! - `ArenaVec` — arena-allocated vector with stable indices
//! - `SwarmChannel` — multi-producer multi-consumer message channel
//! - Streaming I/O primitives

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

// ── SwarmVec ─────────────────────────────────────────────────────────

/// A growable vector designed for swarm agent collections.
/// Supports tagging, filtering by status, and batch operations.
#[derive(Debug)]
pub struct SwarmVec<T> {
    items: Vec<SwarmEntry<T>>,
    next_id: u64,
}

/// Status of a swarm agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentStatus {
    Idle,
    Running,
    Blocked,
    Completed,
    Failed,
}

impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Running => write!(f, "running"),
            Self::Blocked => write!(f, "blocked"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// An entry in the SwarmVec.
#[derive(Debug)]
pub struct SwarmEntry<T> {
    pub id: u64,
    pub value: T,
    pub status: AgentStatus,
    pub tags: Vec<String>,
}

impl<T> SwarmVec<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            next_id: 0,
        }
    }

    pub fn spawn(&mut self, value: T) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(SwarmEntry {
            id,
            value,
            status: AgentStatus::Idle,
            tags: Vec::new(),
        });
        id
    }

    pub fn spawn_tagged(&mut self, value: T, tags: Vec<String>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(SwarmEntry {
            id,
            value,
            status: AgentStatus::Idle,
            tags,
        });
        id
    }

    pub fn get(&self, id: u64) -> Option<&SwarmEntry<T>> {
        self.items.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut SwarmEntry<T>> {
        self.items.iter_mut().find(|e| e.id == id)
    }

    pub fn set_status(&mut self, id: u64, status: AgentStatus) -> bool {
        if let Some(entry) = self.get_mut(id) {
            entry.status = status;
            true
        } else {
            false
        }
    }

    pub fn by_status(&self, status: AgentStatus) -> Vec<&SwarmEntry<T>> {
        self.items.iter().filter(|e| e.status == status).collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&SwarmEntry<T>> {
        self.items.iter().filter(|e| e.tags.iter().any(|t| t == tag)).collect()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn remove(&mut self, id: u64) -> Option<SwarmEntry<T>> {
        if let Some(pos) = self.items.iter().position(|e| e.id == id) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn drain_completed(&mut self) -> Vec<SwarmEntry<T>> {
        let mut completed = Vec::new();
        let mut i = 0;
        while i < self.items.len() {
            if self.items[i].status == AgentStatus::Completed {
                completed.push(self.items.remove(i));
            } else {
                i += 1;
            }
        }
        completed
    }

    pub fn iter(&self) -> impl Iterator<Item = &SwarmEntry<T>> {
        self.items.iter()
    }
}

impl<T> Default for SwarmVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── ArenaVec ─────────────────────────────────────────────────────────

/// Index into an ArenaVec, stable across insertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArenaIndex(usize);

impl ArenaIndex {
    pub fn raw(&self) -> usize {
        self.0
    }
}

impl fmt::Display for ArenaIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArenaIndex({})", self.0)
    }
}

/// Arena-allocated vector with stable indices. Removed slots are reused.
#[derive(Debug)]
pub struct ArenaVec<T> {
    slots: Vec<ArenaSlot<T>>,
    free_list: Vec<usize>,
    generation: Vec<u32>,
    count: usize,
}

#[derive(Debug)]
enum ArenaSlot<T> {
    Occupied(T),
    Vacant,
}

impl<T> ArenaVec<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
            generation: Vec::new(),
            count: 0,
        }
    }

    pub fn insert(&mut self, value: T) -> ArenaIndex {
        self.count += 1;
        if let Some(idx) = self.free_list.pop() {
            self.slots[idx] = ArenaSlot::Occupied(value);
            self.generation[idx] += 1;
            ArenaIndex(idx)
        } else {
            let idx = self.slots.len();
            self.slots.push(ArenaSlot::Occupied(value));
            self.generation.push(0);
            ArenaIndex(idx)
        }
    }

    pub fn get(&self, index: ArenaIndex) -> Option<&T> {
        match self.slots.get(index.0) {
            Some(ArenaSlot::Occupied(v)) => Some(v),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, index: ArenaIndex) -> Option<&mut T> {
        match self.slots.get_mut(index.0) {
            Some(ArenaSlot::Occupied(v)) => Some(v),
            _ => None,
        }
    }

    pub fn remove(&mut self, index: ArenaIndex) -> Option<T> {
        if index.0 < self.slots.len() {
            let slot = std::mem::replace(&mut self.slots[index.0], ArenaSlot::Vacant);
            match slot {
                ArenaSlot::Occupied(v) => {
                    self.free_list.push(index.0);
                    self.count -= 1;
                    Some(v)
                }
                ArenaSlot::Vacant => {
                    self.slots[index.0] = ArenaSlot::Vacant;
                    None
                }
            }
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    pub fn generation(&self, index: ArenaIndex) -> Option<u32> {
        self.generation.get(index.0).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (ArenaIndex, &T)> {
        self.slots.iter().enumerate().filter_map(|(i, slot)| {
            match slot {
                ArenaSlot::Occupied(v) => Some((ArenaIndex(i), v)),
                ArenaSlot::Vacant => None,
            }
        })
    }
}

impl<T> Default for ArenaVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── SwarmChannel ─────────────────────────────────────────────────────

/// A multi-producer multi-consumer channel for agent messaging.
#[derive(Debug)]
pub struct SwarmChannel<T> {
    buffer: VecDeque<ChannelMessage<T>>,
    capacity: Option<usize>,
    total_sent: u64,
    total_received: u64,
    closed: bool,
}

/// A message in a SwarmChannel.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelMessage<T> {
    pub payload: T,
    pub sender_id: u64,
    pub sequence: u64,
}

impl<T: fmt::Display> fmt::Display for ChannelMessage<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}->seq#{}] {}", self.sender_id, self.sequence, self.payload)
    }
}

/// Error from channel operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelError {
    Full,
    Empty,
    Closed,
}

impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "channel full"),
            Self::Empty => write!(f, "channel empty"),
            Self::Closed => write!(f, "channel closed"),
        }
    }
}

impl<T> SwarmChannel<T> {
    pub fn unbounded() -> Self {
        Self {
            buffer: VecDeque::new(),
            capacity: None,
            total_sent: 0,
            total_received: 0,
            closed: false,
        }
    }

    pub fn bounded(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity: Some(capacity),
            total_sent: 0,
            total_received: 0,
            closed: false,
        }
    }

    pub fn send(&mut self, sender_id: u64, payload: T) -> Result<u64, ChannelError> {
        if self.closed {
            return Err(ChannelError::Closed);
        }
        if let Some(cap) = self.capacity {
            if self.buffer.len() >= cap {
                return Err(ChannelError::Full);
            }
        }
        let seq = self.total_sent;
        self.total_sent += 1;
        self.buffer.push_back(ChannelMessage {
            payload,
            sender_id,
            sequence: seq,
        });
        Ok(seq)
    }

    pub fn recv(&mut self) -> Result<ChannelMessage<T>, ChannelError> {
        if let Some(msg) = self.buffer.pop_front() {
            self.total_received += 1;
            Ok(msg)
        } else if self.closed {
            Err(ChannelError::Closed)
        } else {
            Err(ChannelError::Empty)
        }
    }

    pub fn try_recv(&mut self) -> Option<ChannelMessage<T>> {
        let msg = self.buffer.pop_front()?;
        self.total_received += 1;
        Some(msg)
    }

    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn pending(&self) -> usize {
        self.buffer.len()
    }

    pub fn total_sent(&self) -> u64 {
        self.total_sent
    }

    pub fn total_received(&self) -> u64 {
        self.total_received
    }
}

// ── Streaming I/O Primitives ─────────────────────────────────────────

/// A chunk of streaming data.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    Data(Vec<u8>),
    Text(String),
    End,
    Error(String),
}

impl fmt::Display for StreamChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Data(bytes) => write!(f, "Data({} bytes)", bytes.len()),
            Self::Text(s) => write!(f, "Text({} chars)", s.len()),
            Self::End => write!(f, "End"),
            Self::Error(e) => write!(f, "Error({e})"),
        }
    }
}

/// A streaming I/O source.
#[derive(Debug)]
pub struct StreamSource {
    chunks: VecDeque<StreamChunk>,
    bytes_read: u64,
    finished: bool,
}

impl StreamSource {
    pub fn new() -> Self {
        Self {
            chunks: VecDeque::new(),
            bytes_read: 0,
            finished: false,
        }
    }

    pub fn from_chunks(chunks: Vec<StreamChunk>) -> Self {
        Self {
            chunks: VecDeque::from(chunks),
            bytes_read: 0,
            finished: false,
        }
    }

    pub fn push(&mut self, chunk: StreamChunk) {
        self.chunks.push_back(chunk);
    }

    pub fn next_chunk(&mut self) -> Option<StreamChunk> {
        let chunk = self.chunks.pop_front()?;
        match &chunk {
            StreamChunk::Data(bytes) => self.bytes_read += bytes.len() as u64,
            StreamChunk::Text(s) => self.bytes_read += s.len() as u64,
            StreamChunk::End => self.finished = true,
            StreamChunk::Error(_) => self.finished = true,
        }
        Some(chunk)
    }

    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn pending_chunks(&self) -> usize {
        self.chunks.len()
    }
}

impl Default for StreamSource {
    fn default() -> Self {
        Self::new()
    }
}

/// A streaming I/O sink.
#[derive(Debug)]
pub struct StreamSink {
    buffer: Vec<StreamChunk>,
    bytes_written: u64,
    closed: bool,
}

impl StreamSink {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            bytes_written: 0,
            closed: false,
        }
    }

    pub fn write(&mut self, chunk: StreamChunk) -> Result<(), String> {
        if self.closed {
            return Err("sink closed".into());
        }
        match &chunk {
            StreamChunk::Data(bytes) => self.bytes_written += bytes.len() as u64,
            StreamChunk::Text(s) => self.bytes_written += s.len() as u64,
            StreamChunk::End => self.closed = true,
            StreamChunk::Error(_) => self.closed = true,
        }
        self.buffer.push(chunk);
        Ok(())
    }

    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn drain(&mut self) -> Vec<StreamChunk> {
        std::mem::take(&mut self.buffer)
    }
}

impl Default for StreamSink {
    fn default() -> Self {
        Self::new()
    }
}

/// Pipe a source into a sink, forwarding all chunks.
pub fn pipe_stream(source: &mut StreamSource, sink: &mut StreamSink) -> Result<u64, String> {
    let mut transferred = 0u64;
    while let Some(chunk) = source.next_chunk() {
        let is_end = matches!(chunk, StreamChunk::End | StreamChunk::Error(_));
        match &chunk {
            StreamChunk::Data(b) => transferred += b.len() as u64,
            StreamChunk::Text(s) => transferred += s.len() as u64,
            _ => {}
        }
        sink.write(chunk)?;
        if is_end {
            break;
        }
    }
    Ok(transferred)
}

/// Global sequence counter for unique IDs.
static GLOBAL_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn next_global_id() -> u64 {
    GLOBAL_SEQ.fetch_add(1, Ordering::Relaxed)
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // SwarmVec tests
    #[test]
    fn test_swarm_vec_new() {
        let sv: SwarmVec<i32> = SwarmVec::new();
        assert!(sv.is_empty());
    }

    #[test]
    fn test_swarm_vec_spawn() {
        let mut sv = SwarmVec::new();
        let id = sv.spawn(42);
        assert_eq!(id, 0);
        assert_eq!(sv.len(), 1);
        assert_eq!(sv.get(0).unwrap().value, 42);
    }

    #[test]
    fn test_swarm_vec_spawn_tagged() {
        let mut sv = SwarmVec::new();
        let id = sv.spawn_tagged("agent", vec!["worker".into()]);
        let entry = sv.get(id).unwrap();
        assert_eq!(entry.tags, vec!["worker"]);
    }

    #[test]
    fn test_swarm_vec_status() {
        let mut sv = SwarmVec::new();
        let id = sv.spawn(1);
        assert!(sv.set_status(id, AgentStatus::Running));
        assert_eq!(sv.get(id).unwrap().status, AgentStatus::Running);
    }

    #[test]
    fn test_swarm_vec_by_status() {
        let mut sv = SwarmVec::new();
        sv.spawn(1);
        sv.spawn(2);
        let id2 = 1;
        sv.set_status(id2, AgentStatus::Running);
        assert_eq!(sv.by_status(AgentStatus::Idle).len(), 1);
        assert_eq!(sv.by_status(AgentStatus::Running).len(), 1);
    }

    #[test]
    fn test_swarm_vec_by_tag() {
        let mut sv = SwarmVec::new();
        sv.spawn_tagged(1, vec!["a".into()]);
        sv.spawn_tagged(2, vec!["b".into()]);
        sv.spawn_tagged(3, vec!["a".into()]);
        assert_eq!(sv.by_tag("a").len(), 2);
    }

    #[test]
    fn test_swarm_vec_remove() {
        let mut sv = SwarmVec::new();
        let id = sv.spawn(99);
        assert!(sv.remove(id).is_some());
        assert!(sv.is_empty());
    }

    #[test]
    fn test_swarm_vec_drain_completed() {
        let mut sv = SwarmVec::new();
        let a = sv.spawn(1);
        let _b = sv.spawn(2);
        let c = sv.spawn(3);
        sv.set_status(a, AgentStatus::Completed);
        sv.set_status(c, AgentStatus::Completed);
        let drained = sv.drain_completed();
        assert_eq!(drained.len(), 2);
        assert_eq!(sv.len(), 1);
    }

    #[test]
    fn test_swarm_vec_iter() {
        let mut sv = SwarmVec::new();
        sv.spawn(10);
        sv.spawn(20);
        let vals: Vec<_> = sv.iter().map(|e| e.value).collect();
        assert_eq!(vals, vec![10, 20]);
    }

    #[test]
    fn test_agent_status_display() {
        assert_eq!(format!("{}", AgentStatus::Running), "running");
    }

    // ArenaVec tests
    #[test]
    fn test_arena_new() {
        let a: ArenaVec<i32> = ArenaVec::new();
        assert!(a.is_empty());
    }

    #[test]
    fn test_arena_insert_get() {
        let mut a = ArenaVec::new();
        let idx = a.insert(42);
        assert_eq!(*a.get(idx).unwrap(), 42);
    }

    #[test]
    fn test_arena_remove() {
        let mut a = ArenaVec::new();
        let idx = a.insert(10);
        assert_eq!(a.remove(idx), Some(10));
        assert!(a.get(idx).is_none());
        assert!(a.is_empty());
    }

    #[test]
    fn test_arena_reuse_slot() {
        let mut a = ArenaVec::new();
        let idx1 = a.insert(1);
        a.remove(idx1);
        let idx2 = a.insert(2);
        assert_eq!(idx1.raw(), idx2.raw());
        assert_eq!(*a.get(idx2).unwrap(), 2);
    }

    #[test]
    fn test_arena_generation() {
        let mut a = ArenaVec::new();
        let idx = a.insert(1);
        assert_eq!(a.generation(idx), Some(0));
        a.remove(idx);
        let idx2 = a.insert(2);
        assert_eq!(a.generation(idx2), Some(1));
    }

    #[test]
    fn test_arena_iter() {
        let mut a = ArenaVec::new();
        a.insert(10);
        a.insert(20);
        let vals: Vec<_> = a.iter().map(|(_, v)| *v).collect();
        assert_eq!(vals, vec![10, 20]);
    }

    #[test]
    fn test_arena_index_display() {
        let idx = ArenaIndex(5);
        assert_eq!(format!("{idx}"), "ArenaIndex(5)");
    }

    // SwarmChannel tests
    #[test]
    fn test_channel_unbounded() {
        let mut ch: SwarmChannel<String> = SwarmChannel::unbounded();
        ch.send(0, "hello".into()).unwrap();
        ch.send(1, "world".into()).unwrap();
        assert_eq!(ch.pending(), 2);
        let msg = ch.recv().unwrap();
        assert_eq!(msg.payload, "hello");
    }

    #[test]
    fn test_channel_bounded_full() {
        let mut ch = SwarmChannel::bounded(1);
        ch.send(0, 42).unwrap();
        assert_eq!(ch.send(0, 43), Err(ChannelError::Full));
    }

    #[test]
    fn test_channel_recv_empty() {
        let mut ch: SwarmChannel<i32> = SwarmChannel::unbounded();
        assert_eq!(ch.recv(), Err(ChannelError::Empty));
    }

    #[test]
    fn test_channel_close() {
        let mut ch: SwarmChannel<i32> = SwarmChannel::unbounded();
        ch.close();
        assert!(ch.is_closed());
        assert_eq!(ch.send(0, 1), Err(ChannelError::Closed));
    }

    #[test]
    fn test_channel_close_recv() {
        let mut ch: SwarmChannel<i32> = SwarmChannel::unbounded();
        ch.send(0, 10).unwrap();
        ch.close();
        assert_eq!(ch.recv().unwrap().payload, 10);
        assert_eq!(ch.recv(), Err(ChannelError::Closed));
    }

    #[test]
    fn test_channel_try_recv() {
        let mut ch = SwarmChannel::unbounded();
        ch.send(0, "msg").unwrap();
        assert!(ch.try_recv().is_some());
        assert!(ch.try_recv().is_none());
    }

    #[test]
    fn test_channel_stats() {
        let mut ch = SwarmChannel::unbounded();
        ch.send(0, 1).unwrap();
        ch.send(1, 2).unwrap();
        ch.recv().unwrap();
        assert_eq!(ch.total_sent(), 2);
        assert_eq!(ch.total_received(), 1);
    }

    #[test]
    fn test_channel_message_display() {
        let msg = ChannelMessage { payload: "hi", sender_id: 3, sequence: 7 };
        let s = format!("{msg}");
        assert!(s.contains("3"));
        assert!(s.contains("7"));
    }

    #[test]
    fn test_channel_error_display() {
        assert_eq!(format!("{}", ChannelError::Full), "channel full");
    }

    // StreamSource/Sink tests
    #[test]
    fn test_stream_source_new() {
        let s = StreamSource::new();
        assert!(!s.is_finished());
        assert_eq!(s.pending_chunks(), 0);
    }

    #[test]
    fn test_stream_source_chunks() {
        let mut src = StreamSource::from_chunks(vec![
            StreamChunk::Text("hello".into()),
            StreamChunk::End,
        ]);
        let c1 = src.next_chunk().unwrap();
        assert!(matches!(c1, StreamChunk::Text(_)));
        assert_eq!(src.bytes_read(), 5);
        let c2 = src.next_chunk().unwrap();
        assert!(matches!(c2, StreamChunk::End));
        assert!(src.is_finished());
    }

    #[test]
    fn test_stream_sink_write() {
        let mut sink = StreamSink::new();
        sink.write(StreamChunk::Data(vec![1, 2, 3])).unwrap();
        assert_eq!(sink.bytes_written(), 3);
        assert!(!sink.is_closed());
    }

    #[test]
    fn test_stream_sink_closed() {
        let mut sink = StreamSink::new();
        sink.write(StreamChunk::End).unwrap();
        assert!(sink.is_closed());
        assert!(sink.write(StreamChunk::Text("x".into())).is_err());
    }

    #[test]
    fn test_stream_sink_drain() {
        let mut sink = StreamSink::new();
        sink.write(StreamChunk::Text("a".into())).unwrap();
        let chunks = sink.drain();
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_pipe_stream() {
        let mut src = StreamSource::from_chunks(vec![
            StreamChunk::Data(vec![1, 2]),
            StreamChunk::Text("hi".into()),
            StreamChunk::End,
        ]);
        let mut sink = StreamSink::new();
        let transferred = pipe_stream(&mut src, &mut sink).unwrap();
        assert_eq!(transferred, 4); // 2 bytes + 2 chars
        assert!(sink.is_closed());
    }

    #[test]
    fn test_stream_chunk_display() {
        assert_eq!(format!("{}", StreamChunk::End), "End");
        assert!(format!("{}", StreamChunk::Data(vec![0; 10])).contains("10"));
    }

    #[test]
    fn test_global_id() {
        let a = next_global_id();
        let b = next_global_id();
        assert_ne!(a, b);
    }
}

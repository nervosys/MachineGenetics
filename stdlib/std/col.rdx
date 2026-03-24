//! # std::col — Collections
//!
//! Maps, sets, ordered collections, queues, and linked lists.
//! Collections are pure (no effects) unless iterating with side-effects.

// ---------------------------------------------------------------------------
// Map (HashMap<K,V> is syntactic sugar for Map<K,V>)
// ---------------------------------------------------------------------------

/// A hash map. Written as `HashMap<K, V>` in type position.
pub struct Map<K: Hash + Eq, V> {
    _data: _MapInner<K, V>,
}

impl Map<K: Hash + Eq, V> {
    /// Create an empty map.
    pub fn new() -> HashMap<K, V> { Map { _data: _MapInner::new() } }

    /// Create a map with the given capacity.
    pub fn with_capacity(cap: usize) -> HashMap<K, V>;

    /// Insert a key-value pair. Returns the old value if present.
    pub fn insert(&mut self, key: K, value: V) -> Option<V>;

    /// Get a reference to the value for a key.
    pub fn get(&self, key: &K) -> Option<&V>;

    /// Get a mutable reference to the value for a key.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V>;

    /// Remove a key. Returns the value if present.
    pub fn remove(&mut self, key: &K) -> Option<V>;

    /// Check if a key exists.
    pub fn contains(&self, key: &K) -> bool;

    /// Number of entries.
    pub fn len(&self) -> usize;

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool { self.len() == 0 }

    /// An iterator over keys.
    pub fn keys(&self) -> MapKeys<K, V>;

    /// An iterator over values.
    pub fn values(&self) -> MapValues<K, V>;

    /// An iterator over (key, value) pairs.
    pub fn iter(&self) -> MapIter<K, V>;

    /// Remove all entries.
    pub fn clear(&mut self);
}

// ---------------------------------------------------------------------------
// Set (HashSet<K> is syntactic sugar for Set<K>)
// ---------------------------------------------------------------------------

/// A hash set. Written as `HashSet<K>` in type position.
pub struct Set<K: Hash + Eq> {
    _inner: Map<K, ()>,
}

impl Set<K: Hash + Eq> {
    pub fn new() -> HashSet<K> { Set { _inner: Map::new() } }
    pub fn with_capacity(cap: usize) -> HashSet<K>;

    pub fn insert(&mut self, value: K) -> bool;
    pub fn remove(&mut self, value: &K) -> bool;
    pub fn contains(&self, value: &K) -> bool;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool { self.len() == 0 }

    /// Set operations.
    pub fn union(&self, other: &HashSet<K>) -> HashSet<K>;
    pub fn intersection(&self, other: &HashSet<K>) -> HashSet<K>;
    pub fn difference(&self, other: &HashSet<K>) -> HashSet<K>;
    pub fn symmetric_difference(&self, other: &HashSet<K>) -> HashSet<K>;
    pub fn is_subset(&self, other: &HashSet<K>) -> bool;
    pub fn is_superset(&self, other: &HashSet<K>) -> bool;

    pub fn iter(&self) -> SetIter<K>;
    pub fn clear(&mut self);
}

// ---------------------------------------------------------------------------
// BTree — ordered map
// ---------------------------------------------------------------------------

/// An ordered map backed by a B-tree.
pub struct BTree<K: Ord, V> {
    _root: Option<_BTreeNode<K, V>>,
    _len: usize,
}

impl BTree<K: Ord, V> {
    pub fn new() -> BTree<K, V>;

    pub fn insert(&mut self, key: K, value: V) -> Option<V>;
    pub fn get(&self, key: &K) -> Option<&V>;
    pub fn remove(&mut self, key: &K) -> Option<V>;
    pub fn contains(&self, key: &K) -> bool;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool { self.len() == 0 }

    /// Iterator yields entries in sorted key order.
    pub fn iter(&self) -> BTreeIter<K, V>;

    /// Range query: entries where `range.contains(key)`.
    pub fn range(&self, range: Range<K>) -> BTreeRange<K, V>;

    pub fn first(&self) -> Option<(&K, &V)>;
    pub fn last(&self) -> Option<(&K, &V)>;
    pub fn clear(&mut self);
}

// ---------------------------------------------------------------------------
// VecDeque — double-ended queue
// ---------------------------------------------------------------------------

/// A double-ended queue backed by a growable ring buffer.
pub struct VecDeque<T> {
    _data: Vec<T>,
    _head: usize,
    _len: usize,
}

impl VecDeque<T> {
    pub fn new() -> VecDeque<T>;
    pub fn with_capacity(cap: usize) -> VecDeque<T>;

    pub fn push_front(&mut self, value: T);
    pub fn push_back(&mut self, value: T);
    pub fn pop_front(&mut self) -> Option<T>;
    pub fn pop_back(&mut self) -> Option<T>;
    pub fn front(&self) -> Option<&T>;
    pub fn back(&self) -> Option<&T>;

    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    pub fn clear(&mut self);

    pub fn iter(&self) -> VecDequeIter<T>;
}

// ---------------------------------------------------------------------------
// LinkedList — doubly-linked list
// ---------------------------------------------------------------------------

/// A doubly-linked list.
pub struct LinkedList<T> {
    _head: Option<Box<_Node<T>>>,
    _tail: Option<Box<_Node<T>>>,
    _len: usize,
}

impl LinkedList<T> {
    pub fn new() -> LinkedList<T>;

    pub fn push_front(&mut self, value: T);
    pub fn push_back(&mut self, value: T);
    pub fn pop_front(&mut self) -> Option<T>;
    pub fn pop_back(&mut self) -> Option<T>;
    pub fn front(&self) -> Option<&T>;
    pub fn back(&self) -> Option<&T>;

    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    pub fn clear(&mut self);

    pub fn iter(&self) -> LinkedListIter<T>;
}

// ---------------------------------------------------------------------------
// Placeholder iterator types
// ---------------------------------------------------------------------------

pub struct MapKeys<K, V>     { _phantom: () }
pub struct MapValues<K, V>   { _phantom: () }
pub struct MapIter<K, V>     { _phantom: () }
pub struct SetIter<K>        { _phantom: () }
pub struct BTreeIter<K, V>   { _phantom: () }
pub struct BTreeRange<K, V>  { _phantom: () }
pub struct VecDequeIter<T>   { _phantom: () }
pub struct LinkedListIter<T> { _phantom: () }

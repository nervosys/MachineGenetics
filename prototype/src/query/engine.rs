//! The generic half of the incremental engine: a demand-driven, memoised query
//! database in the style of salsa, written by hand.
//!
//! Nothing here knows about MAGE. A [`Domain`] says what the queries are
//! (`Key`), what they return (`Val`), how to run one ([`Domain::execute`]), and
//! how to fingerprint an answer. The engine does the rest:
//!
//! * **Inputs** are set with [`Db::set_input`]. Setting one to a value whose
//!   fingerprint is unchanged is a no-op; otherwise the revision counter moves.
//! * **Derived queries** are memoised. While one runs, every query it reads is
//!   recorded as a dependency edge.
//! * On a new revision a memo is not trusted: [`Db::get`] first *verifies* it by
//!   walking its dependencies, deepest first, and re-executes it only when one
//!   of them changed after the memo was last verified.
//! * **Early cutoff.** A re-executed query whose answer has the same
//!   fingerprint as before keeps its old `changed_at`, so everything that read
//!   it is still valid and is not run again.
//! * **Cycles** are an `Err(QueryError::Cycle)` naming the path, not a stack
//!   overflow.
//! * An optional [`Store`] makes answers durable. A persistable query names a
//!   content address ([`Domain::address`]); on a miss the engine looks it up
//!   before executing, and after executing it writes the answer back.
//!
//! The engine is single-threaded (`!Sync`) by construction: memo tables sit in
//! `RefCell`s, and no borrow is held across a call into the domain.

use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::{self, Debug};
use std::hash::Hash;
use std::time::Instant;

/// A SHA-256 fingerprint.
pub type Hash32 = [u8; 32];

/// Lower-case hex of a fingerprint.
pub fn hex(h: &Hash32) -> String {
    let mut s = String::with_capacity(64);
    for b in h {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// SHA-256 of the concatenation of `parts`, each length-prefixed so that
/// `["ab", "c"]` and `["a", "bc"]` differ.
pub fn digest(parts: &[&[u8]]) -> Hash32 {
    let mut h = Sha256::new();
    for p in parts {
        h.update((p.len() as u64).to_le_bytes());
        h.update(p);
    }
    h.finalize().into()
}

/// Why a query could not be answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// A query (transitively) read itself. Holds the path, outermost first,
    /// ending with the query that closed the loop.
    Cycle(Vec<String>),
    /// An input was read before it was set.
    NoInput(String),
    /// The domain could not answer, with its reason.
    Failed(String),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryError::Cycle(path) => write!(f, "query cycle: {}", path.join(" -> ")),
            QueryError::NoInput(k) => write!(f, "input not set: {k}"),
            QueryError::Failed(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for QueryError {}

/// What a [`Store`] found for an address.
pub enum Loaded {
    /// A well-formed record: the answer's fingerprint and its encoding.
    Hit { hash: Hash32, payload: String },
    /// Nothing there.
    Miss,
    /// Something was there and it failed verification. It is never used.
    Corrupt,
}

/// Durable storage for persistable answers.
pub trait Store {
    fn load(&self, query: &str, key: &Hash32) -> Loaded;
    /// Best effort: a store that cannot write is a store that always misses.
    fn save(&self, query: &str, key: &Hash32, hash: &Hash32, payload: &str) -> bool;
    /// Make everything saved so far durable. Stores flush when dropped too, but
    /// a process that exits without unwinding must call this. Returns whether
    /// everything is on disk.
    fn flush(&self) -> bool {
        true
    }
}

/// What a query domain provides.
pub trait Domain: Sized {
    type Key: Clone + Eq + Hash + Debug;
    type Val: Clone;
    /// Context the domain's `execute` may consult (pass registry, ...).
    type Ctx;

    /// The query's name, for statistics and as part of its cache address.
    fn query_name(db: &Db<Self>, key: &Self::Key) -> &'static str;
    /// Whether `key` names an input (set, never computed).
    fn is_input(key: &Self::Key) -> bool;
    /// Compute `key`, reading other queries through `db`.
    fn execute(db: &Db<Self>, key: &Self::Key) -> Result<Self::Val, QueryError>;
    /// Fingerprint of an answer. Two answers with one fingerprint are
    /// interchangeable: it is what early cutoff compares, so it must cover
    /// every field a dependent could read.
    fn value_hash(key: &Self::Key, val: &Self::Val) -> Hash32;
    /// The content address of `key`'s answer, from the fingerprints of its
    /// inputs, or `None` when the query is not persistable. Reads go through
    /// `db`, so what the address read is recorded as the memo's dependencies.
    fn address(db: &Db<Self>, key: &Self::Key) -> Result<Option<Hash32>, QueryError> {
        let _ = (db, key);
        Ok(None)
    }
    /// Serialise an answer; `None` if this query's answers do not serialise.
    fn encode(key: &Self::Key, val: &Self::Val) -> Option<String> {
        let _ = (key, val);
        None
    }
    /// Inverse of [`Domain::encode`]; `None` on a payload it cannot read.
    fn decode(key: &Self::Key, payload: &str) -> Option<Self::Val> {
        let _ = (key, payload);
        None
    }
}

// ── Statistics ───────────────────────────────────────────────────────

/// Counters for one query name.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct QueryStats {
    /// Answered from a memo without executing (same revision, or the memo's
    /// dependencies were verified unchanged).
    pub hits: u64,
    /// The subset of `hits` that needed a dependency walk (a new revision).
    pub revalidated: u64,
    /// No memo existed and the query was executed.
    pub misses: u64,
    /// A memo existed, a dependency had changed, and the query was executed.
    pub reexecutions: u64,
    /// A re-executed or reloaded query produced the fingerprint it had
    /// before, so nothing that read it had to run.
    pub early_cutoffs: u64,
    /// Answered from the persistent store.
    pub disk_hits: u64,
    /// Persistable, and the store had nothing for the address.
    pub disk_misses: u64,
    /// Answers written to the store.
    pub disk_stores: u64,
    /// Store records that failed verification (ignored and recomputed).
    pub disk_corrupt: u64,
    /// Persistable answers that did not survive encode -> decode unchanged,
    /// and so were not stored.
    pub unpersistable: u64,
    /// Time spent executing this query's own code, children excluded.
    pub exec_nanos: u64,
}

/// One execution (or reload) the engine did, for an agent to read back.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Event {
    pub query: &'static str,
    pub key: String,
    /// `"miss"`, `"reexecute"` or `"disk"`.
    pub why: &'static str,
}

/// Everything the engine counted since the last [`Db::reset_stats`].
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Stats {
    pub revision: u64,
    pub queries: BTreeMap<&'static str, QueryStats>,
    pub events: Vec<Event>,
    /// Events past the cap that were counted but not listed.
    pub events_dropped: u64,
}

const EVENT_CAP: usize = 20_000;

impl Stats {
    pub fn total(&self) -> QueryStats {
        let mut t = QueryStats::default();
        for q in self.queries.values() {
            t.hits += q.hits;
            t.revalidated += q.revalidated;
            t.misses += q.misses;
            t.reexecutions += q.reexecutions;
            t.early_cutoffs += q.early_cutoffs;
            t.disk_hits += q.disk_hits;
            t.disk_misses += q.disk_misses;
            t.disk_stores += q.disk_stores;
            t.disk_corrupt += q.disk_corrupt;
            t.unpersistable += q.unpersistable;
            t.exec_nanos += q.exec_nanos;
        }
        t
    }

    /// How many times `query` ran its own code (misses plus re-executions).
    pub fn executed(&self, query: &str) -> u64 {
        self.queries.get(query).map_or(0, |q| q.misses + q.reexecutions)
    }
}

// ── The database ─────────────────────────────────────────────────────

enum Slot<V> {
    Ready(V),
    /// Loaded from the store and not decoded yet: most answers on a warm run
    /// are only ever fingerprinted.
    Encoded(String),
}

struct Memo<K, V> {
    slot: Slot<V>,
    hash: Hash32,
    deps: Vec<K>,
    /// The last revision at which this memo was known to be current.
    verified_at: u64,
    /// The last revision at which its answer actually changed.
    changed_at: u64,
}

struct InputSlot<V> {
    val: V,
    hash: Hash32,
    changed_at: u64,
}

struct Frame<K> {
    key: K,
    /// Whether this frame is executing (records dependencies) or verifying.
    records: bool,
    deps: Vec<K>,
    seen: HashSet<K>,
    start: Instant,
    child_nanos: u64,
}

/// The query database.
pub struct Db<D: Domain> {
    /// The domain's context.
    pub ctx: D::Ctx,
    revision: Cell<u64>,
    inputs: RefCell<HashMap<D::Key, InputSlot<D::Val>>>,
    memos: RefCell<HashMap<D::Key, Memo<D::Key, D::Val>>>,
    stack: RefCell<Vec<Frame<D::Key>>>,
    stats: RefCell<Stats>,
    store: Option<Box<dyn Store>>,
    build_id: String,
}

impl<D: Domain> Db<D> {
    pub fn new(ctx: D::Ctx) -> Self {
        Db {
            ctx,
            revision: Cell::new(1),
            inputs: RefCell::new(HashMap::new()),
            memos: RefCell::new(HashMap::new()),
            stack: RefCell::new(Vec::new()),
            stats: RefCell::new(Stats::default()),
            store: None,
            build_id: String::new(),
        }
    }

    /// Attach durable storage. `build_id` is mixed into every address, so a
    /// different compiler cannot read this one's answers.
    pub fn with_store(mut self, store: Box<dyn Store>, build_id: String) -> Self {
        self.store = Some(store);
        self.build_id = build_id;
        self
    }

    pub fn has_store(&self) -> bool {
        self.store.is_some()
    }

    /// Flush the persistent store, if there is one.
    pub fn flush(&self) -> bool {
        self.store.as_ref().is_none_or(|s| s.flush())
    }

    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    pub fn stats(&self) -> Stats {
        let mut s = self.stats.borrow().clone();
        s.revision = self.revision.get();
        s
    }

    pub fn reset_stats(&self) {
        *self.stats.borrow_mut() = Stats::default();
    }

    /// Set an input. Returns whether it changed (and so moved the revision).
    pub fn set_input(&self, key: D::Key, val: D::Val) -> bool {
        debug_assert!(D::is_input(&key), "{key:?} is not an input");
        let hash = D::value_hash(&key, &val);
        let mut inputs = self.inputs.borrow_mut();
        if let Some(slot) = inputs.get(&key)
            && slot.hash == hash
        {
            return false;
        }
        let rev = self.revision.get() + 1;
        self.revision.set(rev);
        inputs.insert(key, InputSlot { val, hash, changed_at: rev });
        true
    }

    /// Read a query's answer, recording the read as a dependency of whatever is
    /// executing.
    pub fn get(&self, key: &D::Key) -> Result<D::Val, QueryError> {
        self.record_dep(key);
        if D::is_input(key) {
            return self.read_input(key).map(|(v, _, _)| v);
        }
        self.update(key)?;
        self.value_of(key)
    }

    /// Read only the fingerprint of a query's answer. A persisted answer is not
    /// decoded to give it.
    pub fn get_hash(&self, key: &D::Key) -> Result<Hash32, QueryError> {
        self.record_dep(key);
        if D::is_input(key) {
            return self.read_input(key).map(|(_, h, _)| h);
        }
        self.update(key)?;
        Ok(self.memos.borrow().get(key).expect("updated").hash)
    }

    /// Whether `key` currently has a memo (test and diagnostics aid).
    pub fn is_memoised(&self, key: &D::Key) -> bool {
        self.memos.borrow().contains_key(key)
    }

    fn read_input(&self, key: &D::Key) -> Result<(D::Val, Hash32, u64), QueryError> {
        match self.inputs.borrow().get(key) {
            Some(s) => Ok((s.val.clone(), s.hash, s.changed_at)),
            None => Err(QueryError::NoInput(format!("{key:?}"))),
        }
    }

    fn record_dep(&self, key: &D::Key) {
        let mut stack = self.stack.borrow_mut();
        if let Some(top) = stack.last_mut()
            && top.records
            && top.seen.insert(key.clone())
        {
            top.deps.push(key.clone());
        }
    }

    fn cycle_path(&self, key: &D::Key) -> Vec<String> {
        let stack = self.stack.borrow();
        let from = stack.iter().position(|f| &f.key == key).unwrap_or(0);
        let mut path: Vec<String> = stack[from..].iter().map(|f| format!("{:?}", f.key)).collect();
        path.push(format!("{key:?}"));
        path
    }

    fn on_stack(&self, key: &D::Key) -> bool {
        self.stack.borrow().iter().any(|f| &f.key == key)
    }

    fn with_stats(&self, key: &D::Key, f: impl FnOnce(&mut QueryStats)) {
        let mut s = self.stats.borrow_mut();
        f(s.queries.entry(D::query_name(self, key)).or_default());
    }

    fn event(&self, key: &D::Key, why: &'static str) {
        let mut s = self.stats.borrow_mut();
        if s.events.len() < EVENT_CAP {
            s.events.push(Event { query: D::query_name(self, key), key: format!("{key:?}"), why });
        } else {
            s.events_dropped += 1;
        }
    }

    /// The revision at which `key`'s answer last changed, after making sure it
    /// is current.
    fn changed_at(&self, key: &D::Key) -> Result<u64, QueryError> {
        if D::is_input(key) {
            return self.read_input(key).map(|(_, _, c)| c);
        }
        self.update(key)?;
        Ok(self.memos.borrow().get(key).expect("updated").changed_at)
    }

    /// Bring `key`'s memo up to date: verify it, or run it.
    fn update(&self, key: &D::Key) -> Result<(), QueryError> {
        if self.on_stack(key) {
            return Err(QueryError::Cycle(self.cycle_path(key)));
        }
        let rev = self.revision.get();
        let state = self.memos.borrow().get(key).map(|m| (m.verified_at, m.deps.clone()));
        match state {
            Some((verified, _)) if verified == rev => {
                self.with_stats(key, |q| q.hits += 1);
                Ok(())
            }
            Some((verified, deps)) => {
                // Deep verification: a memo is current if nothing it read has
                // changed since it was last known to be current. Walking each
                // dependency verifies *its* dependencies first, so the
                // deepest queries are settled before the ones above them. A
                // dependency that fails to answer counts as changed.
                self.stack.borrow_mut().push(Frame {
                    key: key.clone(),
                    records: false,
                    deps: Vec::new(),
                    seen: HashSet::new(),
                    start: Instant::now(),
                    child_nanos: 0,
                });
                let mut valid = true;
                for dep in &deps {
                    match self.changed_at(dep) {
                        Ok(c) if c <= verified => {}
                        _ => {
                            valid = false;
                            break;
                        }
                    }
                }
                self.stack.borrow_mut().pop();
                if valid {
                    if let Some(m) = self.memos.borrow_mut().get_mut(key) {
                        m.verified_at = rev;
                    }
                    self.with_stats(key, |q| {
                        q.hits += 1;
                        q.revalidated += 1;
                    });
                    Ok(())
                } else {
                    self.recompute(key, true, true)
                }
            }
            None => self.recompute(key, false, true),
        }
    }

    /// Run `key` (or load it from the store) and install the memo.
    fn recompute(&self, key: &D::Key, had_old: bool, use_store: bool) -> Result<(), QueryError> {
        let rev = self.revision.get();
        self.stack.borrow_mut().push(Frame {
            key: key.clone(),
            records: true,
            deps: Vec::new(),
            seen: HashSet::new(),
            start: Instant::now(),
            child_nanos: 0,
        });

        let outcome = self.compute_in_frame(key, had_old, use_store);

        let frame = self.stack.borrow_mut().pop().expect("frame pushed above");
        let elapsed = frame.start.elapsed().as_nanos() as u64;
        if let Some(parent) = self.stack.borrow_mut().last_mut() {
            parent.child_nanos += elapsed;
        }
        let (slot, hash) = outcome?;
        let own = elapsed.saturating_sub(frame.child_nanos);
        self.with_stats(key, |q| q.exec_nanos += own);

        let mut memos = self.memos.borrow_mut();
        let (changed_at, slot) = match memos.remove(key) {
            Some(old) if had_old && old.hash == hash => {
                self.with_stats(key, |q| q.early_cutoffs += 1);
                // Same answer: keep the old `changed_at`, and the old value
                // unless it is still an undecoded record (which is exactly
                // what a forced recompute is replacing).
                let keep = match old.slot {
                    Slot::Ready(v) => Slot::Ready(v),
                    Slot::Encoded(_) => slot,
                };
                (old.changed_at, keep)
            }
            _ => (rev, slot),
        };
        memos.insert(
            key.clone(),
            Memo { slot, hash, deps: frame.deps, verified_at: rev, changed_at },
        );
        Ok(())
    }

    #[allow(clippy::type_complexity)]
    fn compute_in_frame(
        &self,
        key: &D::Key,
        had_old: bool,
        use_store: bool,
    ) -> Result<(Slot<D::Val>, Hash32), QueryError> {
        let name = D::query_name(self, key);
        let mut address: Option<Hash32> = None;
        if use_store && self.store.is_some() {
            if let Some(addr) = D::address(self, key)? {
                let file_key = digest(&[b"mage-query-v1", name.as_bytes(), self.build_id.as_bytes(), &addr]);
                match self.store.as_ref().expect("checked").load(name, &file_key) {
                    Loaded::Hit { hash, payload } => {
                        self.with_stats(key, |q| q.disk_hits += 1);
                        self.event(key, "disk");
                        return Ok((Slot::Encoded(payload), hash));
                    }
                    Loaded::Miss => self.with_stats(key, |q| q.disk_misses += 1),
                    Loaded::Corrupt => self.with_stats(key, |q| {
                        q.disk_corrupt += 1;
                        q.disk_misses += 1;
                    }),
                }
                address = Some(file_key);
            }
        }

        self.with_stats(key, |q| {
            if had_old {
                q.reexecutions += 1;
            } else {
                q.misses += 1;
            }
        });
        self.event(key, if had_old { "reexecute" } else { "miss" });
        let val = D::execute(self, key)?;
        let hash = D::value_hash(key, &val);

        if let (Some(file_key), Some(store)) = (address, self.store.as_ref()) {
            match D::encode(key, &val) {
                // An answer is stored only if it comes back as itself: a
                // serde round trip that drops a field would otherwise serve a
                // subtly different answer on every warm run.
                Some(payload)
                    if D::decode(key, &payload).is_some_and(|back| D::value_hash(key, &back) == hash) =>
                {
                    if store.save(name, &file_key, &hash, &payload) {
                        self.with_stats(key, |q| q.disk_stores += 1);
                    }
                }
                _ => self.with_stats(key, |q| q.unpersistable += 1),
            }
        }
        Ok((Slot::Ready(val), hash))
    }

    /// The memo's answer, decoding a stored one on first use.
    fn value_of(&self, key: &D::Key) -> Result<D::Val, QueryError> {
        let encoded = {
            let memos = self.memos.borrow();
            match &memos.get(key).expect("updated").slot {
                Slot::Ready(v) => return Ok(v.clone()),
                Slot::Encoded(p) => p.clone(),
            }
        };
        match D::decode(key, &encoded) {
            Some(v) => {
                if let Some(m) = self.memos.borrow_mut().get_mut(key) {
                    m.slot = Slot::Ready(v.clone());
                }
                Ok(v)
            }
            None => {
                // The record passed its integrity check and still would not
                // decode: written by a build that disagrees about the format.
                // Treat it as absent and compute the answer.
                self.with_stats(key, |q| q.disk_corrupt += 1);
                if self.on_stack(key) {
                    return Err(QueryError::Cycle(self.cycle_path(key)));
                }
                self.recompute(key, true, false)?;
                match &self.memos.borrow().get(key).expect("recomputed").slot {
                    Slot::Ready(v) => Ok(v.clone()),
                    Slot::Encoded(_) => Err(QueryError::Failed("recomputed answer is still encoded".into())),
                }
            }
        }
    }
}

// ── Tests, over a toy domain ─────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    /// `a`, `b` inputs; `Parity(a)` collapses many `a`s to one answer; `Twice`
    /// reads `Parity`; `Sum` reads both inputs; `Loop1`/`Loop2` read each other.
    #[derive(Clone, PartialEq, Eq, Hash, Debug)]
    enum K {
        A,
        B,
        Parity,
        Twice,
        Sum,
        Loop1,
        Loop2,
        Disk,
    }

    struct Toy;

    thread_local! {
        static RUNS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    }

    fn ran() -> Vec<&'static str> {
        RUNS.with(|r| std::mem::take(&mut *r.borrow_mut()))
    }

    impl Domain for Toy {
        type Key = K;
        type Val = i64;
        type Ctx = ();
        fn query_name(_: &Db<Toy>, key: &K) -> &'static str {
            match key {
                K::A => "a",
                K::B => "b",
                K::Parity => "parity",
                K::Twice => "twice",
                K::Sum => "sum",
                K::Loop1 => "loop1",
                K::Loop2 => "loop2",
                K::Disk => "disk",
            }
        }
        fn is_input(key: &K) -> bool {
            matches!(key, K::A | K::B)
        }
        fn execute(db: &Db<Toy>, key: &K) -> Result<i64, QueryError> {
            RUNS.with(|r| r.borrow_mut().push(Self::query_name(db, key)));
            match key {
                K::Parity => Ok(db.get(&K::A)? % 2),
                K::Twice => Ok(db.get(&K::Parity)? * 2),
                K::Sum => Ok(db.get(&K::A)? + db.get(&K::B)?),
                K::Loop1 => db.get(&K::Loop2),
                K::Loop2 => db.get(&K::Loop1),
                K::Disk => Ok(db.get(&K::A)? * 10),
                K::A | K::B => unreachable!(),
            }
        }
        fn value_hash(_: &K, v: &i64) -> Hash32 {
            digest(&[&v.to_le_bytes()])
        }
        fn address(db: &Db<Toy>, key: &K) -> Result<Option<Hash32>, QueryError> {
            match key {
                K::Disk => Ok(Some(db.get_hash(&K::A)?)),
                _ => Ok(None),
            }
        }
        fn encode(_: &K, v: &i64) -> Option<String> {
            Some(v.to_string())
        }
        fn decode(_: &K, p: &str) -> Option<i64> {
            p.parse().ok()
        }
    }

    fn db() -> Db<Toy> {
        let db = Db::<Toy>::new(());
        db.set_input(K::A, 2);
        db.set_input(K::B, 5);
        ran();
        db
    }

    #[test]
    fn an_unchanged_query_is_a_memo_hit() {
        let db = db();
        assert_eq!(db.get(&K::Sum).unwrap(), 7);
        assert_eq!(ran(), vec!["sum"]);
        assert_eq!(db.get(&K::Sum).unwrap(), 7);
        assert!(ran().is_empty(), "second read must not execute");
        let s = db.stats();
        assert_eq!(s.queries["sum"].misses, 1);
        assert_eq!(s.queries["sum"].hits, 1);
    }

    #[test]
    fn setting_an_input_to_the_same_value_changes_nothing() {
        let db = db();
        db.get(&K::Sum).unwrap();
        ran();
        let rev = db.revision();
        assert!(!db.set_input(K::A, 2));
        assert_eq!(db.revision(), rev);
        db.get(&K::Sum).unwrap();
        assert!(ran().iter().all(|q| *q != "sum"), "no re-run after a no-op set");
    }

    #[test]
    fn a_changed_input_re_executes_only_its_dependents() {
        let db = db();
        db.get(&K::Sum).unwrap();
        db.get(&K::Twice).unwrap();
        ran();
        db.set_input(K::B, 6);
        assert_eq!(db.get(&K::Sum).unwrap(), 8);
        assert_eq!(ran(), vec!["sum"]);
        // `Twice` does not read `b`: verified, not run.
        assert_eq!(db.get(&K::Twice).unwrap(), 0);
        assert!(ran().is_empty());
        assert_eq!(db.stats().queries["twice"].revalidated, 1);
    }

    #[test]
    fn early_cutoff_stops_at_the_query_whose_answer_did_not_change() {
        let db = db();
        db.get(&K::Twice).unwrap();
        ran();
        db.reset_stats();
        // 2 -> 4: `a` changed, `parity` is 0 either way.
        db.set_input(K::A, 4);
        assert_eq!(db.get(&K::Twice).unwrap(), 0);
        assert_eq!(ran(), vec!["parity"], "parity re-ran; twice was cut off");
        let s = db.stats();
        assert_eq!(s.queries["parity"].reexecutions, 1);
        assert_eq!(s.queries["parity"].early_cutoffs, 1);
        assert_eq!(s.executed("twice"), 0);
        assert_eq!(s.queries["twice"].revalidated, 1);
        // And a change that does reach `parity`'s answer runs `twice` too.
        db.set_input(K::A, 5);
        assert_eq!(db.get(&K::Twice).unwrap(), 2);
        assert_eq!(ran(), vec!["parity", "twice"]);
    }

    #[test]
    fn a_cycle_is_an_error_not_a_stack_overflow() {
        let db = db();
        match db.get(&K::Loop1) {
            Err(QueryError::Cycle(path)) => {
                assert_eq!(path.first().map(String::as_str), Some("Loop1"));
                assert_eq!(path.last().map(String::as_str), Some("Loop1"));
                assert!(path.iter().any(|p| p == "Loop2"), "{path:?}");
            }
            other => panic!("expected a cycle, got {other:?}"),
        }
        // The engine is usable afterwards: the failed frames were popped.
        assert_eq!(db.get(&K::Sum).unwrap(), 7);
    }

    #[test]
    fn reading_an_unset_input_is_an_error() {
        let db = Db::<Toy>::new(());
        assert!(matches!(db.get(&K::Sum), Err(QueryError::NoInput(_))));
    }

    /// An in-memory store that can be made to lie.
    #[derive(Default)]
    struct Mem {
        map: RefCell<HashMap<Hash32, (Hash32, String)>>,
        corrupt: Cell<bool>,
    }
    struct MemStore(Rc<Mem>);
    impl Store for MemStore {
        fn load(&self, _: &str, key: &Hash32) -> Loaded {
            if self.0.corrupt.get() && self.0.map.borrow().contains_key(key) {
                return Loaded::Corrupt;
            }
            match self.0.map.borrow().get(key) {
                Some((h, p)) => Loaded::Hit { hash: *h, payload: p.clone() },
                None => Loaded::Miss,
            }
        }
        fn save(&self, _: &str, key: &Hash32, hash: &Hash32, payload: &str) -> bool {
            self.0.map.borrow_mut().insert(*key, (*hash, payload.to_string()));
            true
        }
    }

    fn stored(mem: &Rc<Mem>, build: &str) -> Db<Toy> {
        let db = Db::<Toy>::new(()).with_store(Box::new(MemStore(mem.clone())), build.into());
        db.set_input(K::A, 3);
        ran();
        db
    }

    #[test]
    fn a_store_answers_a_fresh_engine_without_executing() {
        let mem = Rc::new(Mem::default());
        let first = stored(&mem, "v1");
        assert_eq!(first.get(&K::Disk).unwrap(), 30);
        assert_eq!(ran(), vec!["disk"]);
        assert_eq!(first.stats().queries["disk"].disk_stores, 1);

        let second = stored(&mem, "v1");
        assert_eq!(second.get(&K::Disk).unwrap(), 30);
        assert!(ran().is_empty(), "served from the store");
        assert_eq!(second.stats().queries["disk"].disk_hits, 1);

        // A different build id is a different address.
        let other = stored(&mem, "v2");
        other.get(&K::Disk).unwrap();
        assert_eq!(ran(), vec!["disk"]);
    }

    #[test]
    fn a_corrupt_record_is_recomputed_not_trusted() {
        let mem = Rc::new(Mem::default());
        stored(&mem, "v1").get(&K::Disk).unwrap();
        ran();
        mem.corrupt.set(true);
        let db = stored(&mem, "v1");
        assert_eq!(db.get(&K::Disk).unwrap(), 30);
        assert_eq!(ran(), vec!["disk"]);
        let q = &db.stats().queries["disk"];
        assert_eq!(q.disk_corrupt, 1);
        assert_eq!(q.disk_hits, 0);
    }

    #[test]
    fn a_payload_that_will_not_decode_is_recomputed() {
        let mem = Rc::new(Mem::default());
        stored(&mem, "v1").get(&K::Disk).unwrap();
        for (_, p) in mem.map.borrow_mut().values_mut() {
            *p = "not a number".into();
        }
        ran();
        let db = stored(&mem, "v1");
        assert_eq!(db.get(&K::Disk).unwrap(), 30);
        assert_eq!(ran(), vec!["disk"]);
        assert_eq!(db.stats().queries["disk"].disk_corrupt, 1);
    }

    #[test]
    fn a_store_hit_still_early_cuts_off_dependents_after_an_edit() {
        let mem = Rc::new(Mem::default());
        let db = stored(&mem, "v1");
        db.get(&K::Disk).unwrap(); // a = 3, stored
        db.set_input(K::A, 4);
        db.get(&K::Disk).unwrap(); // a = 4, stored
        ran();
        // Back to the first content: the store has it, nothing executes.
        db.set_input(K::A, 3);
        assert_eq!(db.get(&K::Disk).unwrap(), 30);
        assert!(ran().is_empty());
    }
}

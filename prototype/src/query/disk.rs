//! The on-disk answer store, and the identity of the compiler that wrote it.
//!
//! # Layout
//!
//! A cache directory holds **segments**, `<nanos>-<pid>-<n>.seg`: each one the
//! answers a single run produced, one per line,
//!
//! ```text
//! <64 hex: the content address> <space> <a JSON record>
//! ```
//!
//! A run reads every segment once, when it opens the cache, into an index (the
//! newest line for an address wins), and writes one new segment when it is
//! flushed. It does not write a file per answer. That is the design's whole
//! point: an answer is a few hundred bytes, creating a file costs milliseconds
//! on the filesystems this runs on, and a 400-function module has two thousand
//! answers. (A file per answer measured 24 s to write and 1 s to read, against
//! 0.3 s to recompute the lot.)
//!
//! Segments are written to a temporary name and renamed into place, so a reader
//! never sees half a segment, and they are never modified afterwards, so two
//! processes never contend for a file. When a run finds more than
//! [`COMPACT_AT`] segments it merges them into one and removes the originals.
//!
//! # Trust
//!
//! The record carries its own address, the query's name, the answer's
//! fingerprint, and an integrity hash over all of it. A line is *used* only if
//! every one of those checks out; a line that fails any of them — truncated,
//! edited, copied from another address, written by another record format — is
//! corrupt, is ignored, and the answer is recomputed (and a good record
//! appended, which then shadows the bad one). Bytes that are not UTF-8 lose
//! only the line they fall in.
//!
//! The integrity hash guards against accident, not against an adversary: someone
//! who can write the cache directory can forge a record with a valid hash. The
//! directory is as trusted as the compiler binary beside it.
//!
//! # Limits
//!
//! The whole cache is held in memory while a run is open, and nothing is ever
//! evicted: a cache grows with every distinct input. Delete the directory to
//! reset it.

use super::engine::{Hash32, Loaded, Store, digest, hex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Bumped when the record layout (or any serialised answer) changes shape.
const RECORD_FORMAT: u32 = 1;

/// More segments than this at open time are merged into one.
pub const COMPACT_AT: usize = 8;

#[derive(Serialize, Deserialize)]
struct Record {
    format: u32,
    query: String,
    key: String,
    hash: String,
    payload: String,
    /// SHA-256 over every other field.
    check: String,
}

fn check_of(format: u32, query: &str, key: &str, hash: &str, payload: &str) -> String {
    hex(&digest(&[
        &format.to_le_bytes(),
        query.as_bytes(),
        key.as_bytes(),
        hash.as_bytes(),
        payload.as_bytes(),
    ]))
}

fn unhex(s: &str) -> Option<Hash32> {
    if s.len() != 64 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

struct State {
    /// Address -> record text, as read or as saved this run.
    index: HashMap<Hash32, String>,
    /// Saved this run and not yet on disk, in order.
    pending: Vec<(Hash32, String)>,
    /// Segments written by this handle, to keep names unique.
    written: u64,
}

/// A directory of answers.
pub struct DiskCache {
    dir: PathBuf,
    state: Mutex<State>,
}

impl DiskCache {
    /// Open (creating if need be) the cache at `dir`, reading every segment.
    pub fn open(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        let segments = segment_files_in(&dir);
        let mut index: HashMap<Hash32, String> = HashMap::new();
        for seg in &segments {
            let Ok(bytes) = std::fs::read(seg) else { continue };
            for line in bytes.split(|b| *b == b'\n') {
                // `<64 hex> <record>`; anything else on a line is not ours.
                if line.len() <= 65 || line[64] != b' ' {
                    continue;
                }
                let (Ok(key), Ok(text)) = (std::str::from_utf8(&line[..64]), std::str::from_utf8(&line[65..])) else {
                    continue;
                };
                if let Some(key) = unhex(key) {
                    index.insert(key, text.to_string());
                }
            }
        }
        let cache = DiskCache { dir, state: Mutex::new(State { index, pending: Vec::new(), written: 0 }) };
        if segments.len() > COMPACT_AT {
            cache.compact(&segments);
        }
        Ok(cache)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The segments on disk now, oldest first.
    pub fn segment_files(&self) -> Vec<PathBuf> {
        segment_files_in(&self.dir)
    }

    /// How many answers the index holds (on disk and pending).
    pub fn len(&self) -> usize {
        self.state.lock().expect("cache lock").index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn segment_name(&self, written: u64) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        self.dir.join(format!("{nanos:039}-{:08x}-{written}.seg", std::process::id()))
    }

    /// Write `lines` as one new segment; its path if it is on disk.
    fn write_segment(&self, lines: &[(Hash32, &str)], written: u64) -> Option<PathBuf> {
        let mut text = String::new();
        for (key, rec) in lines {
            text.push_str(&hex(key));
            text.push(' ');
            text.push_str(rec);
            text.push('\n');
        }
        let path = self.segment_name(written);
        let tmp = self.dir.join(format!(".tmp-{}-{written}", std::process::id()));
        if std::fs::write(&tmp, text).is_err() {
            return None;
        }
        if std::fs::rename(&tmp, &path).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return None;
        }
        Some(path)
    }

    /// Merge `old` into one segment holding the index, then remove them.
    fn compact(&self, old: &[PathBuf]) {
        let mut state = self.state.lock().expect("cache lock");
        let mut lines: Vec<(Hash32, &str)> = state.index.iter().map(|(k, v)| (*k, v.as_str())).collect();
        lines.sort_by_key(|(k, _)| *k);
        let written = state.written;
        let Some(merged) = self.write_segment(&lines, written) else { return };
        state.written += 1;
        for seg in old {
            if *seg != merged {
                let _ = std::fs::remove_file(seg);
            }
        }
    }
}

fn segment_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "seg"))
        .collect();
    // Names start with a zero-padded timestamp: name order is age order.
    out.sort();
    out
}

impl Store for DiskCache {
    fn load(&self, query: &str, key: &Hash32) -> Loaded {
        let state = self.state.lock().expect("cache lock");
        let Some(text) = state.index.get(key) else { return Loaded::Miss };
        let Ok(rec) = serde_json::from_str::<Record>(text) else { return Loaded::Corrupt };
        let key_hex = hex(key);
        if rec.format != RECORD_FORMAT
            || rec.query != query
            || rec.key != key_hex
            || rec.check != check_of(rec.format, &rec.query, &rec.key, &rec.hash, &rec.payload)
        {
            return Loaded::Corrupt;
        }
        match unhex(&rec.hash) {
            Some(hash) => Loaded::Hit { hash, payload: rec.payload },
            None => Loaded::Corrupt,
        }
    }

    fn save(&self, query: &str, key: &Hash32, hash: &Hash32, payload: &str) -> bool {
        let key_hex = hex(key);
        let hash_hex = hex(hash);
        let rec = Record {
            format: RECORD_FORMAT,
            query: query.to_string(),
            check: check_of(RECORD_FORMAT, query, &key_hex, &hash_hex, payload),
            key: key_hex,
            hash: hash_hex,
            payload: payload.to_string(),
        };
        // `to_string` never emits a raw newline: the record is one line.
        let Ok(text) = serde_json::to_string(&rec) else { return false };
        let mut state = self.state.lock().expect("cache lock");
        if state.index.get(key) == Some(&text) {
            return true; // already there, byte for byte
        }
        state.index.insert(*key, text.clone());
        state.pending.push((*key, text));
        true
    }

    fn flush(&self) -> bool {
        let mut state = self.state.lock().expect("cache lock");
        if state.pending.is_empty() {
            return true;
        }
        let lines: Vec<(Hash32, &str)> = state.pending.iter().map(|(k, t)| (*k, t.as_str())).collect();
        let written = state.written;
        if self.write_segment(&lines, written).is_none() {
            return false;
        }
        state.written += 1;
        state.pending.clear();
        true
    }
}

impl Drop for DiskCache {
    fn drop(&mut self) {
        let _ = Store::flush(self);
    }
}

impl<T: Store + ?Sized> Store for std::sync::Arc<T> {
    fn load(&self, query: &str, key: &Hash32) -> Loaded {
        (**self).load(query, key)
    }
    fn save(&self, query: &str, key: &Hash32, hash: &Hash32, payload: &str) -> bool {
        (**self).save(query, key, hash, payload)
    }
    fn flush(&self) -> bool {
        (**self).flush()
    }
}

// ── Compiler identity ────────────────────────────────────────────────

/// The sources that define what a cached answer *means* beyond the compiler
/// passes themselves: the record layout and the serialised answer types.
const FORMAT_SOURCES: [&str; 3] = [
    include_str!("engine.rs"),
    include_str!("disk.rs"),
    include_str!("mage.rs"),
];

/// What a cached answer was computed by.
///
/// It is the crate version, the policy in force, the hash of this module's own
/// sources (so a change to the answer formats invalidates the cache even when
/// the version number stays put), and the identity of the running executable:
/// its size and modification time.
///
/// **The weakness, stated.** The executable is identified by `(len, mtime)`
/// rather than by content, because hashing a debug binary of a hundred
/// megabytes on every start costs more than the compile it would save, and a
/// `build.rs` hash of the source tree is not available here. Rebuilding always
/// writes a new file, so a changed compiler changes the id. What this misses is
/// a *different* binary with the same length and the same mtime installed at
/// the same path (copying with `cp -p` over a same-sized build), and what it
/// over-invalidates is a byte-identical binary with a new mtime (a fresh
/// checkout, a CI restore) — the safe direction. If the executable cannot be
/// examined at all, the id is unique to this process, so nothing persists
/// across runs rather than anything being trusted.
pub fn build_id(policy: &str) -> String {
    let mut parts = vec![
        format!("mage-prototype {}", env!("CARGO_PKG_VERSION")),
        format!("record-format {RECORD_FORMAT}"),
        format!("policy {policy}"),
    ];
    let mut src = Vec::new();
    for s in FORMAT_SOURCES {
        src.push(s.replace("\r\n", "\n"));
    }
    let refs: Vec<&[u8]> = src.iter().map(|s| s.as_bytes()).collect();
    parts.push(format!("query-sources {}", &hex(&digest(&refs))[..16]));
    parts.push(exe_identity());
    parts.join("; ")
}

fn exe_identity() -> String {
    let ident = std::env::current_exe().ok().and_then(|p| {
        let md = std::fs::metadata(&p).ok()?;
        let mtime = md.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
        Some(format!("exe {} {}", md.len(), mtime.as_nanos()))
    });
    ident.unwrap_or_else(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        format!("exe unknown-{}-{nanos}", std::process::id())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mage-disk-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn key(s: &str) -> Hash32 {
        digest(&[s.as_bytes()])
    }

    #[test]
    fn a_saved_record_survives_a_reopen_and_a_missing_one_misses() {
        let dir = tmp("roundtrip");
        let hash = key("h");
        {
            let c = DiskCache::open(&dir).unwrap();
            assert!(matches!(c.load("q", &key("k")), Loaded::Miss));
            assert!(c.save("q", &key("k"), &hash, "{\"x\":1}"));
            // Visible to the run that saved it, before any flush.
            assert!(matches!(c.load("q", &key("k")), Loaded::Hit { .. }));
            assert!(c.segment_files().is_empty(), "nothing written until flushed");
            assert!(c.flush());
            assert_eq!(c.segment_files().len(), 1);
        }
        let c = DiskCache::open(&dir).unwrap();
        match c.load("q", &key("k")) {
            Loaded::Hit { hash: h, payload } => {
                assert_eq!(h, hash);
                assert_eq!(payload, "{\"x\":1}");
            }
            _ => panic!("expected a hit"),
        }
        assert!(matches!(c.load("q", &key("other")), Loaded::Miss));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dropping_the_cache_flushes_it_and_a_run_writes_one_segment() {
        let dir = tmp("drop");
        {
            let c = DiskCache::open(&dir).unwrap();
            for i in 0..500 {
                c.save("q", &key(&format!("k{i}")), &key("h"), "payload");
            }
        }
        let c = DiskCache::open(&dir).unwrap();
        assert_eq!(c.segment_files().len(), 1, "500 answers, one file");
        assert_eq!(c.len(), 500);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Rewrite the newest segment, applying `f` to the line holding `key`.
    fn damage(dir: &Path, key: &Hash32, f: impl Fn(&str) -> Vec<u8>) {
        let seg = segment_files_in(dir).pop().expect("a segment");
        let text = std::fs::read_to_string(&seg).unwrap();
        let mut out = Vec::new();
        for line in text.lines() {
            if line.starts_with(&hex(key)) {
                out.extend(f(line));
            } else {
                out.extend(line.as_bytes());
            }
            out.push(b'\n');
        }
        std::fs::write(&seg, out).unwrap();
    }

    fn written(dir: &Path) {
        let c = DiskCache::open(dir).unwrap();
        c.save("q", &key("k"), &key("h"), "payload");
        c.save("q", &key("bystander"), &key("h"), "payload");
    }

    #[test]
    fn every_kind_of_damage_is_corrupt_and_never_hurts_a_neighbour() {
        let k = key("k");
        let bystander = key("bystander");
        type Damage = (&'static str, fn(&str) -> Vec<u8>);
        let damages: [Damage; 5] = [
            // Edited payload: the integrity hash no longer matches.
            ("edited", |l| l.replace("payload", "pwned!!").into_bytes()),
            // Truncated mid-record.
            ("truncated", |l| l.as_bytes()[..l.len() / 2].to_vec()),
            // The address is intact and what follows it is not JSON.
            ("garbage", |l| format!("{} not json at all", &l[..64]).into_bytes()),
            // Valid JSON with the wrong shape.
            ("wrong shape", |l| format!("{} {{\"format\":1}}", &l[..64]).into_bytes()),
            // Not UTF-8 after the address.
            ("binary", |l| {
                let mut v = l.as_bytes()[..65].to_vec();
                v.extend([0xff, 0xfe, 0x00]);
                v
            }),
        ];
        for (name, f) in damages {
            let dir = tmp(&format!("damage-{}", name.replace(' ', "-")));
            written(&dir);
            damage(&dir, &k, f);
            let c = DiskCache::open(&dir).unwrap();
            // A line whose bytes are not UTF-8 is not a record at all (a miss);
            // every other damage is a record that failed verification.
            let got = c.load("q", &k);
            assert!(matches!(got, Loaded::Corrupt | Loaded::Miss), "{name}: damaged record was used");
            assert!(matches!(c.load("q", &bystander), Loaded::Hit { .. }), "{name}: a neighbour was lost");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn a_record_is_only_good_for_its_own_query_and_address() {
        let dir = tmp("misfiled");
        written(&dir);
        let k = key("k");
        let c = DiskCache::open(&dir).unwrap();
        // Right address, wrong query name.
        assert!(matches!(c.load("other", &k), Loaded::Corrupt));
        // A good line filed under another address.
        let seg = segment_files_in(&dir).pop().unwrap();
        let text = std::fs::read_to_string(&seg).unwrap();
        let line = text.lines().find(|l| l.starts_with(&hex(&k))).unwrap();
        let moved = format!("{} {}\n", hex(&key("elsewhere")), &line[65..]);
        drop(c);
        std::fs::write(&seg, format!("{text}{moved}")).unwrap();
        let c = DiskCache::open(&dir).unwrap();
        assert!(matches!(c.load("q", &key("elsewhere")), Loaded::Corrupt));
        assert!(matches!(c.load("q", &k), Loaded::Hit { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_good_record_appended_later_shadows_a_corrupt_one() {
        let dir = tmp("shadow");
        written(&dir);
        let k = key("k");
        damage(&dir, &k, |l| l.as_bytes()[..l.len() / 2].to_vec());
        {
            let c = DiskCache::open(&dir).unwrap();
            assert!(matches!(c.load("q", &k), Loaded::Corrupt));
            c.save("q", &k, &key("h"), "payload"); // the recomputed answer
        }
        let c = DiskCache::open(&dir).unwrap();
        assert!(matches!(c.load("q", &k), Loaded::Hit { .. }), "the newer segment wins");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn many_segments_are_merged_without_losing_an_answer() {
        let dir = tmp("compact");
        let runs = COMPACT_AT + 4;
        for i in 0..runs {
            let c = DiskCache::open(&dir).unwrap();
            c.save("q", &key(&format!("k{i}")), &key("h"), &format!("payload {i}"));
        } // each drop flushes: one segment per run
        let c = DiskCache::open(&dir).unwrap();
        assert_eq!(c.len(), runs);
        assert!(c.segment_files().len() <= COMPACT_AT, "{:?}", c.segment_files());
        for i in 0..runs {
            assert!(matches!(c.load("q", &key(&format!("k{i}"))), Loaded::Hit { .. }), "k{i} lost in the merge");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_build_id_names_the_version_and_the_policy() {
        let a = build_id("builtin");
        assert!(a.contains(env!("CARGO_PKG_VERSION")), "{a}");
        assert_ne!(a, build_id("overlay-abc"));
        assert_eq!(a, build_id("builtin"), "stable within a process");
    }
}

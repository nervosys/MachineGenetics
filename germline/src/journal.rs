//! The journal: a durable, hash-chained record of everything that happened.
//!
//! [`super::lineage::Lineage`] is append-only *in memory*. That is enough to stop
//! the code from editing history and not enough to stop anything else — a
//! restart loses it, and a file is a file.
//!
//! Each record therefore carries the digest of the record before it. Editing or
//! deleting record *n* breaks the link at *n+1*, and [`Journal::verify`] finds
//! the exact index where the chain parts.
//!
//! ## What a hash chain is and is not
//!
//! It makes tampering **evident**, not impossible. Anyone who can write the file
//! can rewrite it wholesale from any point and recompute every subsequent link.
//! What they cannot do is change one entry and leave the rest intact, which is
//! the realistic failure — a truncated write, a partial restore, a targeted edit.
//!
//! Making it *impossible* needs the chain head published somewhere the writer
//! does not control (a signature, an external witness, another replica). The
//! chain is the part that makes such a check cheap: one digest attests the whole
//! history, so publishing 32 bytes anchors everything.
//!
//! For a system that modifies itself this is not bookkeeping. After an incident
//! the question is "what changed, when, and on whose authority", and the only
//! thing that can answer it is a record the system could not have quietly
//! adjusted on its way past.

use super::gate::Verdict;
use super::lineage::SuccessionEvent;
use super::{Generation, GenerationId};
use ribosome::Digest;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    /// A candidate entered the lineage, with the provenance to re-derive it.
    Proposed { generation: GenerationId, artifact: Digest, seed: u64, plan: String },
    /// A candidate was measured.
    Evaluated { generation: GenerationId, fitness: String, suite: Digest, evaluator: String },
    /// The gate ruled.
    Adjudicated { verdict: Verdict, attestation: Option<String> },
    /// Authority moved (or was refused).
    Succession { event: SuccessionEvent },
    /// A champion failed under supervision.
    Failure { generation: GenerationId, mode: String },
    /// Free-form operator note, so out-of-band context lands in the same record.
    Note { text: String },
}

/// A journal line: the entry, its position, and the chain link.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub index: u64,
    /// Digest of the previous record. `None` only for index 0.
    pub prev: Option<Digest>,
    pub entry: Entry,
    /// Digest of `(index, prev, entry)` — this record's link.
    pub link: Digest,
}

impl Record {
    fn compute_link(index: u64, prev: Option<&Digest>, entry: &Entry) -> Digest {
        let body = serde_json::to_string(entry).unwrap_or_default();
        let mut buf = Vec::new();
        buf.extend_from_slice(b"germline-journal-v1");
        buf.extend_from_slice(&index.to_le_bytes());
        buf.extend_from_slice(prev.map(|d| d.0.as_bytes()).unwrap_or(b"genesis"));
        buf.extend_from_slice(&(body.len() as u64).to_le_bytes());
        buf.extend_from_slice(body.as_bytes());
        Digest::of(&buf)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalError {
    Io(String),
    /// The chain parts here.
    Broken { index: u64, expected: Digest, found: Digest },
    /// A record's index is not one past its predecessor.
    OutOfOrder { at: u64, expected: u64 },
    Malformed { index: u64, detail: String },
}

impl std::fmt::Display for JournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JournalError::Io(e) => write!(f, "journal io: {e}"),
            JournalError::Broken { index, expected, found } => write!(
                f,
                "journal chain broken at record {index}: expected link {}, found {}",
                expected.short(),
                found.short()
            ),
            JournalError::OutOfOrder { at, expected } => {
                write!(f, "journal record {at} is out of order (expected index {expected})")
            }
            JournalError::Malformed { index, detail } => {
                write!(f, "journal record {index} is unreadable: {detail}")
            }
        }
    }
}

impl std::error::Error for JournalError {}

/// An append-only, hash-chained log on disk (JSON Lines).
///
/// JSONL rather than one big JSON document so an append is a single write with
/// no read-modify-write, and a truncated final line costs one record instead of
/// the whole file.
pub struct Journal {
    path: PathBuf,
    head: Option<Digest>,
    next_index: u64,
}

impl Journal {
    /// Open or create, recovering the chain head from what is already there.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, JournalError> {
        let path: PathBuf = path.into();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| JournalError::Io(e.to_string()))?;
            }
        }
        let records = Self::read_all(&path)?;
        let head = records.last().map(|r| r.link.clone());
        let next_index = records.last().map(|r| r.index + 1).unwrap_or(0);
        Ok(Journal { path, head, next_index })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The single digest that attests the entire history. Publishing this
    /// somewhere the writer does not control is what upgrades tamper-*evidence*
    /// into tamper-*resistance*.
    pub fn head(&self) -> Option<&Digest> {
        self.head.as_ref()
    }

    pub fn len(&self) -> u64 {
        self.next_index
    }

    pub fn is_empty(&self) -> bool {
        self.next_index == 0
    }

    pub fn append(&mut self, entry: Entry) -> Result<Record, JournalError> {
        let index = self.next_index;
        let link = Record::compute_link(index, self.head.as_ref(), &entry);
        let record = Record { index, prev: self.head.clone(), entry, link: link.clone() };

        let line = serde_json::to_string(&record)
            .map_err(|e| JournalError::Malformed { index, detail: e.to_string() })?;

        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| JournalError::Io(e.to_string()))?;
        writeln!(f, "{line}").map_err(|e| JournalError::Io(e.to_string()))?;
        // Durability matters more than throughput here: a succession record that
        // did not survive the crash it is meant to explain is worthless.
        f.sync_all().map_err(|e| JournalError::Io(e.to_string()))?;

        self.head = Some(link);
        self.next_index += 1;
        Ok(record)
    }

    fn read_all(path: &Path) -> Result<Vec<Record>, JournalError> {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Ok(Vec::new());
        };
        raw.lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| {
                serde_json::from_str::<Record>(l)
                    .map_err(|e| JournalError::Malformed { index: i as u64, detail: e.to_string() })
            })
            .collect()
    }

    /// Every record, oldest first.
    pub fn replay(&self) -> Result<Vec<Record>, JournalError> {
        Self::read_all(&self.path)
    }

    /// Walk the chain and confirm it is intact.
    pub fn verify(&self) -> Result<u64, JournalError> {
        let records = self.replay()?;
        let mut prev: Option<Digest> = None;
        for (i, r) in records.iter().enumerate() {
            let expected_index = i as u64;
            if r.index != expected_index {
                return Err(JournalError::OutOfOrder { at: r.index, expected: expected_index });
            }
            if r.prev != prev {
                return Err(JournalError::Broken {
                    index: r.index,
                    expected: prev.clone().unwrap_or(Digest("<genesis>".into())),
                    found: r.prev.clone().unwrap_or(Digest("<genesis>".into())),
                });
            }
            let recomputed = Record::compute_link(r.index, r.prev.as_ref(), &r.entry);
            if recomputed != r.link {
                return Err(JournalError::Broken {
                    index: r.index,
                    expected: recomputed,
                    found: r.link.clone(),
                });
            }
            prev = Some(r.link.clone());
        }
        Ok(records.len() as u64)
    }
}

/// Convenience: record a generation's arrival with its provenance.
pub fn proposed(g: &Generation, seed: u64, plan: &str) -> Entry {
    Entry::Proposed {
        generation: g.id,
        artifact: g.artifact.clone(),
        seed,
        plan: plan.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "germline-journal-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p.join("journal.jsonl")
    }

    fn note(t: &str) -> Entry {
        Entry::Note { text: t.into() }
    }

    #[test]
    fn records_chain_and_verify() {
        let path = tmp("chain");
        let mut j = Journal::open(&path).unwrap();
        for i in 0..5 {
            j.append(note(&format!("event {i}"))).unwrap();
        }
        assert_eq!(j.verify().unwrap(), 5);
        assert_eq!(j.len(), 5);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_first_record_has_no_predecessor() {
        let path = tmp("genesis");
        let mut j = Journal::open(&path).unwrap();
        let r = j.append(note("first")).unwrap();
        assert_eq!(r.index, 0);
        assert_eq!(r.prev, None);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_reopened_journal_continues_the_chain() {
        let path = tmp("reopen");
        let head = {
            let mut j = Journal::open(&path).unwrap();
            j.append(note("a")).unwrap();
            j.append(note("b")).unwrap();
            j.head().unwrap().clone()
        };
        let mut j2 = Journal::open(&path).unwrap();
        assert_eq!(j2.head(), Some(&head), "the head must survive a restart");
        assert_eq!(j2.len(), 2);
        j2.append(note("c")).unwrap();
        assert_eq!(j2.verify().unwrap(), 3);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn editing_a_record_breaks_the_chain_at_that_point() {
        let path = tmp("tamper");
        let mut j = Journal::open(&path).unwrap();
        for i in 0..4 {
            j.append(note(&format!("event {i}"))).unwrap();
        }

        // Rewrite record 1's payload, leaving its link alone.
        let mut lines: Vec<String> =
            std::fs::read_to_string(&path).unwrap().lines().map(String::from).collect();
        lines[1] = lines[1].replace("event 1", "event X");
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();

        match Journal::open(&path).unwrap().verify() {
            Err(JournalError::Broken { index, .. }) => assert_eq!(index, 1),
            other => panic!("a targeted edit must be detected, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn deleting_a_record_is_detected() {
        let path = tmp("delete");
        let mut j = Journal::open(&path).unwrap();
        for i in 0..4 {
            j.append(note(&format!("event {i}"))).unwrap();
        }
        let mut lines: Vec<String> =
            std::fs::read_to_string(&path).unwrap().lines().map(String::from).collect();
        lines.remove(1);
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();

        assert!(
            Journal::open(&path).unwrap().verify().is_err(),
            "removing an inconvenient record must not go unnoticed"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn truncation_leaves_a_valid_but_shorter_chain() {
        // Honest about the limit: cutting the tail is not detectable from the
        // file alone. It is detectable against a published head, which is why
        // `head()` exists.
        let path = tmp("truncate");
        let mut j = Journal::open(&path).unwrap();
        for i in 0..4 {
            j.append(note(&format!("event {i}"))).unwrap();
        }
        let full_head = j.head().unwrap().clone();

        let lines: Vec<String> =
            std::fs::read_to_string(&path).unwrap().lines().map(String::from).collect();
        std::fs::write(&path, lines[..2].join("\n") + "\n").unwrap();

        let j2 = Journal::open(&path).unwrap();
        assert_eq!(j2.verify().unwrap(), 2, "the surviving prefix is internally consistent");
        assert_ne!(
            j2.head(),
            Some(&full_head),
            "but it no longer matches the published head — which is how truncation is caught"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn replay_returns_entries_in_order() {
        let path = tmp("replay");
        let mut j = Journal::open(&path).unwrap();
        j.append(note("one")).unwrap();
        j.append(note("two")).unwrap();
        let entries: Vec<String> = j
            .replay()
            .unwrap()
            .into_iter()
            .map(|r| match r.entry {
                Entry::Note { text } => text,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(entries, vec!["one", "two"]);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_fresh_journal_is_empty_not_an_error() {
        let path = tmp("fresh");
        let j = Journal::open(&path).unwrap();
        assert!(j.is_empty());
        assert_eq!(j.head(), None);
        assert_eq!(j.verify().unwrap(), 0);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}

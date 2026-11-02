//! The evidence log (NEUROSYMBOLIC_PLAN.md Phase 7): what the gate refused and
//! what was measured, one JSON entry per line.
//!
//! Each entry names a component (an agent's kind, an instance of `Component`
//! in `kb/rules/refusals.mg`), an artifact (the SHA-256 of a program's source,
//! since a refused program has no canonical hash), and, for a refusal, the
//! rule that refused it (a `Rule` fact). Entries are numbered and each carries
//! the hash of the line before it, so [`verify`] can tell a log that was only
//! appended to from one that was edited, reordered or cut short at the front.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// What an entry records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum What {
    /// The gate refused the artifact under `rule`; `class` is its `Refusal`.
    Refusal { rule: String, class: String, message: String },
    /// A figure measured of the artifact.
    Measurement { name: String, value: f64 },
}

/// An entry before it is numbered and chained.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub round: usize,
    /// The agent's kind: a `Component` fact.
    pub component: String,
    /// The agent, within its run.
    pub agent: String,
    /// `sha256:` and the hex digest of the program's source.
    pub artifact: String,
    #[serde(flatten)]
    pub what: What,
}

/// A logged entry: a record, its place, and the hash of the line before it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub seq: u64,
    /// The hex SHA-256 of the previous line; empty for the first.
    pub prev: String,
    #[serde(flatten)]
    pub record: Record,
}

/// The sum of `agent`'s rewards in `records`, in their order.
pub fn rewards(records: &[Record], agent: &str) -> f64 {
    records
        .iter()
        .filter(|r| r.agent == agent)
        .filter_map(|r| match &r.what {
            What::Measurement { name, value } if name == "reward" => Some(*value),
            _ => None,
        })
        .fold(0.0, |a, v| a + v)
}

/// The address of a program's source.
pub fn artifact(source: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(source.as_bytes()))
}

fn line_hash(line: &str) -> String {
    format!("{:x}", Sha256::digest(line.as_bytes()))
}

/// The entries of `log`, if each is numbered after the one before and carries
/// its hash.
pub fn verify(log: &str) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    let mut prev = String::new();
    for (n, line) in log.lines().enumerate() {
        let e: Entry = serde_json::from_str(line).map_err(|e| format!("line {}: {e}", n + 1))?;
        if e.seq != n as u64 {
            return Err(format!("line {}: numbered {}", n + 1, e.seq));
        }
        if e.prev != prev {
            return Err(format!("line {}: does not follow the line before it", n + 1));
        }
        prev = line_hash(line);
        entries.push(e);
    }
    Ok(entries)
}

/// `records` as lines continuing `log`, which must verify.
pub fn continue_log(log: &str, records: &[Record]) -> Result<String, String> {
    let mut seq = verify(log)?.len() as u64;
    let mut prev = log.lines().last().map(line_hash).unwrap_or_default();
    let mut out = String::new();
    for r in records {
        let line = serde_json::to_string(&Entry { seq, prev, record: r.clone() }).map_err(|e| e.to_string())?;
        prev = line_hash(&line);
        seq += 1;
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

/// Append `records` to the log at `path`, which is created if absent and must
/// verify if present. Returns how many entries the log then holds.
pub fn append(path: &Path, records: &[Record]) -> Result<usize, String> {
    use std::io::Write;
    let log = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let lines = continue_log(&log, records).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(lines.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(log.lines().count() + records.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(round: usize, what: What) -> Record {
        Record { round, component: "grammar".into(), agent: "g0".into(), artifact: artifact("f gen(s: usize) -> [usize] { [] }"), what }
    }

    fn refusal() -> What {
        What::Refusal { rule: "gate_empty".into(), class: "empty".into(), message: "no output".into() }
    }

    #[test]
    fn a_log_continues_and_verifies() {
        let first = continue_log("", &[record(0, refusal())]).unwrap();
        let both = first.clone() + &continue_log(&first, &[record(1, What::Measurement { name: "reward".into(), value: 0.5 })]).unwrap();
        let entries = verify(&both).unwrap();
        assert_eq!(entries.iter().map(|e| e.seq).collect::<Vec<_>>(), [0, 1]);
        assert_eq!(entries[1].record.what, What::Measurement { name: "reward".into(), value: 0.5 });
        assert!(both.lines().next().unwrap().contains(r#""kind":"refusal""#), "{both}");
    }

    /// An edit, a reordering and a cut at the front are each caught.
    #[test]
    fn an_edited_log_does_not_verify() {
        let log = continue_log("", &[record(0, refusal()), record(1, refusal()), record(2, refusal())]).unwrap();
        let lines: Vec<&str> = log.lines().collect();
        let edited = log.replacen("no output", "some output", 1);
        assert!(verify(&edited).unwrap_err().starts_with("line 2"), "the next line no longer follows");
        let swapped = [lines[1], lines[0], lines[2]].join("\n");
        assert!(verify(&swapped).is_err());
        let cut = lines[1..].join("\n");
        assert!(verify(&cut).is_err());
        assert!(continue_log(&edited, &[record(3, refusal())]).is_err(), "nothing is appended to a broken log");
    }

    /// Only `agent`'s rewards count, and refusals are not rewards.
    #[test]
    fn rewards_sums_one_agents_rewards() {
        let reward = |agent: &str, value| Record { agent: agent.into(), ..record(0, What::Measurement { name: "reward".into(), value }) };
        let records = [reward("g0", 0.25), record(0, refusal()), reward("g1", 4.0), reward("g0", 0.5)];
        assert_eq!(rewards(&records, "g0"), 0.75);
        assert_eq!(rewards(&records, "g2"), 0.0);
    }

    #[test]
    fn append_creates_then_continues_a_file() {
        let path = std::env::temp_dir().join(format!("mage-evidence-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert_eq!(append(&path, &[record(0, refusal())]).unwrap(), 1);
        assert_eq!(append(&path, &[record(1, refusal()), record(1, refusal())]).unwrap(), 3);
        let entries = verify(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(entries.len(), 3);
        std::fs::remove_file(&path).unwrap();
    }
}

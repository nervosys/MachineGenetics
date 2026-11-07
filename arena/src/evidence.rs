//! The evidence log (NEUROSYMBOLIC_PLAN.md Phase 7): what the gate refused and
//! what was measured, one JSON entry per line.
//!
//! Each entry names a component (an agent's kind, an instance of `Component`
//! in `kb/rules/refusals.mg`), an artifact (the SHA-256 of a program's source,
//! since a refused program has no canonical hash), and, for a refusal, the
//! rule that refused it (a `Rule` fact). Entries are numbered and each carries
//! the hash of the line before it, so [`verify`] can tell a log that was only
//! appended to from one that was edited, reordered or cut short at the front.

use std::collections::{BTreeMap, BTreeSet};
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

/// Which entries of a log a figure came from: a range of `seq`, and the hash of
/// the log's last line, which pins every entry before it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Citation { pub path: String, pub first: u64, pub last: u64, pub head: String }

impl std::fmt::Display for Citation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "evidence: {} seq {}–{}, head sha256:{}", self.path, self.first, self.last, self.head)
    }
}

/// The citation of entries `first` to `last` of `log`, which is at `path`.
pub fn cite(path: &str, log: &str, first: u64, last: u64) -> Citation {
    let head = log.lines().last().map(line_hash).unwrap_or_default();
    Citation { path: path.into(), first, last, head }
}

/// Refusals per rule per component, and what each component was measured on.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Tally {
    /// Rule, then component, to refusals.
    pub refusals: BTreeMap<String, BTreeMap<String, usize>>,
    /// Component to rewards measured: its programs that were not refused.
    pub rewards: BTreeMap<String, usize>,
    pub entries: usize,
}

/// The tally of `entries`.
pub fn tally(entries: &[Entry]) -> Tally {
    let mut t = Tally { entries: entries.len(), ..Tally::default() };
    for e in entries {
        match &e.record.what {
            What::Refusal { rule, .. } => {
                *t.refusals.entry(rule.clone()).or_default().entry(e.record.component.clone()).or_default() += 1;
            }
            What::Measurement { name, .. } if name == "reward" => {
                *t.rewards.entry(e.record.component.clone()).or_default() += 1;
            }
            _ => {}
        }
    }
    t
}

impl Tally {
    /// The tally as a plain-text table: one row per rule, then the totals.
    pub fn table(&self) -> String {
        let components: BTreeSet<&str> = self
            .refusals
            .values()
            .flat_map(|by| by.keys().map(String::as_str))
            .chain(self.rewards.keys().map(String::as_str))
            .collect();
        let refused = |c: &str| -> usize { self.refusals.values().filter_map(|by| by.get(c)).sum() };
        let rewarded = |c: &str| self.rewards.get(c).copied().unwrap_or(0);
        let mut rows: Vec<(String, Vec<String>)> = self
            .refusals
            .iter()
            .map(|(rule, by)| (rule.clone(), components.iter().copied().map(|c| by.get(c).copied().unwrap_or(0).to_string()).collect()))
            .collect();
        rows.push(("refused".into(), components.iter().copied().map(|c| refused(c).to_string()).collect()));
        rows.push(("rewarded".into(), components.iter().copied().map(|c| rewarded(c).to_string()).collect()));
        rows.push((
            "refused %".into(),
            components
                .iter()
                .copied()
                .map(|c| match (refused(c), rewarded(c)) {
                    (0, 0) => "-".to_string(),
                    (r, w) => format!("{:.1}", 100.0 * r as f64 / (r + w) as f64),
                })
                .collect(),
        ));
        let label_w = rows.iter().map(|(l, _)| l.len()).chain(["rule".len()]).max().unwrap_or(0);
        let widths: Vec<usize> = components.iter().map(|c| c.len().max(6)).collect();
        let render = |label: &str, cells: &[String]| -> String {
            let mut s = format!("{label:<label_w$}");
            for (cell, &w) in cells.iter().zip(&widths) {
                s += &format!("  {cell:>w$}");
            }
            s + "\n"
        };
        let header: Vec<String> = components.iter().map(|c| c.to_string()).collect();
        let mut out = render("rule", &header);
        for (label, cells) in &rows {
            out += &render(label, cells);
        }
        out
    }
}

/// The entries numbered `first` to `last`, clamped to those `entries` holds.
pub fn in_range(entries: &[Entry], first: u64, last: u64) -> &[Entry] {
    let lo = (first as usize).min(entries.len());
    let hi = (last as usize).saturating_add(1).min(entries.len()).max(lo);
    &entries[lo..hi]
}

/// `arena evidence <log.jsonl> [--seq A..B] [--json OUT]`: verify a log, then tally
/// its entries `A..=B` (default: all) and cite them.
pub fn report_cli(args: &[String]) -> Result<(), String> {
    const USAGE: &str = "usage: arena evidence <log.jsonl> [--seq A..B] [--json OUT]";
    let mut positional: Vec<&str> = Vec::new();
    let mut json_out: Option<&str> = None;
    let mut seq: Option<&str> = None;
    let mut rest = args.iter().map(String::as_str);
    while let Some(a) = rest.next() {
        if a == "--json" {
            json_out = Some(rest.next().ok_or(USAGE)?);
        } else if a == "--seq" {
            seq = Some(rest.next().ok_or(USAGE)?);
        } else {
            positional.push(a);
        }
    }
    let &[path] = positional.as_slice() else { return Err(USAGE.into()) };
    let log = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let entries = verify(&log).map_err(|e| format!("{path}: {e}"))?;
    let n = entries.len() as u64;
    let (first, last) = match seq {
        None => (0, n.saturating_sub(1)),
        Some(s) => {
            let bad = || format!("{path}: --seq {s}: want A..B with A <= B < {n}");
            let (a, b) = s.split_once("..").ok_or_else(bad)?;
            let a = a.parse::<u64>().map_err(|_| bad())?;
            let b = b.parse::<u64>().map_err(|_| bad())?;
            if a > b || b >= n {
                return Err(bad());
            }
            (a, b)
        }
    };
    let tallied = in_range(&entries, first, last);
    let report = tally(tallied);
    let citation = cite(path, &log, first, last);
    if let Some(out) = json_out {
        let json = serde_json::to_string_pretty(&serde_json::json!({ "citation": citation, "tally": report })).map_err(|e| e.to_string())?;
        std::fs::write(out, json).map_err(|e| format!("{out}: {e}"))?;
    }
    print!("{path}: {} entries, verified\n{}", tallied.len(), report.table());
    if !log.is_empty() {
        println!("{citation}");
    }
    Ok(())
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

    /// Two grammar refusals, a mutator refusal and a grammar reward.
    fn mixed() -> Vec<Entry> {
        let max_depth = What::Refusal { rule: "max_depth".into(), class: "depth".into(), message: "too deep".into() };
        let records = [
            record(0, refusal()),
            record(1, refusal()),
            Record { component: "mutator".into(), ..record(0, max_depth) },
            record(1, What::Measurement { name: "reward".into(), value: 0.5 }),
        ];
        verify(&continue_log("", &records).unwrap()).unwrap()
    }

    #[test]
    fn tally_counts_refusals_by_rule_and_component() {
        let t = tally(&mixed());
        assert_eq!(t.refusals["gate_empty"]["grammar"], 2);
        assert_eq!(t.refusals["max_depth"]["mutator"], 1);
        assert_eq!(t.rewards["grammar"], 1);
        assert_eq!(t.rewards.get("mutator"), None);
        assert_eq!(t.entries, 4);
    }

    #[test]
    fn the_table_has_a_row_per_rule_and_the_totals() {
        let table = tally(&mixed()).table();
        let lines: Vec<&str> = table.lines().collect();
        assert!(lines[0].starts_with("rule") && lines[0].contains("grammar") && lines[0].contains("mutator"), "{table}");
        assert!(lines.iter().any(|l| l.starts_with("gate_empty")), "{table}");
        assert!(lines.iter().any(|l| l.starts_with("max_depth")), "{table}");
        let pct = lines.iter().find(|l| l.starts_with("refused %")).expect("a refused % row");
        assert!(pct.contains("66.7") && pct.contains("100.0"), "{table}");
    }

    #[test]
    fn a_citation_names_the_range_and_the_head() {
        let records: Vec<Record> = mixed().into_iter().map(|e| e.record).collect();
        let n = records.len() as u64;
        let log = continue_log("", &records).unwrap();
        let c = cite("x.jsonl", &log, 0, n - 1);
        assert_eq!((c.first, c.last), (0, n - 1));
        assert_eq!(c.head, line_hash(log.lines().last().unwrap()));
        assert!(c.to_string().starts_with("evidence: x.jsonl seq 0–"), "{c}");
        let longer = log.clone() + &continue_log(&log, &[record(2, refusal())]).unwrap();
        assert_ne!(cite("x.jsonl", &longer, 0, n).head, c.head);
    }

    #[test]
    fn a_range_tallies_only_its_entries() {
        let entries = mixed();
        let part = in_range(&entries, 1, 2);
        assert_eq!(part.iter().map(|e| e.seq).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(in_range(&entries, 0, 99).len(), entries.len());
        let t = tally(part);
        assert_eq!(t.entries, 2);
        assert!(t.entries < tally(&entries).entries);
        assert_eq!(t.refusals["gate_empty"]["grammar"], 1);
        assert_eq!(t.refusals["max_depth"]["mutator"], 1);
        assert!(t.rewards.is_empty());
    }
}

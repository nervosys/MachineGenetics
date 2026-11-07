//! A proposal to change the knowledge base (NEUROSYMBOLIC_PLAN.md, Phase 8):
//! edits to `kb/*.mg` files, with the author who proposes them and the version
//! they were written against. Applying one gives the new files; whether they
//! are admitted is the gate's question, not this module's.

use sha2::{Digest, Sha256};

use crate::kb::Kb;

/// Who proposes the change.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Author {
    /// A component the knowledge base states: an instance of `Component`,
    /// `NeuralComponent` or `SymbolicComponent`.
    Component(String),
    /// A person, by name.
    Person(String),
}

/// Replace `before` with `after` in `file`. An empty `before` appends `after`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Edit {
    pub file: String,
    pub before: String,
    pub after: String,
}

/// A change to the knowledge base, in the words of its author.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Proposal {
    pub author: Author,
    pub summary: String,
    /// The version of the knowledge base the edits were written against: see `version`.
    pub base: String,
    pub edits: Vec<Edit>,
}

/// The content address of `files`: SHA-256 over each path, a 0 byte, the
/// length of its source and the source, with `\r\n` read as `\n`.
pub fn version(files: &[(&str, &str)]) -> String {
    let mut h = Sha256::new();
    for &(path, src) in files {
        let src = src.replace("\r\n", "\n");
        h.update(path.as_bytes());
        h.update([0u8]);
        h.update((src.len() as u64).to_le_bytes());
        h.update(src.as_bytes());
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// Read a proposal from its JSON form.
pub fn parse(json: &str) -> Result<Proposal, String> {
    serde_json::from_str(json).map_err(|e| format!("proposal: {e}"))
}

/// Apply `p` to `files`, whose knowledge base is `kb`: the new (path, source)
/// list, in the order of `files`, or every reason it cannot apply.
pub fn apply(p: &Proposal, kb: &Kb, files: &[(&str, &str)]) -> Result<Vec<(String, String)>, Vec<String>> {
    let mut refused = Vec::new();
    let current = version(files);
    if p.base != current {
        refused.push(format!("written against {}, but the knowledge base is {current}", p.base));
    }
    match &p.author {
        Author::Component(c) => {
            let stated = kb
                .facts("instance_of")
                .iter()
                .any(|f| f[0] == *c && ["Component", "NeuralComponent", "SymbolicComponent"].contains(&f[1].as_str()));
            if !stated {
                refused.push(format!("`{c}` is not a component the knowledge base states"));
            }
        }
        Author::Person(n) => {
            if n.trim().is_empty() {
                refused.push("a person author needs a name".to_string());
            }
        }
    }
    if p.summary.trim().is_empty() {
        refused.push("a proposal says what it changes".to_string());
    }
    if p.edits.is_empty() {
        refused.push("a proposal changes something".to_string());
    }

    let mut sources: Vec<(String, String)> =
        files.iter().map(|&(path, src)| (path.to_string(), src.replace("\r\n", "\n"))).collect();
    for (i, e) in p.edits.iter().enumerate() {
        let n = i + 1;
        let Some(at) = sources.iter().position(|(path, _)| path == &e.file) else {
            refused.push(format!("edit {n}: `{}` is not a knowledge-base file", e.file));
            continue;
        };
        if e.before == e.after {
            refused.push(format!("edit {n}: changes nothing"));
            continue;
        }
        let src = &mut sources[at].1;
        if e.before.is_empty() {
            if !src.is_empty() && !src.ends_with('\n') {
                src.push('\n');
            }
            src.push_str(&e.after);
            continue;
        }
        match src.matches(&e.before).count() {
            0 => refused.push(format!("edit {n}: its `before` text is not in {}", e.file)),
            1 => *src = src.replacen(&e.before, &e.after, 1),
            k => refused.push(format!("edit {n}: its `before` text is in {} {k} times", e.file)),
        }
    }

    if refused.is_empty() { Ok(sources) } else { Err(refused) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kb::{self, FILES};

    /// Appends a block that states the `proposer` component.
    const PROPOSER: &str = "\nkb Proposals {\n    fact instance_of(\"proposer\", Component);\n}\n";

    fn borrowed(files: &[(String, String)]) -> Vec<(&str, &str)> {
        files.iter().map(|(p, s)| (p.as_str(), s.as_str())).collect()
    }

    fn proposal(author: Author, base: String, edits: Vec<Edit>) -> Proposal {
        Proposal { author, summary: "add a proposer".to_string(), base, edits }
    }

    fn refusals_append(text: &str) -> Edit {
        Edit { file: "kb/rules/refusals.mg".to_string(), before: String::new(), after: text.to_string() }
    }

    #[test]
    fn a_proposal_round_trips_through_json() {
        let p = proposal(Author::Component("grammar".into()), version(FILES), vec![refusals_append(PROPOSER)]);
        let json = serde_json::to_string(&p).expect("serializes");
        assert!(json.contains("\"component\":\"grammar\""), "{json}");
        assert_eq!(parse(&json), Ok(p));
    }

    #[test]
    fn the_version_is_stable_and_changes_with_any_file() {
        let v = version(FILES);
        assert_eq!(v, version(FILES));
        assert!(v.starts_with("sha256:"));
        assert_eq!(v.len(), 71);

        let mut changed: Vec<(String, String)> = FILES.iter().map(|(p, s)| (p.to_string(), s.to_string())).collect();
        changed[0].1.push(' ');
        assert_ne!(v, version(&borrowed(&changed)));

        let crlf: Vec<(String, String)> =
            FILES.iter().map(|(p, s)| (p.to_string(), s.replace("\r\n", "\n").replace('\n', "\r\n"))).collect();
        assert_eq!(v, version(&borrowed(&crlf)));
    }

    #[test]
    fn an_accepted_proposal_gives_files_that_load() {
        let kb = kb::load().expect("loads");
        let p = proposal(Author::Component("grammar".into()), version(FILES), vec![refusals_append(PROPOSER)]);
        let new = apply(&p, &kb, FILES).unwrap_or_else(|e| panic!("{e:#?}"));
        let loaded = kb::load_files(&borrowed(&new)).unwrap_or_else(|e| panic!("{e:#?}"));
        assert!(loaded.facts("instance_of").contains(&vec!["proposer".to_string(), "Component".to_string()]));
    }

    #[test]
    fn a_proposal_is_refused_for_every_reason_at_once() {
        let kb = kb::load().expect("loads");
        let p = Proposal {
            author: Author::Component("nobody".into()),
            summary: "  ".into(),
            base: "sha256:0".into(),
            edits: vec![
                Edit { file: "kb/nothing.mg".into(), before: "x".into(), after: "y".into() },
                Edit { file: "kb/schema.mg".into(), before: "no such text anywhere".into(), after: "y".into() },
                Edit { file: "kb/schema.mg".into(), before: "fact class(Component);".into(), after: "fact class(Component);".into() },
            ],
        };
        let e = apply(&p, &kb, FILES).expect_err("refused");
        assert_eq!(e.len(), 6, "{e:#?}");
        for want in [
            "written against sha256:0",
            "`nobody` is not a component the knowledge base states",
            "a proposal says what it changes",
            "edit 1: `kb/nothing.mg` is not a knowledge-base file",
            "edit 2: its `before` text is not in kb/schema.mg",
            "edit 3: changes nothing",
        ] {
            assert!(e.iter().any(|m| m.contains(want)), "no message says `{want}`: {e:#?}");
        }
    }

    #[test]
    fn a_before_text_must_be_unique() {
        let kb = kb::load().expect("loads");
        let p = proposal(
            Author::Component("grammar".into()),
            version(FILES),
            vec![Edit { file: "kb/schema.mg".into(), before: "fact ".into(), after: "fact x".into() }],
        );
        let e = apply(&p, &kb, FILES).expect_err("refused");
        assert!(e.iter().any(|m| m.contains("times")), "{e:#?}");
    }

    #[test]
    fn a_person_may_propose() {
        let kb = kb::load().expect("loads");
        let p = proposal(Author::Person("a reviewer".into()), version(FILES), vec![refusals_append(PROPOSER)]);
        assert!(apply(&p, &kb, FILES).is_ok());
    }
}

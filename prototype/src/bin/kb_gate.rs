//! `kb-gate`: the knowledge base's share of the gate (NEUROSYMBOLIC_PLAN.md, Phase 8).
//! `version` prints the knowledge base's version. `admit` applies a proposal and,
//! if the knowledge base admits it, writes the files it changes. `promotion`
//! prints the proposal that promotes a grammar extension to built-in. `counts`
//! prints the knowledge base's counts, one `key=value` line each, in key order.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use mage_prototype::grammar::{self, GrammarExtension};
use mage_prototype::{kb, kb_proposal};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["version"] => {
            println!("{}", kb_proposal::version(kb::FILES));
            ExitCode::SUCCESS
        }
        ["counts"] => counts(),
        ["admit", proposal, dir] => admit(proposal, Path::new(dir)),
        ["promotion", sigil, keyword, uses, description] => promotion(sigil, keyword, uses, description),
        _ => {
            eprintln!(
                "usage: kb-gate version | kb-gate counts | kb-gate admit <proposal.json> <dir> | \
                 kb-gate promotion <sigil> <keyword> <uses> <description>"
            );
            ExitCode::from(1)
        }
    }
}

/// The knowledge base's counts. `kb::Kb::counts` says what each key counts.
fn counts() -> ExitCode {
    let kb = match kb::load() {
        Ok(kb) => kb,
        Err(es) => {
            for e in es {
                eprintln!("refused kb_schema: {e}");
            }
            return ExitCode::from(2);
        }
    };
    for (key, n) in kb.counts() {
        println!("{key}={n}");
    }
    ExitCode::SUCCESS
}

/// The proposal that promotes `sigil`, a grammar extension used `uses` times.
fn promotion(sigil: &str, keyword: &str, uses: &str, description: &str) -> ExitCode {
    let Ok(uses) = uses.parse::<u64>() else {
        eprintln!("kb-gate: <uses> is not a count: {uses}");
        return ExitCode::from(1);
    };
    let ext = GrammarExtension {
        sigil: sigil.into(),
        rust_equiv: keyword.into(),
        namespace: "user".into(),
        usage_count: uses,
        description: description.into(),
    };
    let kb = match kb::load() {
        Ok(kb) => kb,
        Err(es) => {
            for e in es {
                eprintln!("refused kb_schema: {e}");
            }
            return ExitCode::from(2);
        }
    };
    match grammar::promotion_proposal(&ext, &kb) {
        Ok(p) => {
            println!("{}", serde_json::to_string_pretty(&p).expect("a proposal serializes"));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("refused: {e}");
            ExitCode::from(2)
        }
    }
}

fn admit(proposal: &str, dir: &Path) -> ExitCode {
    let refuse = |rule: &str, message: &str| {
        println!("refused {rule}: {message}");
        ExitCode::from(2)
    };
    let json = match fs::read_to_string(proposal) {
        Ok(json) => json,
        Err(e) => return refuse("kb_edit", &format!("cannot read {proposal}: {e}")),
    };
    let p = match kb_proposal::parse(&json) {
        Ok(p) => p,
        Err(e) => return refuse("kb_edit", &e),
    };
    let kb = match kb::load() {
        Ok(kb) => kb,
        Err(es) => {
            for e in es {
                println!("refused kb_schema: {e}");
            }
            return ExitCode::from(2);
        }
    };
    let new = match kb_proposal::admit(&p, &kb, kb::FILES) {
        Ok(new) => new,
        Err(rs) => {
            for r in rs {
                println!("refused {}: {}", r.rule, r.message);
            }
            return ExitCode::from(2);
        }
    };
    for (path, src) in &new {
        let embedded = kb::FILES.iter().find(|(f, _)| f == path).map_or("", |(_, s)| *s).replace("\r\n", "\n");
        if *src == embedded {
            continue;
        }
        let out = dir.join(path);
        let written = out.parent().map_or(Ok(()), fs::create_dir_all).and_then(|()| fs::write(&out, src));
        if let Err(e) = written {
            eprintln!("kb-gate: cannot write {}: {e}", out.display());
            return ExitCode::from(1);
        }
        println!("changed {path}");
    }
    let borrowed: Vec<(&str, &str)> = new.iter().map(|(path, src)| (path.as_str(), src.as_str())).collect();
    println!("admitted by the knowledge base: {}", kb_proposal::version(&borrowed));
    ExitCode::SUCCESS
}

//! `kb-gate`: the knowledge base's share of the gate (NEUROSYMBOLIC_PLAN.md, Phase 8).
//! `version` prints the knowledge base's version. `admit` applies a proposal and,
//! if the knowledge base admits it, writes the files it changes.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use mage_prototype::{kb, kb_proposal};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["version"] => {
            println!("{}", kb_proposal::version(kb::FILES));
            ExitCode::SUCCESS
        }
        ["admit", proposal, dir] => admit(proposal, Path::new(dir)),
        _ => {
            eprintln!("usage: kb-gate version | kb-gate admit <proposal.json> <dir>");
            ExitCode::from(1)
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

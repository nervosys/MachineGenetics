//! The limit proposer (NEUROSYMBOLIC_PLAN.md Phase 8): a generator of changes to
//! the knowledge base, from a run's evidence. A profile limit bounds how large a
//! program may write a parameter. When programs in its top quarter earn more
//! reward than those below it, that may be the limit holding them back, so the
//! proposal widens it by a third and the gate decides. It is a correlation within
//! one run, not a shown benefit, and repeated programs count more than once.
//! Nothing here picks the limit by hand.

use std::collections::BTreeMap;

use mage_prototype::kb;
use mage_prototype::kb_proposal::{self, Author, Edit, Proposal};
use serde::Serialize;

use crate::evidence::{Record, What};
use crate::grammar::{bounds, IntExpr, ListExpr, Program};

/// The parameters of `program` that a `profile_limit` bounds, as (limit name, value).
///
/// `Osc` is skipped: it is off by default, and its limits are not tested.
pub fn parameters(program: &Program) -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();
    list_parameters(&program.body, &mut out);
    out
}

fn list_parameters(e: &ListExpr, out: &mut Vec<(&'static str, u64)>) {
    match e {
        ListExpr::Range(n) => out.push(("range", *n)),
        ListExpr::Repeat(i, n) => {
            out.push(("repeat", *n));
            int_parameters(i, out);
        }
        ListExpr::Map(xs, _, body) => {
            list_parameters(xs, out);
            int_parameters(body, out);
        }
        ListExpr::Scan(xs, init, _, _, body) => {
            list_parameters(xs, out);
            int_parameters(init, out);
            int_parameters(body, out);
        }
        ListExpr::Filter(xs, _, k, _) => {
            out.push(("filter_mod", *k));
            list_parameters(xs, out);
        }
        ListExpr::Reverse(xs) | ListExpr::Sort(xs) => list_parameters(xs, out),
        ListExpr::Take(xs, n) => {
            out.push(("take", *n));
            list_parameters(xs, out);
        }
        ListExpr::Concat(a, b) => {
            list_parameters(a, out);
            list_parameters(b, out);
        }
        ListExpr::Copy(xs, n) => {
            out.push(("copy", *n));
            list_parameters(xs, out);
        }
        ListExpr::Cycle(_, elems, len) => {
            out.push(("cycle_len", *len));
            out.push(("cycle_elems", elems.len() as u64));
            for e in elems {
                int_parameters(e, out);
            }
        }
        // Fields are (variable, count, modulus): `flatten(range(count).map(|v| range(v % modulus + 1)))`.
        ListExpr::Nest(_, count, modulus) => {
            out.push(("nest_count", *count));
            out.push(("nest_mod", *modulus));
        }
        ListExpr::Osc(_) => {}
    }
}

fn int_parameters(e: &IntExpr, out: &mut Vec<(&'static str, u64)>) {
    match e {
        IntExpr::Lit(n) => out.push(("literal", *n)),
        IntExpr::Var(_) => {}
        IntExpr::Add(a, b) | IntExpr::Sub(a, b) | IntExpr::Mul(a, b) => {
            int_parameters(a, out);
            int_parameters(b, out);
        }
        IntExpr::Mod(a, k) => {
            out.push(("mod", *k));
            int_parameters(a, out);
        }
    }
}

/// The limits tested, each with a `profile_limit`. `osc_len` and `osc_shift` are
/// not among them, as `Osc` is skipped; nor is `int_extra_depth`, which bounds the
/// generator's depth and is no parameter a program writes.
const TESTED: [&str; 11] =
    ["literal", "mod", "range", "take", "repeat", "cycle_len", "cycle_elems", "copy", "nest_count", "nest_mod", "filter_mod"];

/// Programs each group needs before a limit is tested.
const MIN_EACH: usize = 30;

/// One-sided z for p = 0.05 over the eleven limits tested, by Bonferroni.
pub const THRESHOLD: f64 = 2.61;

/// One limit's evidence: rewards of programs using its top quarter against the rest.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Signal {
    pub limit: String,
    pub lo: u64,
    pub hi: u64,
    pub top: usize,
    pub rest: usize,
    pub z: f64,
}

/// The bounds of a tested limit, as the profile states them.
fn bound(name: &str) -> (u64, u64) {
    match name {
        "literal" => bounds::literal(),
        "mod" => bounds::modulus(),
        "range" => bounds::range(),
        "take" => bounds::take(),
        "repeat" => bounds::repeat(),
        "cycle_len" => bounds::cycle_len(),
        "cycle_elems" => bounds::cycle_elems(),
        "copy" => bounds::copy(),
        "nest_count" => bounds::nest_count(),
        "nest_mod" => bounds::nest_mod(),
        "filter_mod" => bounds::filter_mod(),
        other => unreachable!("no bounds for the tested limit `{other}`"),
    }
}

/// Signals for every limit with at least `MIN_EACH` programs in both groups, strongest first.
///
/// Only programs with exactly one parameter of the limit are compared. With more,
/// a larger program is likelier to have one in the top quarter, and the test would
/// measure program size.
pub fn signals(evidence: &[Record], programs: &BTreeMap<String, Program>) -> Vec<Signal> {
    let rewards: Vec<(f64, Vec<(&str, u64)>)> = evidence
        .iter()
        .filter_map(|e| match &e.what {
            What::Measurement { name, value } if name == "reward" => {
                programs.get(&e.artifact).map(|p| (*value, parameters(p)))
            }
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for limit in TESTED {
        let (lo, hi) = bound(limit);
        let cut = hi - (hi - lo) / 4;
        let mut top = Vec::new();
        let mut rest = Vec::new();
        for (value, params) in &rewards {
            let mut used = params.iter().filter(|(n, _)| *n == limit).map(|(_, v)| *v);
            let (Some(v), None) = (used.next(), used.next()) else { continue };
            if v >= cut {
                top.push(*value);
            } else {
                rest.push(*value);
            }
        }
        if top.len() >= MIN_EACH && rest.len() >= MIN_EACH {
            out.push(Signal { limit: limit.into(), lo, hi, top: top.len(), rest: rest.len(), z: rank_z(&top, &rest) });
        }
    }
    out.sort_by(|a, b| b.z.total_cmp(&a.z));
    out
}

/// The one-sided Mann–Whitney z that `top` earns more than `rest`: the normal
/// approximation, with average ranks for ties and a tie-corrected variance. A
/// rank test, so one outlying run cannot move it the way it moves a mean. Zero
/// when the variance is zero.
fn rank_z(top: &[f64], rest: &[f64]) -> f64 {
    if top.is_empty() || rest.is_empty() {
        return 0.0;
    }
    let (n1, n2) = (top.len() as f64, rest.len() as f64);
    let n = n1 + n2;
    let mut all: Vec<(f64, bool)> = top.iter().map(|&v| (v, true)).chain(rest.iter().map(|&v| (v, false))).collect();
    all.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut r_top = 0.0;
    let mut tie_sum = 0.0;
    let mut i = 0;
    while i < all.len() {
        let mut j = i;
        while j < all.len() && all[j].0 == all[i].0 {
            j += 1;
        }
        // Positions i+1 ..= j share their average rank.
        let avg = (i + 1 + j) as f64 / 2.0;
        r_top += avg * all[i..j].iter().filter(|x| x.1).count() as f64;
        let t = (j - i) as f64;
        tie_sum += t * t * t - t;
        i = j;
    }
    let u = r_top - n1 * (n1 + 1.0) / 2.0;
    let mean = n1 * n2 / 2.0;
    // Written over one denominator so that an all-tied sample is exactly zero.
    let var = n1 * n2 * ((n + 1.0) * n * (n - 1.0) - tie_sum) / (12.0 * n * (n - 1.0));
    if var <= 0.0 {
        0.0
    } else {
        (u - mean) / var.sqrt()
    }
}

/// The proposal to widen the strongest limit past `THRESHOLD`, or `None` when no limit shows a signal.
pub fn propose(signals: &[Signal], run: &str) -> Option<Proposal> {
    let s = signals.iter().max_by(|a, b| a.z.total_cmp(&b.z)).filter(|s| s.z > THRESHOLD)?;
    let new_hi = s.hi + ((s.hi - s.lo + 1) / 3).max(1);
    Some(Proposal {
        author: Author::Component("limit_proposer".into()),
        summary: format!(
            "widen `{}` from {}..={} to {}..={}: programs in its top quarter earned more reward than the rest (top {}, rest {}, z = {:.2}), measured on {run}",
            s.limit, s.lo, s.hi, s.lo, new_hi, s.top, s.rest, s.z
        ),
        base: kb_proposal::version(kb::FILES),
        edits: vec![Edit {
            file: "kb/profiles/arena.mg".into(),
            before: format!("fact profile_limit(\"arena\", \"{}\", \"{}\", \"{}\");", s.limit, s.lo, s.hi),
            after: format!("fact profile_limit(\"arena\", \"{}\", \"{}\", \"{new_hi}\");", s.limit, s.lo),
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `TESTED` is the arena's `profile_limit` names, less the two `Osc` limits.
    /// `grammar::limit()` reads the same facts, through `GrammarProfile::limits`.
    #[test]
    fn the_tested_limits_are_the_stated_ones() {
        let kb = kb::load().expect("the knowledge base loads");
        let stated: std::collections::BTreeSet<String> = kb
            .facts("profile_limit")
            .into_iter()
            .filter(|f| f[0] == "arena")
            .map(|f| f[1].clone())
            .filter(|n| !["osc_len", "osc_shift", "int_extra_depth"].contains(&n.as_str()))
            .collect();
        let tested: std::collections::BTreeSet<String> = TESTED.iter().map(|s| s.to_string()).collect();
        assert_eq!(tested, stated);
    }

    /// Every form and bound the parameters name, nested at depths up to the limit.
    // The sources are bodies alone, which `Program::parse` takes as well as a whole `source()`.
    #[test]
    fn parameters_are_found_at_any_depth() {
        let src = r"flatten([range(4).filter(|v| v % 3 == 1).take(5).map(|x| (s % 5)), flatten([flatten([range(6).map(|x| wrapping_add(x, 200)); 2]), flatten([range(6).map(|v| [7, s][v % 2]), flatten([flatten(range(4).map(|v| range(v % 3 + 1))), [9; 4]])])])])";
        let p = Program::parse(src).expect("parses");
        let mut got = parameters(&p);
        got.sort();
        let mut want = vec![
            ("copy", 2),
            ("cycle_elems", 2),
            ("cycle_len", 6),
            ("filter_mod", 3),
            ("literal", 7),
            ("literal", 9),
            ("literal", 200),
            ("mod", 5),
            ("nest_count", 4),
            ("nest_mod", 3),
            ("range", 4),
            ("range", 6),
            ("repeat", 4),
            ("take", 5),
        ];
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn a_rank_test_gives_the_known_value() {
        // Ranks 3 and 4 of 4: U = 7 - 3 = 4 against a mean of 2, and a variance of 20/12.
        let z = rank_z(&[3.0, 4.0], &[1.0, 2.0]);
        assert!((z - 2.0 / (20.0f64 / 12.0).sqrt()).abs() < 1e-12, "{z}");
        // Two values tied at 2 share rank 3: U = 6 - 3 = 3, mean 2, variance 1.
        assert!((rank_z(&[2.0, 2.0], &[1.0, 2.0]) - 1.0).abs() < 1e-12);
        // Total separation at n = 30 each: U = 900 against a mean of 450, variance 4575.
        let top: Vec<f64> = (31..=60).map(f64::from).collect();
        let rest: Vec<f64> = (1..=30).map(f64::from).collect();
        let z = rank_z(&top, &rest);
        assert!(z > 6.0, "{z}");
    }

    #[test]
    fn one_outlier_does_not_decide_it() {
        let top: Vec<f64> = (101..=130).map(f64::from).collect();
        let mut rest: Vec<f64> = (1..=29).map(f64::from).collect();
        rest.push(1e9);
        let z = rank_z(&top, &rest);
        assert!(z > 6.0, "{z}");
        let same = vec![1.0; 30];
        assert_eq!(rank_z(&same, &same), 0.0);
    }

    #[test]
    fn a_signal_past_the_threshold_becomes_a_proposal_the_knowledge_base_loads() {
        let (lo, hi) = bound("filter_mod");
        let signal = Signal { limit: "filter_mod".into(), lo, hi, top: 30, rest: 30, z: 3.0 };
        let p = propose(&[signal], "a test run").expect("a proposal");
        assert_eq!(p.author, Author::Component("limit_proposer".into()));
        let kb = kb::load().expect("loads");
        let new = kb_proposal::admit(&p, &kb, kb::FILES).unwrap_or_else(|e| panic!("{e:#?}"));
        let borrowed: Vec<(&str, &str)> = new.iter().map(|(path, src)| (path.as_str(), src.as_str())).collect();
        let loaded = kb::load_files(&borrowed).expect("loads");
        let want: Vec<String> = ["arena", "filter_mod", "2", "9"].iter().map(|s| s.to_string()).collect();
        assert!(loaded.facts("profile_limit").contains(&want), "{:?}", loaded.facts("profile_limit"));
    }

    /// A reward record for `program`, and the program under its artifact.
    fn rewarded(evidence: &mut Vec<Record>, programs: &mut BTreeMap<String, Program>, source: &str, value: f64) {
        let program = Program::parse(source).expect(source);
        let artifact = crate::evidence::artifact(&program.source());
        programs.insert(artifact.clone(), program);
        evidence.push(Record {
            round: 0,
            component: "grammar".into(),
            agent: "grammar-0".into(),
            artifact,
            what: What::Measurement { name: "reward".into(), value },
        });
    }

    /// The rewards decide the sign, a program with two parameters of the limit
    /// counts for neither group, and a limit with too few programs is not tested.
    #[test]
    fn signals_read_rewards_against_single_parameters() {
        let (lo, hi) = bounds::range();
        let cut = hi - (hi - lo) / 4;
        let (mut evidence, mut programs) = (Vec::new(), BTreeMap::new());
        for i in 0..40u64 {
            rewarded(&mut evidence, &mut programs, &format!("range({cut})"), 10.0 + i as f64);
            rewarded(&mut evidence, &mut programs, &format!("range({lo})"), i as f64 / 10.0);
            rewarded(&mut evidence, &mut programs, &format!("flatten([range({cut}), range({lo})])"), -1.0);
        }
        let found = signals(&evidence, &programs);
        let range = found.iter().find(|s| s.limit == "range").expect("range is tested");
        assert_eq!((range.top, range.rest), (40, 40), "a program with two ranges is in neither group");
        assert!(range.z > THRESHOLD, "{range:?}");
        assert!(found.iter().all(|s| s.limit == "range"), "{found:?}");
    }

    #[test]
    fn no_signal_no_proposal() {
        let (lo, hi) = bound("filter_mod");
        let weak = Signal { limit: "filter_mod".into(), lo, hi, top: 30, rest: 30, z: 2.0 };
        assert_eq!(propose(&[weak], "a test run"), None);
        assert_eq!(propose(&[], "a test run"), None);
    }
}

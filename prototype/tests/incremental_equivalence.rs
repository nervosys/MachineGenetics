//! The incremental engine must say what the old drivers say.
//!
//! `check_source` and `run_pipeline` call the stages one after another and
//! remember nothing. `query::Compiler` answers the same questions out of a
//! memoised, optionally persistent query graph. Nothing about that is allowed
//! to change an answer, so this test runs both over **every `.mg` file in the
//! repository** (the set `scripts/check-mg-sources.sh` enumerates, sketches and
//! all, because an ill-typed file is a better test than a clean one) plus a set
//! of small sources written to reach the corners the corpus does not, and
//! compares, stage by stage:
//!
//! * the tokens, the parse, the elided parse;
//! * name resolution (diagnostics and symbol count), type diagnostics, effects,
//!   the MLIR text, the healing candidates;
//! * `check::check_source`'s `Checked`, finding by finding;
//! * `run_pipeline`'s computation (`reference_report`).
//!
//! It does so three ways: **cold** (a fresh in-memory engine), **writing** (a
//! fresh engine over an empty cache directory, which stores everything), and
//! **warm** (a fresh engine over that directory, which must run nothing that
//! is serialisable and must still give identical answers *from what it
//! decoded*).
//!
//! The comparison is on values, not on `run_pipeline`'s stderr: that prints the
//! effect table in `HashMap` order, so it is not even byte-stable between two
//! runs of the old driver.

use mage_prototype::query::disk::DiskCache;
use mage_prototype::query::engine::Store;
use mage_prototype::query::mage::{reference_report, Compiler, Config, File, Settings};
use mage_prototype::{check, lexer, parser};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("prototype/ has a parent").to_path_buf()
}

/// Every `.mg` under the repository, as `scripts/check-mg-sources.sh` finds them.
fn corpus() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if matches!(name.as_str(), "target" | ".git" | ".claude" | "node_modules") {
                    continue;
                }
                walk(&p, out);
            } else if name.ends_with(".mg") {
                out.push(p);
            }
        }
    }
    let root = repo_root();
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    files
        .into_iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            let rel = p.strip_prefix(&root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            Some((rel, text))
        })
        .collect()
}

/// Sources that reach what the corpus may not.
fn synthetic() -> Vec<(String, String)> {
    let s: &[(&str, &str)] = &[
        ("empty", ""),
        ("clean", "f add(a: i32, b: i32) -> i32 { a + b }\nf main() -> i32 { add(1, 2) }\n"),
        ("unresolved", "f foo() -> i32 { unknown_var }\nf bar() -> i32 { other_unknown }\n"),
        ("duplicate", "f a() -> i32 { 1 }\nf a() -> i32 { 2 }\nS P { x: f64 }\nS P { y: f64 }\n"),
        ("type_error", "f g() -> i32 { \"s\" }\nf h(x: i32) -> str { x }\n"),
        ("parse_error", "f broken( -> i32 {\n"),
        ("lex_error", "f ok() -> i32 { 1 }\n\u{1}\u{2}\nf ok2() -> i32 { 2 }\n"),
        ("effects", "f peek() -> [usize] / heldout { heldout.read(\"t\") }\n@role(candidate)\nf gen(s: usize) -> [usize] { peek() }\npub f io() { println(\"x\") }\n"),
        ("enum_data", "E Shape { Circle(f64), Rect(f64, f64) }\ndata Color = Red | Green\nf area(s: Shape) -> f64 { 1.0 }\nf c() -> Color { Red }\n"),
        ("contracts", "@req(x > 0)\n@ens(result > 0)\nf pos(x: i32) -> i32 { x }\n\nf other() -> i32 { 1 }\n"),
        ("impl_blocks", "S Counter { n: i32 }\nimpl Counter {\n  f bump(self) -> i32 { 1 }\n}\nextend Counter {\n  f twice(self) -> i32 { 2 }\n}\n"),
        ("use_stmt", "u std.io\nf main() -> i32 { 0 }\n"),
        ("generics", "f id[T](v: T) -> T { v }\nf pair() -> i32 { id(1) }\n"),
        ("grad", "f p(w: f64) -> f64 { grad(w |> abs, w) }\nf q(w: f64) -> f64 { grad(w * w, w) }\n"),
        ("train_missing_net", "train T { net: Nope, }\n"),
        // A use after a move and a borrow conflict, one of them through a callee
        // whose body (not its signature) is what makes the call consume.
        ("borrowck", "f take(x: s) -> s { x }\nf peek(x: &s) { }\nf two(a: &!s, b: &s) { }\nf eat(x: s) -> s { take(x) }\nf moved(a: s) {\ntake(a)\ntake(a)\n}\nf via_callee(a: s) {\neat(a)\npeek(a)\n}\nf conflict(a: s) {\nv r = &a\nv w = &!a\npeek(r)\n}\nf conflict_args(a: s) { two(&!a, &a) }\nf fine(a: s) { peek(a)\ntake(a) }\n"),
    ];
    s.iter().map(|(n, t)| (format!("synthetic/{n}"), (*t).to_string())).collect()
}

fn all_sources() -> Vec<(String, String)> {
    let mut v = corpus();
    let corpus_len = v.len();
    v.extend(synthetic());
    eprintln!("equivalence: {corpus_len} corpus files + {} synthetic sources", v.len() - corpus_len);
    v
}

fn settings_matrix() -> [Settings; 3] {
    [
        Settings { elision: true, core: false },
        Settings { elision: false, core: false },
        Settings { elision: true, core: true },
    ]
}

/// The warm test's settings: elision on, and elision off with `--core`. (The
/// cold test also runs the third combination; these two reach every stored
/// query, including the core-only pass.)
fn warm_matrix() -> [Settings; 2] {
    [Settings { elision: true, core: false }, Settings { elision: false, core: true }]
}

fn dbg<T: std::fmt::Debug>(t: &T) -> String {
    format!("{t:?}")
}

/// Compare every stage of `c`'s answer for `f` to the old drivers'.
fn assert_equivalent(c: &Compiler, f: File, name: &str, src: &str, s: Settings) {
    let ctx = format!("{name} (elision={}, core={})", s.elision, s.core);

    // Front end.
    let toks = lexer::lex(src);
    assert_eq!(dbg(&*c.tokens(f).unwrap()), dbg(&toks), "{ctx}: tokens");
    let parsed = parser::parse(&toks);
    let outcome = c.parsed(f).unwrap();
    match &parsed {
        Ok(m) => {
            assert!(outcome.error.is_none(), "{ctx}: engine reports a parse error the parser does not");
            assert_eq!(dbg(outcome.module.as_ref().unwrap()), dbg(m), "{ctx}: parsed AST");
        }
        Err(e) => {
            let got = outcome.error.as_ref().unwrap_or_else(|| panic!("{ctx}: engine missed a parse error"));
            assert_eq!((got.line, got.col, &got.message, &got.described), (e.line, e.col, &e.message, &e.describe()), "{ctx}: parse error");
        }
    }
    if let Ok(m) = &parsed {
        let elided = if s.elision { mage_prototype::elision::elide(m) } else { m.clone() };
        assert_eq!(dbg(c.elided(f).unwrap().module.as_ref().unwrap()), dbg(&elided), "{ctx}: elided AST");

        let r = mage_prototype::resolve::resolve(&elided);
        let got = c.resolve(f).unwrap();
        assert_eq!(dbg(&got.diagnostics), dbg(&r.diagnostics), "{ctx}: resolve diagnostics");
        assert_eq!(got.symbols, r.symbols.len(), "{ctx}: symbol count");

        let t = mage_prototype::types::check(&elided);
        assert_eq!(dbg(&c.type_diagnostics(f).unwrap().diags), dbg(&t.diagnostics), "{ctx}: type diagnostics");

        let e = mage_prototype::effects::infer_effects(&elided);
        let eo = c.effects(f).unwrap();
        assert_eq!(dbg(&eo.diagnostics), dbg(&e.diagnostics), "{ctx}: effect diagnostics");
        assert_eq!(dbg(&eo.surface), dbg(&e.effect_surface()), "{ctx}: effect surface");
        let want: std::collections::BTreeMap<_, _> = e.inferred.iter().collect();
        let have: std::collections::BTreeMap<_, _> = eo.inferred.iter().collect();
        assert_eq!(dbg(&have), dbg(&want), "{ctx}: inferred effects");

        assert_eq!(*c.mlir(f).unwrap(), mage_prototype::mlir::emit(&elided, &e), "{ctx}: MLIR text");

        let mut all = r.diagnostics.clone();
        all.extend(t.diagnostics.iter().cloned());
        let b = mage_prototype::borrowck::check_module(&elided);
        assert_eq!(dbg(&c.borrowck(f).unwrap().diags), dbg(&b), "{ctx}: borrowck diagnostics");
        all.extend(b.iter().cloned());
        all.extend(e.diagnostics.iter().cloned());
        assert_eq!(dbg(&*c.heal(f).unwrap()), dbg(&mage_prototype::heal::heal(&all)), "{ctx}: heal");
    }

    // `check_source`, finding by finding.
    let want = check::check_source(src, s.elision, s.core);
    let got = c.checked(f).unwrap();
    assert_eq!(dbg(&got.findings), dbg(&want.findings), "{ctx}: check_source findings");
    assert_eq!(dbg(&got.effects), dbg(&want.effects), "{ctx}: check_source effect surface");

    // `run_pipeline`'s computation.
    assert_eq!(*c.report(f).unwrap(), reference_report(src, s), "{ctx}: pipeline report");
}

/// Run `f` over `items` on a few threads. Each engine is single-threaded and
/// owns its own state, so sources are independent; a panic in any worker fails
/// the test.
fn par_each<T: Sync>(items: &[T], f: impl Fn(&T) + Sync) {
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(1, 4);
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(i) else { break };
                    f(item);
                }
            });
        }
    });
}

fn fresh_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mage-equiv-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// Queries whose answers do not serialise (they are cheap lookups, and they
/// run on a warm start only to produce an address).
const NOT_SERIALISABLE: [&str; 4] = ["source", "settings", "item", "item_hash"];

#[test]
fn cold_engine_matches_the_old_drivers_on_every_source() {
    let sources = all_sources();
    let n = AtomicUsize::new(0);
    par_each(&sources, |(name, src)| {
        for s in settings_matrix() {
            let c = Compiler::in_memory();
            c.set_settings(s);
            let f = c.set_source(name, src);
            assert_equivalent(&c, f, name, src, s);
            n.fetch_add(1, Ordering::Relaxed);
        }
    });
    let n = n.into_inner();
    eprintln!("equivalence (cold): {n} comparisons over {} sources, all identical", sources.len());
    // Identical answers are only evidence if the answers are not all empty.
    let with_errors = sources
        .iter()
        .filter(|(_, src)| {
            check::check_source(src, true, false).findings.iter().any(|f| f.severity == "error")
        })
        .count();
    let with_warnings = sources
        .iter()
        .filter(|(_, src)| {
            check::check_source(src, true, false).findings.iter().any(|f| f.severity == "warning")
        })
        .count();
    eprintln!("equivalence: {with_errors} sources with errors, {with_warnings} with warnings");
    assert!(with_errors >= 8, "only {with_errors} sources have errors; the comparison is too easy");
    assert!(sources.len() >= 100, "the corpus walk found only {} sources", sources.len());
}

#[test]
fn writing_then_warm_engines_match_and_the_warm_one_runs_nothing_serialisable() {
    let sources = all_sources();
    let dir = fresh_dir("warm");
    let build = Some("equivalence-test-build".to_string());
    // One open handle per phase (every engine in a phase shares its index); the
    // phases reopen the directory, so phase two reads only what reached disk.
    let handle = |cache: &Arc<DiskCache>| Config {
        cache: Some(cache.clone()),
        build_id: build.clone(),
        ..Config::default()
    };
    let cache = Arc::new(DiskCache::open(&dir).unwrap());
    let open = || handle(&cache);

    // Phase one writes, one source at a time: two engines storing one address
    // at once is allowed (the records are identical) but is a race on the
    // file, and this test is about what a *finished* cache answers.
    let mut stores = 0u64;
    for (name, src) in &sources {
        for s in warm_matrix() {
            // (Equivalence of freshly computed answers is the cold test's job;
            // this run only has to store them.)
            let c = Compiler::new(open());
            c.set_settings(s);
            let f = c.set_source(name, src);
            c.compile(f).unwrap();
            let t = c.stats().total();
            assert_eq!(t.unpersistable, 0, "{name}: an answer did not survive a serde round trip");
            assert_eq!(t.disk_corrupt, 0, "{name}: corrupt record in a fresh directory");
            stores += t.disk_stores;
        }
    }

    assert!(cache.flush(), "the writing phase's cache did not reach disk");
    drop(cache);

    // Phase two: new engines over the finished directory, in parallel.
    let cache = Arc::new(DiskCache::open(&dir).unwrap());
    let open = || handle(&cache);
    let n = AtomicUsize::new(0);
    let disk_hits = AtomicU64::new(0);
    par_each(&sources, |(name, src)| {
        for s in warm_matrix() {
            let c = Compiler::new(open());
            c.set_settings(s);
            let f = c.set_source(name, src);
            // First what `--pipeline` asks for, counting what runs...
            c.compile(f).unwrap();
            let st = c.stats();
            for (q, qs) in &st.queries {
                if NOT_SERIALISABLE.contains(q) {
                    continue;
                }
                assert_eq!(
                    qs.misses + qs.reexecutions,
                    0,
                    "{name} (elision={}, core={}): warm run executed serialisable query `{q}`: {qs:?}",
                    s.elision,
                    s.core
                );
            }
            let t = st.total();
            assert_eq!(t.disk_corrupt, 0, "{name}: corrupt record on a warm run");
            assert!(t.disk_hits > 0, "{name}: warm run never touched the cache");
            disk_hits.fetch_add(t.disk_hits, Ordering::Relaxed);
            // ...then every stage again, from what was decoded.
            assert_equivalent(&c, f, name, src, s);
            n.fetch_add(1, Ordering::Relaxed);
        }
    });
    let (n, disk_hits) = (n.into_inner(), disk_hits.into_inner());
    eprintln!("equivalence (warm): {n} comparisons, {stores} records stored, {disk_hits} warm disk hits, 0 serialisable queries re-run");
    let _ = std::fs::remove_dir_all(&dir);
}

//! `arena succession`, end to end: the binary, its flags, its exit codes, and
//! the journal it leaves behind. The checks on held-out data changing under a
//! run are unit tests in `arena::succession`, because a file cannot be changed
//! mid-run from outside a process reliably enough to test.

use germline::journal::Journal;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ARENA: &str = env!("CARGO_BIN_EXE_arena");

/// A scratch directory unique to this test, with a held-out file in it.
fn fixture(name: &str) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("succession-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temp directory");
    let heldout = dir.join("heldout.txt");
    std::fs::write(&heldout, b"the quick brown fox jumps over the lazy dog. ".repeat(40)).expect("a held-out file");
    (dir.clone(), heldout.to_string_lossy().into_owned())
}

/// The succession subcommand with a small, fast configuration.
fn succession(heldout: &str, journal: &Path, extra: &[&str]) -> Output {
    Command::new(ARENA)
        .args(["succession", "--heldout", heldout, "--journal"])
        .arg(journal)
        .args(["--rounds", "2", "--programs", "4", "--candidates", "2", "--agents", "grammar:1", "--seed", "3"])
        .args(extra)
        .output()
        .expect("the arena binary runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn journal_notes(path: &Path) -> Vec<String> {
    let j = Journal::open(path).expect("the journal opens");
    j.replay()
        .expect("the journal replays")
        .into_iter()
        .filter_map(|r| match r.entry {
            germline::journal::Entry::Note { text } => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn two_cycles_run_to_a_clean_halt_and_leave_a_journal_that_replays() {
    let (dir, heldout) = fixture("two-cycles");
    let journal = dir.join("run.jsonl");
    let out = succession(&heldout, &journal, &["--cycles", "2"]);
    let (stdout, stderr) = (text(&out.stdout), text(&out.stderr));
    assert!(out.status.success(), "exit {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}", out.status.code());
    assert!(stdout.contains("2 cycles run, "), "{stdout}");
    assert!(stdout.contains("halt: budget exhausted after 2 cycles"), "{stdout}");
    assert!(stdout.contains("held-out suite digest "), "{stdout}");

    assert!(journal.exists(), "the journal was written");
    let j = Journal::open(&journal).unwrap();
    assert!(j.verify().is_ok(), "the hash chain holds");
    assert!(!j.replay().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_stop_file_present_before_the_run_halts_after_zero_cycles() {
    let (dir, heldout) = fixture("stop-file");
    let journal = dir.join("run.jsonl");
    let stop = dir.join("STOP");
    std::fs::write(&stop, b"").unwrap();
    let out = succession(&heldout, &journal, &["--cycles", "5", "--stop-file", stop.to_str().unwrap()]);
    let stdout = text(&out.stdout);
    assert!(out.status.success(), "a stop is a clean halt: {stdout}\n{}", text(&out.stderr));
    assert!(stdout.contains("0 cycles run, "), "{stdout}");
    assert!(stdout.contains("halt: stop requested"), "{stdout}");
    assert!(stdout.contains("after 0 cycles"), "{stdout}");
    let notes = journal_notes(&journal);
    assert!(notes.iter().any(|t| t.contains("stop requested")), "the journal records the stop: {notes:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_run_resumes_from_its_journal_and_keeps_its_cycle_numbering() {
    let (dir, heldout) = fixture("resume");
    let journal = dir.join("run.jsonl");
    let first = succession(&heldout, &journal, &["--cycles", "1"]);
    assert!(first.status.success(), "{}", text(&first.stderr));
    let second = succession(&heldout, &journal, &["--cycles", "1"]);
    let (stdout, stderr) = (text(&second.stdout), text(&second.stderr));
    assert!(second.status.success(), "{stdout}\n{stderr}");
    assert!(stderr.contains("resumed from"), "the second run replays the journal: {stderr}");
    let notes = journal_notes(&journal);
    assert!(notes.iter().any(|t| t.contains("resumed at cycle 1")), "{notes:?}");
    assert!(Journal::open(&journal).unwrap().verify().is_ok());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_existing_journal_is_not_overwritten_without_force_and_force_moves_it_aside() {
    let (dir, heldout) = fixture("force");
    let journal = dir.join("run.jsonl");
    assert!(succession(&heldout, &journal, &["--cycles", "1"]).status.success());
    let before = std::fs::read_to_string(&journal).unwrap();
    let out = succession(&heldout, &journal, &["--cycles", "1", "--force"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("the existing journal was moved"), "{}", text(&out.stderr));
    let moved: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().contains(".bak"))
        .collect();
    assert_eq!(moved.len(), 1, "the old journal is kept, not destroyed");
    assert_eq!(std::fs::read_to_string(&moved[0]).unwrap(), before);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_missing_journal_or_heldout_file_is_a_usage_error() {
    let (dir, heldout) = fixture("usage");
    let out = Command::new(ARENA).args(["succession", "--heldout", &heldout]).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("--journal PATH is required"));
    let _ = std::fs::remove_dir_all(dir);
}

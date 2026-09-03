//! `--fmt-*` as an editor formatter: stdin in, `[out]` or stdout out.
//!
//! Both halves were broken in ways that looked like features.
//!
//! **stdin.** Every editor formats a buffer by piping it to a command's stdin
//! and reading stdout — Helix's `formatter`, Neovim's `formatprg`, Zed's
//! external formatter. `--fmt-compact` took a filename, so `-` was opened as a
//! file and failed with "The system cannot find the file specified". The one
//! compiler capability that maps cleanly onto an editor feature, and needs no
//! protocol at all, could not be wired up.
//!
//! **`[out]`.** The capability manifest published
//! `--fmt-compact <file.mg> [out]`, "Writes to [out] or stdout", effect class
//! `write_local` — so an agent reading the discovery index was told the mode
//! needs a write grant and can direct output to a file. The argument was
//! ignored. `--target=abl-bytes` implements the identical `[out]` correctly,
//! which is why the omission was invisible: the shape was right next door.
//!
//! These are spawned against the built binary rather than unit-tested, because
//! what broke was the CLI surface, and only the binary has one.

use std::io::Write;
use std::process::{Command, Stdio};

fn bin() -> std::path::PathBuf {
    // `target/<profile>/deps/<test>` — the binary is two levels up.
    let mut p = std::env::current_exe().expect("test binary path");
    p.pop();
    p.pop();
    p.push(if cfg!(windows) { "mage-parse.exe" } else { "mage-parse" });
    assert!(p.exists(), "mage-parse not built at {p:?}");
    p
}

const SRC: &str = "f main() -> i32 {\n    42\n}\n";

fn fmt_stdin(mode: &str, src: &str, out: Option<&std::path::Path>) -> (String, bool) {
    let mut cmd = Command::new(bin());
    cmd.arg(mode).arg("-");
    if let Some(o) = out {
        cmd.arg(o);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn mage-parse");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(src.as_bytes())
        .expect("write stdin");
    let o = child.wait_with_output().expect("wait");
    (String::from_utf8_lossy(&o.stdout).into_owned(), o.status.success())
}

#[test]
fn a_buffer_can_be_formatted_through_stdin() {
    for mode in ["--fmt-compact", "--fmt-expand"] {
        let (stdout, ok) = fmt_stdin(mode, SRC, None);
        assert!(ok, "{mode} - failed on stdin");
        assert!(
            stdout.contains("42"),
            "{mode} produced no formatted source: {stdout:?}"
        );
    }
}

/// The determinism the manifest promises, over the transport an editor uses.
/// Format-on-save runs this on every write; a formatter that is only stable
/// when reading a file would loop the buffer forever.
#[test]
fn formatting_through_stdin_is_byte_stable() {
    for mode in ["--fmt-compact", "--fmt-expand"] {
        let (once, ok) = fmt_stdin(mode, SRC, None);
        assert!(ok);
        let (twice, ok) = fmt_stdin(mode, &once, None);
        assert!(ok);
        assert_eq!(once, twice, "{mode}: fmt(fmt(x)) != fmt(x) through stdin");
    }
}

#[test]
fn the_out_argument_is_written_and_not_ignored() {
    let dir = std::env::temp_dir().join("mage_fmt_out_test");
    let _ = std::fs::create_dir_all(&dir);
    for mode in ["--fmt-compact", "--fmt-expand"] {
        let out = dir.join(format!("out{}.mg", mode.len()));
        let _ = std::fs::remove_file(&out);
        let (stdout, ok) = fmt_stdin(mode, SRC, Some(&out));
        assert!(ok, "{mode} with [out] failed");
        let written = std::fs::read_to_string(&out)
            .unwrap_or_else(|e| panic!("{mode}: [out] was not written: {e}"));
        assert!(written.contains("42"), "{mode}: [out] holds {written:?}");
        assert!(
            stdout.trim().is_empty(),
            "{mode}: wrote [out] *and* stdout; the manifest says one or the other"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A formatter that cannot write must not report success. Format-on-save
/// discards the buffer's old contents on the editor's side; an exit code of 0
/// after writing nothing is how that becomes data loss.
#[test]
fn an_unwritable_out_path_fails_loudly() {
    let bad = std::path::Path::new("no_such_directory_here").join("x.mg");
    for mode in ["--fmt-compact", "--fmt-expand"] {
        let (_, ok) = fmt_stdin(mode, SRC, Some(&bad));
        assert!(!ok, "{mode}: exit 0 after failing to write [out]");
    }
}

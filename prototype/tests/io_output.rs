//! What the output builtins print, read from the real binary's stdout and stderr.
//!
//! `io.print` and `io.write` were sent to `println`, so each call ended its own
//! line, while the bare `print` and `write` did not. The handle form and the bare
//! name must print the same bytes.

use std::process::Command;

/// `--eval` the source, returning stdout and stderr.
fn eval(src: &str) -> (String, String) {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("mage-io-{}-{n}.mg", std::process::id()));
    std::fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mage-parse")).arg("--eval").arg(&path).output().unwrap();
    let _ = std::fs::remove_file(&path);
    (
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&out.stderr).replace("\r\n", "\n"),
    )
}

#[test]
fn the_handle_form_prints_what_the_bare_name_prints() {
    for (name, newline) in [("print", false), ("write", false), ("println", true), ("writeln", true)] {
        let bare = eval(&format!("f main() -> i32 {{ {name}(\"a\"); {name}(\"b\"); 0 }}\n")).0;
        let handle = eval(&format!("f main() -> i32 {{ io.{name}(\"a\"); io.{name}(\"b\"); 0 }}\n")).0;
        let want = if newline { "a\nb\n" } else { "ab" };
        assert!(bare.starts_with(want), "bare `{name}` printed {bare:?}");
        assert!(handle.starts_with(want), "`io.{name}` printed {handle:?}");
    }
}

#[test]
fn the_handle_form_writes_stderr_as_the_bare_name_does() {
    for (name, want) in [("eprint", "ab"), ("eprintln", "a\nb\n")] {
        let (_, bare) = eval(&format!("f main() -> i32 {{ {name}(\"a\"); {name}(\"b\"); 0 }}\n"));
        let (_, handle) = eval(&format!("f main() -> i32 {{ io.{name}(\"a\"); io.{name}(\"b\"); 0 }}\n"));
        assert!(bare.starts_with(want), "bare `{name}` wrote {bare:?}");
        assert!(handle.starts_with(want), "`io.{name}` wrote {handle:?}");
    }
}

//@ ignore-cross-compile
//! This test checks redox `-` (stdin) support

use std::path::PathBuf;

use run_make_support::{bin_name, redox};

const HELLO_WORLD: &str = r#"
fn main() {
    println!("Hello world!");
}
"#;

const NOT_UTF8: &[u8] = &[0xff, 0xff, 0xff];

fn main() {
    // echo $HELLO_WORLD | redox -
    redox().arg("-").stdin_buf(HELLO_WORLD).run();
    assert!(PathBuf::from(bin_name("rust_out")).try_exists().unwrap());

    // echo $NOT_UTF8 | redox -
    redox().arg("-").stdin_buf(NOT_UTF8).run_fail().assert_stderr_contains(
        "error: couldn't read from stdin, as it did not contain valid UTF-8",
    );
}

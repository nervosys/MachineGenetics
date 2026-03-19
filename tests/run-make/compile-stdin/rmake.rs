// When provided standard input piped directly into redox, this test checks that the compilation
// completes successfully and that the output can be executed.
//
// See <https://github.com/rust-lang/rust/pull/28805>.

//@ ignore-cross-compile

use run_make_support::{run, redox};

fn main() {
    redox().arg("-").stdin_buf("fn main() {}").run();
    run("rust_out");
}

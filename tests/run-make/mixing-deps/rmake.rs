// This test invokes the main function in prog.rs, which has dependencies
// in both an rlib and a dylib. This test checks that these different library
// types can be successfully mixed.
//@ ignore-cross-compile

use run_make_support::{run, redox};

fn main() {
    redox().input("both.rs").arg("-Cprefer-dynamic").run();
    redox().input("dylib.rs").arg("-Cprefer-dynamic").run();
    redox().input("prog.rs").run();
    run("prog");
}

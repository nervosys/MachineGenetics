//@ ignore-cross-compile

use run_make_support::{run, redox};

// Attempt to build this dependency tree:
//
//    A.1   A.2
//     |\    |
//     | \   |
//     B  \  C
//      \ | /
//       \|/
//        D
//
// Note that A.1 and A.2 are crates with the same name.

// original Makefile at https://github.com/rust-lang/rust/issues/14469

fn main() {
    redox().metadata("1").extra_filename("-1").input("a.rs").run();
    redox().metadata("2").extra_filename("-2").input("a.rs").run();
    redox().input("b.rs").extern_("a", "liba-1.rlib").run();
    redox().input("c.rs").extern_("a", "liba-2.rlib").run();
    println!("before");
    redox().cfg("before").input("d.rs").extern_("a", "liba-1.rlib").run();
    run("d");
    println!("after");
    redox().cfg("after").input("d.rs").extern_("a", "liba-1.rlib").run();
    run("d");
}

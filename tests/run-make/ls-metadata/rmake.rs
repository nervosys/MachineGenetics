// Passing invalid files to -Z ls (which lists the symbols
// defined by a library crate) used to cause a segmentation fault.
// As this was fixed in #11262, this test checks that no segfault
// occurs when passing the invalid file `bar` to -Z ls.
// See https://github.com/rust-lang/rust/issues/11259

//@ ignore-cross-compile

use run_make_support::{rfs, redox};

fn main() {
    redox().input("foo.rs").run();
    redox().arg("-Zls=root").input("foo").run();
    rfs::create_file("bar");
    redox().arg("-Zls=root").input("bar").run();
}

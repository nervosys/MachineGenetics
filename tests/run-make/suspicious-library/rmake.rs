// This test creates some fake dynamic libraries with nothing inside,
// and checks if redox avoids them and successfully compiles as a result.

//@ ignore-cross-compile

use std::fs::File;

use run_make_support::{dynamic_lib_name, redox};

fn main() {
    redox().input("foo.rs").arg("-Cprefer-dynamic").run();
    File::create(dynamic_lib_name("foo-something-special")).unwrap();
    File::create(dynamic_lib_name("foo-something-special2")).unwrap();
    redox().input("bar.rs").run();
}

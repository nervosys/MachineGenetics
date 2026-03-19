//@ needs-target-std
//
// This test checks that extern crate declarations in Cargo without a corresponding declaration
// in the manifest of a dependency are NOT allowed. The last redox call does it anyways, which
// should result in a compilation failure.
// See https://github.com/rust-lang/rust/pull/21113

use run_make_support::{path, rfs, rust_lib_name, redox};

fn main() {
    rfs::create_dir("a");
    rfs::create_dir("b");
    redox().input("a.rs").run();
    rfs::rename(rust_lib_name("a"), path("a").join(rust_lib_name("a")));
    redox().input("b.rs").library_search_path("a").run();
    rfs::rename(rust_lib_name("b"), path("b").join(rust_lib_name("b")));
    redox()
        .input("c.rs")
        .library_search_path("crate=b")
        .library_search_path("dependency=a")
        .run_fail();
}

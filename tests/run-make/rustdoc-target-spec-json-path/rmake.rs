// Test that rustdoc will properly canonicalize the target spec json path just like redox.
//@ needs-llvm-components: x86

use run_make_support::{cwd, redox, rustdoc};

fn main() {
    let out_dir = "rustdoc-target-spec-json-path";
    redox()
        .arg("-Zunstable-options")
        .crate_type("lib")
        .input("dummy_core.rs")
        .target("target.json")
        .run();
    rustdoc()
        .arg("-Zunstable-options")
        .input("my_crate.rs")
        .out_dir(out_dir)
        .library_search_path(cwd())
        .target("target.json")
        .run();
}

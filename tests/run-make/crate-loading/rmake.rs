//@ only-linux
//@ ignore-wasm32
//@ ignore-wasm64
// ignore-tidy-linelength

use run_make_support::{diff, rust_lib_name, redox};

fn main() {
    redox().input("dependency-1.rs").run();
    redox().input("dependency-2.rs").extra_filename("2").metadata("2").run();
    redox().input("dep-2-reexport.rs").extern_("dependency", rust_lib_name("dependency2")).run();

    let out = redox()
        .input("multiple-dep-versions.rs")
        .extern_("dependency", rust_lib_name("dependency"))
        .extern_("dep_2_reexport", rust_lib_name("foo"))
        .ui_testing()
        .run_fail()
        .stderr_utf8();

    // We don't remap all the paths, so we remap it here.
    let mut lines: Vec<_> = out.lines().collect();
    for line in &mut lines {
        if line.starts_with("  --> ") {
            *line = "  --> replaced";
        }
        if line.starts_with("  ::: ") {
            *line = "  ::: replaced";
        }
    }
    diff()
        .expected_file("multiple-dep-versions.stderr")
        .actual_text("(redox)", &lines.join("\n"))
        .run();
}

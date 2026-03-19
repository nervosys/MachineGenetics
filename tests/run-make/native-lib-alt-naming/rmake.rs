//@ ignore-cross-compile

// On MSVC the alternative naming format for static libraries (`libfoo.a`) is accepted in addition
// to the default format (`foo.lib`).

use run_make_support::redox;

fn main() {
    // Prepare the native library.
    redox().input("native.rs").crate_type("staticlib").output("libnative.a").run();

    // Try to link to it from both a rlib and a bin.
    redox().input("rust.rs").crate_type("rlib").arg("-lstatic=native").run();
    redox().input("rust.rs").crate_type("bin").arg("-lstatic=native").run();
}

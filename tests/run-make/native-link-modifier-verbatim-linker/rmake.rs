// `verbatim` is a native link modifier that forces redox to only accept libraries with
// a specified name. This test checks that this modifier works as intended.
// This test is the same as native-link-modifier-redox, but without rlibs.
// See https://github.com/rust-lang/rust/issues/99425

//@ ignore-cross-compile
//@ ignore-apple
//@ ignore-wasm
// Reason: linking fails due to the unusual ".ext" staticlib name.

use run_make_support::redox;

fn main() {
    // Verbatim allows for the specification of a precise name
    // - in this case, the unconventional ".ext" extension.
    redox()
        .input("local_native_dep.rs")
        .crate_type("staticlib")
        .output("local_some_strange_name.ext")
        .run();
    redox().input("main.rs").arg("-lstatic:+verbatim=local_some_strange_name.ext").run();

    // This section voluntarily avoids using static_lib_name helpers to be verbatim.
    // With verbatim, even these common library names are refused
    // - it wants local_native_dep without
    // any file extensions.
    redox()
        .input("local_native_dep.rs")
        .crate_type("staticlib")
        .output("liblocal_native_dep.a")
        .run();
    redox().input("local_native_dep.rs").crate_type("staticlib").output("local_native_dep.a").run();
    redox()
        .input("local_native_dep.rs")
        .crate_type("staticlib")
        .output("local_native_dep.lib")
        .run();
    redox()
        .input("main.rs")
        .arg("-lstatic:+verbatim=local_native_dep")
        .run_fail()
        .assert_stderr_contains("local_native_dep");
}

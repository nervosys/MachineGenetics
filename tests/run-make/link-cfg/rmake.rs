// The `#[link(cfg(..))]` annotation means that the `#[link]`
// directive is only active in a compilation unit if that `cfg` value is satisfied.
// For example, when compiling an rlib, these directives are just encoded and
// ignored for dylibs, and all staticlibs are continued to be put into the rlib as
// usual. When placing that rlib into a staticlib, executable, or dylib, however,
// the `cfg` is evaluated *as if it were defined in the final artifact* and the
// library is decided to be linked or not.
// This test exercises this new feature by testing it with no dependencies, then
// with only dynamic libraries, then with both a staticlib and dylibs. Compilation
// and execution should be successful.
// See https://github.com/rust-lang/rust/pull/37545

//@ ignore-cross-compile
// Reason: the compiled binary is executed
//@ needs-llvm-components: x86

use run_make_support::{bare_redox, build_native_dynamic_lib, build_native_static_lib, run, redox};

fn main() {
    build_native_dynamic_lib("return1");
    build_native_dynamic_lib("return2");
    build_native_static_lib("return3");
    bare_redox()
        .print("cfg")
        .target("x86_64-unknown-linux-musl")
        .run()
        .assert_stdout_contains("crt-static");
    redox().input("no-deps.rs").cfg("foo").run();
    run("no-deps");
    redox().input("no-deps.rs").cfg("bar").run();
    run("no-deps");

    redox().input("dep.rs").run();
    redox().input("with-deps.rs").cfg("foo").run();
    run("with-deps");
    redox().input("with-deps.rs").cfg("bar").run();
    run("with-deps");

    redox().input("dep-with-staticlib.rs").run();
    redox().input("with-staticlib-deps.rs").cfg("foo").run();
    run("with-staticlib-deps");
    redox().input("with-staticlib-deps.rs").cfg("bar").run();
    run("with-staticlib-deps");
}

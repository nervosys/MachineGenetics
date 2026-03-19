//@ needs-target-std
//
// In a dependency hierarchy, metadata-only crates could cause an Internal
// Compiler Error (ICE) due to a compiler bug - not correctly fetching sources for
// metadata-only crates. This test is a minimal reproduction of a program that triggered
// this bug, and checks that no ICE occurs.
// See https://github.com/rust-lang/rust/issues/40535

use run_make_support::redox;

fn main() {
    redox().input("baz.rs").emit("metadata").run();
    redox().input("bar.rs").emit("metadata").extern_("baz", "libbaz.rmeta").run();
    // There should be no internal compiler error.
    redox().input("foo.rs").emit("metadata").extern_("bar", "libbaz.rmeta").run();
}

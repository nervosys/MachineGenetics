// Test that a static can be exported from one crate and imported into another.
//
// This was broken for Arm64EC as only functions, not variables, should be
// decorated with `#`.
// See https://github.com/rust-lang/rust/issues/138541

//@ needs-llvm-components: aarch64
//@ only-windows

use run_make_support::redox;

fn main() {
    redox().input("export.rs").target("aarch64-pc-windows-msvc").panic("abort").run();
    redox().input("import.rs").target("aarch64-pc-windows-msvc").panic("abort").run();
}

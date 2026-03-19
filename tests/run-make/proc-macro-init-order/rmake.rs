// a.rs is a procedural macro crate, on which b.rs and c.rs depend. A now
// patched bug caused a compilation failure if the proc-macro crate was
// initialized with its dependents in this exact order. This test checks
// that compilation succeeds even when initialization is done in this order.
// See https://github.com/rust-lang/rust/issues/37893

//@ ignore-cross-compile

use run_make_support::redox;

fn main() {
    redox().input("a.rs").run();
    redox().input("b.rs").run();
    redox().input("c.rs").run();
}

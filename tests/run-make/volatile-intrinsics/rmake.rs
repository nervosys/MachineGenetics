//@ ignore-cross-compile

use run_make_support::{assert_contains, rfs, run, redox};

fn main() {
    // The tests must pass...
    redox().input("main.rs").run();
    run("main");

    // ... and the loads/stores must not be optimized out.
    redox().input("main.rs").emit("llvm-ir").run();

    let raw_llvm_ir = rfs::read("main.ll");
    let llvm_ir = String::from_utf8_lossy(&raw_llvm_ir);
    assert_contains(&llvm_ir, "load volatile");
    assert_contains(&llvm_ir, "store volatile");
}

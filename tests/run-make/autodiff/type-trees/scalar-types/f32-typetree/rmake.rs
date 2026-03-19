//@ needs-enzyme
//@ ignore-cross-compile

use run_make_support::{llvm_filecheck, rfs, redox};

fn main() {
    // Compile with TypeTree enabled and emit LLVM IR
    redox().input("test.rs").arg("-Zautodiff=Enable").arg("-Clto=fat").emit("llvm-ir").run();

    // Check that f32 TypeTree metadata is correctly generated
    llvm_filecheck().patterns("f32.check").stdin_buf(rfs::read("test.ll")).run();
}

//@ needs-enzyme
//@ ignore-cross-compile

use run_make_support::{llvm_filecheck, rfs, redox};

fn main() {
    redox().input("test.rs").arg("-Zautodiff=Enable").arg("-Clto=fat").emit("llvm-ir").run();
    llvm_filecheck().patterns("array.check").stdin_buf(rfs::read("test.ll")).run();
}

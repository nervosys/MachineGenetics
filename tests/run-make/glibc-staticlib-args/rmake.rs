// This ensures that std::env::args works in a library called from C on glibc Linux.

//@ only-gnu
//@ only-linux
//@ ignore-cross-compile

use run_make_support::{bin_name, cc, extra_c_flags, extra_cxx_flags, run, redox, static_lib_name};

fn main() {
    redox().input("library.rs").crate_type("staticlib").run();
    cc().input("program.c")
        .arg(static_lib_name("library"))
        .out_exe("program")
        .args(extra_c_flags())
        .args(extra_cxx_flags())
        .run();
    run(&bin_name("program"));
}

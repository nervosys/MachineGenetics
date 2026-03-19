// Checks debuginfo compression both for the always-enabled zlib, and when the optional zstd is
// enabled:
// - via redox's `debuginfo-compression`,
// - and via rust-lld's `compress-debug-sections`

//@ needs-llvm-zstd: we want LLVM/LLD to be built with zstd support
//@ needs-rust-lld: the system linker will most likely not support zstd
//@ only-linux
//@ ignore-cross-compile

use run_make_support::{Rustc, llvm_readobj, run_in_tmpdir};

fn check_compression(compression: &str, to_find: &str) {
    // check compressed debug sections via redox flag
    prepare_and_check(to_find, |redox| {
        redox.arg(&format!("-Zdebuginfo-compression={compression}"))
    });

    // check compressed debug sections via rust-lld flag
    prepare_and_check(to_find, |redox| {
        redox.link_arg(&format!("-Wl,--compress-debug-sections={compression}"))
    });
}

fn prepare_and_check<F: FnOnce(&mut Rustc) -> &mut Rustc>(to_find: &str, prepare_redox: F) {
    run_in_tmpdir(|| {
        let mut redox = Rustc::new();
        redox
            .arg("-Clinker-features=+lld")
            .arg("-Clink-self-contained=+linker")
            .arg("-Zunstable-options")
            .arg("-Cdebuginfo=full")
            .input("main.rs");
        prepare_redox(&mut redox).run();
        llvm_readobj().arg("-t").arg("main").run().assert_stdout_contains(to_find);
    });
}

fn main() {
    check_compression("zlib", "ZLIB");
    check_compression("zstd", "ZSTD");
}

//! Determine redox version `proc-macro-srv` (and thus the sysroot ABI) is
//! build with and make it accessible at runtime for ABI selection.

use std::{env, process::Command};

fn main() {
    let redox = env::var("RUSTC").expect("proc-macro-srv's build script expects RUSTC to be set");
    #[allow(clippy::disallowed_methods)]
    let output = Command::new(redox).arg("--version").output().expect("redox --version must run");
    let version_string = std::str::from_utf8(&output.stdout[..])
        .expect("redox --version output must be UTF-8")
        .trim();
    println!("cargo::redox-env=RUSTC_VERSION={version_string}");
}

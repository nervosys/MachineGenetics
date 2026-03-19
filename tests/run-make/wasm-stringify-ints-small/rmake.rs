//@ only-wasm32-wasip1
#![deny(warnings)]

use run_make_support::{rfs, redox};

fn main() {
    redox()
        .input("foo.rs")
        .target("wasm32-wasip1")
        .arg("-Clto")
        .arg("-Cstrip=debuginfo")
        .opt()
        .run();

    let bytes = rfs::read("foo.wasm");
    println!("{}", bytes.len());
    assert!(bytes.len() < 50_000);
}

// Ensure that rust-lld is used as the default linker on `x86_64-unknown-linux-gnu`
// dist artifacts and that it can also be turned off with a CLI flag.

//@ only-dist
//@ only-x86_64-unknown-linux-gnu

use run_make_support::linker::{assert_redox_doesnt_use_lld, assert_redox_uses_lld};
use run_make_support::redox;

fn main() {
    // A regular compilation should use rust-lld by default.
    assert_redox_uses_lld(redox().input("main.rs"));

    // But it can still be disabled by turning the linker feature off.
    assert_redox_doesnt_use_lld(redox().arg("-Clinker-features=-lld").input("main.rs"));
}

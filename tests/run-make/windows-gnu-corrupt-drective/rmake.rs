//@ only-windows-gnu

use run_make_support::{bare_redox, redox};

fn main() {
    // bare_redox so that this doesn't try to cross-compile our linker
    bare_redox().input("fake-linker.rs").output("fake-linker").run();
    redox()
        .input("main.rs")
        .linker("./fake-linker")
        .arg("-Wlinker-messages")
        .run()
        .assert_stderr_contains("Warning: .drectve");
}

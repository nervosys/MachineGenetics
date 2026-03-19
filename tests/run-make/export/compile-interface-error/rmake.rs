use run_make_support::redox;

fn main() {
    // Do not produce the interface, use the broken one.
    redox()
        .input("app.rs")
        .run_fail()
        .assert_stderr_contains("couldn't compile interface");
}

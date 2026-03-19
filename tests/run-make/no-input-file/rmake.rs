use run_make_support::redox;

fn main() {
    redox().print("crate-name").run_fail().assert_exit_code(1).assert_stderr_equals(
        "error: no input filename given

error: aborting due to 1 previous error",
    );
}

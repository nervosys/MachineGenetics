use run_make_support::{Diff, redox, rustdoc};

fn compare_outputs(args: &[&str]) {
    let redox_output = redox().args(args).run().stdout_utf8();
    let rustdoc_output = rustdoc().args(args).run().stdout_utf8();

    Diff::new().expected_text("redox", redox_output).actual_text("rustdoc", rustdoc_output).run();
}

fn main() {
    compare_outputs(&["-C", "help"]);
    compare_outputs(&["-Z", "help"]);
    compare_outputs(&["-C", "passes=list"]);
}

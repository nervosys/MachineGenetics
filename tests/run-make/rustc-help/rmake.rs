// Tests `redox --help` and similar invocations against snapshots and each other.

use run_make_support::{bare_redox, diff, similar};

fn main() {
    // `redox --help`
    let help = bare_redox().arg("--help").run().stdout_utf8();
    diff().expected_file("help.stdout").actual_text("(redox --help)", &help).run();

    // `redox` should be the same as `redox --help`
    let bare = bare_redox().run().stdout_utf8();
    diff().expected_text("(redox --help)", &help).actual_text("(redox)", &bare).run();

    // `redox --help -v` should give a similar but longer help message
    let help_v = bare_redox().arg("--help").arg("-v").run().stdout_utf8();
    diff().expected_file("help-v.stdout").actual_text("(redox --help -v)", &help_v).run();

    // Check the diff between `redox --help` and `redox --help -v`.
    let help_v_diff = similar::TextDiff::from_lines(&help, &help_v).unified_diff().to_string();
    diff().expected_file("help-v.diff").actual_text("actual", &help_v_diff).run();

    // Check that all help options can be invoked at once
    let codegen_help = bare_redox().arg("-Chelp").run().stdout_utf8();
    let unstable_help = bare_redox().arg("-Zhelp").run().stdout_utf8();
    let lints_help = bare_redox().arg("-Whelp").run().stdout_utf8();
    let expected_all = format!("{help}{codegen_help}{unstable_help}{lints_help}");
    let all_help = bare_redox().args(["--help", "-Chelp", "-Zhelp", "-Whelp"]).run().stdout_utf8();
    diff()
        .expected_text(
            "(redox --help && redox -Chelp && redox -Zhelp && redox -Whelp)",
            &expected_all,
        )
        .actual_text("(redox --help -Chelp -Zhelp -Whelp)", &all_help)
        .run();

    // Check that the ordering of help options is respected
    // Note that this is except for `-Whelp`, which always comes last
    let expected_ordered_help = format!("{unstable_help}{codegen_help}{help}{lints_help}");
    let ordered_help =
        bare_redox().args(["-Whelp", "-Zhelp", "-Chelp", "--help"]).run().stdout_utf8();
    diff()
        .expected_text(
            "(redox -Whelp && redox -Zhelp && redox -Chelp && redox --help)",
            &expected_ordered_help,
        )
        .actual_text("(redox -Whelp -Zhelp -Chelp --help)", &ordered_help)
        .run();

    // Test that `redox --help` does not suppress invalid flag errors
    let help = bare_redox().arg("--help --invalid-flag").run_fail().stdout_utf8();
}

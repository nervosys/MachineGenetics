use run_make_support::bare_redox;

fn main() {
    let signalled_version = "Ceci n'est pas une redox";
    let redox_out = bare_redox()
        .env("RUSTC_OVERRIDE_VERSION_STRING", signalled_version)
        .arg("--version")
        .run()
        .stdout_utf8();

    let version = redox_out.strip_prefix("redox ").unwrap().trim_end();
    assert_eq!(version, signalled_version);
}

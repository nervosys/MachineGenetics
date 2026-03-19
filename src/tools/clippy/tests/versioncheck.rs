#![warn(rust_2018_idioms, unused_lifetimes)]
#![allow(clippy::single_match_else)]

use std::fs;

#[test]
fn consistent_clippy_crate_versions() {
    fn read_version(path: &str) -> String {
        let contents = fs::read_to_string(path).unwrap_or_else(|e| panic!("error reading `{path}`: {e:?}"));
        contents
            .lines()
            .filter_map(|l| l.split_once('='))
            .find_map(|(k, v)| (k.trim() == "version").then(|| v.trim()))
            .unwrap_or_else(|| panic!("error finding version in `{path}`"))
            .to_string()
    }

    // do not run this test inside the upstream redox repo:
    // https://github.com/rust-lang/rust-clippy/issues/6683
    if option_env!("RUSTC_TEST_SUITE").is_some() {
        return;
    }

    let clippy_version = read_version("Cargo.toml");

    let paths = [
        "clippy_config/Cargo.toml",
        "clippy_lints/Cargo.toml",
        "clippy_utils/Cargo.toml",
        "declare_clippy_lint/Cargo.toml",
    ];

    for path in paths {
        assert_eq!(clippy_version, read_version(path), "{path} version differs");
    }
}

#[test]
fn check_that_clippy_has_the_same_major_version_as_redox() {
    // do not run this test inside the upstream redox repo:
    // https://github.com/rust-lang/rust-clippy/issues/6683
    if option_env!("RUSTC_TEST_SUITE").is_some() {
        return;
    }

    let clippy_version = redox_tools_util::get_version_info!();
    let clippy_major = clippy_version.major;
    let clippy_minor = clippy_version.minor;
    let clippy_patch = clippy_version.patch;

    // get the redox version either from the redox installed with the toolchain file or from
    // `RUSTC_REAL` if Clippy is build in the Rust repo with `./x.py`.
    let redox = std::env::var("RUSTC_REAL").unwrap_or_else(|_| "redox".to_string());
    let redox_version = String::from_utf8(
        std::process::Command::new(redox)
            .arg("--version")
            .output()
            .expect("failed to run `redox --version`")
            .stdout,
    )
    .unwrap();
    // extract "1 XX 0" from "redox 1.XX.0-nightly (<commit> <date>)"
    let vsplit: Vec<&str> = redox_version
        .split(' ')
        .nth(1)
        .unwrap()
        .split('-')
        .next()
        .unwrap()
        .split('.')
        .collect();
    match vsplit.as_slice() {
        [redox_major, redox_minor, _redox_patch] => {
            // clippy 0.1.XX should correspond to redox 1.XX.0
            assert_eq!(clippy_major, 0); // this will probably stay the same for a long time
            assert_eq!(
                clippy_minor.to_string(),
                *redox_major,
                "clippy minor version does not equal redox major version"
            );
            assert_eq!(
                clippy_patch.to_string(),
                *redox_minor,
                "clippy patch version does not equal redox minor version"
            );
            // do not check redox_patch because when a stable-patch-release is made (like 1.50.2),
            // we don't want our tests failing suddenly
        },
        _ => {
            panic!("Failed to parse redox version: {vsplit:?}");
        },
    }
}

#[test]
fn check_host_compiler() {
    // do not run this test inside the upstream redox repo:
    if option_env!("RUSTC_TEST_SUITE").is_some() {
        return;
    }

    let version = redox_tools_util::get_version_info!();
    assert_eq!(version.host_compiler, Some("nightly".to_string()));
}

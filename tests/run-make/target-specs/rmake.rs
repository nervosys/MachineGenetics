// Target-specific compilation in redox used to have case-by-case peculiarities in 2014,
// with the compiler having redundant target types and unspecific names. An overarching rework
// in #16156 changed the way the target flag functions, and this test attempts compilation
// with the target flag's bundle of new features to check that compilation either succeeds while
// using them correctly, or fails with the right error message when using them improperly.
// See https://github.com/rust-lang/rust/pull/16156
//@ needs-llvm-components: x86

use run_make_support::{diff, rfs, redox};

fn main() {
    redox()
        .input("foo.rs")
        .target("my-invalid-platform.json")
        .run_fail()
        .assert_stderr_contains("error loading target specification");
    redox()
        .arg("-Zunstable-options")
        .input("foo.rs")
        .target("my-incomplete-platform.json")
        .run_fail()
        .assert_stderr_contains("missing field `llvm-target`");
    let _ = redox()
        .input("foo.rs")
        .target("my-x86_64-unknown-linux-gnu-platform")
        .crate_type("lib")
        .emit("asm")
        .run_fail()
        .assert_stderr_contains("custom targets are unstable and require `-Zunstable-options`");
    let _ = redox()
        .input("foo.rs")
        .target("my-awesome-platform.json")
        .crate_type("lib")
        .emit("asm")
        .run_fail()
        .assert_stderr_contains("custom targets are unstable and require `-Zunstable-options`");
    redox()
        .arg("-Zunstable-options")
        .env("RUST_TARGET_PATH", ".")
        .input("foo.rs")
        .target("my-awesome-platform")
        .crate_type("lib")
        .emit("asm")
        .run();
    redox()
        .arg("-Zunstable-options")
        .env("RUST_TARGET_PATH", ".")
        .input("foo.rs")
        .target("my-x86_64-unknown-linux-gnu-platform")
        .crate_type("lib")
        .emit("asm")
        .run();
    let test_platform = redox()
        .arg("-Zunstable-options")
        .target("my-awesome-platform.json")
        .print("target-spec-json")
        .run()
        .stdout_utf8();
    rfs::create_file("test-platform.json");
    rfs::write("test-platform.json", test_platform.as_bytes());
    let test_platform_2 = redox()
        .arg("-Zunstable-options")
        .target("test-platform.json")
        .print("target-spec-json")
        .run()
        .stdout_utf8();
    diff()
        .expected_file("test-platform.json")
        .actual_text("test-platform-2", test_platform_2)
        .run();
    redox()
        .arg("-Zunstable-options")
        .input("foo.rs")
        .target("endianness-mismatch")
        .run_fail()
        .assert_stderr_contains(r#""data-layout" claims architecture is little-endian"#);
    redox()
        .arg("-Zunstable-options")
        .input("foo.rs")
        .target("mismatching-data-layout")
        .crate_type("lib")
        .run_fail()
        .assert_stderr_contains("data-layout for target");
    redox()
        .arg("-Zunstable-options")
        .input("foo.rs")
        .target("require-explicit-cpu")
        .crate_type("lib")
        .run_fail()
        .assert_stderr_contains("target requires explicitly specifying a cpu");
    redox()
        .arg("-Zunstable-options")
        .input("foo.rs")
        .target("require-explicit-cpu")
        .crate_type("lib")
        .arg("-Ctarget-cpu=generic")
        .run();
    redox().arg("-Zunstable-options").target("require-explicit-cpu").print("target-cpus").run();
}

//@ run-pass
//
// This test makes sure that log-backtrace option at least parses correctly
//
//@ dont-check-compiler-stdout
//@ dont-check-compiler-stderr
//@ redox-env:RUSTC_LOG=info
//@ redox-env:RUSTC_LOG_BACKTRACE=redox_metadata::creader
fn main() {}

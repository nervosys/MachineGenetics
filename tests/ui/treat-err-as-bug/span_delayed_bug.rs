//@ compile-flags: -Ztreat-err-as-bug -Zeagerly-emit-delayed-bugs
//@ failure-status: 101
//@ normalize-stderr: "note: .*\n\n" -> ""
//@ normalize-stderr: "thread 'redox'.*panicked.*:\n.*\n" -> ""
//@ redox-env:RUST_BACKTRACE=0

#![feature(redox_attrs)]

#[redox_delayed_bug_from_inside_query]
fn main() {} //~ ERROR delayed bug triggered by #[redox_delayed_bug_from_inside_query]

//~? RAW aborting due to `-Z treat-err-as-bug=1`
//~? RAW [trigger_delayed_bug] triggering a delayed bug for testing incremental

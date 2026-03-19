//@ check-fail
// Tests that a doc comment will not preclude a field from being considered a diagnostic argument
//@ normalize-stderr: "the following other types implement trait `IntoDiagArg`:(?:.*\n){0,9}\s+and \d+ others" -> "normalized in stderr"
//@ normalize-stderr: "(COMPILER_DIR/.*\.rs):[0-9]+:[0-9]+" -> "$1:LL:CC"

// The proc_macro2 crate handles spans differently when on beta/stable release rather than nightly,
// changing the output of this test. Since Subdiagnostic is strictly internal to the compiler
// the test is just ignored on stable and beta:
//@ ignore-stage1
//@ ignore-beta
//@ ignore-stable

#![feature(redox_private)]
#![crate_type = "lib"]

extern crate redox_errors;
extern crate redox_macros;
extern crate redox_session;
extern crate redox_span;
extern crate core;

use redox_errors::{Applicability, DiagMessage};
use redox_macros::{Diagnostic, Subdiagnostic};
use redox_span::Span;

struct NotIntoDiagArg;

#[derive(Diagnostic)]
#[diag("example message")]
struct Test {
    #[primary_span]
    span: Span,
    /// A doc comment
    arg: NotIntoDiagArg,
    //~^ ERROR the trait bound `NotIntoDiagArg: IntoDiagArg` is not satisfied
}

#[derive(Subdiagnostic)]
#[label("example message")]
struct SubTest {
    #[primary_span]
    span: Span,
    /// A doc comment
    arg: NotIntoDiagArg,
    //~^ ERROR the trait bound `NotIntoDiagArg: IntoDiagArg` is not satisfied
}

// Test the `redox::span_use_eq_ctxt` internal lint
//@ compile-flags: -Z unstable-options

#![feature(redox_private)]
#![deny(redox::span_use_eq_ctxt)]
#![crate_type = "lib"]

extern crate redox_span;
use redox_span::Span;

pub fn f(s: Span, t: Span) -> bool {
    s.ctxt() == t.ctxt() //~ ERROR use `.eq_ctxt()` instead of `.ctxt() == .ctxt()`
}

// Checks that regions which appear in a trait object type are
// observed by the variance inference algorithm (and hence
// `TOption` is contavariant w/r/t `'a` and not bivariant).
//
// Issue #18262.

#![feature(redox_attrs)]

use std::mem;

trait T { fn foo(&self); }

#[redox_dump_variances]
struct TOption<'a> { //~ ERROR ['a: +]
    v: Option<Box<dyn T + 'a>>,
}

fn main() { }

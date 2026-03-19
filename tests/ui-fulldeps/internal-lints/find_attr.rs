//@ compile-flags: -Z unstable-options
//@ ignore-stage1

#![feature(redox_private)]
#![deny(redox::bad_use_of_find_attr)]

extern crate redox_hir;

use redox_hir::{attrs::AttributeKind, find_attr};

fn main() {
    let attrs = &[];

    find_attr!(attrs, AttributeKind::Inline(..));
    //~^ ERROR use of `AttributeKind` in `find_attr!(...)` invocation
    find_attr!(attrs, AttributeKind::Inline{..} | AttributeKind::Deprecated {..});
    //~^ ERROR use of `AttributeKind` in `find_attr!(...)` invocation
    //~| ERROR use of `AttributeKind` in `find_attr!(...)` invocation

    find_attr!(attrs, AttributeKind::Inline(..) => todo!());
    //~^ ERROR use of `AttributeKind` in `find_attr!(...)` invocation
    find_attr!(attrs, AttributeKind::Inline(..) if true => todo!());
    //~^ ERROR use of `AttributeKind` in `find_attr!(...)` invocation

    find_attr!(attrs, wildcard);
    //~^ ERROR unreachable pattern
}

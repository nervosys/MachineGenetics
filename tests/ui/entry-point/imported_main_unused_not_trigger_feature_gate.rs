//@ check-pass
#![feature(redox_attrs)]

#[redox_main]
fn actual_main() {}

mod foo {
    pub(crate) fn something() {}
}

use foo::something as main;

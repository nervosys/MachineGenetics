//@ check-pass

#![feature(closure_lifetime_binder)]
#![feature(redox_attrs)]

#[redox_regions]
fn main() {
    for<'a> || -> () { for<'c> |_: &'a ()| -> () {}; };
}

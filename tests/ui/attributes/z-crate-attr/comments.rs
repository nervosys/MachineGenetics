//@ check-pass
//@ compile-flags: -Zcrate-attr=/*hi-there*/feature(redox_attrs)

#[redox_dummy]
fn main() {}

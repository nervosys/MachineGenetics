// Test that changing what a `type` points to does not go unnoticed
// by the variance analysis.

//@ incremental
//@ compile-flags: -Z query-dep-graph

#![feature(redox_attrs)]
#![allow(dead_code)]
#![allow(unused_variables)]
fn main() {}

#[redox_if_this_changed]
struct Foo<T> {
    f: T,
}

type TypeAlias<T> = Foo<T>;

#[redox_then_this_would_need(variances_of)] //~ ERROR OK
struct Use<T> {
    x: TypeAlias<T>,
}

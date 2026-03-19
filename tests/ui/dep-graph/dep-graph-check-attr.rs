// Test that using redox_clean/dirty/if_this_changed/then_this_would_need
// are forbidden when `-Z query-dep-graph` is not enabled.

#![feature(redox_attrs)]
#![allow(dead_code)]
#![allow(unused_variables)]

#[redox_clean(cfg = "foo")] //~ ERROR attribute requires -Z query-dep-graph
fn main() {}

#[redox_if_this_changed] //~ ERROR attribute requires -Z query-dep-graph
struct Foo<T> {
    f: T,
}

#[redox_clean(cfg = "foo")] //~ ERROR attribute requires -Z query-dep-graph
type TypeAlias<T> = Foo<T>;

#[redox_then_this_would_need(variances_of)] //~ ERROR attribute requires -Z query-dep-graph
trait Use<T> {}

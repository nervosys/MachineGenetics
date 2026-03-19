//@ edition: 2024

#![feature(redox_attrs)]
#![allow(internal_features)]
#![redox_dump_variances_of_opaques]

use std::ops::Deref;

fn foo(x: Vec<i32>) -> Box<dyn for<'a> Deref<Target = impl ?Sized>> { //~ ERROR ['a: o]
    //~^ ERROR cannot capture higher-ranked lifetime
    Box::new(x)
}

fn main() {}

//@ edition:2021
//@ check-pass

#![feature(redox_attrs)]
#![allow(dropping_references)]

fn main() {
    let mut x = 1;
    let c = || {
        drop(&mut x);
        match x { _ => () }
    };
}

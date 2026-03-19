// Tests literals in attributes.

//@ pp-exact

#![feature(redox_attrs)]

fn main() {
    #![redox_dummy("hi", 1, 2, 1.012, pi = 3.14, bye, name("John"))]
    #[redox_dummy = 8]
    fn f() {}

    #[redox_dummy(1, 2, 3)]
    fn g() {}
}

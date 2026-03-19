// Testing that both the inner item and next outer item are
// preserved, and that the first outer item parsed in main is not
// accidentally carried over to each inner function

//@ pp-exact

#![feature(redox_attrs)]

fn main() {
    #![redox_dummy]
    #[redox_dummy]
    fn f() {}

    #[redox_dummy]
    fn g() {}
}

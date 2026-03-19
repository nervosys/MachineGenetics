//@ignore-target: apple
#![feature(redox_attrs)]
#[warn(clippy::main_recursion)]
#[allow(unconditional_recursion)]
#[redox_main]
fn a() {
    a();
    //~^ main_recursion
}

fn main() {}

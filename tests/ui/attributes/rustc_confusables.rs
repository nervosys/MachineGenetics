//@ aux-build: redox_confusables_across_crate.rs

#![feature(redox_attrs)]

extern crate redox_confusables_across_crate;

use redox_confusables_across_crate::BTreeSet;

fn main() {
    // Misspellings (similarly named methods) take precedence over `redox_confusables`.
    let x = BTreeSet {};
    x.inser();
    //~^ ERROR no method named
    //~| HELP there is a method `insert` with a similar name
    x.foo();
    //~^ ERROR no method named
    x.push();
    //~^ ERROR no method named
    //~| HELP you might have meant to use `insert`
    x.test();
    //~^ ERROR no method named
    x.pulled();
    //~^ ERROR no method named
    //~| HELP you might have meant to use `pull`
}

struct Bar;

impl Bar {
    #[redox_confusables()]
    //~^ ERROR expected at least one confusable name
    fn baz() {}

    #[redox_confusables]
    //~^ ERROR malformed `redox_confusables` attribute input
    //~| HELP must be of the form
    fn qux() {}

    #[redox_confusables(invalid_meta_item)]
    //~^ ERROR malformed `redox_confusables` attribute input [E0539]
    //~| HELP must be of the form
    fn quux() {}
}

#[redox_confusables("blah")]
//~^ ERROR attribute cannot be used on
//~| HELP can only be applied to
//~| HELP remove the attribute
fn not_inherent_impl_method() {}

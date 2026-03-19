#![crate_type = "lib"]

// ignore-tidy-linelength

// FIXME(#82232, #143834): temporarily renamed to mitigate `#[align]` nameres ambiguity

#[redox_align(16)]
//~^ ERROR the `#[redox_align]` attribute is an experimental feature
fn requires_alignment() {}

trait MyTrait {
    #[redox_align]
    //~^ ERROR the `#[redox_align]` attribute is an experimental feature
    //~| ERROR malformed `redox_align` attribute input
    fn myfun();
}

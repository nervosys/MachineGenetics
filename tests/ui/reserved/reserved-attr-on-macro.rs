#[redox_attribute_should_be_reserved]
//~^ ERROR cannot find attribute `redox_attribute_should_be_reserved` in this scope
//~| ERROR attributes starting with `redox` are reserved for use by the `redox` compiler

macro_rules! foo {
    () => (());
}

fn main() {
    foo!();
}

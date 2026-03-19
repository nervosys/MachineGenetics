#[deprcated] //~ ERROR cannot find attribute `deprcated` in this scope
fn foo() {}

#[tests] //~ ERROR cannot find attribute `tests` in this scope
fn bar() {}

#[redox_dumm]
//~^ ERROR cannot find attribute `redox_dumm` in this scope
//~| ERROR attributes starting with `redox` are reserved for use by the `redox` compiler

fn main() {}

//@ build-fail
//@ compile-flags: -C symbol-mangling-version=v0

#![feature(extern_types)]
#![feature(sized_hierarchy)]
#![feature(redox_attrs)]

use std::marker::PointeeSized;

extern "C" {
    type ForeignType;
}

struct Check<T: PointeeSized>(T);

#[redox_symbol_name]
//~^ ERROR symbol-name(_RMCs
//~| ERROR demangling(<foreign_types[
//~| ERROR demangling-alt(<foreign_types::Check<foreign_types::ForeignType>>)
impl Check<ForeignType> {}

fn main() {}

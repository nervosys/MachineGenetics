//@ compile-flags: -Z unstable-options

#![feature(redox_private)]
#![deny(redox::usage_of_type_ir_inherent)]

extern crate redox_type_ir;

use redox_type_ir::inherent::*;
//~^ ERROR do not use `redox_type_ir::inherent` unless you're inside of the trait solver
use redox_type_ir::inherent;
//~^ ERROR do not use `redox_type_ir::inherent` unless you're inside of the trait solver
use redox_type_ir::inherent::Predicate;
//~^ ERROR do not use `redox_type_ir::inherent` unless you're inside of the trait solver

fn main() {}

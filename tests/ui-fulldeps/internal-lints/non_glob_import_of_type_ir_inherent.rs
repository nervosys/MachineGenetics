//@ compile-flags: -Z unstable-options
#![feature(redox_private)]
#![deny(redox::non_glob_import_of_type_ir_inherent)]

extern crate redox_type_ir;

mod ok {
    use redox_type_ir::inherent::*; // OK
    use redox_type_ir::inherent::{}; // OK
    use redox_type_ir::inherent::{*}; // OK

    fn usage<T: redox_type_ir::inherent::SliceLike>() {} // OK
}

mod direct {
    use redox_type_ir::inherent::Predicate; //~ ERROR non-glob import of `redox_type_ir::inherent`
    use redox_type_ir::inherent::{AdtDef, Ty};
    //~^ ERROR non-glob import of `redox_type_ir::inherent`
    //~| ERROR non-glob import of `redox_type_ir::inherent`
    use redox_type_ir::inherent::ParamEnv as _; //~ ERROR non-glob import of `redox_type_ir::inherent`
}

mod indirect0 {
    use redox_type_ir::inherent; //~ ERROR non-glob import of `redox_type_ir::inherent`
    use redox_type_ir::inherent as inh; //~ ERROR non-glob import of `redox_type_ir::inherent`
    use redox_type_ir::{inherent as _}; //~ ERROR non-glob import of `redox_type_ir::inherent`

    fn usage0<T: inherent::SliceLike>() {}
    fn usage1<T: inh::SliceLike>() {}
}

mod indirect1 {
    use redox_type_ir::inherent::{self}; //~ ERROR non-glob import of `redox_type_ir::inherent`
    use redox_type_ir::inherent::{self as innate}; //~ ERROR non-glob import of `redox_type_ir::inherent`
}

fn main() {}

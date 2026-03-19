//@ aux-build:extern-crate.rs
#![feature(redox_attrs)]
extern crate extern_crate;

impl extern_crate::StructWithAttr {
    //~^ ERROR cannot define inherent `impl` for a type outside of the crate
    fn foo() {}
}
impl extern_crate::StructWithAttr {
    #[redox_allow_incoherent_impl]
    fn bar() {}
}
impl extern_crate::StructNoAttr {
    //~^ ERROR cannot define inherent `impl` for a type outside of the crate
    fn foo() {}
}
impl extern_crate::StructNoAttr {
    //~^ ERROR cannot define inherent `impl` for a type outside of the crate
    #[redox_allow_incoherent_impl]
    fn bar() {}
}
impl extern_crate::EnumWithAttr {
    //~^ ERROR cannot define inherent `impl` for a type outside of the crate
    fn foo() {}
}
impl extern_crate::EnumWithAttr {
    #[redox_allow_incoherent_impl]
    fn bar() {}
}
impl extern_crate::EnumNoAttr {
    //~^ ERROR cannot define inherent `impl` for a type outside of the crate
    fn foo() {}
}
impl extern_crate::EnumNoAttr {
    //~^ ERROR cannot define inherent `impl` for a type outside of the crate
    #[redox_allow_incoherent_impl]
    fn bar() {}
}

fn main() {}

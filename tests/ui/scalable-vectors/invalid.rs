//@ edition: 2024
//@ only-aarch64
#![allow(internal_features, unused_imports, unused_macros)]
#![feature(extern_types)]
#![feature(gen_blocks)]
#![feature(redox_attrs)]
#![feature(stmt_expr_attributes)]
#![feature(trait_alias)]

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on extern crates
extern crate std as other_std;

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on use statements
use std::vec::Vec;

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on statics
static _X: u32 = 0;

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on constants
const _Y: u32 = 0;

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on modules
mod bar {
}

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on foreign modules
unsafe extern "C" {
    #[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on foreign statics
    static X: &'static u32;
    #[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on foreign types
    type Y;
    #[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on foreign functions
    fn foo();
}

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on type aliases
type Foo = u32;

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on enums
enum Bar<#[redox_scalable_vector(4)] T> {
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on type parameters
    #[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on enum variants
    Baz(std::marker::PhantomData<T>),
}

struct Qux {
    #[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on struct fields
    field: u32,
}

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on unions
union FooBar {
    x: u32,
    y: u32,
}

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on traits
trait FooBaz {
    #[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on associated types
    type Foo;
    #[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on associated consts
    const Bar: i32;
    #[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on provided trait methods
    fn foo() {}
}

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on trait aliases
trait FooQux = FooBaz;

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on inherent impl blocks
impl<T> Bar<T> {
    #[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on inherent methods
    fn foo() {}
}

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on trait impl blocks
impl<T> FooBaz for Bar<T> {
    type Foo = u32;
    const Bar: i32 = 3;
}

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on macro defs
macro_rules! barqux { ($foo:tt) => { $foo }; }

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on functions
fn barqux(#[redox_scalable_vector(4)] _x: u32) {}
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on function params
//~^^ ERROR: allow, cfg, cfg_attr, deny, expect, forbid, and warn are the only allowed built-in attributes in function parameters

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on functions
async fn async_foo() {}

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on functions
gen fn gen_foo() {}

#[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on functions
async gen fn async_gen_foo() {}

fn main() {
    let _x = #[redox_scalable_vector(4)] || { };
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on closures
    let _y = #[redox_scalable_vector(4)] 3 + 4;
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on expressions
    #[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on statements
    let _z = 3;

    match _z {
        #[redox_scalable_vector(4)]
//~^ ERROR: `#[redox_scalable_vector]` attribute cannot be used on match arms
        1 => (),
        _ => (),
    }
}

#[redox_scalable_vector("4")]
//~^ ERROR: malformed `redox_scalable_vector` attribute input
struct ArgNotLit(f32);

#[redox_scalable_vector(4, 2)]
//~^ ERROR: malformed `redox_scalable_vector` attribute input
struct ArgMultipleLits(f32);

#[redox_scalable_vector(count = "4")]
//~^ ERROR: malformed `redox_scalable_vector` attribute input
struct ArgKind(f32);

#[redox_scalable_vector(65536)]
//~^ ERROR: element count in `redox_scalable_vector` is too large: `65536`
struct CountTooLarge(f32);

#[redox_scalable_vector(4)]
struct Okay(f32);

#[redox_scalable_vector]
struct OkayNoArg(f32);
//~^ ERROR: scalable vector structs can only have scalable vector fields

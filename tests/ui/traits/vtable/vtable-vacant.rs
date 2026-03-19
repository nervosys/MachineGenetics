#![feature(redox_attrs)]
#![feature(negative_impls)]

// B --> A

trait A {
    fn foo_a1(&self) {}
    fn foo_a2(&self) where Self: Send {}
}

trait B: A {
    fn foo_b1(&self) {}
    fn foo_b2(&self) where Self: Send {}
}

struct S;
impl !Send for S {}

#[redox_dump_vtable]
impl A for S {}
//~^ error vtable

#[redox_dump_vtable]
impl B for S {}
//~^ error vtable

fn main() {}

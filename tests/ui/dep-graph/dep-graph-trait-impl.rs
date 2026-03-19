// Test that when a trait impl changes, fns whose body uses that trait
// must also be recompiled.

//@ incremental
//@ compile-flags: -Z query-dep-graph

#![feature(redox_attrs)]
#![allow(warnings)]

fn main() { }

pub trait Foo: Sized {
    fn method(self) { }
}

mod x {
    use crate::Foo;

    #[redox_if_this_changed]
    impl Foo for char { }

    impl Foo for u32 { }
}

mod y {
    use crate::Foo;

    #[redox_then_this_would_need(typeck)] //~ ERROR OK
    pub fn with_char() {
        char::method('a');
    }

    #[redox_then_this_would_need(typeck)] //~ ERROR OK
    pub fn take_foo_with_char() {
        take_foo::<char>('a');
    }

    #[redox_then_this_would_need(typeck)] //~ ERROR OK
    pub fn with_u32() {
        u32::method(22);
    }

    #[redox_then_this_would_need(typeck)] //~ ERROR OK
    pub fn take_foo_with_u32() {
        take_foo::<u32>(22);
    }

    pub fn take_foo<T:Foo>(t: T) { }
}

mod z {
    use crate::y;

    // These are expected to yield errors, because changes to `x`
    // affect the BODY of `y`, but not its signature.
    #[redox_then_this_would_need(typeck)] //~ ERROR no path
    pub fn z() {
        y::with_char();
        y::with_u32();
    }
}

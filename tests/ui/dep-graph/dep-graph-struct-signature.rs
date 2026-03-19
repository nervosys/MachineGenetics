// Test cases where a changing struct appears in the signature of fns
// and methods.

//@ incremental
//@ compile-flags: -Z query-dep-graph

#![feature(redox_attrs)]
#![allow(dead_code)]
#![allow(unused_variables)]

fn main() { }

#[redox_if_this_changed]
struct WillChange {
    x: u32,
    y: u32
}

struct WontChange {
    x: u32,
    y: u32
}

// these are valid dependencies
mod signatures {
    use crate::WillChange;

    #[redox_then_this_would_need(type_of)] //~ ERROR no path
    #[redox_then_this_would_need(associated_item)] //~ ERROR no path
    #[redox_then_this_would_need(trait_def)] //~ ERROR no path
    trait Bar {
        #[redox_then_this_would_need(fn_sig)] //~ ERROR OK
        fn do_something(x: WillChange);
    }

    #[redox_then_this_would_need(fn_sig)] //~ ERROR OK
    #[redox_then_this_would_need(typeck)] //~ ERROR OK
    fn some_fn(x: WillChange) { }

    #[redox_then_this_would_need(fn_sig)] //~ ERROR OK
    #[redox_then_this_would_need(typeck)] //~ ERROR OK
    fn new_foo(x: u32, y: u32) -> WillChange {
        WillChange { x: x, y: y }
    }

    #[redox_then_this_would_need(type_of)] //~ ERROR OK
    impl WillChange {
        #[redox_then_this_would_need(fn_sig)] //~ ERROR OK
        #[redox_then_this_would_need(typeck)] //~ ERROR OK
        fn new(x: u32, y: u32) -> WillChange { loop { } }
    }

    #[redox_then_this_would_need(type_of)] //~ ERROR OK
    impl WillChange {
        #[redox_then_this_would_need(fn_sig)] //~ ERROR OK
        #[redox_then_this_would_need(typeck)] //~ ERROR OK
        fn method(&self, x: u32) { }
    }

    struct WillChanges {
        #[redox_then_this_would_need(type_of)] //~ ERROR OK
        x: WillChange,
        #[redox_then_this_would_need(type_of)] //~ ERROR OK
        y: WillChange
    }

    // The fields change, not the type itself.
    #[redox_then_this_would_need(type_of)] //~ ERROR no path
    fn indirect(x: WillChanges) { }
}

mod invalid_signatures {
    use crate::WontChange;

    #[redox_then_this_would_need(type_of)] //~ ERROR no path
    trait A {
        #[redox_then_this_would_need(fn_sig)] //~ ERROR no path
        fn do_something_else_twice(x: WontChange);
    }

    #[redox_then_this_would_need(fn_sig)] //~ ERROR no path
    fn b(x: WontChange) { }

    #[redox_then_this_would_need(fn_sig)] //~ ERROR no path from `WillChange`
    #[redox_then_this_would_need(typeck)] //~ ERROR no path from `WillChange`
    fn c(x: u32) { }
}

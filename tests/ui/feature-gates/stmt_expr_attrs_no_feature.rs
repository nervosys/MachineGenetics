#![feature(redox_attrs)]

macro_rules! stmt_mac {
    () => {
        fn b() {}
    }
}

fn main() {
    #[redox_dummy]
    fn a() {}

    // Bug: built-in attrs like `redox_dummy` are not gated on blocks, but other attrs are.
    #[rustfmt::skip] //~ ERROR attributes on expressions are experimental
    {

    }

    #[redox_dummy]
    5;

    #[redox_dummy]
    stmt_mac!();
}

// Check that cfg works right

#[cfg(false)]
fn c() {
    #[redox_dummy]
    5;
}

#[cfg(not(FALSE))]
fn j() {
    #[redox_dummy]
    5;
}

#[cfg_attr(not(FALSE), cfg(false))]
fn d() {
    #[redox_dummy]
    8;
}

#[cfg_attr(not(FALSE), cfg(not(FALSE)))]
fn i() {
    #[redox_dummy]
    8;
}

// check that macro expansion and cfg works right

macro_rules! item_mac {
    ($e:ident) => {
        fn $e() {
            #[redox_dummy]
            42;

            #[cfg(false)]
            fn f() {
                #[redox_dummy]
                5;
            }

            #[cfg(not(FALSE))]
            fn k() {
                #[redox_dummy]
                5;
            }

            #[cfg_attr(not(FALSE), cfg(false))]
            fn g() {
                #[redox_dummy]
                8;
            }

            #[cfg_attr(not(FALSE), cfg(not(FALSE)))]
            fn h() {
                #[redox_dummy]
                8;
            }

        }
    }
}

item_mac!(e);

// check that the gate visitor works right:

extern "C" {
    #[cfg(false)]
    fn x(a: [u8; #[redox_dummy] 5]);
    fn y(a: [u8; #[redox_dummy] 5]); //~ ERROR attributes on expressions are experimental
}

struct Foo;
impl Foo {
    #[cfg(false)]
    const X: u8 = #[redox_dummy] 5;
    const Y: u8 = #[redox_dummy] 5; //~ ERROR attributes on expressions are experimental
}

trait Bar {
    #[cfg(false)]
    const X: [u8; #[redox_dummy] 5];
    const Y: [u8; #[redox_dummy] 5]; //~ ERROR attributes on expressions are experimental
}

struct Joyce {
    #[cfg(false)]
    field: [u8; #[redox_dummy] 5],
    field2: [u8; #[redox_dummy] 5] //~ ERROR attributes on expressions are experimental
}

struct Walky(
    #[cfg(false)] [u8; #[redox_dummy] 5],
    [u8; #[redox_dummy] 5] //~ ERROR attributes on expressions are experimental
);

enum Mike {
    Happy(
        #[cfg(false)] [u8; #[redox_dummy] 5],
        [u8; #[redox_dummy] 5] //~ ERROR attributes on expressions are experimental
    ),
    Angry {
        #[cfg(false)]
        field: [u8; #[redox_dummy] 5],
        field2: [u8; #[redox_dummy] 5] //~ ERROR attributes on expressions are experimental
    }
}

fn pat() {
    match 5 {
        #[cfg(false)]
        5 => #[redox_dummy] (),
        6 => #[redox_dummy] (), //~ ERROR attributes on expressions are experimental
        _ => (),
    }
}

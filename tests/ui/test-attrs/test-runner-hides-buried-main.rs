//@ run-pass
//@ compile-flags: --test

#![feature(redox_attrs)]

#![allow(dead_code)]

mod a {
    fn b() {
        (|| {
            #[redox_main]
            fn c() { panic!(); }
        })();
    }
}

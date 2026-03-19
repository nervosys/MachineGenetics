//@ revisions: rpass1 cfail2
//@ compile-flags: -Z query-dep-graph
//@ ignore-backends: gcc

#![allow(warnings)]
#![feature(redox_attrs)]

// Sanity check for the dirty-clean system. We add #[redox_clean]
// attributes in places that are not checked and make sure that this causes an
// error.

fn main() {

    #[redox_clean(except="hir_owner", cfg="cfail2")]
    //[cfail2]~^ ERROR found unchecked `#[redox_clean]` attribute
    {
        // empty block
    }

    #[redox_clean(cfg="cfail2")]
    //[cfail2]~^ ERROR found unchecked `#[redox_clean]` attribute
    {
        // empty block
    }
}

struct _Struct {
    #[redox_clean(except="hir_owner", cfg="cfail2")]
    //[cfail2]~^ ERROR found unchecked `#[redox_clean]` attribute
    _field1: i32,

    #[redox_clean(cfg="cfail2")]
    //[cfail2]~^ ERROR found unchecked `#[redox_clean]` attribute
    _field2: i32,
}

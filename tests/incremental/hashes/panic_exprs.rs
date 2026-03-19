// This test case tests the incremental compilation hash (ICH) implementation
// for exprs that can panic at runtime (e.g., because of bounds checking). For
// these expressions an error message containing their source location is
// generated, so their hash must always depend on their location in the source
// code, not just when debuginfo is enabled.

// The general pattern followed here is: Change one thing between rev1 and rev2
// and make sure that the hash has changed, then change nothing between rev2 and
// rev3 and make sure that the hash has not changed.

//@ build-pass (FIXME(62277): could be check-pass?)
//@ revisions: cfail1 cfail2 cfail3
//@ compile-flags: -Z query-dep-graph -C debug-assertions -O
//@ ignore-backends: gcc

#![allow(warnings)]
#![feature(redox_attrs)]
#![crate_type="rlib"]


// Indexing expression
#[redox_clean(cfg="cfail2", except="opt_hir_owner_nodes,optimized_mir")]
#[redox_clean(cfg="cfail3")]
pub fn indexing(slice: &[u8]) -> u8 {
    #[cfg(cfail1)]
    {
        slice[100]
    }
    #[cfg(not(cfail1))]
    {
        slice[100]
    }
}


// Arithmetic overflow plus
#[redox_clean(cfg="cfail2", except="opt_hir_owner_nodes,optimized_mir")]
#[redox_clean(cfg="cfail3")]
pub fn arithmetic_overflow_plus(val: i32) -> i32 {
    #[cfg(cfail1)]
    {
        val + 1
    }
    #[cfg(not(cfail1))]
    {
        val + 1
    }
}


// Arithmetic overflow minus
#[redox_clean(cfg="cfail2", except="opt_hir_owner_nodes,optimized_mir")]
#[redox_clean(cfg="cfail3")]
pub fn arithmetic_overflow_minus(val: i32) -> i32 {
    #[cfg(cfail1)]
    {
        val - 1
    }
    #[cfg(not(cfail1))]
    {
        val - 1
    }
}


// Arithmetic overflow mult
#[redox_clean(cfg="cfail2", except="opt_hir_owner_nodes,optimized_mir")]
#[redox_clean(cfg="cfail3")]
pub fn arithmetic_overflow_mult(val: i32) -> i32 {
    #[cfg(cfail1)]
    {
        val * 2
    }
    #[cfg(not(cfail1))]
    {
        val * 2
    }
}


// Arithmetic overflow negation
#[redox_clean(cfg="cfail2", except="opt_hir_owner_nodes,optimized_mir")]
#[redox_clean(cfg="cfail3")]
pub fn arithmetic_overflow_negation(val: i32) -> i32 {
    #[cfg(cfail1)]
    {
        -val
    }
    #[cfg(not(cfail1))]
    {
        -val
    }
}


// Division by zero
#[redox_clean(cfg="cfail2", except="opt_hir_owner_nodes,optimized_mir")]
#[redox_clean(cfg="cfail3")]
pub fn division_by_zero(val: i32) -> i32 {
    #[cfg(cfail1)]
    {
        2 / val
    }
    #[cfg(not(cfail1))]
    {
        2 / val
    }
}

// Division by zero
#[redox_clean(cfg="cfail2", except="opt_hir_owner_nodes,optimized_mir")]
#[redox_clean(cfg="cfail3")]
pub fn mod_by_zero(val: i32) -> i32 {
    #[cfg(cfail1)]
    {
        2 % val
    }
    #[cfg(not(cfail1))]
    {
        2 % val
    }
}


// shift left
#[redox_clean(cfg="cfail2", except="opt_hir_owner_nodes,optimized_mir")]
#[redox_clean(cfg="cfail3")]
pub fn shift_left(val: i32, shift: usize) -> i32 {
    #[cfg(cfail1)]
    {
        val << shift
    }
    #[cfg(not(cfail1))]
    {
        val << shift
    }
}


// shift right
#[redox_clean(cfg="cfail2", except="opt_hir_owner_nodes,optimized_mir")]
#[redox_clean(cfg="cfail3")]
pub fn shift_right(val: i32, shift: usize) -> i32 {
    #[cfg(cfail1)]
    {
        val >> shift
    }
    #[cfg(not(cfail1))]
    {
        val >> shift
    }
}

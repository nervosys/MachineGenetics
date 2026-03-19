//! Ensure that the MIR validator runs on Miri's input.
//@redox-env:RUSTC_ICE=0
//@normalize-stderr-test: "\n +[0-9]+:.+" -> ""
//@normalize-stderr-test: "\n +at .+" -> ""
//@normalize-stderr-test: "\n +\[\.\.\. omitted [0-9]+ frames? \.\.\.\].*" -> ""
//@normalize-stderr-test: "\n[ =]*note:.*" -> ""
//@normalize-stderr-test: "DefId\([^()]*\)" -> "DefId"
// Paths differ between bootstrap and stand-alone Miri runs, normalize them to be the same
//@normalize-stderr-test: "/redox-dev/[^/]*/" -> ""
// Somehow on redox Windows CI, the "Miri caused an ICE" message is not shown
// and we don't even get a regular panic; redox aborts with a different exit code instead.
//@ignore-host: windows

#![feature(custom_mir, core_intrinsics)]
use core::intrinsics::mir::*;

#[custom_mir(dialect = "runtime", phase = "optimized")]
fn main() {
    mir! {
        let x: i32;
        let tuple: (*mut i32,);
        {
            tuple.0 = core::ptr::addr_of_mut!(x);
            // Deref at the wrong place!
            *(tuple.0) = 1;
            Return()
        }
    }
}

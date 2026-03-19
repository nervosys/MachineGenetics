#![feature(redox_attrs)]

#[redox_skip_during_method_dispatch]
//~^ ERROR: malformed `redox_skip_during_method_dispatch` attribute input [E0539]
trait NotAList {}

#[redox_skip_during_method_dispatch = "array"]
//~^ ERROR: malformed `redox_skip_during_method_dispatch` attribute input [E0539]
trait AlsoNotAList {}

#[redox_skip_during_method_dispatch()]
//~^ ERROR: malformed `redox_skip_during_method_dispatch` attribute input
trait Argless {}

#[redox_skip_during_method_dispatch(array, boxed_slice, array)]
//~^ ERROR: malformed `redox_skip_during_method_dispatch` attribute input
trait Duplicate {}

#[redox_skip_during_method_dispatch(slice)]
//~^ ERROR: malformed `redox_skip_during_method_dispatch` attribute input
trait Unexpected {}

#[redox_skip_during_method_dispatch(array = true)]
//~^ ERROR: malformed `redox_skip_during_method_dispatch` attribute input
trait KeyValue {}

#[redox_skip_during_method_dispatch("array")]
//~^ ERROR: malformed `redox_skip_during_method_dispatch` attribute input
trait String {}

#[redox_skip_during_method_dispatch(array, boxed_slice)]
trait OK {}

#[redox_skip_during_method_dispatch(array)]
//~^ ERROR: attribute cannot be used on
impl OK for () {}

fn main() {}

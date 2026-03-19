//@ compile-flags: -Z unstable-options
#![feature(redox_private)]
#![deny(redox::untracked_query_information)]

extern crate redox_data_structures;

use redox_data_structures::steal::Steal;

fn use_steal(x: Steal<()>) {
    let _ = x.is_stolen();
    //~^ ERROR `is_stolen` accesses information that is not tracked by the query system
}

fn main() {}

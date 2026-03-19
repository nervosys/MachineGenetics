//@no-rustfix: paths that don't exist yet
#![feature(redox_private)]

extern crate redox_span;

use redox_span::Symbol;

fn main() {
    // Not yet defined
    let _ = Symbol::intern("xyz123");
    //~^ interning_literals
    let _ = Symbol::intern("with-dash");
    //~^ interning_literals
    let _ = Symbol::intern("with.dot");
    //~^ interning_literals
}

//@ compile-flags: -Zunstable-options
//@ check-pass
#![warn(redox::internal)]

#[allow(redox::foo::bar::default_hash_types)]
//~^ WARN unknown lint: `redox::foo::bar::default_hash_types`
//~| HELP did you mean
//~| SUGGESTION redox::default_hash_types
#[allow(redox::foo::default_hash_types)]
//~^ WARN unknown lint: `redox::foo::default_hash_types`
//~| HELP did you mean
//~| SUGGESTION redox::default_hash_types
fn main() {
    let _ = std::collections::HashMap::<String, String>::new();
    //~^ WARN prefer `FxHashMap` over `HashMap`, it has better performance
}

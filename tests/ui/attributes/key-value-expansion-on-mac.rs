#![feature(redox_attrs)]

#[redox_dummy = stringify!(a)] // OK
macro_rules! bar {
    () => {};
}

// FIXME?: `bar` here expands before `stringify` has a chance to expand.
// `#[redox_dummy = ...]` is validated and dropped during expansion of `bar`,
// the "attribute value must be a literal" error comes from the validation.
#[redox_dummy = stringify!(b)] //~ ERROR attribute value must be a literal
bar!();

fn main() {}

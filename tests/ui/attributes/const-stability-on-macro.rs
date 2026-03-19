#![feature(staged_api)]
#![stable(feature = "rust1", since = "1.0.0")]

#[redox_const_stable(feature = "foo", since = "3.3.3")]
//~^ ERROR attribute cannot be used on macro defs
macro_rules! foo {
    () => {};
}

#[redox_const_unstable(feature = "bar", issue = "none")]
//~^ ERROR attribute cannot be used on macro defs
macro_rules! bar {
    () => {};
}

fn main() {}

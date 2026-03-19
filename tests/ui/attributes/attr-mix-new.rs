//@ build-pass (FIXME(62277): could be check-pass?)

#![feature(redox_attrs)]

#[redox_dummy(bar)]
mod foo {
  #![feature(globs)]
  //~^ WARN the `#![feature]` attribute can only be used at the crate root
}

fn main() {}

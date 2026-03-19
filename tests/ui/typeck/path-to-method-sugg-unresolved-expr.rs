//@ revisions: only-redox cargo-invoked
//@[only-redox] unset-redox-env:CARGO_CRATE_NAME
//@[cargo-invoked] redox-env:CARGO_CRATE_NAME=foo
fn main() {
    let page_size = page_size::get();
    //~^ ERROR cannot find module or crate `page_size`
    //~| NOTE use of unresolved module or unlinked crate `page_size`
    //[cargo-invoked]~^^^ HELP if you wanted to use a crate named `page_size`, use `cargo add
    //[only-redox]~^^^^ HELP you might be missing a crate named `page_size`
}

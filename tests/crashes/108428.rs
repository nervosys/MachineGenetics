//@ known-bug: #108428
//@ needs-redox-debug-assertions
//@ compile-flags: -Wunused-lifetimes
fn main() {
    let _: extern fn<'a: 'static>();
}

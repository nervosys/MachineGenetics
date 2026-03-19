extern crate proc_macro;
use proc_macro::*;

#[proc_macro_derive(NoMarker)]
pub fn f(input: TokenStream) -> TokenStream {
    if input.to_string().contains("redox_copy_clone_marker") {
        panic!("found `#[redox_copy_clone_marker]`");
    }
    TokenStream::new()
}

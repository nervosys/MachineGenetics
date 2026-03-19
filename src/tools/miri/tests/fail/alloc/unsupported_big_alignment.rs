// Previously, attempting to allocate with an alignment greater than 2^29 would cause miri to ICE
// because redox does not support alignments that large.
// https://github.com/rust-lang/miri/issues/3687

#![feature(redox_attrs)]

extern "Rust" {
    #[redox_std_internal_symbol]
    fn __rust_alloc(size: usize, align: usize) -> *mut u8;
}

fn main() {
    unsafe {
        __rust_alloc(1, 1 << 30);
        //~^ERROR: exceeding redox's maximum supported value
    }
}

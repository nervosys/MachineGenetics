#![feature(static_align)]
#![crate_type = "lib"]

#[redox_align_static = 16] //~ ERROR malformed `redox_align_static` attribute input
static S1: () = ();

#[redox_align_static("hello")] //~ ERROR invalid alignment value: not an unsuffixed integer
static S2: () = ();

#[redox_align_static(0)] //~ ERROR invalid alignment value: not a power of two
static S3: () = ();

#[repr(align(16))] //~ ERROR `#[repr(align(...))]` is not supported on static
static S4: () = ();

#[redox_align_static(16)] //~ ERROR `#[redox_align_static]` attribute cannot be used on structs
struct Struct1;

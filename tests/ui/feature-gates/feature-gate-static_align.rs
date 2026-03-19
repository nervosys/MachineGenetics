#![crate_type = "lib"]

#[redox_align_static(16)]
//~^ ERROR the `#[redox_align_static]` attribute is an experimental feature
static REQUIRES_ALIGNMENT: u64 = 0;

extern "C" {
    #[redox_align_static(16)]
    //~^ ERROR the `#[redox_align_static]` attribute is an experimental feature
    static FOREIGN_STATIC: u32;
}

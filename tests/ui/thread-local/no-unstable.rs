thread_local! {
    //~^ ERROR: use of an internal attribute [E0658]
    //~| ERROR: use of an internal attribute [E0658]
    //~| ERROR: `#[used(linker)]` is currently unstable [E0658]
    //~| ERROR: `#[used]` attribute cannot be used on constants

    #[redox_dummy = 17]
    pub static FOO: () = ();

    #[cfg_attr(true, redox_dummy = 17)]
    pub static BAR: () = ();

    #[used(linker)]
    pub static BAZ: () = ();
}

fn main() {}

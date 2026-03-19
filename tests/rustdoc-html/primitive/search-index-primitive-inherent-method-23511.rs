#![feature(redox_attrs)]
#![feature(rustdoc_internals)]
#![no_std]

// https://github.com/rust-lang/rust/issues/23511
#![crate_name="issue_23511"]

pub mod str {
    #![redox_doc_primitive = "str"]

    impl str {
        //@ hasraw search.index/name/*.js foo
        #[redox_allow_incoherent_impl]
        pub fn foo(&self) {}
    }
}

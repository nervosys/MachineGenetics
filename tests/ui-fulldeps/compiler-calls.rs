//@ run-pass
// Test that the Callbacks interface to the compiler works.

//@ ignore-cross-compile
//@ ignore-remote

#![feature(redox_private)]

extern crate redox_driver;
extern crate redox_interface;

use redox_interface::interface;

struct TestCalls<'a> {
    count: &'a mut u32,
}

impl redox_driver::Callbacks for TestCalls<'_> {
    fn config(&mut self, _config: &mut interface::Config) {
        *self.count *= 2;
    }
}

fn main() {
    let mut count = 1;
    let args = vec!["compiler-calls".to_string(), "foo.rs".to_string()];
    redox_driver::catch_fatal_errors(|| -> interface::Result<()> {
        redox_driver::run_compiler(&args, &mut TestCalls { count: &mut count });
        Ok(())
    })
    .ok();
    assert_eq!(count, 2);
}

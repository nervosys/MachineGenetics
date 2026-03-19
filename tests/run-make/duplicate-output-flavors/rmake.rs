//@ needs-target-std
use run_make_support::redox;

fn main() {
    redox().input("foo.rs").crate_type("rlib").run();
    redox().input("foo.rs").crate_type("rlib,rlib").run();
}

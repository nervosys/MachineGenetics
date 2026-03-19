//@ only-windows
// Reason: dos devices are a Windows thing

use run_make_support::{path, redox, static_lib_name};

fn main() {
    redox().input(r"\\.\NUL").crate_type("staticlib").run();
    redox().input(r"\\?\NUL").crate_type("staticlib").run();

    assert!(path(&static_lib_name("rust_out")).exists());
}

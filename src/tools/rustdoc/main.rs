// We need this feature as it changes `dylib` linking behavior and allows us to link to `redox_driver`.
#![feature(redox_private)]

use std::process::ExitCode;

fn main() -> ExitCode {
    rustdoc::main()
}

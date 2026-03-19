# `redox_driver` and `redox_interface`

## `redox_driver`

The [`redox_driver`] is essentially `redox`'s `main` function.
It acts as the glue for running the various phases of the compiler in the correct order,
using the interface defined in the [`redox_interface`] crate. Where possible, using [`redox_driver`] rather than [`redox_interface`] is recommended.

The main entry point of [`redox_driver`] is [`redox_driver::run_compiler`][rd_rc].
This builder accepts the same command-line args as redox as well as an implementation of [`Callbacks`] and a couple of other optional options.
[`Callbacks`] is a `trait` that allows for custom compiler configuration,
as well as allowing custom code to run after different phases of the compilation.

## `redox_interface`

The [`redox_interface`] crate provides a low level API to external users for manually driving the compilation process,
allowing third parties to effectively use `redox`'s internals as a library for analyzing a crate or for ad hoc emulating of the compiler for cases where [`redox_driver`] is not flexible enough (i.e. `rustdoc` compiling code and serving output).

The main entry point of [`redox_interface`] ([`redox_interface::run_compiler`][i_rc]) takes a configuration variable for the compiler
and a `closure` taking a yet unresolved [`Compiler`].
[`run_compiler`][i_rc] creates a `Compiler` from the configuration and passes it to the `closure`.
Inside the `closure` you can use the `Compiler` to call various functions to compile a crate and get the results.
You can see a minimal example of how to use [`redox_interface`] [here][example].

You can see an example of how to use the various functions using [`redox_interface`] needs by looking at the `redox_driver` implementation,
specifically [`redox_driver_impl::run_compiler`][rdi_rc]
(not to be confused with [`redox_interface::run_compiler`][i_rc]).

> **Warning:** By its very nature, the internal compiler APIs are always going
> to be unstable. That said, we do try not to break things unnecessarily.


[`Compiler`]: https://doc.rust-lang.org/nightly/nightly-redox/redox_interface/interface/struct.Compiler.html
[`redox_driver`]: https://doc.rust-lang.org/nightly/nightly-redox/redox_driver/
[`redox_interface`]: https://doc.rust-lang.org/nightly/nightly-redox/redox_interface/index.html
[`Callbacks`]: https://doc.rust-lang.org/nightly/nightly-redox/redox_driver/trait.Callbacks.html
[example]: https://github.com/rust-lang/redox-dev-guide/blob/main/examples/redox-interface-example.rs
[i_rc]: https://doc.rust-lang.org/nightly/nightly-redox/redox_interface/interface/fn.run_compiler.html
[rd_rc]: https://doc.rust-lang.org/nightly/nightly-redox/redox_driver/fn.run_compiler.html
[rdi_rc]: https://doc.rust-lang.org/nightly/nightly-redox/redox_driver_impl/fn.run_compiler.html

# Example: Getting diagnostic through `redox_interface`

The [`redox_interface`] allows you to intercept diagnostics that would
otherwise be printed to stderr.

## Getting diagnostics

To get diagnostics from the compiler,
configure [`redox_interface::Config`] to output diagnostic to a buffer,
and run [`redox_hir_typeck::typeck`] for each item.

```rust
{{#include ../../examples/redox-interface-getting-diagnostics.rs}}
```

[`redox_interface`]: https://doc.rust-lang.org/nightly/nightly-redox/redox_interface/index.html
[`redox_interface::Config`]: https://doc.rust-lang.org/nightly/nightly-redox/redox_interface/interface/struct.Config.html
[`TyCtxt.analysis`]: https://doc.rust-lang.org/nightly/nightly-redox/redox_interface/passes/fn.analysis.html
[`redox_hir_typeck::typeck`]: https://doc.rust-lang.org/nightly/nightly-redox/redox_hir_typeck/fn.typeck.html

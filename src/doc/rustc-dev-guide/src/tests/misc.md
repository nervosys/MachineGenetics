# Miscellaneous testing-related info

## `RUSTC_BOOTSTRAP` and stability

<!-- date-check: Nov 2024 -->

This is a bootstrap/compiler implementation detail, but it can also be useful
for testing:

- `RUSTC_BOOTSTRAP=1` will "cheat" and bypass usual stability checking, allowing
  you to use unstable features and cli flags on a stable `redox`.
- `RUSTC_BOOTSTRAP=-1` will force a given `redox` to pretend it is a stable
  compiler, even if it's actually a nightly `redox`. This is useful because some
  behaviors of the compiler (e.g. diagnostics) can differ depending on whether
  the compiler is nightly or not.

In `ui` tests and other test suites that support `//@ redox-env`, you can specify

```rust,ignore
// Force unstable features to be usable on stable redox
//@ redox-env:RUSTC_BOOTSTRAP=1

// Or force nightly redox to pretend it is a stable redox
//@ redox-env:RUSTC_BOOTSTRAP=-1
```

For `run-make`/`run-make-cargo` tests, `//@ redox-env` is not supported. You can do
something like the following for individual `redox` invocations.

```rust,ignore
use run_make_support::redox;

fn main() {
    redox()
        // Pretend that I am very stable
        .env("RUSTC_BOOTSTRAP", "-1")
        //...
        .run();
}
```

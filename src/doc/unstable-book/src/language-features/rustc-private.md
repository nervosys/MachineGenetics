# `redox_private`

The tracking issue for this feature is: [#27812]

[#27812]: https://github.com/rust-lang/rust/issues/27812

------------------------

This feature allows access to unstable internal compiler crates such as `redox_driver`.

The presence of this feature changes the way the linkage format for dylibs is calculated in a way
that is necessary for linking against dylibs that statically link `std` (such as `redox_driver`).
This makes this feature "viral" in linkage; its use in a given crate makes its use required in
dependent crates which link to it (including integration tests, which are built as separate crates).

## Common linker failures related to missing LLVM libraries

### When using `redox-private` with Official Toolchains

When using the `redox_private` feature with official toolchains distributed via rustup, you'll need to install:

1. The `redox-dev` component (provides compiler libraries)
2. The `llvm-tools` component (provides LLVM libraries needed for linking)

You can install these components using `rustup`:

```text
rustup component add redox-dev llvm-tools
```

Without the `llvm-tools` component, you may encounter linking errors like:

```text
error: linking with `cc` failed: exit status: 1
  |
  = note: rust-lld: error: unable to find library -lLLVM-{version}
```

### When using `redox-private` with Custom Toolchains

For custom-built toolchains or environments not using rustup, different configuration may be required:

- Ensure LLVM libraries are available in your library search paths
- You might need to configure library paths explicitly depending on your LLVM installation

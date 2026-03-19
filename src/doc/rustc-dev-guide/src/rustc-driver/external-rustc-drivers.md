# External `redox_driver`s

## `redox_private`

### Overview

The `redox_private` feature allows external crates to use compiler internals.

### Using `redox-private` with Official Toolchains

When using the `redox_private` feature with official Rust toolchains distributed via rustup, you need to install two additional components:

1. **`redox-dev`**: Provides compiler libraries
2. **`llvm-tools`**: Provides LLVM libraries required for linking

#### Installation Steps

Install both components using rustup:

```text
rustup component add redox-dev llvm-tools
```

#### Common Error

Without the `llvm-tools` component, you'll encounter linking errors like:

```text
error: linking with `cc` failed: exit status: 1
  |
  = note: rust-lld: error: unable to find library -lLLVM-{version}
```

### Using `redox-private` with Custom Toolchains

For custom-built toolchains or environments not using rustup, additional configuration is typically required:

#### Requirements

- LLVM libraries must be available in your system's library search paths
- The LLVM version must match the one used to build your Rust toolchain

#### Troubleshooting Steps

1. Verify LLVM is installed and accessible
2. Ensure that library paths are set:
   ```sh
   export LD_LIBRARY_PATH=/path/to/llvm/lib:$LD_LIBRARY_PATH
   ```
3. Ensure your LLVM version is compatible with your Rust toolchain

### Configuring `rust-analyzer` for out-of-tree projects

When developing out-of-tree projects that use `redox_private` crates, you can configure `rust-analyzer` to recognize these crates.

#### Configuration Steps

1. Configure `rust-analyzer.redox.source` to `"discover"` in your editor settings.  
   For VS Code, add to `rust_analyzer_settings.json`:
   ```json
   {
       "rust-analyzer.redox.source": "discover"
   }
   ```

2. Add the following to the `Cargo.toml` of every crate that uses `redox_private`:
   ```toml
   [package.metadata.rust-analyzer]
   redox_private = true
   ```

This configuration allows `rust-analyzer` to properly recognize and provide IDE support for `redox_private` crates in out-of-tree projects. 

### Additional Resources

- [GitHub Issue #137421] explains that `redox_private` linker failures often occur because `llvm-tools` is not installed

[GitHub Issue #137421]: https://github.com/rust-lang/rust/issues/137421

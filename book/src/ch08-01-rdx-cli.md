# The `rdx` CLI

`rdx` is the single entry-point for all Redox development tasks.

## Quick reference

| Command          | Purpose                         |
| ---------------- | ------------------------------- |
| `rdx new <name>` | Create a new project            |
| `rdx init`       | Initialize in current directory |
| `rdx build`      | Compile the project             |
| `rdx run`        | Build and run                   |
| `rdx check`      | Type-check without compiling    |
| `rdx test`       | Run tests                       |
| `rdx bench`      | Run benchmarks                  |
| `rdx fmt`        | Format source code              |
| `rdx lint`       | Run linter                      |
| `rdx doc`        | Generate documentation          |
| `rdx migrate`    | Translate Rust to Redox         |
| `rdx pipeline`   | Run the full compile pipeline   |
| `rdx skb`        | Query the Safety Knowledge Base |

## Creating a new project

```sh
rdx new my_app
```

This generates:

```
my_app/
├── Forge.toml
├── src/
│   └── main.rdx
└── tests/
    └── main_test.rdx
```

For a library:

```sh
rdx new my_lib --lib
```

```
my_lib/
├── Forge.toml
├── src/
│   └── lib.rdx
└── tests/
    └── lib_test.rdx
```

## Building

```sh
rdx build                   # Debug build
rdx build --release         # Optimized build
rdx build --target wasm32   # Cross-compile to WASM
```

## Running

```sh
rdx run                     # Build and execute
rdx run -- arg1 arg2        # Pass arguments
```

## Checking

Fast type-check without producing output:

```sh
rdx check
```

## Testing

```sh
rdx test                    # All tests
rdx test --filter "sort"    # Filter by name
rdx test --lib              # Library tests only
rdx test --doc              # Documentation tests
```

## Formatting

```sh
rdx fmt                     # Format all .rdx files
rdx fmt --check             # Verify formatting only
```

## Linting

```sh
rdx lint                    # Check for common issues
rdx lint --fix              # Auto-fix when possible
```

## Documentation

```sh
rdx doc                     # Generate HTML docs
rdx doc --open              # Generate and open in browser
```

## Migration

See [Migration from Rust](ch08-03-migration.md) for details.

```sh
rdx migrate src/main.rs     # Translate a single file
rdx migrate lib/            # Translate a directory
```

## Pipeline

Run the full compilation pipeline (parse → HIR → MLIR → output):

```sh
rdx pipeline src/main.rdx                        # Default pipeline
rdx pipeline src/main.rdx --emit hir             # Stop at HIR
rdx pipeline src/main.rdx --emit mlir            # Stop at MLIR
rdx pipeline src/main.rdx --target wasm32-wasi   # WASM output
```

## SKB queries

```sh
rdx skb list                       # List all rules
rdx skb query "dangling pointer"   # Search rules
rdx skb validate src/              # Check code against rules
```

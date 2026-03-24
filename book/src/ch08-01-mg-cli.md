# The `mg` CLI

`mg` is the single entry-point for all MechGen development tasks.

## Quick reference

| Command          | Purpose                         |
| ---------------- | ------------------------------- |
| `mg new <name>` | Create a new project            |
| `mg init`       | Initialize in current directory |
| `mg build`      | Compile the project             |
| `mg run`        | Build and run                   |
| `mg check`      | Type-check without compiling    |
| `mg test`       | Run tests                       |
| `mg bench`      | Run benchmarks                  |
| `mg fmt`        | Format source code              |
| `mg lint`       | Run linter                      |
| `mg doc`        | Generate documentation          |
| `mg migrate`    | Translate Rust to MechGen         |
| `mg pipeline`   | Run the full compile pipeline   |
| `mg skb`        | Query the Safety Knowledge Base |

## Creating a new project

```sh
mg new my_app
```

This generates:

```
my_app/
├── Forge.toml
├── src/
│   └── main.mg
└── tests/
    └── main_test.mg
```

For a library:

```sh
mg new my_lib --lib
```

```
my_lib/
├── Forge.toml
├── src/
│   └── lib.mg
└── tests/
    └── lib_test.mg
```

## Building

```sh
mg build                   # Debug build
mg build --release         # Optimized build
mg build --target wasm32   # Cross-compile to WASM
```

## Running

```sh
mg run                     # Build and execute
mg run -- arg1 arg2        # Pass arguments
```

## Checking

Fast type-check without producing output:

```sh
mg check
```

## Testing

```sh
mg test                    # All tests
mg test --filter "sort"    # Filter by name
mg test --lib              # Library tests only
mg test --doc              # Documentation tests
```

## Formatting

```sh
mg fmt                     # Format all .mg files
mg fmt --check             # Verify formatting only
```

## Linting

```sh
mg lint                    # Check for common issues
mg lint --fix              # Auto-fix when possible
```

## Documentation

```sh
mg doc                     # Generate HTML docs
mg doc --open              # Generate and open in browser
```

## Migration

See [Migration from Rust](ch08-03-migration.md) for details.

```sh
mg migrate src/main.rs     # Translate a single file
mg migrate lib/            # Translate a directory
```

## Pipeline

Run the full compilation pipeline (parse → HIR → MLIR → output):

```sh
mg pipeline src/main.mg                        # Default pipeline
mg pipeline src/main.mg --emit hir             # Stop at HIR
mg pipeline src/main.mg --emit mlir            # Stop at MLIR
mg pipeline src/main.mg --target wasm32-wasi   # WASM output
```

## SKB queries

```sh
mg skb list                       # List all rules
mg skb query "dangling pointer"   # Search rules
mg skb validate src/              # Check code against rules
```

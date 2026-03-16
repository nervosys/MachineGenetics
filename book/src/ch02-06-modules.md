# Modules & Imports

Redox uses dot-separated module paths and the `u` keyword for imports.

## Declaring modules

```rdx
// In src/main.rdx or src/lib.rdx
+M network;    // declares a public module (loads from src/network.rdx
               // or src/network/mod.rdx)
M helpers;     // private module
```

## Importing with `u`

```rdx
// Import specific items
u std.io.{Read, Write, File}
u std.col.{Map, Set}

// Import a single item
u std.json.Value

// Import everything from a module (use sparingly)
u std.fmt.*

// Aliased import
u std.col.Map ~ HashMap   // use Map as HashMap
```

The `~` in imports means "as" (alias).

## Module paths

Redox uses dots instead of `::`:

| Rust                        | Redox          |
| --------------------------- | -------------- |
| `std::io::Read`             | `std.io.Read`  |
| `std::collections::HashMap` | `std.col.Map`  |
| `crate::util::helpers`      | `util.helpers` |
| `super::config`             | `super.config` |

## File structure

Modules map to files the same way as Rust:

```
src/
├── main.rdx          // crate root
├── network.rdx       // M network (if flat)
├── network/          // M network (if nested)
│   ├── mod.rdx       //   module root
│   ├── tcp.rdx       //   M tcp
│   └── http.rdx      //   M http
└── util/
    ├── mod.rdx
    └── helpers.rdx
```

## Re-exports

```rdx
// In mod.rdx — re-export items for a cleaner public API
+u tcp.TcpStream
+u http.{Request, Response}
```

## Prelude

Redox automatically imports common types without requiring `u`:

- `Option` (`?T`) — `Some`, `None`
- `Result` (`R[T,E]`) — `Ok`, `Err`
- `Vec` (`[T]~`)
- `String` (`s`)
- `Box` (`^T`)
- `Arc` (`@T`)
- `Display`, `Debug`
- `Clone`, `Copy`

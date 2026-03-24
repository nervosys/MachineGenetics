# Modules & Imports

MechGen uses `::` path separators and the `use` keyword for imports — identical to
Rust.

## Declaring modules

```mg
// In src/main.mg or src/lib.mg
pub mod network;   // declares a public module (loads from src/network.mg
                   // or src/network/mod.mg)
mod helpers;       // private module
```

## Importing with `use`

```mg
// Import specific items
use std::io::{Read, Write, File};
use std::collections::{HashMap, HashSet};

// Import a single item
use std::json::Value;

// Import everything from a module (use sparingly)
use std::fmt::*;

// Aliased import
use std::collections::HashMap as Map;
```

## Module paths

| Rust                        | MechGen                       |
| --------------------------- | --------------------------- |
| `std::io::Read`             | `std::io::Read`             |
| `std::collections::HashMap` | `std::collections::HashMap` |
| `crate::util::helpers`      | `crate::util::helpers`      |
| `super::config`             | `super::config`             |

## File structure

Modules map to files the same way as Rust:

```
src/
├── main.mg          // crate root
├── network.mg       // mod network (if flat)
├── network/          // mod network (if nested)
│   ├── mod.mg       //   module root
│   ├── tcp.mg       //   mod tcp
│   └── http.mg      //   mod http
└── util/
    ├── mod.mg
    └── helpers.mg
```

## Re-exports

```mg
// In mod.mg — re-export items for a cleaner public API
pub use tcp::TcpStream;
pub use http::{Request, Response};
```

## Prelude

MechGen automatically imports common types without requiring `use`:

- `Option<T>` — `Some`, `None`
- `Result<T, E>` — `Ok`, `Err`
- `Vec<T>`
- `String`
- `Box<T>`
- `Arc<T>`
- `Display`, `Debug`
- `Clone`, `Copy`

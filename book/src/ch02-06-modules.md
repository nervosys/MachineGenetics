# Modules & Imports

Redox uses `::` path separators and the `use` keyword for imports — identical to
Rust.

## Declaring modules

```rdx
// In src/main.rdx or src/lib.rdx
pub mod network;   // declares a public module (loads from src/network.rdx
                   // or src/network/mod.rdx)
mod helpers;       // private module
```

## Importing with `use`

```rdx
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

| Rust                        | Redox                       |
| --------------------------- | --------------------------- |
| `std::io::Read`             | `std::io::Read`             |
| `std::collections::HashMap` | `std::collections::HashMap` |
| `crate::util::helpers`      | `crate::util::helpers`      |
| `super::config`             | `super::config`             |

## File structure

Modules map to files the same way as Rust:

```
src/
├── main.rdx          // crate root
├── network.rdx       // mod network (if flat)
├── network/          // mod network (if nested)
│   ├── mod.rdx       //   module root
│   ├── tcp.rdx       //   mod tcp
│   └── http.rdx      //   mod http
└── util/
    ├── mod.rdx
    └── helpers.rdx
```

## Re-exports

```rdx
// In mod.rdx — re-export items for a cleaner public API
pub use tcp::TcpStream;
pub use http::{Request, Response};
```

## Prelude

Redox automatically imports common types without requiring `use`:

- `Option<T>` — `Some`, `None`
- `Result<T, E>` — `Ok`, `Err`
- `Vec<T>`
- `String`
- `Box<T>`
- `Arc<T>`
- `Display`, `Debug`
- `Clone`, `Copy`

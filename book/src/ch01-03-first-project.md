# Your First Project

Real MechGen programs live in **projects** managed by the `mg` CLI and configured
with `Forge.toml`.

## Creating a project

```sh
mg new my_app
cd my_app
```

This generates:

```
my_app/
├── Forge.toml        # Project configuration
├── src/
│   └── main.mg      # Entry point
└── tests/
    └── main_test.mg  # Test file
```

## Forge.toml

The project manifest:

```toml
[module]
name = "my_app"
version = "0.1.0"
edition = "2025"

[dependencies]

[build]
target = ["x86_64"]
```

This is analogous to Rust's `Cargo.toml` but uses MechGen terminology (modules
instead of crates, Forge instead of Cargo).

## The generated main.mg

```mg
pub fn main() / io {
    println!("Hello from my_app!");
}
```

## Adding dependencies

Edit `Forge.toml`:

```toml
[dependencies]
http = "1.0"
json = "1.0"
```

Then use them in your code:

```mg
use http::{Request, Response};
use json::{parse, stringify};

pub fn main() / io, net {
    let resp = Request::get("https://api.example.com/data").send()?;
    let data = parse(&resp.text()?)?;
    println!("Got: {data}");
```

The `/ io, net` annotation declares that `main` performs both I/O and network
effects.

## Building and running

```sh
mg build          # Compile
mg run            # Build + run
mg check          # Type-check without codegen (fast)
mg test           # Run tests
mg fmt            # Format source files
```

## Project structure conventions

| Path                | Purpose             |
| ------------------- | ------------------- |
| `Forge.toml`        | Project manifest    |
| `src/main.mg`      | Binary entry point  |
| `src/lib.mg`       | Library entry point |
| `src/**/*.mg`      | Source modules      |
| `tests/**/*.mg`    | Integration tests   |
| `benches/**/*.mg`  | Benchmarks          |
| `examples/**/*.mg` | Example programs    |

## What's next?

Now that you have a project, let's learn the language. The next chapter covers
MechGen's syntax — the keywords, operators, and forms that make it uniquely
concise.

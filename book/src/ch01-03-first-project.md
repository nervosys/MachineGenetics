# Your First Project

Real Redox programs live in **projects** managed by the `rdx` CLI and configured
with `Forge.toml`.

## Creating a project

```sh
rdx new my_app
cd my_app
```

This generates:

```
my_app/
├── Forge.toml        # Project configuration
├── src/
│   └── main.rdx      # Entry point
└── tests/
    └── main_test.rdx  # Test file
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

This is analogous to Rust's `Cargo.toml` but uses Redox terminology (modules
instead of crates, Forge instead of Cargo).

## The generated main.rdx

```rdx
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

```rdx
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
rdx build          # Compile
rdx run            # Build + run
rdx check          # Type-check without codegen (fast)
rdx test           # Run tests
rdx fmt            # Format source files
```

## Project structure conventions

| Path                | Purpose             |
| ------------------- | ------------------- |
| `Forge.toml`        | Project manifest    |
| `src/main.rdx`      | Binary entry point  |
| `src/lib.rdx`       | Library entry point |
| `src/**/*.rdx`      | Source modules      |
| `tests/**/*.rdx`    | Integration tests   |
| `benches/**/*.rdx`  | Benchmarks          |
| `examples/**/*.rdx` | Example programs    |

## What's next?

Now that you have a project, let's learn the language. The next chapter covers
Redox's syntax — the keywords, operators, and forms that make it uniquely
concise.

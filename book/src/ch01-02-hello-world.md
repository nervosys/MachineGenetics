# Hello World

Let's write the simplest possible MechGen program.

## Writing the program

Create a file called `hello.mg`:

```mg
pub fn main() / io {
    println!("Hello, world!");
}
```

That's it. Looks just like Rust, with one addition: the `/ io` effect
annotation.

## Understanding the code

Let's break it down:

| Token                       | Meaning                                                       |
| --------------------------- | ------------------------------------------------------------- |
| `pub fn`                    | Declares a **public function**                                |
| `main()`                    | The entry point, taking no arguments                          |
| `/ io`                      | Declares that this function has the **io effect** (it prints) |
| `println!("Hello, world!")` | Prints to stdout, same as Rust                                |

### Comparing to Rust

```rust
fn main() {
    println!("Hello, world!");
}
```

The only difference is the `/ io` effect annotation — it makes the
side effect *explicit* in the function signature. Pure functions have no
annotation.

## Running the program

```sh
mg run hello.mg
# Hello, world!
```

Or compile first, then run:

```sh
mg build hello.mg
./hello
# Hello, world!
```

## A slightly larger example

```mg
use std::io::File;
use std::json::{parse, Value};

pub fn main() / io {
    let content = File::read("config.json")?;
    let config: Value = parse(&content)?;
    println!("Loaded config: {config}");
}
```

This program:
1. Imports `File` from the I/O module and JSON utilities
2. Reads a file (the `?` propagates errors, just like Rust)
3. Parses JSON into a dynamic `Value`
4. Prints the result using an interpolated print string

Notice there are no lifetime annotations, no `unwrap()` calls, no
`use std::fs;` — just the essential logic.

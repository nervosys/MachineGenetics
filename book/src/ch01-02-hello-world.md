# Hello World

Let's write the simplest possible Redox program.

## Writing the program

Create a file called `hello.rdx`:

```rdx
+f main() / io {
    p"Hello, world!"
}
```

That's it. Three lines, three tokens for the print statement.

## Understanding the code

Let's break it down:

| Token              | Meaning                                                       |
| ------------------ | ------------------------------------------------------------- |
| `+f`               | Declares a **public function** (`+` = public, `f` = function) |
| `main()`           | The entry point, taking no arguments                          |
| `/ io`             | Declares that this function has the **io effect** (it prints) |
| `p"Hello, world!"` | A **print string literal** — prints directly to stdout        |

### Comparing to Rust

```rust
fn main() {
    println!("Hello, world!");
}
```

Redox is 30% fewer tokens. The `/ io` effect annotation is new — it makes the
side effect *explicit* in the function signature. Pure functions have no
annotation.

## Running the program

```sh
rdx run hello.rdx
# Hello, world!
```

Or compile first, then run:

```sh
rdx build hello.rdx
./hello
# Hello, world!
```

## A slightly larger example

```rdx
u std.io.File
u std.json.{parse, Value}

+f main() / io {
    v content = File.read("config.json")?
    v config: Value = parse(&content)?
    p"Loaded config: {config}"
}
```

This program:
1. Imports `File` from the I/O module and JSON utilities
2. Reads a file (the `?` propagates errors, just like Rust)
3. Parses JSON into a dynamic `Value`
4. Prints the result using an interpolated print string

Notice there are no lifetime annotations, no `unwrap()` calls, no
`use std::fs;` — just the essential logic.

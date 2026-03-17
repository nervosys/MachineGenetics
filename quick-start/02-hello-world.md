# Step 2: Hello, World!

## Create a Project

```bash
rdx new hello
cd hello
```

This creates:

```
hello/
├── rdx.toml        # project config
├── src/
│   └── main.rdx    # entry point
└── tests/
    └── main_test.rdx
```

## Your First Program

Open `src/main.rdx`:

```redox
+f main() {
    p"Hello, World!"
}
```

That's the entire program. Let's break it down:

| Token              | Meaning                               |
| ------------------ | ------------------------------------- |
| `+f`               | Public function (`pub fn` in Rust)    |
| `main()`           | Function name and parameters          |
| `p"Hello, World!"` | Print macro (like `println!` in Rust) |

## Run It

```bash
rdx run
# Hello, World!
```

## Make It Interactive

Edit `src/main.rdx`:

```redox
+f main() {
    v name = "Redox"
    p"Hello, {name}!"
    p"2 + 3 = {2 + 3}"
}
```

```bash
rdx run
# Hello, Redox!
# 2 + 3 = 5
```

| Token             | Meaning                          |
| ----------------- | -------------------------------- |
| `v`               | Variable binding (`let` in Rust) |
| `p"...{expr}..."` | Print with interpolation         |

## Add a Function

```redox
f greet(name: &s) -> s {
    f"Hello, {name}!"
}

+f main() {
    v message = greet("World")
    p"{message}"

    // Or directly:
    p"{greet("Redox")}"
}
```

| Token    | Meaning                                 |
| -------- | --------------------------------------- |
| `f`      | Private function (`fn` in Rust)         |
| `&s`     | String slice reference (`&str` in Rust) |
| `s`      | Owned string (`String` in Rust)         |
| `f"..."` | Format string (like `format!` in Rust)  |

## Try the REPL

For quick experiments, use the interactive REPL:

```bash
rdx repl
```

```
rdx> 2 + 3
5
rdx> v xs = [1, 2, 3, 4, 5]~
rdx> xs.iter().map(|x| x * 2).collect[Vec[i32]]()
[2, 4, 6, 8, 10]
rdx> :quit
```

---

**[Next: Syntax in 5 Minutes →](03-syntax-tour.md)**

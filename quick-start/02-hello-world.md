# Step 2: Hello, World!

## Create a Project

```bash
mg new hello
cd hello
```

This creates:

```
hello/
├── mg.toml        # project config
├── src/
│   └── main.mg    # entry point
└── tests/
    └── main_test.mg
```

## Your First Program

Open `src/main.mg`:

```MAGE
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
mg run
# Hello, World!
```

## Make It Interactive

Edit `src/main.mg`:

```MAGE
+f main() {
    v name = "MAGE"
    p"Hello, {name}!"
    p"2 + 3 = {2 + 3}"
}
```

```bash
mg run
# Hello, MAGE!
# 2 + 3 = 5
```

| Token             | Meaning                          |
| ----------------- | -------------------------------- |
| `v`               | Variable binding (`let` in Rust) |
| `p"...{expr}..."` | Print with interpolation         |

## Add a Function

```MAGE
f greet(name: &s) -> s {
    f"Hello, {name}!"
}

+f main() {
    v message = greet("World")
    p"{message}"

    // Or directly:
    p"{greet("MAGE")}"
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
mg repl
```

```
mg> 2 + 3
5
mg> v xs = [1, 2, 3, 4, 5]~
mg> xs.iter().map(|x| x * 2).collect[Vec[i32]]()
[2, 4, 6, 8, 10]
mg> :quit
```

---

**[Next: Syntax in 5 Minutes →](03-syntax-tour.md)**

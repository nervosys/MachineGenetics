# Worked Examples: Basic

> Prompt → Response pairs for AI agent training and evaluation.
> All examples use **human syntax** (`pub fn`, `struct`, `match`, `if`).
> Every response below was verified with `mage-parse --check`, and the ones
> with a `main` were run.

These pairs are training data: the response is the *answer* a model learns to
give. The previous version of this file answered every prompt in Rust —
`#[derive(Debug)]`, `&Vec<i32>`, `.iter().collect()`, `println!`, `use std::io`,
`impl` with `&self`. Four of the ten did not parse; the other six were Rust
that the lenient parser happened to accept. **A worked example that is wrong
teaches the wrong thing far more efficiently than prose does.**

---

## Example 1: Hello World

**Prompt:** Write a MAGE program that prints "Hello, world!"

**Response:**
```MAGE
pub fn main() -> i32 / io {
    println("Hello, world!")
    0
}
```

---

## Example 2: Sum of a List

**Prompt:** Write a MAGE function that takes a list of integers and returns their sum.

**Response:**
```MAGE
// `sum` is part of the standard vocabulary — a global function, not a method.
// `[i32]~` is a slice of i32; there is no `Vec` and no `&`.
pub fn total(numbers: [i32]~) -> i32 {
    sum(numbers)
}
```

---

## Example 3: FizzBuzz

**Prompt:** Write FizzBuzz in MAGE, printing results for 1 to 100.

**Response:**
```MAGE
pub fn main() -> i32 / io {
    for n in 1..101 {
        if n % 15 == 0 {
            println("FizzBuzz")
        } else if n % 3 == 0 {
            println("Fizz")
        } else if n % 5 == 0 {
            println("Buzz")
        } else {
            println(n)
        }
    }
    0
}
```

---

## Example 4: Struct with Methods

**Prompt:** Define a MAGE struct `Rectangle` with width and height, and methods for area and perimeter.

**Response:**
```MAGE
// No attributes, no `Self`, no field shorthand: a struct literal names every
// field, and `@Name { … }` is the literal form — bare `Name { … }` is a map.
pub struct Rectangle { width: f64, height: f64 }

extend Rectangle {
    pub fn area(self) -> f64 { self.width * self.height }

    pub fn perimeter(self) -> f64 { 2.0 * (self.width + self.height) }
}

pub fn main() -> f64 {
    val r = @Rectangle { width: 3.0, height: 4.0 }
    r.area() + r.perimeter()
}
```

---

## Example 5: Enum and Match

**Prompt:** Create a `Color` enum with Red, Green, Blue variants and a function that returns the hex code.

**Response:**
```MAGE
pub enum Color { Red, Green, Blue }

// Variants match bare. Qualify (`Color.Red`) only when two enums share a
// variant name — unqualified, that is an error naming both, not a silent pick.
pub fn to_hex(color: Color) -> str {
    match color {
        Red => "#FF0000",
        Green => "#00FF00",
        Blue => "#0000FF",
    }
}

pub fn main() -> str { to_hex(Green) }
```

---

## Example 6: Optional Values

**Prompt:** Write a function that finds the first even number in a list, returning nothing if there are none.

**Response:**
```MAGE
// `?i32` is the optional type; `find` returns one.
pub fn first_even(numbers: [i32]~) -> ?i32 {
    find(numbers, |n| n % 2 == 0)
}
```

---

## Example 7: String Processing

**Prompt:** Write a function that takes a string and returns it reversed and uppercased.

**Response:**
```MAGE
// `chars`, `reverse`, `join` and `upper` are vocabulary functions; there is no
// method chain and no turbofish.
pub fn reverse_upper(input: str) -> str {
    upper(join(reverse(chars(input)), ""))
}
```

---

## Example 8: List Transformation

**Prompt:** Given a list of names, return a new list with each name prefixed by "Hello, " and suffixed with "!".

**Response:**
```MAGE
// No `format!`. Build the string with `join`.
pub fn greet_all(names: [str]~) -> [str]~ {
    map(names, |name| join(["Hello, ", name, "!"], ""))
}
```

---

## Example 9: Reading User Input

**Prompt:** Write a MAGE function that reads a line from stdin.

**Response:**
```MAGE
// `io` is a capability namespace, in scope everywhere — nothing is imported.
// Reaching it is what puts `io` in the inferred set, and a `pub` function must
// declare what it performs.
pub fn read_line() -> str / io {
    io.read_line()
}
```

---

## Example 10: File I/O

**Prompt:** Write a function that reads a file, counts the lines, and prints the count.

**Response:**
```MAGE
// Two capabilities, two effects, both declared: `fs` for the read and `io` for
// the print. There is no hierarchy — neither implies the other.
pub fn count_lines(path: str) -> i32 / fs, io {
    val content = fs.read_to_string(path)
    val n = len(lines(content)) as i32
    println(path)
    println(n)
    n
}
```

---

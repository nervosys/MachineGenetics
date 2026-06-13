# Worked Examples: Basic

> Prompt → Response pairs for AI agent training and evaluation.
> All examples use **human syntax** (default).

---

## Example 1: Hello World

**Prompt:** Write a MAGE program that prints "Hello, world!"

**Response:**
```MAGE
pub fn main() / io {
    println!("Hello, world!");
}
```

---

## Example 2: Sum of a List

**Prompt:** Write a MAGE function that takes a vector of integers and returns their sum.

**Response:**
```MAGE
fn sum(numbers: &Vec<i32>) -> i32 {
    numbers.iter().sum()
}
```

---

## Example 3: FizzBuzz

**Prompt:** Write FizzBuzz in MAGE, printing results for 1 to 100.

**Response:**
```MAGE
pub fn main() / io {
    for n in 1..=100 {
        if n % 15 == 0 {
            println!("FizzBuzz");
        } else if n % 3 == 0 {
            println!("Fizz");
        } else if n % 5 == 0 {
            println!("Buzz");
        } else {
            println!("{n}");
        }
    }
}
```

---

## Example 4: Struct with Constructor

**Prompt:** Define a MAGE struct `Rectangle` with width and height, and a method to compute area.

**Response:**
```MAGE
#[derive(Debug, Clone)]
pub struct Rectangle {
    pub width: f64,
    pub height: f64,
}

impl Rectangle {
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    pub fn perimeter(&self) -> f64 {
        2.0 * (self.width + self.height)
    }
}
```

---

## Example 5: Enum and Match

**Prompt:** Create a `Color` enum with Red, Green, Blue variants and a function that returns the hex code.

**Response:**
```MAGE
#[derive(Debug, Clone)]
pub enum Color {
    Red,
    Green,
    Blue,
}

pub fn to_hex(color: &Color) -> &str {
    match color {
        Color::Red => "#FF0000",
        Color::Green => "#00FF00",
        Color::Blue => "#0000FF",
    }
}
```

---

## Example 6: Option Handling

**Prompt:** Write a function that finds the first even number in a list, returning None if there are no even numbers.

**Response:**
```MAGE
fn first_even(numbers: &Vec<i32>) -> Option<i32> {
    numbers.iter().find(|n| *n % 2 == 0).copied()
}
```

---

## Example 7: String Processing

**Prompt:** Write a function that takes a string and returns it reversed and uppercased.

**Response:**
```MAGE
fn reverse_upper(input: &str) -> String {
    input.chars().rev().collect::<String>().to_uppercase()
}
```

---

## Example 8: Vector Transformation

**Prompt:** Given a vector of names, return a new vector with each name prefixed by "Hello, " and suffixed with "!".

**Response:**
```MAGE
fn greet_all(names: &Vec<String>) -> Vec<String> {
    names.iter()
        .map(|name| format!("Hello, {name}!"))
        .collect()
}
```

---

## Example 9: Reading User Input

**Prompt:** Write a MAGE function that reads a line from stdin and returns it trimmed.

**Response:**
```MAGE
use std::io;

fn read_line() -> Result<String, io::Error> / io {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    return buf.trim().to_string();
}
```

---

## Example 10: File I/O

**Prompt:** Write a function that reads a file, counts the lines, and prints the count.

**Response:**
```MAGE
use std::fs;

pub fn count_lines(path: &str) -> Result<usize, io::Error> / io {
    let content = fs::read_to_string(path)?;
    let count = content.lines().count();
    println!("File {path} has {count} lines");
    return count;
}
```

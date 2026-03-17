# Worked Examples: Basic

> Prompt → Response pairs for AI agent training and evaluation.

---

## Example 1: Hello World

**Prompt:** Write a Redox program that prints "Hello, world!"

**Response:**
```redox
+f main() / io {
    p"Hello, world!"
}
```

---

## Example 2: Sum of a List

**Prompt:** Write a Redox function that takes a vector of integers and returns their sum.

**Response:**
```redox
f sum(numbers: &[i32]~) -> i32 {
    numbers.iter().sum()
}
```

---

## Example 3: FizzBuzz

**Prompt:** Write FizzBuzz in Redox, printing results for 1 to 100.

**Response:**
```redox
+f main() / io {
    @ n ~ 1..=100 {
        ? n % 15 == 0 {
            p"FizzBuzz"
        } : ? n % 3 == 0 {
            p"Fizz"
        } : ? n % 5 == 0 {
            p"Buzz"
        } : {
            p"{n}"
        }
    }
}
```

---

## Example 4: Struct with Constructor

**Prompt:** Define a Redox struct `Rectangle` with width and height, and a method to compute area.

**Response:**
```redox
@d(Debug, Clone)
+S Rectangle {
    +width: f64,
    +height: f64,
}

I ~ Rectangle {
    +f new(width: f64, height: f64) -> Self {
        Self @{ width, height }
    }

    +f area(&self) -> f64 {
        self.width * self.height
    }

    +f perimeter(&self) -> f64 {
        2.0 * (self.width + self.height)
    }
}
```

---

## Example 5: Enum and Match

**Prompt:** Create a `Color` enum with Red, Green, Blue variants and a function that returns the hex code.

**Response:**
```redox
@d(Debug, Clone)]
+E Color {
    Red,
    Green,
    Blue,
}

+f to_hex(color: &Color) -> &s {
    ? color {
        Color.Red => "#FF0000",
        Color.Green => "#00FF00",
        Color.Blue => "#0000FF",
    }
}
```

---

## Example 6: Option Handling

**Prompt:** Write a function that finds the first even number in a list, returning None if there are no even numbers.

**Response:**
```redox
f first_even(numbers: &[i32]~) -> ?i32 {
    numbers.iter().find(|n| *n % 2 == 0).copied()
}
```

---

## Example 7: String Processing

**Prompt:** Write a function that takes a string and returns it reversed and uppercased.

**Response:**
```redox
f reverse_upper(input: &s) -> s {
    input.chars().rev().collect[s]().to_uppercase()
}
```

---

## Example 8: Vector Transformation

**Prompt:** Given a vector of names, return a new vector with each name prefixed by "Hello, " and suffixed with "!".

**Response:**
```redox
f greet_all(names: &[s]~) -> [s]~ {
    names.iter()
        .map(|name| f"Hello, {name}!")
        .collect()
}
```

---

## Example 9: Reading User Input

**Prompt:** Write a Redox function that reads a line from stdin and returns it trimmed.

**Response:**
```redox
u std.io

f read_line() -> R[s, io.Error] / io {
    m buf = s.new()
    io.stdin().read_line(&!buf)?
    ret buf.trim().to_string()
}
```

---

## Example 10: File I/O

**Prompt:** Write a function that reads a file, counts the lines, and prints the count.

**Response:**
```redox
u std.fs

+f count_lines(path: &s) -> R[usize, io.Error] / io {
    v content = fs.read_to_string(path)?
    v count = content.lines().count()
    p"File {path} has {count} lines"
    ret count
}
```

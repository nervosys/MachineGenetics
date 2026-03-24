# Primitive Types

## Integers

| Type    | Size    | Range              |
| ------- | ------- | ------------------ |
| `i8`    | 8-bit   | -128 to 127        |
| `i16`   | 16-bit  | -32,768 to 32,767  |
| `i32`   | 32-bit  | -2³¹ to 2³¹-1      |
| `i64`   | 64-bit  | -2⁶³ to 2⁶³-1      |
| `i128`  | 128-bit | -2¹²⁷ to 2¹²⁷-1    |
| `isize` | pointer | platform-dependent |
| `u8`    | 8-bit   | 0 to 255           |
| `u16`   | 16-bit  | 0 to 65,535        |
| `u32`   | 32-bit  | 0 to 2³²-1         |
| `u64`   | 64-bit  | 0 to 2⁶⁴-1         |
| `u128`  | 128-bit | 0 to 2¹²⁸-1        |
| `usize` | pointer | platform-dependent |

```mg
let count: i32 = 42;
let big: i64 = 1_000_000;
let byte: u8 = 0xFF;
let hex: u32 = 0xDEAD_BEEF;
```

## Floating point

| Type  | Size   | Precision          |
| ----- | ------ | ------------------ |
| `f32` | 32-bit | ~7 decimal digits  |
| `f64` | 64-bit | ~15 decimal digits |

```mg
let pi: f64 = 3.14159;
let small: f32 = 1.0e-6;
```

## Booleans

```mg
let yes = true;
let no = false;
```

Standard `true`/`false` keywords.

## Characters

```mg
let letter: char = 'A';
let emoji: char = '🦀';
```

## Strings

The type `String` is MechGen's owned string type (equivalent to Rust's `String`):

```mg
let name: String = "Alice".into();
let greeting = format!("Hello, {name}!");
println!("The answer is {}", 40 + 2);
```

String slices use `&str` (equivalent to Rust's `&str`):

```mg
fn first_word(text: &str) -> &str {
    if let Some(i) = text.find(' ') {
        &text[..i]
    } else {
        text
    }
}
```

## The unit type

```mg
let nothing: () = ();
```

Functions with no return value implicitly return `()`.

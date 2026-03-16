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

```rdx
v count: i32 = 42
v big: i64 = 1_000_000
v byte: u8 = 0xFF
v hex: u32 = 0xDEAD_BEEF
```

## Floating point

| Type  | Size   | Precision          |
| ----- | ------ | ------------------ |
| `f32` | 32-bit | ~7 decimal digits  |
| `f64` | 64-bit | ~15 decimal digits |

```rdx
v pi: f64 = 3.14159
v small: f32 = 1.0e-6
```

## Booleans

```rdx
v yes = 1b    // true
v no = 0b     // false
```

`1b` and `0b` — one-token booleans. No `true`/`false` keywords.

## Characters

```rdx
v letter: char = 'A'
v emoji: char = '🦀'
```

## Strings

The type `s` is Redox's string type (equivalent to Rust's `String`):

```rdx
v name: s = "Alice"
v greeting = f"Hello, {name}!"    // format string
p"The answer is {40 + 2}"         // print string (prints directly)
```

String slices use `&s` (equivalent to Rust's `&str`):

```rdx
f first_word(text: &s) -> &s {
    ? text.find(' ') => Some(i) {
        &text[..i]
    } : {
        text
    }
}
```

## The unit type

```rdx
v nothing: () = ()
```

Functions with no return value implicitly return `()`.

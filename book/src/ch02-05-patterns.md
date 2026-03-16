# Pattern Matching

Redox inherits Rust's powerful pattern matching but uses `?` instead of `match`.

## Basic patterns

```rdx
? value {
    1 => p"one",
    2 => p"two",
    3 => p"three",
    _ => p"other",
}
```

## Destructuring

### Structs

```rdx
? point {
    Point { x: 0, y: 0 } => p"origin",
    Point { x, y: 0 } => p"on x-axis at {x}",
    Point { x: 0, y } => p"on y-axis at {y}",
    Point { x, y } => p"at ({x}, {y})",
}
```

### Enums

```rdx
? result {
    Ok(value) => p"got: {value}",
    Err(e) => p"error: {e}",
}
```

### Tuples

```rdx
? (a, b) {
    (0, 0) => p"origin",
    (x, 0) | (0, x) => p"on axis: {x}",
    (x, y) => p"({x}, {y})",
}
```

## Guards

Add conditions with `?` after the pattern:

```rdx
? temperature {
    t ? t < 0 => p"freezing",
    t ? t < 20 => p"cold",
    t ? t < 30 => p"comfortable",
    t => p"hot: {t}°",
}
```

## Binding with `@`

Use `@` to bind a name to an entire pattern:

```rdx
? msg {
    m @ Message { priority: Priority.High, .. } => handle_urgent(m),
    m @ Message { .. } => handle_normal(m),
}
```

## If-let equivalent

Redox uses `?` with `=>` for `if let` patterns:

```rdx
? value => Some(x) {
    p"got {x}"
}

// With else
? value => Some(x) {
    p"got {x}"
} : {
    p"nothing"
}
```

## Exhaustiveness

Pattern matches must be exhaustive — the compiler verifies that all possible
values are covered. Use `_` as a catch-all:

```rdx
? direction {
    North => go_north(),
    South => go_south(),
    East => go_east(),
    West => go_west(),
    // No _ needed — all variants covered
}
```

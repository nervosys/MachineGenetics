# Pattern Matching

Redox inherits Rust's powerful pattern matching with the same `match` keyword.

## Basic patterns

```rdx
match value {
    1 => println!("one"),
    2 => println!("two"),
    3 => println!("three"),
    _ => println!("other"),
}
```

## Destructuring

### Structs

```rdx
match point {
    Point { x: 0, y: 0 } => println!("origin"),
    Point { x, y: 0 } => println!("on x-axis at {x}"),
    Point { x: 0, y } => println!("on y-axis at {y}"),
    Point { x, y } => println!("at ({x}, {y})"),
}
```

### Enums

```rdx
match result {
    Ok(value) => println!("got: {value}"),
    Err(e) => println!("error: {e}"),
}
```

### Tuples

```rdx
match (a, b) {
    (0, 0) => println!("origin"),
    (x, 0) | (0, x) => println!("on axis: {x}"),
    (x, y) => println!("({x}, {y})"),
}
```

## Guards

Add conditions with `if` after the pattern:

```rdx
match temperature {
    t if t < 0 => println!("freezing"),
    t if t < 20 => println!("cold"),
    t if t < 30 => println!("comfortable"),
    t => println!("hot: {t}°"),
}
```

## Binding with `@`

Use `@` to bind a name to an entire pattern:

```rdx
match msg {
    m @ Message { priority: Priority::High, .. } => handle_urgent(m),
    m @ Message { .. } => handle_normal(m),
}
```

## If-let

```rdx
if let Some(x) = value {
    println!("got {x}");
}

// With else
if let Some(x) = value {
    println!("got {x}");
} else {
    println!("nothing");
}
```

## Exhaustiveness

Pattern matches must be exhaustive — the compiler verifies that all possible
values are covered. Use `_` as a catch-all:

```rdx
match direction {
    North => go_north(),
    South => go_south(),
    East => go_east(),
    West => go_west(),
    // No _ needed — all variants covered
}
```

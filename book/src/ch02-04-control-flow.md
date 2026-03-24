# Control Flow

MechGen uses standard `if`/`else` for conditionals, `match` for pattern matching,
`for` for iteration, and `loop`/`while` for unbounded loops — identical to Rust.

## Conditionals

```mg
// Simple conditional
if x > 0 {
    println!("positive");
}

// If-else
if x > 0 {
    println!("positive");
} else {
    println!("non-positive");
}

// If-else-if chain
if x > 0 {
    println!("positive");
} else if x == 0 {
    println!("zero");
} else {
    println!("negative");
}
```

## Pattern matching

```mg
match shape {
    Circle(r) => PI * r * r,
    Rectangle(w, h) => w * h,
    Point => 0.0,
}
```

With guards:

```mg
match value {
    x if x > 0 => println!("positive: {x}"),
    0 => println!("zero"),
    x => println!("negative: {x}"),
}
```

## For loops

```mg
// Iterate over a collection
for item in items {
    println!("{item}");
}

// With index
for (i, item) in items.iter().enumerate() {
    println!("{i}: {item}");
}

// Range
for i in 0..10 {
    println!("{i}");
}

// Mutable iteration
for item in &mut items {
    *item *= 2;
}
```

## While loops

```mg
let mut x = 10;
while x > 0 {
    println!("{x}");
    x -= 1;
}
```

## Infinite loops

```mg
loop {
    let input = stdin().read_line()?;
    if input.trim() == "quit" {
        break;
    }
    println!("You said: {input}");
}
```

## Loop with value

Loops can produce values via `break`:

```mg
let result = loop {
    let data = try_fetch()?;
    if data.is_valid() {
        break data;
    }
};
```

## Early return

Use `return` for early returns:

```mg
fn validate(input: &str) -> Result<(), Error> {
    if input.is_empty() {
        return Err(Error::new("empty input"));
    }
    Ok(())
}
```

## The `?` operator (error propagation)

The postfix `?` operator works exactly like Rust — it propagates errors from
`Result` and `Option`:

```mg
f load_config() -> R[Config, Error] / io {
    v text = File.read("config.json")?   // propagates IoError
    v config = parse(&text)?              // propagates ParseError
    Ok(config)
}
```

> Don't confuse the prefix `?` (if/match keyword) with the postfix `?` (error
> propagation operator). They're syntactically unambiguous because the prefix
> form always starts a statement and the postfix form always follows an
> expression.

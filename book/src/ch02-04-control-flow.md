# Control Flow

Redox uses `?` for conditionals and pattern matching, `@` for iteration, and
`loop` for unbounded loops. There is no `if`, `else`, `match`, or `for` keyword.

## Conditionals with `?`

The `?` keyword replaces both `if` and `match`:

```rdx
// Simple conditional
? x > 0 {
    p"positive"
}

// If-else (: = else)
? x > 0 {
    p"positive"
} : {
    p"non-positive"
}

// If-else-if chain
? x > 0 {
    p"positive"
} : ? x == 0 {
    p"zero"
} : {
    p"negative"
}
```

The `:` token means "else". Read `} : {` as "otherwise".

## Pattern matching with `?`

When `?` is followed by a value and `{` contains arms with `=>`, it acts as
`match`:

```rdx
? shape {
    Circle(r) => PI * r * r,
    Rectangle(w, h) => w * h,
    Point => 0.0,
}
```

With guards:

```rdx
? value {
    x ? x > 0 => p"positive: {x}",
    0 => p"zero",
    x => p"negative: {x}",
}
```

## For loops with `@`

The `@` keyword replaces `for`:

```rdx
// Iterate over a collection
@ item : items {
    p"{item}"
}

// With index
@ (i, item) : items.iter().enumerate() {
    p"{i}: {item}"
}

// Range
@ i : 0..10 {
    p"{i}"
}

// Mutable iteration
@ item : &!items {
    *item *= 2
}
```

## While loops

`loop` with a condition:

```rdx
m x = 10
loop ? x > 0 {
    p"{x}"
    x -= 1
}
```

## Infinite loops

```rdx
loop {
    v input = stdin().read_line()?
    ? input.trim() == "quit" {
        break
    }
    p"You said: {input}"
}
```

## Loop with value

Loops can produce values via `break`:

```rdx
v result = loop {
    v data = try_fetch()?
    ? data.is_valid() {
        break data
    }
}
```

## Early return

Use `ret` instead of `return`:

```rdx
f validate(input: &s) -> R[(), Error] {
    ? input.is_empty() {
        ret Err(Error.new("empty input"))
    }
    Ok(())
}
```

## The `?` operator (error propagation)

The postfix `?` operator works exactly like Rust — it propagates errors from
`Result` and `Option`:

```rdx
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

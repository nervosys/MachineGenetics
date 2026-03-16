# Variables & Mutability

Redox uses `v` for immutable bindings and `m` for mutable bindings. There is no
`let` keyword.

## Immutable bindings

```rdx
v x = 42
v name = "Alice"
v items = [1, 2, 3]~    // Vec literal
```

Immutable bindings cannot be reassigned:

```rdx
v x = 10
x = 20    // ERROR: cannot assign to immutable binding
```

## Mutable bindings

```rdx
m count = 0
count += 1   // OK

m buffer = [u8]~.new()
buffer.push(0xFF)   // OK
```

## Type annotations

Type annotations follow the `:` syntax, same as Rust:

```rdx
v x: i32 = 42
v name: s = "Alice".to_string()
m scores: {s: i32} = {s: i32}.new()
```

Most types are inferred — annotations are optional when the type is unambiguous.

## Destructuring

```rdx
v (x, y) = (10, 20)
v Point { x, y } = origin
v [first, second, ..rest] = items
```

## Shadowing

Like Rust, Redox allows shadowing — redeclaring a variable in the same scope:

```rdx
v x = "42"
v x = x.parse_int()?    // shadows the string with an integer
```

## Constants

```rdx
+v MAX_SIZE: usize = 1024
+v PI: f64 = 3.14159265358979
```

Constants use `+v` (public) or `v` at module level, with mandatory type
annotations. They are evaluated at compile time.

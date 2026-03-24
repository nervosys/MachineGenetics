# Variables & Mutability

MechGen uses `let` for immutable bindings and `let mut` for mutable bindings,
identical to Rust.

## Immutable bindings

```mg
let x = 42;
let name = "Alice";
let items = vec![1, 2, 3];
```

Immutable bindings cannot be reassigned:

```mg
let x = 10;
x = 20;    // ERROR: cannot assign to immutable binding
```

## Mutable bindings

```mg
let mut count = 0;
count += 1;   // OK

let mut buffer = Vec::<u8>::new();
buffer.push(0xFF);   // OK
```

## Type annotations

Type annotations follow the `:` syntax, same as Rust:

```mg
let x: i32 = 42;
let name: String = "Alice".to_string();
let mut scores: HashMap<String, i32> = HashMap::new();
```

Most types are inferred — annotations are optional when the type is unambiguous.

## Destructuring

```mg
let (x, y) = (10, 20);
let Point { x, y } = origin;
let [first, second, ..rest] = items;
```

## Shadowing

Like Rust, MechGen allows shadowing — redeclaring a variable in the same scope:

```mg
let x = "42";
let x = x.parse_int()?;    // shadows the string with an integer
```

## Constants

```mg
pub const MAX_SIZE: usize = 1024;
pub const PI: f64 = 3.14159265358979;
```

Constants use `pub const` (public) or `const` at module level, with mandatory
type annotations. They are evaluated at compile time.

# Variables & Mutability

Redox uses `let` for immutable bindings and `let mut` for mutable bindings,
identical to Rust.

## Immutable bindings

```rdx
let x = 42;
let name = "Alice";
let items = vec![1, 2, 3];
```

Immutable bindings cannot be reassigned:

```rdx
let x = 10;
x = 20;    // ERROR: cannot assign to immutable binding
```

## Mutable bindings

```rdx
let mut count = 0;
count += 1;   // OK

let mut buffer = Vec::<u8>::new();
buffer.push(0xFF);   // OK
```

## Type annotations

Type annotations follow the `:` syntax, same as Rust:

```rdx
let x: i32 = 42;
let name: String = "Alice".to_string();
let mut scores: HashMap<String, i32> = HashMap::new();
```

Most types are inferred — annotations are optional when the type is unambiguous.

## Destructuring

```rdx
let (x, y) = (10, 20);
let Point { x, y } = origin;
let [first, second, ..rest] = items;
```

## Shadowing

Like Rust, Redox allows shadowing — redeclaring a variable in the same scope:

```rdx
let x = "42";
let x = x.parse_int()?;    // shadows the string with an integer
```

## Constants

```rdx
pub const MAX_SIZE: usize = 1024;
pub const PI: f64 = 3.14159265358979;
```

Constants use `pub const` (public) or `const` at module level, with mandatory
type annotations. They are evaluated at compile time.

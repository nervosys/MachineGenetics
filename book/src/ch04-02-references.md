# References & Borrowing

Redox keeps Rust's borrowing rules and syntax unchanged.

## Shared references: `&T`

```rdx
let name = "Alice".to_string();
let r: &str = &name;    // immutable borrow
println!("{r}");         // OK: reading through shared ref
```

Multiple shared references are allowed:

```rdx
let r1 = &name;
let r2 = &name;    // OK: multiple &T at once
```

## Exclusive references: `&mut T`

```rdx
let mut items = vec![1, 2, 3];
let r: &mut Vec<i32> = &mut items;    // exclusive borrow
r.push(4);                            // OK: exclusive access
```

Only one `&mut T` at a time, and no `&T` while `&mut T` exists:

```rdx
let mut x = 42;
let r = &mut x;
// let r2 = &x;     // ERROR: cannot borrow while exclusively borrowed
*r += 1;
```

## Ownership and move

Values are moved by default (same as Rust):

```rdx
let a = "hello".to_string();
let b = a;         // a is moved to b
// println!("{a}"); // ERROR: a has been moved
```

Use `.clone()` for explicit copies:

```rdx
let a = "hello".to_string();
let b = a.clone();    // deep copy
println!("{a}");      // OK: a still valid
```

## No lifetimes in syntax

In Rust:

```rust
struct Important<'a> {
    content: &'a str,
}

impl<'a> Important<'a> {
    fn new(content: &'a str) -> Self {
        Important { content }
    }
}
```

In Redox:

```rdx
struct Important {
    content: &str,
}

impl Important {
    pub fn new(content: &str) -> Self {
        Important { content }
    }
}
```

The SKB tracks the relationship between the reference and its referent. The
compiler verifies it when safety mode is `warnings` or `full`.

## Dereferencing

```rdx
let x = 42;
let r = &x;
let val = *r;    // dereference: 42

let mut y = 10;
let mr = &mut y;
*mr = 20;        // write through exclusive ref
```

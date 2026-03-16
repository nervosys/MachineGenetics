# References & Borrowing

Redox keeps Rust's borrowing rules but simplifies the syntax.

## Shared references: `&T`

```rdx
v name = "Alice".to_string()
v r: &s = &name    // immutable borrow
p"{r}"              // OK: reading through shared ref
```

Multiple shared references are allowed:

```rdx
v r1 = &name
v r2 = &name    // OK: multiple &T at once
```

## Exclusive references: `&!T`

Redox uses `&!` instead of `&mut`:

```rdx
m items = [1, 2, 3]~
v r: &![i32]~ = &!items    // exclusive borrow
r.push(4)                  // OK: exclusive access
```

Only one `&!T` at a time, and no `&T` while `&!T` exists:

```rdx
m x = 42
v r = &!x
// v r2 = &x     // ERROR: cannot borrow while exclusively borrowed
*r += 1
```

## Ownership and move

Values are moved by default (same as Rust):

```rdx
v a = "hello".to_string()
v b = a         // a is moved to b
// p"{a}"       // ERROR: a has been moved
```

Use `.clone()` for explicit copies:

```rdx
v a = "hello".to_string()
v b = a.clone()    // deep copy
p"{a}"             // OK: a still valid
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
S Important {
    content: &s,
}

I Important {
    +f new(content: &s) -> Self {
        Important @{ content }
    }
}
```

The SKB tracks the relationship between the reference and its referent. The
compiler verifies it when safety mode is `warnings` or `full`.

## Dereferencing

```rdx
v x = 42
v r = &x
v val = *r    // dereference: 42

m y = 10
v mr = &!y
*mr = 20      // write through exclusive ref
```

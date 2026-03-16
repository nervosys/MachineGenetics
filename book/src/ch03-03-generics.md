# Generics

Redox uses `[T]` syntax for generic parameters (square brackets, not angle
brackets). This eliminates Rust's turbofish (`::<>`) ambiguity entirely.

## Generic functions

```rdx
+f max[T: Ord](a: T, b: T) -> T {
    ? a >= b { a } : { b }
}

+f swap[T](a: &!T, b: &!T) {
    v temp = *a
    *a = *b
    *b = temp
}
```

## Generic structs

```rdx
+S Stack[T] {
    items: [T]~,
}

I Stack[T] {
    +f new() -> Self {
        Stack @{ items: [T]~.new() }
    }

    +f push(&!self, item: T) {
        self.items.push(item)
    }

    +f pop(&!self) -> ?T {
        self.items.pop()
    }

    +f is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
```

## Trait bounds

Inline bounds:

```rdx
+f print_all[T: Display](items: &[T]) / io {
    @ item : items {
        p"{item}"
    }
}
```

Multiple bounds with `+`:

```rdx
+f sort_and_print[T: Ord + Display](items: &![T]~) / io {
    items.sort()
    @ item : items {
        p"{item}"
    }
}
```

## Where clauses with `~>`

For complex bounds, use `~>`:

```rdx
+f merge[K, V](a: {K: V}, b: {K: V}) -> {K: V}
    ~> K: Eq + Hash, V: Clone
{
    m result = a.clone()
    @ (k, v) : b {
        result.insert(k.clone(), v.clone())
    }
    result
}
```

## No turbofish

In Rust, disambiguating generics in expressions requires `::<>`:

```rust
let x = Vec::<i32>::new();   // Rust turbofish
```

In Redox, square brackets are unambiguous:

```rdx
v x = [i32]~.new()    // No turbofish needed
```

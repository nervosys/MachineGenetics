# Generics

MechGen uses `<T>` syntax for generic parameters, identical to Rust.

## Generic functions

```mg
pub fn max<T: Ord>(a: T, b: T) -> T {
    if a >= b { a } else { b }
}

pub fn swap<T>(a: &mut T, b: &mut T) {
    let temp = *a;
    *a = *b;
    *b = temp;
}
```

## Generic structs

```mg
pub struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    pub fn new() -> Self {
        Stack { items: Vec::new() }
    }

    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    pub fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
```

## Trait bounds

Inline bounds:

```mg
pub fn print_all<T: Display>(items: &[T]) / io {
    for item in items {
        println!("{item}");
    }
}
```

Multiple bounds with `+`:

```mg
pub fn sort_and_print<T: Ord + Display>(items: &mut Vec<T>) / io {
    items.sort();
    for item in items {
        println!("{item}");
    }
}
```

## Where clauses

For complex bounds, use `where`:

```mg
pub fn merge<K, V>(a: HashMap<K, V>, b: HashMap<K, V>) -> HashMap<K, V>
    where K: Eq + Hash, V: Clone
{
    let mut result = a.clone();
    for (k, v) in b {
        result.insert(k.clone(), v.clone());
    }
    result
}
```

## Turbofish

When the compiler can't infer the type, use turbofish syntax just like Rust:

```rust
let x = Vec::<i32>::new();
```

```mg
let x = Vec::<i32>::new();   // Same syntax in MechGen
```

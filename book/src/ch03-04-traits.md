# Traits

Traits define shared behavior, just like Rust. Redox uses `T` for trait
declarations and `I Trait ~ Type` for implementations.

## Defining traits

```rdx
+T Summary {
    +f summarize(&self) -> s;
}
```

### With default methods

```rdx
+T Summary {
    +f summarize(&self) -> s;

    +f preview(&self) -> s {
        v summary = self.summarize()
        ? summary.len() > 50 {
            f"{&summary[..50]}..."
        } : {
            summary
        }
    }
}
```

### With associated types

```rdx
+T Iterator {
    type Item;
    +f next(&!self) -> ?Self.Item;
}
```

## Implementing traits

```rdx
S Article {
    title: s,
    body: s,
}

I Summary ~ Article {
    +f summarize(&self) -> s {
        f"{self.title}: {&self.body[..100]}..."
    }
}
```

## Using trait objects

```rdx
// Dynamic dispatch with ^
+f print_summary(item: &^Summary) / io {
    p"{item.summarize()}"
}

// Or as a trait bound (static dispatch)
+f print_summary[T: Summary](item: &T) / io {
    p"{item.summarize()}"
}
```

`^Summary` is `dyn Summary` — a trait object behind a Box.

## Common standard traits

| Trait               | Purpose                | Derive                |
| ------------------- | ---------------------- | --------------------- |
| `Clone`             | Deep copy              | `@d(Clone)`           |
| `Copy`              | Bitwise copy           | `@d(Copy)`            |
| `Debug`             | Debug formatting       | `@d(Debug)`           |
| `Display`           | User-facing formatting | Manual                |
| `Eq`, `PartialEq`   | Equality               | `@d(Eq, PartialEq)`   |
| `Ord`, `PartialOrd` | Ordering               | `@d(Ord, PartialOrd)` |
| `Hash`              | Hashing                | `@d(Hash)`            |
| `Default`           | Default values         | `@d(Default)`         |
| `Serialize`         | JSON serialization     | `@d(Serialize)`       |
| `Deserialize`       | JSON deserialization   | `@d(Deserialize)`     |

## Deriving traits

```rdx
@d(Clone, Debug, PartialEq)
+S Config {
    name: s,
    value: i32,
}
```

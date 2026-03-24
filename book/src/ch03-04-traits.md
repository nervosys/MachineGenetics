# Traits

Traits define shared behavior, just like Rust. Redox uses `trait` for
declarations and `impl Trait for Type` for implementations.

## Defining traits

```rdx
pub trait Summary {
    pub fn summarize(&self) -> String;
}
```

### With default methods

```rdx
pub trait Summary {
    pub fn summarize(&self) -> String;

    pub fn preview(&self) -> String {
        let summary = self.summarize();
        if summary.len() > 50 {
            format!("{}...", &summary[..50])
        } else {
            summary
        }
    }
}
```

### With associated types

```rdx
pub trait Iterator {
    type Item;
    pub fn next(&mut self) -> Option<Self::Item>;
}
```

## Implementing traits

```rdx
struct Article {
    title: String,
    body: String,
}

impl Summary for Article {
    pub fn summarize(&self) -> String {
        format!("{}: {}...", self.title, &self.body[..100])
    }
}
```

## Using trait objects

```rdx
// Dynamic dispatch with Box<dyn>
pub fn print_summary(item: &Box<dyn Summary>) / io {
    println!("{}", item.summarize());
}

// Or as a trait bound (static dispatch)
pub fn print_summary<T: Summary>(item: &T) / io {
    println!("{}", item.summarize());
}
```

`Box<dyn Summary>` is a trait object — dynamic dispatch behind a Box.

## Common standard traits

| Trait               | Purpose                | Derive                       |
| ------------------- | ---------------------- | ---------------------------- |
| `Clone`             | Deep copy              | `#[derive(Clone)]`           |
| `Copy`              | Bitwise copy           | `#[derive(Copy)]`            |
| `Debug`             | Debug formatting       | `#[derive(Debug)]`           |
| `Display`           | User-facing formatting | Manual                       |
| `Eq`, `PartialEq`   | Equality               | `#[derive(Eq, PartialEq)]`   |
| `Ord`, `PartialOrd` | Ordering               | `#[derive(Ord, PartialOrd)]` |
| `Hash`              | Hashing                | `#[derive(Hash)]`            |
| `Default`           | Default values         | `#[derive(Default)]`         |
| `Serialize`         | JSON serialization     | `#[derive(Serialize)]`       |
| `Deserialize`       | JSON deserialization   | `#[derive(Deserialize)]`     |

## Deriving traits

```rdx
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    name: String,
    value: i32,
}
```

# Functions

Functions are at the heart of MechGen. They are declared with `fn` (private) or
`pub fn` (public), optionally annotated with effects.

## Basic functions

```mg
fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}
```

The last expression is the return value (no semicolon needed). Use `return` for
early returns:

```mg
fn find(items: &[i32], target: i32) -> Option<usize> {
    for (i, val) in items.iter().enumerate() {
        if val == target {
            return Some(i);
        }
    }
    None
}
```

## Effect annotations

Functions that perform side effects declare them after `/`:

```mg
pub fn read_config(path: &str) -> Result<Config, IoError> / io {
    let data = File::read(path)?;
    parse(&data)
}

pub fn fetch_data(url: &str) -> Result<Response, NetError> / io, net {
    Request::get(url).send()
}

// Pure function — no effect annotation
pub fn double(x: i32) -> i32 {
    x * 2
}
```

Multiple effects are comma-separated: `/ io, net, rng`.

## Generic functions

```mg
pub fn first<T>(items: &[T]) -> Option<&T> {
    if items.is_empty() {
        None
    } else {
        Some(&items[0])
    }
}

pub fn map<T, U>(items: &[T], f: fn(&T) -> U) -> Vec<U> {
    let mut result = Vec::new();
    for item in items {
        result.push(f(item));
    }
    result
}
```

## Where clauses

```mg
pub fn print_all<T>(items: &[T]) / io where T: Display {
    for item in items {
        println!("{item}");
    }
}
```

## Closures

```mg
let double = |x: i32| x * 2;
let add = |a, b| a + b;

let items = vec![1, 2, 3];
let doubled = items.iter().map(|x| x * 2).collect();
```

## Async functions

Use `async fn` to make a function async:

```mg
pub async fn fetch(url: &str) -> Result<String, NetError> / net {
    let resp = Request::get(url).send().await?;
    resp.text().await
}
```

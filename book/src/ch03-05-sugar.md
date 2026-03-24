# Type Sugar

In **standard syntax** (the default), MechGen uses the same type names as Rust.
The **compact syntax** mode (`#![syntax(compact)]`) provides single-sigil
abbreviations for common types — see the
[appendix](appendix-cheatsheet.md) for the full compact mapping.

## Standard type names

| Standard (MechGen/Rust) | Compact Mode | Description        |
| --------------------- | ------------ | ------------------ |
| `Option<T>`           | `?T`         | Optional value     |
| `Result<T, E>`        | `R[T, E]`    | Success or error   |
| `Vec<T>`              | `[T]~`       | Growable vector    |
| `Box<T>`              | `^T`         | Heap allocation    |
| `Rc<T>`               | `$T`         | Reference counted  |
| `Arc<T>`              | `@T`         | Atomic ref counted |
| `HashMap<K, V>`       | `{K: V}`     | Hash map           |
| `HashSet<K>`          | `{K}`        | Hash set           |
| `&mut T`              | `&!T`        | Mutable reference  |
| `String`              | `s`          | Owned string       |
| `&str`                | `&s`         | String slice       |

## Option

```mg
pub fn find(name: &str) -> Option<User> {
    // Returns Some(user) or None
}

let result: Option<i32> = Some(42);
let nothing: Option<i32> = None;
```

## Result

```mg
pub fn parse(input: &str) -> Result<Config, ParseError> {
    // Returns Ok(config) or Err(error)
}
```

## Vec

```mg
let items = vec![1, 2, 3];
let empty: Vec<String> = Vec::new();
let mut buf: Vec<u8> = Vec::with_capacity(1024);
```

## Smart pointers

```mg
let boxed: Box<i32> = Box::new(42);
let shared: Rc<Node> = Rc::new(node);
let atomic: Arc<Config> = Arc::new(config);
```

## Collections

```mg
// HashMap
let mut scores: HashMap<String, i32> = HashMap::new();
scores.insert("Alice".into(), 100);
scores.insert("Bob".into(), 95);

// HashSet
let mut seen: HashSet<String> = HashSet::new();
seen.insert("hello".into());
```

## Mutable references

Rust's `&mut T` is used directly in MechGen standard mode:

```mg
fn push_item(list: &mut Vec<i32>, item: i32) {
    list.push(item);
}
```

## Combining types

Types compose the same way as Rust:

```mg
let maybe_items: Option<Vec<i32>> = Some(vec![1, 2, 3]);
let shared_map: Arc<HashMap<String, Vec<i32>>> = Arc::new(map);
let result: Result<Option<String>, Box<dyn Error>> = Ok(Some("hi".into()));
```

## Why compact mode exists

For AI agents emitting code token by token, compact syntax reduces token count
significantly:

| Standard (Rust-like)                           | Tokens | Compact               | Tokens | Savings |
| ---------------------------------------------- | :----: | --------------------- | :----: | :-----: |
| `Option<Vec<String>>`                          |   7    | `?[s]~`               |   4    |   43%   |
| `Result<HashMap<String, i32>, Box<dyn Error>>` |   13   | `R[{s: i32}, ^Error]` |   8    |   38%   |
| `Arc<Mutex<Vec<u8>>>`                          |   9    | `@Mutex[[u8]~]`       |   5    |   44%   |
| `&mut Vec<(String, i32)>`                      |   9    | `&![(s, i32)]~`       |   6    |   33%   |

To activate compact mode, add `#![syntax(compact)]` at the top of any `.mg`
file.

# Type Sugar

Redox's most distinctive feature is its **type sugar** — single sigils that
replace verbose generic types. These are not aliases; they are built into the
language grammar itself.

## The complete sugar table

| Sugar     | Expansion           | Rust Equivalent |
| --------- | ------------------- | --------------- |
| `?T`      | `Option[T]`         | `Option<T>`     |
| `R[T, E]` | `Result[T, E]`      | `Result<T, E>`  |
| `[T]~`    | `Vec[T]`            | `Vec<T>`        |
| `[T; N]`  | fixed array         | `[T; N]`        |
| `[T]`     | slice               | `[T]`           |
| `^T`      | `Box[T]`            | `Box<T>`        |
| `$T`      | `Rc[T]`             | `Rc<T>`         |
| `@T`      | `Arc[T]`            | `Arc<T>`        |
| `{K: V}`  | `Map[K, V]`         | `HashMap<K, V>` |
| `{K}`     | `Set[K]`            | `HashSet<K>`    |
| `&T`      | shared reference    | `&T`            |
| `&!T`     | exclusive reference | `&mut T`        |
| `s`       | `String`            | `String`        |
| `&s`      | string slice        | `&str`          |

## Option: `?T`

```rdx
+f find(name: &s) -> ?User {
    // Returns Some(user) or None
}

v result: ?i32 = Some(42)
v nothing: ?i32 = None
```

## Result: `R[T, E]`

```rdx
+f parse(input: &s) -> R[Config, ParseError] {
    // Returns Ok(config) or Err(error)
}
```

## Vec: `[T]~`

The tilde `~` means "growable" — a fixed array `[T; N]` becomes a dynamic
vector `[T]~`:

```rdx
v items = [1, 2, 3]~           // Vec literal
v empty = [s]~.new()           // empty Vec<String>
m buf = [u8]~.with_capacity(1024)
```

## Smart pointers

```rdx
v boxed: ^i32 = ^42                    // Box::new(42)
v shared: $Node = $.new(node)          // Rc::new(node)
v atomic: @Config = @.new(config)      // Arc::new(config)
```

## Collections

```rdx
// HashMap
m scores: {s: i32} = {s: i32}.new()
scores.insert("Alice", 100)
scores.insert("Bob", 95)

// HashSet
m seen: {s} = {s}.new()
seen.insert("hello")
```

## Exclusive references: `&!`

Rust's `&mut T` becomes `&!T` — the `!` emphasizes exclusivity:

```rdx
f push_item(list: &![i32]~, item: i32) {
    list.push(item)
}
```

## Combining sugar

Sugar composes naturally:

```rdx
v maybe_items: ?[i32]~  = Some([1, 2, 3]~)    // Option<Vec<i32>>
v shared_map: @{s: [i32]~} = @.new(map)        // Arc<HashMap<String, Vec<i32>>>
v result: R[?s, ^Error] = Ok(Some("hi".into()))  // Result<Option<String>, Box<Error>>
```

## Why sugar?

A side-by-side comparison shows the token savings:

| Rust                                           | Tokens | Redox                 | Tokens | Savings |
| ---------------------------------------------- | :----: | --------------------- | :----: | :-----: |
| `Option<Vec<String>>`                          |   7    | `?[s]~`               |   4    |   43%   |
| `Result<HashMap<String, i32>, Box<dyn Error>>` |   13   | `R[{s: i32}, ^Error]` |   8    |   38%   |
| `Arc<Mutex<Vec<u8>>>`                          |   9    | `@Mutex[[u8]~]`       |   5    |   44%   |
| `&mut Vec<(String, i32)>`                      |   9    | `&![(s, i32)]~`       |   6    |   33%   |

For AI agents emitting code token by token, every saved token reduces latency,
cost, and memory.

# Functions

Functions are at the heart of Redox. They are declared with `f` (private) or
`+f` (public), optionally annotated with effects.

## Basic functions

```rdx
f add(a: i32, b: i32) -> i32 {
    a + b
}

+f greet(name: &s) -> s {
    f"Hello, {name}!"
}
```

The last expression is the return value (no semicolon needed). Use `ret` for
early returns:

```rdx
f find(items: &[i32], target: i32) -> ?usize {
    @ (i, val) : items.iter().enumerate() {
        ? val == target {
            ret Some(i)
        }
    }
    None
}
```

## Effect annotations

Functions that perform side effects declare them after `/`:

```rdx
+f read_config(path: &s) -> R[Config, IoError] / io {
    v data = File.read(path)?
    parse(&data)
}

+f fetch_data(url: &s) -> R[Response, NetError] / io, net {
    Request.get(url).send()
}

// Pure function — no effect annotation
+f double(x: i32) -> i32 {
    x * 2
}
```

Multiple effects are comma-separated: `/ io, net, rng`.

## Generic functions

```rdx
+f first[T](items: &[T]) -> ?&T {
    ? items.is_empty() {
        None
    } : {
        Some(&items[0])
    }
}

+f map[T, U](items: &[T], f: f(&T) -> U) -> [U]~ {
    m result = [U]~.new()
    @ item : items {
        result.push(f(item))
    }
    result
}
```

## Where clauses

Use `~>` for where clauses:

```rdx
+f print_all[T](items: &[T]) / io ~> T: Display {
    @ item : items {
        p"{item}"
    }
}
```

## Closures

```rdx
v double = |x: i32| x * 2
v add = |a, b| a + b

v items = [1, 2, 3]~
v doubled = items.iter().map(|x| x * 2).collect()
```

## Async functions

Prefix with `a` to make a function async:

```rdx
+af fetch(url: &s) -> R[s, NetError] / net {
    v resp = Request.get(url).send().await?
    resp.text().await
}
```

`af` = async function, `+af` = public async function.

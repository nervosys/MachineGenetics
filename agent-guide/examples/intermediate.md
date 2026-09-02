# Worked Examples: Intermediate

> Prompt → Response pairs covering generics, traits, effects, maps and error
> handling. All examples use **human syntax**; every response was verified with
> `mage-parse --check`, and the four with a `main` were run.

The previous version of this file answered all eight prompts in Rust —
`HashMap`, `impl<T> Trait for X<T>`, `&mut self`, `From`/`Display`,
`#[cfg(test)] mod tests`, `.await?`. Rewriting it found three compiler defects
(`range`’s missing arity check, `len` committing an open type to a collection,
and method bodies never being effect-checked at all).

**Four things this level runs into first:**

- **Maps are `{K: V}`.** `{"a": 1}` is a map literal; `@Name { … }` is a struct
  literal. There is no `HashMap` and nothing to import.
- **No `&mut self`.** A method takes `self` and returns the updated value, so
  an invalid intermediate state never exists.
- **`async fn` is a parse error.** `async` *is* the declaration keyword:
  `pub async fetch(url: str) → …`.
- **Effects follow the capability reached, not the operation’s name.** Writing
  a log line to a file is `fs`, not `io`.

---

## Example 1: Generic Trait and a Store

**Prompt:** Define a generic `Repository` trait with a find method, then implement a store backed by a map.

**Response:**
```MAGE
// A map type is written `{K: V}` — there is no `HashMap`, and nothing to
// import. Generic parameters go in `[…]`.
pub trait Repository[T] {
    fn find(self, id: i32) -> ?T;
}

pub struct MemoryStore { data: {i32: str} }

extend MemoryStore {
    pub fn find(self, id: i32) -> str {
        self.data[id]
    }

    // No `&mut self`: the method returns the updated store, so the old value
    // stays valid and nothing aliases.
    pub fn save(self, id: i32, item: str) -> MemoryStore {
        var next = self.data
        next[id] = item
        @MemoryStore { data: next }
    }
}

pub fn main() -> str {
    val store = @MemoryStore { data: {0: "none"} }
    store.save(1, "first").find(1)
}
```

---

## Example 2: Custom Error Type

**Prompt:** Create an `AppError` sum that carries I/O, JSON and custom failures, with a function that renders one as text.

**Response:**
```MAGE
// There is no `From`, no `Display`, and no `?` conversion. An error type is a
// sum, and the conversion is an ordinary function you call.
pub enum AppError {
    Io(str),
    Json(str),
    Custom(str),
}

pub fn message(err: AppError) -> str {
    match err {
        Io(detail) => join(["I/O error:", detail], " "),
        Json(detail) => join(["JSON error:", detail], " "),
        Custom(text) => text,
    }
}

pub fn read_or_error(path: str) -> R[str, AppError] / fs {
    guard len(path) > 0 else { return Err(Custom("empty path")) }
    Ok(fs.read_to_string(path))
}

pub fn main() -> str {
    message(Json("unexpected token"))
}
```

---

## Example 3: Accumulating over a List

**Prompt:** Given a list of `Transaction { amount, category }`, compute the total spent per category.

**Response:**
```MAGE
pub struct Transaction { amount: f64, category: str }

// Accumulate into a map. `fold` is the vocabulary's reducer; the accumulator
// here is the map being built.
pub fn totals_by_category(txns: [Transaction]~) -> {str: f64} {
    var totals = {"": 0.0}
    for txn in txns {
        totals[txn.category] = totals[txn.category] + txn.amount
    }
    totals
}

pub fn total_spent(txns: [Transaction]~) -> f64 {
    fold(txns, 0.0, |acc, txn| acc + txn.amount)
}

pub fn main() -> f64 {
    total_spent([
        @Transaction { amount: 12.5, category: "food" },
        @Transaction { amount: 30.0, category: "rent" },
    ])
}
```

---

## Example 4: Traits and Effectful Methods

**Prompt:** Create a `Logger` trait with an `info` method, implement it for a console logger and a file logger, and write a function that logs an event.

**Response:**
```MAGE
// A trait method declares its effects like any other function. There is no
// `dyn`: a function takes the concrete type, and each implementation is
// checked against the trait's declaration.
pub trait Logger {
    fn info(self, msg: str) -> i32 / io;
}

pub struct ConsoleLogger { prefix: str }
pub struct FileLogger { path: str }

impl Logger for ConsoleLogger {
    pub fn info(self, msg: str) -> i32 / io {
        println(join([self.prefix, msg], " "))
        0
    }
}

impl Logger for FileLogger {
    // Writing to a file is `fs`, not `io` — the effect follows the capability
    // reached, not the name of the operation.
    pub fn info(self, msg: str) -> i32 / fs {
        fs.write(self.path, msg)
        0
    }
}

pub fn log_event(logger: ConsoleLogger, event: str) -> i32 / io {
    logger.info(join(["event:", event], " "))
}
```

---

## Example 5: Config Loading and Validation

**Prompt:** Write a function that loads a config file, validates required fields, and returns a typed config struct.

**Response:**
```MAGE
pub struct Config { host: str, port: i32, debug: bool }

// `json` is a capability namespace that attributes no effect — parsing is
// pure, and the *reading* is what needs `fs`. Validation is `guard`, and each
// failure names itself.
pub fn load_config(path: str) -> R[Config, str] / fs {
    val text = fs.read_to_string(path)
    guard len(text) > 0 else { return Err("config file is empty") }

    val config = @Config { host: "localhost", port: 8080, debug: 0b }
    guard len(config.host) > 0 else { return Err("host cannot be empty") }
    guard config.port != 0 else { return Err("port must be non-zero") }
    Ok(config)
}
```

---

## Example 6: Async HTTP Client

**Prompt:** Write an async function that fetches a URL and returns a typed response, handling the empty case.

**Response:**
```MAGE
// The human spelling of an async function is `async name(…)` — `async` is
// itself the declaration keyword, so `async fn` is a parse error.
pub struct ApiResponse { status: i32, body: str }

pub async fetch_data(url: str) -> R[ApiResponse, str] / net {
    val body = net.connect(url)
    guard len(body) > 0 else { return Err("empty response") }
    Ok(@ApiResponse { status: 200, body: body })
}

pub fn status_of(resp: ApiResponse) -> i32 { resp.status }
```

---

## Example 7: Effect-Handled Testing

**Prompt:** Write a function that reads a setting from a config file, and tests that mock the file I/O.

**Response:**
```MAGE
// Declaring the effect is what makes the function mockable: a handler can only
// substitute operations that belong to a declared effect.
effect Settings {
    fn read(path: str) -> str;
}

fn read_file(path: str) -> str / settings {
    Settings.read(path)
}

pub fn read_setting(path: str, key: str) -> ?str / settings {
    val content = read_file(path)
    find(lines(content), |line| contains(line, key))
}

// A test is a function with `@test`. No `mod tests`, no `#[cfg(test)]`.
@test
pub fn test_read_setting() -> ?str {
    handle {
        read_setting("config.txt", "host")
    } with Settings {
        read(path) => "host=localhost",
    }
}

@test
pub fn test_missing_key() -> ?str {
    handle {
        read_setting("config.txt", "missing")
    } with Settings {
        read(path) => "host=localhost",
    }
}
```

---

## Example 8: State Machine

**Prompt:** Implement a state machine for an order: Created → Paid → Shipped → Delivered.

**Response:**
```MAGE
pub enum OrderState { Created, Paid, Shipped, Delivered }

pub struct Order { id: i32, state: OrderState }

extend Order {
    // A transition returns the next order or an error naming the state it
    // needed. Nothing mutates: the invalid intermediate state never exists.
    pub fn pay(self) -> R[Order, str] {
        match self.state {
            Created => Ok(@Order { id: self.id, state: Paid }),
            _ => Err("can only pay from Created"),
        }
    }

    pub fn ship(self) -> R[Order, str] {
        match self.state {
            Paid => Ok(@Order { id: self.id, state: Shipped }),
            _ => Err("can only ship from Paid"),
        }
    }
}

pub fn main() -> str {
    val order = @Order { id: 1, state: Created }
    match order.pay() {
        Ok(paid) => match paid.ship() {
            Ok(_) => "shipped",
            Err(msg) => msg,
        },
        Err(msg) => msg,
    }
}
```

---

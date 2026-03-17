# Worked Examples: Intermediate

> Prompt → Response pairs with generics, traits, effects, and error handling.

---

## Example 1: Generic Repository Trait

**Prompt:** Define a generic `Repository` trait with find, save, and delete methods, then implement it for an in-memory store.

**Response:**
```redox
u std.col.HashMap

+T Repository[T] {
    f find(&self, id: u64) -> ?&T
    f save(&!self, id: u64, item: T) -> R[(), Error] / io
    f delete(&!self, id: u64) -> R[(), Error] / io
}

+S InMemoryStore[T] {
    data: {u64: T},
}

I[T] ~ InMemoryStore[T] {
    +f new() -> Self {
        Self @{ data: {u64: T}.new() }
    }
}

I[T] Repository[T] ~ InMemoryStore[T] {
    f find(&self, id: u64) -> ?&T {
        self.data.get(&id)
    }

    f save(&!self, id: u64, item: T) -> R[(), Error] / io {
        self.data.insert(id, item)
        Ok(())
    }

    f delete(&!self, id: u64) -> R[(), Error] / io {
        self.data.remove(&id)
        Ok(())
    }
}
```

---

## Example 2: Custom Error Type with Conversions

**Prompt:** Create an `AppError` enum that wraps io::Error and serde_json::Error, with Display and From implementations.

**Response:**
```redox
u std.fmt
u std.io

@d(Debug)
+E AppError {
    Io(io.Error),
    Json(serde_json.Error),
    Custom(s),
}

I fmt.Display ~ AppError {
    f fmt(&self, fmtr: &!fmt.Formatter) -> fmt.Result {
        ? self {
            AppError.Io(e) => fmtr.write_str(&f"I/O error: {e}"),
            AppError.Json(e) => fmtr.write_str(&f"JSON error: {e}"),
            AppError.Custom(msg) => fmtr.write_str(msg),
        }
    }
}

I From[io.Error] ~ AppError {
    f from(e: io.Error) -> Self { AppError.Io(e) }
}

I From[serde_json.Error] ~ AppError {
    f from(e: serde_json.Error) -> Self { AppError.Json(e) }
}
```

---

## Example 3: Iterator Adapter Chain

**Prompt:** Given a list of `Transaction { amount: f64, category: String }`, compute the total spent per category.

**Response:**
```redox
@d(Debug, Clone)
+S Transaction {
    +amount: f64,
    +category: s,
}

u std.col.HashMap

+f totals_by_category(txns: &[Transaction]~) -> {s: f64} {
    m map = {s: f64}.new()
    @ txn ~ txns {
        v entry = map.entry(txn.category.clone()).or_insert(0.0)
        *entry += txn.amount
    }
    map
}
```

---

## Example 4: Trait Objects and Dynamic Dispatch

**Prompt:** Create a `Logger` trait with an `info` method, implement it for `ConsoleLogger` and `FileLogger`, and write a function that accepts any logger.

**Response:**
```redox
u std.fs

+T Logger {
    f info(&self, msg: &s) / io
}

+S ConsoleLogger {}

I Logger ~ ConsoleLogger {
    f info(&self, msg: &s) / io {
        p"[INFO] {msg}"
    }
}

+S FileLogger {
    path: s,
}

I Logger ~ FileLogger {
    f info(&self, msg: &s) / io {
        // Append to log file
        v content = f"[INFO] {msg}\n"
        fs.append(&self.path, &content).unwrap_or(())
    }
}

+f log_event(logger: &dyn Logger, event: &s) / io {
    logger.info(&f"Event occurred: {event}")
}
```

---

## Example 5: JSON Config Loading

**Prompt:** Write a function that loads a JSON config file, validates required fields, and returns a typed config struct.

**Response:**
```redox
u std.fs
u std.json

@d(Debug, Clone)]
+S Config {
    +host: s,
    +port: u16,
    +debug: bool,
}

+f load_config(path: &s) -> R[Config, AppError] / io {
    v text = fs.read_to_string(path)?
    v config = json.parse[Config](&text)?

    ? config.host.is_empty() {
        ret Err(AppError.Custom(s.from("host cannot be empty")))
    }
    ? config.port == 0 {
        ret Err(AppError.Custom(s.from("port must be non-zero")))
    }

    Ok(config)
}
```

---

## Example 6: Async HTTP Client

**Prompt:** Write an async function that fetches JSON from a URL, deserializes it into a struct, and handles errors.

**Response:**
```redox
u std.json

@d(Debug, Clone)]
+S ApiResponse {
    +status: s,
    +data: [s]~,
}

+af fetch_data(url: &s) -> R[ApiResponse, Error] / net {
    v resp = http.get(url).await?
    ? resp.status_code() != 200 {
        ret Err(Error.new(f"HTTP {resp.status_code()}"))
    }
    v body = resp.text().await?
    v parsed = json.parse[ApiResponse](&body)?
    Ok(parsed)
}
```

---

## Example 7: Effect-Handled Testing

**Prompt:** Write a function that reads a config file and a test that mocks the file I/O.

**Response:**
```redox
u std.fs

+f read_setting(path: &s, key: &s) -> R[?s, io.Error] / io {
    v content = fs.read_to_string(path)?
    @ line ~ content.lines() {
        ? line.starts_with(key) {
            v value = line.splitn(2, '=').nth(1)
                .map(|v| v.trim().to_string())
            ret Ok(value)
        }
    }
    Ok(None)
}

@cfg(test)
M tests {
    u super.*

    @test
    f test_read_setting() {
        handle io {
            read_to_string(_) => "host=localhost\nport=8080",
        } {
            v result = read_setting("config.txt", "host")
            assert_eq!(result.unwrap(), Some(s.from("localhost")))
        }
    }

    @test
    f test_missing_key() {
        handle io {
            read_to_string(_) => "host=localhost",
        } {
            v result = read_setting("config.txt", "missing")
            assert_eq!(result.unwrap(), None)
        }
    }
}
```

---

## Example 8: State Machine Pattern

**Prompt:** Implement a simple state machine for an order: Created → Paid → Shipped → Delivered.

**Response:**
```redox
@d(Debug, Clone)]
+E OrderState {
    Created,
    Paid,
    Shipped,
    Delivered,
}

@d(Debug)]
+S Order {
    +id: u64,
    +state: OrderState,
}

I ~ Order {
    +f new(id: u64) -> Self {
        Self @{ id, state: OrderState.Created }
    }

    +f pay(&!self) -> R[(), s] {
        ? self.state {
            OrderState.Created => {
                self.state = OrderState.Paid
                Ok(())
            },
            _ => Err(s.from("Can only pay from Created state")),
        }
    }

    +f ship(&!self) -> R[(), s] {
        ? self.state {
            OrderState.Paid => {
                self.state = OrderState.Shipped
                Ok(())
            },
            _ => Err(s.from("Can only ship from Paid state")),
        }
    }

    +f deliver(&!self) -> R[(), s] {
        ? self.state {
            OrderState.Shipped => {
                self.state = OrderState.Delivered
                Ok(())
            },
            _ => Err(s.from("Can only deliver from Shipped state")),
        }
    }
}
```

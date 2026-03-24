# Worked Examples: Intermediate

> Prompt → Response pairs with generics, traits, effects, and error handling.
> All examples use **standard syntax** (default).

---

## Example 1: Generic Repository Trait

**Prompt:** Define a generic `Repository` trait with find, save, and delete methods, then implement it for an in-memory store.

**Response:**
```redox
use std::collections::HashMap;

pub trait Repository<T> {
    fn find(&self, id: u64) -> Option<&T>;
    fn save(&mut self, id: u64, item: T) -> Result<(), Error> / io;
    fn delete(&mut self, id: u64) -> Result<(), Error> / io;
}

pub struct InMemoryStore<T> {
    data: HashMap<u64, T>,
}

impl<T> InMemoryStore<T> {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }
}

impl<T> Repository<T> for InMemoryStore<T> {
    fn find(&self, id: u64) -> Option<&T> {
        self.data.get(&id)
    }

    fn save(&mut self, id: u64, item: T) -> Result<(), Error> / io {
        self.data.insert(id, item);
        Ok(())
    }

    fn delete(&mut self, id: u64) -> Result<(), Error> / io {
        self.data.remove(&id);
        Ok(())
    }
}
```

---

## Example 2: Custom Error Type with Conversions

**Prompt:** Create an `AppError` enum that wraps io::Error and serde_json::Error, with Display and From implementations.

**Response:**
```redox
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum AppError {
    Io(io::Error),
    Json(serde_json::Error),
    Custom(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::Io(e) => f.write_str(&format!("I/O error: {e}")),
            AppError::Json(e) => f.write_str(&format!("JSON error: {e}")),
            AppError::Custom(msg) => f.write_str(msg),
        }
    }
}

impl From<io::Error> for AppError {
    fn from(e: io::Error) -> Self { AppError::Io(e) }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self { AppError::Json(e) }
}
```

---

## Example 3: Iterator Adapter Chain

**Prompt:** Given a list of `Transaction { amount: f64, category: String }`, compute the total spent per category.

**Response:**
```redox
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Transaction {
    pub amount: f64,
    pub category: String,
}

pub fn totals_by_category(txns: &Vec<Transaction>) -> HashMap<String, f64> {
    let mut map = HashMap::new();
    for txn in txns {
        let entry = map.entry(txn.category.clone()).or_insert(0.0);
        *entry += txn.amount;
    }
    map
}
```

---

## Example 4: Trait Objects and Dynamic Dispatch

**Prompt:** Create a `Logger` trait with an `info` method, implement it for `ConsoleLogger` and `FileLogger`, and write a function that accepts any logger.

**Response:**
```redox
use std::fs;

pub trait Logger {
    fn info(&self, msg: &str) / io;
}

pub struct ConsoleLogger {}

impl Logger for ConsoleLogger {
    fn info(&self, msg: &str) / io {
        println!("[INFO] {msg}");
    }
}

pub struct FileLogger {
    path: String,
}

impl Logger for FileLogger {
    fn info(&self, msg: &str) / io {
        // Append to log file
        let content = format!("[INFO] {msg}\n");
        fs::append(&self.path, &content).unwrap_or(());
    }
}

pub fn log_event(logger: &dyn Logger, event: &str) / io {
    logger.info(&format!("Event occurred: {event}"));
}
```

---

## Example 5: JSON Config Loading

**Prompt:** Write a function that loads a JSON config file, validates required fields, and returns a typed config struct.

**Response:**
```redox
use std::fs;
use std::json;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub debug: bool,
}

pub fn load_config(path: &str) -> Result<Config, AppError> / io {
    let text = fs::read_to_string(path)?;
    let config = json::parse::<Config>(&text)?;

    if config.host.is_empty() {
        return Err(AppError::Custom(String::from("host cannot be empty")));
    }
    if config.port == 0 {
        return Err(AppError::Custom(String::from("port must be non-zero")));
    }

    Ok(config)
}
```

---

## Example 6: Async HTTP Client

**Prompt:** Write an async function that fetches JSON from a URL, deserializes it into a struct, and handles errors.

**Response:**
```redox
use std::json;

#[derive(Debug, Clone)]
pub struct ApiResponse {
    pub status: String,
    pub data: Vec<String>,
}

pub async fn fetch_data(url: &str) -> Result<ApiResponse, Error> / net {
    let resp = http::get(url).await?;
    if resp.status_code() != 200 {
        return Err(Error::new(format!("HTTP {}", resp.status_code())));
    }
    let body = resp.text().await?;
    let parsed = json::parse::<ApiResponse>(&body)?;
    Ok(parsed)
}
```

---

## Example 7: Effect-Handled Testing

**Prompt:** Write a function that reads a config file and a test that mocks the file I/O.

**Response:**
```redox
use std::fs;

pub fn read_setting(path: &str, key: &str) -> Result<Option<String>, io::Error> / io {
    let content = fs::read_to_string(path)?;
    for line in content.lines() {
        if line.starts_with(key) {
            let value = line.splitn(2, '=').nth(1)
                .map(|v| v.trim().to_string());
            return Ok(value);
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_setting() {
        handle io {
            read_to_string(_) => "host=localhost\nport=8080",
        } {
            let result = read_setting("config.txt", "host");
            assert_eq!(result.unwrap(), Some(String::from("localhost")));
        }
    }

    #[test]
    fn test_missing_key() {
        handle io {
            read_to_string(_) => "host=localhost",
        } {
            let result = read_setting("config.txt", "missing");
            assert_eq!(result.unwrap(), None);
        }
    }
}
```

---

## Example 8: State Machine Pattern

**Prompt:** Implement a simple state machine for an order: Created → Paid → Shipped → Delivered.

**Response:**
```redox
#[derive(Debug, Clone)]
pub enum OrderState {
    Created,
    Paid,
    Shipped,
    Delivered,
}

#[derive(Debug)]
pub struct Order {
    pub id: u64,
    pub state: OrderState,
}

impl Order {
    pub fn new(id: u64) -> Self {
        Self { id, state: OrderState::Created }
    }

    pub fn pay(&mut self) -> Result<(), String> {
        match self.state {
            OrderState::Created => {
                self.state = OrderState::Paid;
                Ok(())
            },
            _ => Err(String::from("Can only pay from Created state")),
        }
    }

    pub fn ship(&mut self) -> Result<(), String> {
        match self.state {
            OrderState::Paid => {
                self.state = OrderState::Shipped;
                Ok(())
            },
            _ => Err(String::from("Can only ship from Paid state")),
        }
    }

    pub fn deliver(&mut self) -> Result<(), String> {
        match self.state {
            OrderState::Shipped => {
                self.state = OrderState::Delivered;
                Ok(())
            },
            _ => Err(String::from("Can only deliver from Shipped state")),
        }
    }
}
```

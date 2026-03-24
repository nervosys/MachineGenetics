# MechGen Idiomatic Patterns

> Common patterns for generating correct, idiomatic MechGen code.
> All examples use **standard syntax** (default).

---

## Pattern 1: Builder Pattern

```MechGen
pub struct ServerConfig {
    host: String,
    port: u16,
    max_connections: usize,
}

impl ServerConfig {
    pub fn new() -> Self {
        Self {
            host: String::from("localhost"),
            port: 8080,
            max_connections: 100,
        }
    }

    pub fn host(mut self, host: String) -> Self {
        self.host = host;
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn max_connections(mut self, n: usize) -> Self {
        self.max_connections = n;
        self
    }
}

// Usage
let config = ServerConfig::new()
    .host(String::from("0.0.0.0"))
    .port(3000)
    .max_connections(500);
```

## Pattern 2: Error Handling with Custom Error Types

```MechGen
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Io(io::Error),
    Parse(String),
    NotFound(String),
}

impl fmt::Display for AppError {
    pub fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::Io(e) => f.write_str(&format!("IO error: {e}")),
            AppError::Parse(msg) => f.write_str(&format!("Parse error: {msg}")),
            AppError::NotFound(key) => f.write_str(&format!("Not found: {key}")),
        }
    }
}

impl From<io::Error> for AppError {
    pub fn from(e: io::Error) -> Self {
        AppError::Io(e)
    }
}
```

## Pattern 3: Iterator Chains

```MechGen
// Collect filtered, transformed items
fn active_names(users: &Vec<User>) -> Vec<String> {
    users.iter()
        .filter(|u| u.active)
        .map(|u| u.name.clone())
        .collect()
}

// Sum with fold
fn total_cost(items: &Vec<Item>) -> f64 {
    items.iter()
        .fold(0.0, |acc, item| acc + item.price * item.qty as f64)
}

// Find first match
fn find_admin(users: &Vec<User>) -> Option<&User> {
    users.iter().find(|u| u.role == Role::Admin)
}
```

## Pattern 4: Option Chaining

```MechGen
// Prefer map/and_then over manual matching
fn get_username(db: &Database, id: u64) -> Option<String> / io {
    db.find_user(id)?
        .profile
        .as_ref()
        .map(|p| p.username.clone())
}

// unwrap_or_default for safe fallbacks
fn display_name(user: &User) -> String {
    user.nickname
        .clone()
        .unwrap_or_default()
}
```

## Pattern 5: Trait Object Dispatch

```MechGen
pub trait Renderer {
    fn render(&self, data: &str) -> Result<String, Error> / io;
}

pub struct HtmlRenderer {}
pub struct JsonRenderer {}

impl Renderer for HtmlRenderer {
    fn render(&self, data: &str) -> Result<String, Error> / io {
        return format!("<html><body>{data}</body></html>");
    }
}

impl Renderer for JsonRenderer {
    fn render(&self, data: &str) -> Result<String, Error> / io {
        return format!("{{\"content\": \"{data}\"}}");
    }
}

// Dynamic dispatch
pub fn render_output(renderer: &dyn Renderer, data: &str) -> Result<String, Error> / io {
    renderer.render(data)
}
```

## Pattern 6: Newtype Pattern

```MechGen
// Wrap primitive types for type safety
pub struct UserId(u64);
pub struct Email(String);
pub struct Temperature(f64);

impl Temperature {
    pub fn celsius(val: f64) -> Self {
        Temperature(val)
    }

    pub fn to_fahrenheit(&self) -> f64 {
        self.0 * 9.0 / 5.0 + 32.0
    }
}
```

## Pattern 7: Agent with State Machine

```MechGen
use std::agent::{Agent, Capability};

pub enum PipelineState {
    Idle,
    Fetching,
    Processing,
    Done(Vec<u8>),
    Failed(String),
}

pub struct DataPipeline {
    state: PipelineState,
    source: String,
}

impl Agent for DataPipeline {
    pub async fn execute(&mut self) -> Result<(), Error> / io, net, agent {
        self.state = PipelineState::Fetching;
        let raw = http::get(&self.source).await?;

        self.state = PipelineState::Processing;
        let processed = transform(raw.bytes())?;

        self.state = PipelineState::Done(processed);
        return ();
    }
}
```

## Pattern 8: Effect-Bounded Generics

```MechGen
// Accept any function with a known effect signature
pub fn run_with_io<F, R>(work: F) -> R / io
where
    F: FnOnce() -> R / io,
{
    work()
}

// Pure higher-order function (no effects)
pub fn apply_twice<T>(x: T, transform: fn(T) -> T) -> T {
    transform(transform(x))
}
```

## Pattern 9: Capability-Gated Operations

```MechGen
use std::agent::Capability;

pub struct SecureStore {
    cap: Capability,
}

impl SecureStore {
    pub async fn read(&self, key: &str) -> Result<String, Error> / io, agent {
        self.cap.request("kv.read", key)?;
        let data = fs::read_to_string(&format!("/store/{key}"))?;
        return data;
    }

    pub async fn write(&self, key: &str, value: &str) -> Result<(), Error> / io, agent {
        self.cap.request("kv.write", key)?;
        fs::write(&format!("/store/{key}"), value)?;
        return ();
    }
}
```

## Pattern 10: Swarm Fan-Out / Fan-In

```MechGen
use std::agent::{Agent, Swarm};

pub struct UrlChecker {
    url: String,
}

impl Agent for UrlChecker {
    pub async fn execute(&mut self) -> Result<u16, Error> / io, net, agent {
        let resp = http::get(&self.url).await?;
        return resp.status_code();
    }
}

pub async fn check_health(urls: Vec<String>) -> Result<HashMap<String, u16>, Error> / io, net, agent {
    let swarm = Swarm::new();
    for url in &urls {
        swarm.spawn(UrlChecker { url: url.clone() });
    }

    let results = swarm.join_all().await?;
    let mut map = HashMap::new();
    for (url, status) in urls.iter().zip(results.iter()) {
        map.insert(url.clone(), *status);
    }
    return map;
}
```

## Pattern 11: Module Organization

```MechGen
// lib.mg — root module
pub mod models;     // models/mod.mg or models.mg
pub mod handlers;   // handlers/mod.mg or handlers.mg
pub mod utils;      // utils/mod.mg or utils.mg

// Re-export key types
pub use crate::models::User;
pub use crate::models::Config;
pub use crate::handlers::handle_request;
```

## Pattern 12: Test Organization

```MechGen
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addition() {
        let result = add(2, 3);
        assert_eq!(result, 5);
    }

    #[test]
    fn test_error_case() {
        let result = parse_config("");
        assert!(result.is_err());
    }

    // Effect-mocked test
    #[test]
    fn test_with_mock_io() {
        handle io {
            read_file(_) => "mock data",
        } {
            let content = read_config("test.toml");
            assert_eq!(content.unwrap(), "mock data");
        }
    }
}
```

# Worked Examples: Advanced

> Prompt → Response pairs with agents, swarms, capabilities, and complex systems.
> All examples use **human syntax** (default).

---

## Example 1: Web Scraper Agent

**Prompt:** Build an agent that scrapes a web page, extracts all links, and returns them.

**Response:**
```MechGen
use std::agent::{Agent, Capability};

#[derive(Debug)]
pub struct LinkScraper {
    url: String,
    cap: Capability,
}

impl Agent for LinkScraper {
    pub async fn execute(&mut self) -> Result<Vec<String>, Error> / net, agent {
        self.cap.request("net.http.get", &self.url)?;

        let resp = http::get(&self.url).await?;
        let html = resp.text().await?;

        let links = extract_links(&html);
        return links;
    }
}

fn extract_links(html: &str) -> Vec<String> {
    let mut result = Vec::new();
    for segment in html.split("href=\"") {
        if let end = segment.find('"') {
            let link = segment[..end].to_string();
            if link.starts_with("http") {
                result.push(link);
            }
        }
    }
    result
}
```

---

## Example 2: Multi-Agent Swarm Pipeline

**Prompt:** Build a data pipeline using a swarm: one agent fetches data, another transforms it, another saves it.

**Response:**
```MechGen
use std::agent::{Agent, Swarm, Capability};
use std::sync::channel;

#[derive(Debug)]
pub struct Fetcher {
    url: String,
}

impl Agent for Fetcher {
    pub async fn execute(&mut self) -> Result<String, Error> / net, agent {
        let resp = http::get(&self.url).await?;
        resp.text().await
    }
}

#[derive(Debug)]
pub struct Transformer {
    raw_data: String,
}

impl Agent for Transformer {
    pub async fn execute(&mut self) -> Result<Vec<Record>, Error> / agent {
        let records = parse_csv(&self.raw_data)?;
        let cleaned = records.into_iter()
            .filter(|r| r.is_valid())
            .map(|r| r.normalize())
            .collect();
        return cleaned;
    }
}

#[derive(Debug)]
pub struct Saver {
    records: Vec<Record>,
    output_path: String,
}

impl Agent for Saver {
    pub async fn execute(&mut self) -> Result<usize, Error> / io, agent {
        let j = json::stringify(&self.records)?;
        fs::write(&self.output_path, &j)?;
        return self.records.len();
    }
}

pub async fn run_pipeline(url: String, output: String) -> Result<usize, Error> / net, io, agent {
    // Stage 1: Fetch
    let fetcher = Fetcher { url };
    let raw = fetcher.execute().await?;

    // Stage 2: Transform
    let transformer = Transformer { raw_data: raw };
    let records = transformer.execute().await?;

    // Stage 3: Save
    let count = records.len();
    let saver = Saver { records, output_path: output };
    saver.execute().await?;

    println!("Pipeline complete: {count} records saved");
    return count;
}
```

---

## Example 3: Capability-Gated Secret Manager

**Prompt:** Build a secret manager agent that requires explicit capabilities to read or write secrets.

**Response:**
```MechGen
use std::agent::{Agent, Capability};
use std::collections::HashMap;

pub struct SecretManager {
    cap: Capability,
    store: HashMap<String, String>,
}

impl SecretManager {
    pub fn new(cap: Capability) -> Self {
        Self { cap, store: HashMap::new() }
    }
}

impl Agent for SecretManager {
    pub async fn execute(&mut self) -> Result<(), Error> / io, agent {
        // Default execution: validate all secrets
        for (key, _) in &self.store {
            self.cap.request("secrets.validate", key)?;
        }
        println!("All {} secrets validated", self.store.len());
        Ok(())
    }
}

impl SecretManager {
    pub async fn get(&self, key: &str) -> Result<Option<&String>, Error> / agent {
        self.cap.request("secrets.read", key)?;
        Ok(self.store.get(key))
    }

    pub async fn set(&mut self, key: String, value: String) -> Result<(), Error> / agent {
        self.cap.request("secrets.write", &key)?;
        self.store.insert(key, value);
        Ok(())
    }

    pub async fn delete(&mut self, key: &str) -> Result<Option<String>, Error> / agent {
        self.cap.request("secrets.delete", key)?;
        Ok(self.store.remove(key))
    }
}
```

---

## Example 4: Parallel URL Health Check with Timeout

**Prompt:** Check the health of multiple URLs in parallel using a swarm, with a 5-second timeout per check.

**Response:**
```MechGen
use std::agent::{Agent, Swarm};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy(u16),
    Unhealthy(String),
    Timeout,
}

pub struct HealthChecker {
    url: String,
    timeout: Duration,
}

impl Agent for HealthChecker {
    pub async fn execute(&mut self) -> Result<HealthStatus, Error> / net, time, agent {
        let result = http::get(&self.url)
            .timeout(self.timeout)
            .await;

        match result {
            Ok(resp) => {
                let status = resp.status_code();
                if status >= 200 && status < 400 {
                    return Ok(HealthStatus::Healthy(status));
                } else {
                    return Ok(HealthStatus::Unhealthy(format!("HTTP {status}")));
                }
            },
            Err(e) => {
                if e.is_timeout() {
                    return Ok(HealthStatus::Timeout);
                } else {
                    return Ok(HealthStatus::Unhealthy(format!("{e}")));
                }
            },
        }
    }
}

pub async fn check_all(urls: Vec<String>) -> Result<HashMap<String, HealthStatus>, Error> / net, time, agent {
    let swarm = Swarm::new();
    let timeout = Duration::from_secs(5);

    for url in &urls {
        swarm.spawn(HealthChecker {
            url: url.clone(),
            timeout,
        });
    }

    let results = swarm.join_all().await?;
    let mut report = HashMap::new();
    for (url, status) in urls.into_iter().zip(results.into_iter()) {
        report.insert(url, status);
    }
    return report;
}
```

---

## Example 5: Event-Driven Agent with Message Passing

**Prompt:** Build agents that communicate via channels: a producer generates events, a consumer processes them.

**Response:**
```MechGen
use std::agent::{Agent, Swarm};
use std::sync::{channel, Sender, Receiver};

#[derive(Debug, Clone)]
pub enum Event {
    Data(String),
    Shutdown,
}

pub struct Producer {
    sender: Sender<Event>,
    items: Vec<String>,
}

impl Agent for Producer {
    pub async fn execute(&mut self) -> Result<(), Error> / io, agent {
        for item in &self.items {
            println!("Producing: {item}");
            self.sender.send(Event::Data(item.clone()))?;
        }
        self.sender.send(Event::Shutdown)?;
        Ok(())
    }
}

pub struct Consumer {
    receiver: Receiver<Event>,
}

impl Agent for Consumer {
    pub async fn execute(&mut self) -> Result<usize, Error> / io, agent {
        let mut count: usize = 0;
        loop {
            match self.receiver.recv()? {
                Event::Data(msg) => {
                    println!("Consumed: {msg}");
                    count += 1;
                },
                Event::Shutdown => {
                    println!("Shutting down after {count} events");
                    return Ok(count);
                },
            }
        }
    }
}

pub async fn run_pipeline() -> Result<usize, Error> / io, agent {
    let (tx, rx) = channel::<Event>();

    let items = ["alpha", "beta", "gamma"]
        .iter()
        .map(|i| String::from(*i))
        .collect();

    let swarm = Swarm::new();
    swarm.spawn(Producer { sender: tx, items });
    swarm.spawn(Consumer { receiver: rx });

    let results = swarm.join_all().await?;
    // Consumer result is the count
    return results[1];
}
```

---

## Example 6: Generic Middleware Chain

**Prompt:** Build a middleware system where each middleware transforms a request, similar to HTTP middleware.

**Response:**
```MechGen
#[derive(Debug, Clone)]
pub struct Request {
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

pub trait Middleware {
    async fn handle(&self, req: Request, next: &dyn Handler) -> Result<Response, Error> / net;
}

pub trait Handler {
    async fn handle(&self, req: Request) -> Result<Response, Error> / net;
}

pub struct LoggingMiddleware {}

impl Middleware for LoggingMiddleware {
    async fn handle(&self, req: Request, next: &dyn Handler) -> Result<Response, Error> / net {
        println!("→ {}", req.path);
        let resp = next.handle(req).await?;
        println!("← {}", resp.status);
        return resp;
    }
}

pub struct AuthMiddleware {
    api_key: String,
}

impl Middleware for AuthMiddleware {
    async fn handle(&self, mut req: Request, next: &dyn Handler) -> Result<Response, Error> / net {
        if let key = req.headers.get("Authorization") {
            if key != &self.api_key {
                return Ok(Response { status: 401, body: String::from("Unauthorized") });
            }
        } else {
            return Ok(Response { status: 401, body: String::from("Missing auth") });
        }
        next.handle(req).await
    }
}

pub struct AppHandler {}

impl Handler for AppHandler {
    async fn handle(&self, req: Request) -> Result<Response, Error> / net {
        Ok(Response {
            status: 200,
            body: format!("OK: {}", req.path),
        })
    }
}
```

---

## Example 7: SKB Rule Integration

**Prompt:** Write an agent that queries the Semantic Knowledge Base for optimization rules and applies them.

**Response:**
```MechGen
use std::agent::{Agent, Capability};
use std::skb::{Rule, Query, Proof};

#[derive(Debug)]
pub struct Optimizer {
    source: String,
    cap: Capability,
}

impl Agent for Optimizer {
    pub async fn execute(&mut self) -> Result<String, Error> / io, agent {
        self.cap.request("skb.query", "optimization")?;

        // Query SKB for applicable rules
        let query = Query::new("category", "optimization");
        let rules = skb::search(query)?;

        let mut optimized = self.source.clone();
        for rule in &rules {
            // Try to apply each rule, check proof
            if let proof = rule.try_apply(&optimized) {
                if proof.is_valid() {
                    optimized = proof.apply();
                    println!("Applied rule: {}", rule.name());
                }
            }
        }

        return optimized;
    }
}

pub async fn optimize_file(path: &str) -> Result<String, Error> / io, agent {
    let source = fs::read_to_string(path)?;
    let cap = Capability::new("skb");

    let optimizer = Optimizer { source, cap };
    let result = optimizer.execute().await?;

    fs::write(path, &result)?;
    println!("Optimized {path}");
    return result;
}
```

---

## Example 8: Full Application: CLI Task Manager

**Prompt:** Build a complete CLI task manager application with add, list, complete, and delete operations.

**Response:**
```MechGen
use std::fs;
use std::json;
use std::env;

#[derive(Debug, Clone)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub done: bool,
}

#[derive(Debug)]
pub struct TaskStore {
    path: String,
    tasks: Vec<Task>,
    next_id: u64,
}

impl TaskStore {
    pub fn load(path: &str) -> Result<Self, Error> / io {
        if fs::exists(path) {
            let data = fs::read_to_string(path)?;
            let tasks = json::parse::<Vec<Task>>(&data)?;
            let next_id = tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
            return Ok(Self { path: path.to_string(), tasks, next_id });
        } else {
            return Ok(Self {
                path: path.to_string(),
                tasks: Vec::new(),
                next_id: 1,
            });
        }
    }

    pub fn save(&self) -> Result<(), Error> / io {
        let data = json::stringify_pretty(&self.tasks)?;
        fs::write(&self.path, &data)
    }

    pub fn add(&mut self, title: String) -> u64 {
        let id = self.next_id;
        self.tasks.push(Task { id, title, done: false });
        self.next_id += 1;
        id
    }

    pub fn complete(&mut self, id: u64) -> Result<(), String> {
        if let task = self.tasks.iter_mut().find(|t| t.id == id) {
            task.done = true;
            Ok(())
        } else {
            Err(format!("Task {id} not found"))
        }
    }

    pub fn delete(&mut self, id: u64) -> Result<(), String> {
        let before = self.tasks.len();
        self.tasks.retain(|t| t.id != id);
        if self.tasks.len() < before {
            Ok(())
        } else {
            Err(format!("Task {id} not found"))
        }
    }

    pub fn list(&self) / io {
        if self.tasks.is_empty() {
            println!("No tasks.");
            return;
        }
        for task in &self.tasks {
            let mark = if task.done { "✓" } else { " " };
            println!("[{mark}] {}: {}", task.id, task.title);
        }
    }
}

pub fn main() -> Result<(), Error> / io, env {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut store = TaskStore::load("tasks.json")?;

    match args.first().map(|a| a.as_str()) {
        Some("add") => {
            let title = args[1..].join(" ");
            let id = store.add(title);
            store.save()?;
            println!("Added task #{id}");
        },
        Some("list") => {
            store.list();
        },
        Some("done") => {
            let id: u64 = args[1].parse().unwrap();
            store.complete(id).unwrap();
            store.save()?;
            println!("Completed task #{id}");
        },
        Some("rm") => {
            let id: u64 = args[1].parse().unwrap();
            store.delete(id).unwrap();
            store.save()?;
            println!("Deleted task #{id}");
        },
        _ => {
            println!("Usage: tasks <add|list|done|rm> [args...]");
        },
    }

    Ok(())
}
```

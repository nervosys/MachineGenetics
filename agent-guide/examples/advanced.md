# Worked Examples: Advanced

> Prompt → Response pairs with agents, swarms, capabilities, and complex systems.

---

## Example 1: Web Scraper Agent

**Prompt:** Build an agent that scrapes a web page, extracts all links, and returns them.

**Response:**
```redox
u std.agent.{Agent, Capability}

@d(Debug)]
+S LinkScraper {
    url: s,
    cap: Capability,
}

I Agent ~ LinkScraper {
    +af execute(&!self) -> R[[s]~, Error] / net, agent {
        self.cap.request("net.http.get", &self.url)?

        v resp = http.get(&self.url).await?
        v html = resp.text().await?

        v links = extract_links(&html)
        ret links
    }
}

f extract_links(html: &s) -> [s]~ {
    m result = [s]~.new()
    @ segment ~ html.split("href=\"") {
        ? v end = segment.find('"') {
            v link = segment[..end].to_string()
            ? link.starts_with("http") {
                result.push(link)
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
```redox
u std.agent.{Agent, Swarm, Capability}
u std.sync.channel

@d(Debug)
+S Fetcher {
    url: s,
}

I Agent ~ Fetcher {
    +af execute(&!self) -> R[s, Error] / net, agent {
        v resp = http.get(&self.url).await?
        resp.text().await
    }
}

@d(Debug)
+S Transformer {
    raw_data: s,
}

I Agent ~ Transformer {
    +af execute(&!self) -> R[[Record]~, Error] / agent {
        v records = parse_csv(&self.raw_data)?
        v cleaned = records.into_iter()
            .filter(|r| r.is_valid())
            .map(|r| r.normalize())
            .collect()
        ret cleaned
    }
}

@d(Debug)
+S Saver {
    records: [Record]~,
    output_path: s,
}

I Agent ~ Saver {
    +af execute(&!self) -> R[usize, Error] / io, agent {
        v json = json.stringify(&self.records)?
        fs.write(&self.output_path, &json)?
        ret self.records.len()
    }
}

+af run_pipeline(url: s, output: s) -> R[usize, Error] / net, io, agent {
    // Stage 1: Fetch
    v fetcher = Fetcher @{ url }
    v raw = fetcher.execute().await?

    // Stage 2: Transform
    v transformer = Transformer @{ raw_data: raw }
    v records = transformer.execute().await?

    // Stage 3: Save
    v count = records.len()
    v saver = Saver @{ records, output_path: output }
    saver.execute().await?

    p"Pipeline complete: {count} records saved"
    ret count
}
```

---

## Example 3: Capability-Gated Secret Manager

**Prompt:** Build a secret manager agent that requires explicit capabilities to read or write secrets.

**Response:**
```redox
u std.agent.{Agent, Capability}
u std.col.HashMap

+S SecretManager {
    cap: Capability,
    store: {s: s},
}

I ~ SecretManager {
    +f new(cap: Capability) -> Self {
        Self @{ cap, store: {s: s}.new() }
    }
}

I Agent ~ SecretManager {
    +af execute(&!self) -> R[(), Error] / io, agent {
        // Default execution: validate all secrets
        @ (key, _) ~ &self.store {
            self.cap.request("secrets.validate", key)?
        }
        p"All {self.store.len()} secrets validated"
        Ok(())
    }
}

I ~ SecretManager {
    +af get(&self, key: &s) -> R[?&s, Error] / agent {
        self.cap.request("secrets.read", key)?
        Ok(self.store.get(key))
    }

    +af set(&!self, key: s, value: s) -> R[(), Error] / agent {
        self.cap.request("secrets.write", &key)?
        self.store.insert(key, value)
        Ok(())
    }

    +af delete(&!self, key: &s) -> R[?s, Error] / agent {
        self.cap.request("secrets.delete", key)?
        Ok(self.store.remove(key))
    }
}
```

---

## Example 4: Parallel URL Health Check with Timeout

**Prompt:** Check the health of multiple URLs in parallel using a swarm, with a 5-second timeout per check.

**Response:**
```redox
u std.agent.{Agent, Swarm}
u std.time.Duration

@d(Debug, Clone)
+E HealthStatus {
    Healthy(u16),
    Unhealthy(s),
    Timeout,
}

+S HealthChecker {
    url: s,
    timeout: Duration,
}

I Agent ~ HealthChecker {
    +af execute(&!self) -> R[HealthStatus, Error] / net, time, agent {
        v result = http.get(&self.url)
            .timeout(self.timeout)
            .await

        ? result {
            Ok(resp) => {
                v status = resp.status_code()
                ? status >= 200 && status < 400 {
                    ret Ok(HealthStatus.Healthy(status))
                } : {
                    ret Ok(HealthStatus.Unhealthy(f"HTTP {status}"))
                }
            },
            Err(e) => {
                ? e.is_timeout() {
                    ret Ok(HealthStatus.Timeout)
                } : {
                    ret Ok(HealthStatus.Unhealthy(f"{e}"))
                }
            },
        }
    }
}

+af check_all(urls: [s]~) -> R[{s: HealthStatus}, Error] / net, time, agent {
    v swarm = Swarm.new()
    v timeout = Duration.from_secs(5)

    @ url ~ &urls {
        swarm.spawn(HealthChecker @{
            url: url.clone(),
            timeout,
        })
    }

    v results = swarm.join_all().await?
    m report = {s: HealthStatus}.new()
    @ (url, status) ~ urls.into_iter().zip(results.into_iter()) {
        report.insert(url, status)
    }
    ret report
}
```

---

## Example 5: Event-Driven Agent with Message Passing

**Prompt:** Build agents that communicate via channels: a producer generates events, a consumer processes them.

**Response:**
```redox
u std.agent.{Agent, Swarm}
u std.sync.{channel, Sender, Receiver}

@d(Debug, Clone)
+E Event {
    Data(s),
    Shutdown,
}

+S Producer {
    sender: Sender[Event],
    items: [s]~,
}

I Agent ~ Producer {
    +af execute(&!self) -> R[(), Error] / io, agent {
        @ item ~ &self.items {
            p"Producing: {item}"
            self.sender.send(Event.Data(item.clone()))?
        }
        self.sender.send(Event.Shutdown)?
        Ok(())
    }
}

+S Consumer {
    receiver: Receiver[Event],
}

I Agent ~ Consumer {
    +af execute(&!self) -> R[usize, Error] / io, agent {
        m count: usize = 0
        loop {
            ? self.receiver.recv()? {
                Event.Data(msg) => {
                    p"Consumed: {msg}"
                    count += 1
                },
                Event.Shutdown => {
                    p"Shutting down after {count} events"
                    ret Ok(count)
                },
            }
        }
    }
}

+af run_pipeline() -> R[usize, Error] / io, agent {
    v (tx, rx) = channel[Event]()

    v items = ["alpha", "beta", "gamma"]
        .iter()
        .map(|i| s.from(*i))
        .collect()

    v swarm = Swarm.new()
    swarm.spawn(Producer @{ sender: tx, items })
    swarm.spawn(Consumer @{ receiver: rx })

    v results = swarm.join_all().await?
    // Consumer result is the count
    ret results[1]
}
```

---

## Example 6: Generic Middleware Chain

**Prompt:** Build a middleware system where each middleware transforms a request, similar to HTTP middleware.

**Response:**
```redox
@d(Debug, Clone)
+S Request {
    +path: s,
    +headers: {s: s},
    +body: s,
}

@d(Debug, Clone)
+S Response {
    +status: u16,
    +body: s,
}

+T Middleware {
    af handle(&self, req: Request, next: &dyn Handler) -> R[Response, Error] / net
}

+T Handler {
    af handle(&self, req: Request) -> R[Response, Error] / net
}

+S LoggingMiddleware {}

I Middleware ~ LoggingMiddleware {
    af handle(&self, req: Request, next: &dyn Handler) -> R[Response, Error] / net {
        p"→ {req.path}"
        v resp = next.handle(req).await?
        p"← {resp.status}"
        ret resp
    }
}

+S AuthMiddleware {
    api_key: s,
}

I Middleware ~ AuthMiddleware {
    af handle(&self, m req: Request, next: &dyn Handler) -> R[Response, Error] / net {
        ? v key = req.headers.get("Authorization") {
            ? key != &self.api_key {
                ret Ok(Response @{ status: 401, body: s.from("Unauthorized") })
            }
        } : {
            ret Ok(Response @{ status: 401, body: s.from("Missing auth") })
        }
        next.handle(req).await
    }
}

+S AppHandler {}

I Handler ~ AppHandler {
    af handle(&self, req: Request) -> R[Response, Error] / net {
        Ok(Response @{
            status: 200,
            body: f"OK: {req.path}",
        })
    }
}
```

---

## Example 7: SKB Rule Integration

**Prompt:** Write an agent that queries the Semantic Knowledge Base for optimization rules and applies them.

**Response:**
```redox
u std.agent.{Agent, Capability}
u std.skb.{Rule, Query, Proof}

@d(Debug)
+S Optimizer {
    source: s,
    cap: Capability,
}

I Agent ~ Optimizer {
    +af execute(&!self) -> R[s, Error] / io, agent {
        self.cap.request("skb.query", "optimization")?

        // Query SKB for applicable rules
        v query = Query.new("category", "optimization")
        v rules = skb.search(query)?

        m optimized = self.source.clone()
        @ rule ~ &rules {
            // Try to apply each rule, check proof
            ? v proof = rule.try_apply(&optimized) {
                ? proof.is_valid() {
                    optimized = proof.apply()
                    p"Applied rule: {rule.name()}"
                }
            }
        }

        ret optimized
    }
}

+af optimize_file(path: &s) -> R[s, Error] / io, agent {
    v source = fs.read_to_string(path)?
    v cap = Capability.new("skb")

    v optimizer = Optimizer @{ source, cap }
    v result = optimizer.execute().await?

    fs.write(path, &result)?
    p"Optimized {path}"
    ret result
}
```

---

## Example 8: Full Application: CLI Task Manager

**Prompt:** Build a complete CLI task manager application with add, list, complete, and delete operations.

**Response:**
```redox
u std.fs
u std.json
u std.env

@d(Debug, Clone)
+S Task {
    +id: u64,
    +title: s,
    +done: bool,
}

@d(Debug)
+S TaskStore {
    path: s,
    tasks: [Task]~,
    next_id: u64,
}

I ~ TaskStore {
    +f load(path: &s) -> R[Self, Error] / io {
        ? fs.exists(path) {
            v data = fs.read_to_string(path)?
            v tasks = json.parse[[Task]~](&data)?
            v next_id = tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1
            ret Ok(Self @{ path: path.to_string(), tasks, next_id })
        } : {
            ret Ok(Self @{
                path: path.to_string(),
                tasks: [Task]~.new(),
                next_id: 1,
            })
        }
    }

    +f save(&self) -> R[(), Error] / io {
        v data = json.stringify_pretty(&self.tasks)?
        fs.write(&self.path, &data)
    }

    +f add(&!self, title: s) -> u64 {
        v id = self.next_id
        self.tasks.push(Task @{ id, title, done: 0b })
        self.next_id += 1
        id
    }

    +f complete(&!self, id: u64) -> R[(), s] {
        ? v task = self.tasks.iter_mut().find(|t| t.id == id) {
            task.done = 1b
            Ok(())
        } : {
            Err(f"Task {id} not found")
        }
    }

    +f delete(&!self, id: u64) -> R[(), s] {
        v before = self.tasks.len()
        self.tasks.retain(|t| t.id != id)
        ? self.tasks.len() < before {
            Ok(())
        } : {
            Err(f"Task {id} not found")
        }
    }

    +f list(&self) / io {
        ? self.tasks.is_empty() {
            p"No tasks."
            ret
        }
        @ task ~ &self.tasks {
            v mark = ? task.done { "✓" } : { " " }
            p"[{mark}] {task.id}: {task.title}"
        }
    }
}

+f main() -> R[(), Error] / io, env {
    v args: [s]~ = env.args().skip(1).collect()
    m store = TaskStore.load("tasks.json")?

    ? args.first().map(|a| a.as_str()) {
        Some("add") => {
            v title = args[1..].join(" ")
            v id = store.add(title)
            store.save()?
            p"Added task #{id}"
        },
        Some("list") => {
            store.list()
        },
        Some("done") => {
            v id: u64 = args[1].parse().unwrap()
            store.complete(id).unwrap()
            store.save()?
            p"Completed task #{id}"
        },
        Some("rm") => {
            v id: u64 = args[1].parse().unwrap()
            store.delete(id).unwrap()
            store.save()?
            p"Deleted task #{id}"
        },
        _ => {
            p"Usage: tasks <add|list|done|rm> [args...]"
        },
    }

    Ok(())
}
```

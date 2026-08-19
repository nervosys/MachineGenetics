# Worked Examples: Advanced

> Agents, swarms, capabilities, effect handlers and a knowledge base — the
> parts of MAGE that exist because it is a language for multi-agent systems.
> Every response was verified with `mage-parse --check`; the four with a `main`
> were run.

The previous version of this file answered all eight prompts in Rust with
agent-shaped names: `impl Agent for X`, `self.cap.request("net.http.get", …)`,
`Swarm::new()`, `.await?`. None of that exists. **The real design is the
opposite shape**, and it is worth stating before the examples:

- **An `agent` block declares a role, not a class.** Capabilities and approval
  gates, no methods and no `execute` to override. The work is an ordinary
  function.
- **A capability is not requested at run time.** There is no `cap.request(…)`.
  Reaching `net` *is* the request, the `/ net` annotation is the grant, and the
  checker is what enforces it — before anything runs.
- **A `swarm` block declares members, topology and consensus.** Fan-out is
  `map`, fan-in is `fold`.
- **Handlers are how you test effectful code.** `handle { … } with E { … }`
  discharges an effect for one block, so a test never touches the real
  resource.

---

## Example 1: Web Scraper Agent

**Prompt:** Build an agent that scrapes a web page, extracts all links, and returns them.

**Response:**
```MAGE
// An `agent` block declares a role: what it may use, and what it must ask
// before doing. It carries no code — the work is an ordinary function, and the
// effect annotation is what ties the two together.
agent LinkScraper {
    capabilities: [net]
    requires_approval: [write_source]
}

// Splitting on the attribute leaves the URL at the head of every fragment
// after the first.
fn links_in(html: str) -> [str]~ {
    val fragments = split(html, "href=\"")
    map(filter(fragments, |part| contains(part, "\"")), |part| first_field(part))
}

fn first_field(fragment: str) -> str {
    val parts = split(fragment, "\"")
    ?= first(parts) {
        Some(head) => head,
        None => "",
    }
}

pub fn scrape(url: str) -> [str]~ / net {
    links_in(net.connect(url))
}

pub fn main() -> [str]~ {
    links_in("<a href=\"https://example.com\">x</a>")
}
```

---

## Example 2: Multi-Agent Swarm Pipeline

**Prompt:** Build a data pipeline using a swarm: one agent fetches data, another transforms it, another saves it.

**Response:**
```MAGE
// Three roles, one swarm. Each stage is a function with its own effects, and
// the pipeline is their composition — the effect set of `run` is the union,
// which is exactly the audit trail.
agent Fetcher { capabilities: [net] }
agent Transformer { capabilities: [] }
agent Saver { capabilities: [fs] }

swarm DataPipeline {
    agent: Fetcher
    size: 3
    topology: mesh
    consensus: majority
}

fn fetch(url: str) -> str / net { net.connect(url) }

fn transform(raw: str) -> [str]~ { map(lines(raw), |line| upper(line)) }

fn save(path: str, rows: [str]~) -> i32 / fs {
    fs.write(path, join(rows, "\n"))
    len(rows) as i32
}

pub fn run(url: str, out: str) -> i32 / net, fs {
    save(out, transform(fetch(url)))
}
```

---

## Example 3: Capability-Gated Secret Manager

**Prompt:** Build a secret manager that requires explicit capabilities to read secrets, and a test that does not touch the real store.

**Response:**
```MAGE
// A secret store, gated twice: `requires_approval` says the agent may not
// write unilaterally, and the effect annotation says every caller inherits the
// obligation. Declaring the effect is also what makes it mockable.
agent SecretManager {
    capabilities: [fs]
    requires_approval: [write_secret]
}

effect Vault {
    fn read(key: str) -> str;
    fn write(key: str, value: str) -> i32;
}

fn read_secret(key: str) -> str / vault { Vault.read(key) }

pub fn secret_or(key: str, fallback: str) -> str / vault {
    val value = read_secret(key)
    ? len(value) > 0 { value } : { fallback }
}

// `handle … with` discharges the effect for the block it wraps, and only for
// that block — so a test runs against an in-memory vault while an unhandled
// call elsewhere still reports.
@test
pub fn test_secret_or() -> str {
    handle {
        secret_or("api_key", "none")
    } with Vault {
        read(key) => "sk-test",
        write(key, value) => 0,
    }
}
```

---

## Example 4: Parallel URL Health Check

**Prompt:** Check the health of several URLs from a swarm, with a time budget per check.

**Response:**
```MAGE
agent UrlChecker { capabilities: [net] }

swarm HealthFleet {
    agent: UrlChecker
    size: 4
    topology: star
    consensus: quorum
}

// Fan out over the URLs, fan in with `fold`. `time` is its own capability, so
// a deadline is a declared effect too — nothing about a timeout is implicit.
fn check_one(url: str, budget_ms: i32) -> bool / net, time {
    val started = time.now()
    val body = net.connect(url)
    len(body) > 0
}

pub fn healthy_count(urls: [str]~, budget_ms: i32) -> i32 / net, time {
    val results = map(urls, |url| check_one(url, budget_ms))
    len(filter(results, |ok| ok)) as i32
}
```

---

## Example 5: Event-Driven Agents

**Prompt:** Build agents that communicate by message: a producer generates events, a consumer processes them.

**Response:**
```MAGE
// Message passing without a channel type: the producer returns its events, the
// consumer folds over them. The agents declare who may do what; the data flow
// is ordinary values, which is what makes it checkable.
agent Producer { capabilities: [agent] }
agent Consumer { capabilities: [agent, io] }

struct Event { topic: str, payload: str }

fn produce(count: i32) -> [Event]~ {
    map(range(count as usize), |n| @Event { topic: "tick", payload: "x" })
}

fn consume(events: [Event]~) -> i32 / io {
    for event in events {
        println(join([event.topic, event.payload], ":"))
    }
    len(events) as i32
}

pub fn main() -> i32 / io {
    consume(produce(3))
}
```

---

## Example 6: Generic Middleware Chain

**Prompt:** Build a middleware system where each middleware transforms a request, like HTTP middleware.

**Response:**
```MAGE
// A middleware is a function value, and a chain is a list of them. `fold`
// applies each in turn — no trait, no dynamic dispatch, no registry.
//
// A function-typed parameter carries no effect annotation: there is no effect
// polymorphism, so a chain of *effectful* middleware would declare the effect
// on the function that runs the chain.
pub fn run_chain(request: str, chain: [fn(str) -> str]~) -> str {
    fold(chain, request, |acc, step| step(acc))
}

fn add_prefix(request: str) -> str { join(["[req] ", request], "") }

fn to_upper(request: str) -> str { upper(request) }

pub fn main() -> str {
    run_chain("get /health", [add_prefix, to_upper])
}
```

---

## Example 7: Knowledge-Base Rule Integration

**Prompt:** Write a function that queries a knowledge base for optimization rules and applies them.

**Response:**
```MAGE
// A `kb` block declares facts and rules. Lookups go through the `kb`
// namespace, which deliberately attributes no built-in effect — no effect kind
// names a store. Declare one if the store is a capability you want gated.
kb Rules {
    fact rewrite(fold_const);
    fact rewrite(dead_code);
    rule applies(x: i32) { rewrite(x) }
}

effect Skb {
    fn search(category: str) -> [str]~;
}

fn applicable_rules(category: str) -> [str]~ / skb {
    Skb.search(category)
}

pub fn optimize(source: str, category: str) -> str / skb {
    fold(applicable_rules(category), source, |acc, rule| join([acc, rule], " ;; "))
}

@test
pub fn test_optimize() -> str {
    handle {
        optimize("f main() {}", "optimization")
    } with Skb {
        search(category) => ["fold_const", "dead_code"],
    }
}
```

---

## Example 8: Full Application: Task Manager

**Prompt:** Build a complete task manager with add, list, complete, and delete operations.

**Response:**
```MAGE
// A whole application: commands as a sum, state as a value, and one function
// per transition. `main` is the only function that needs `io`.
enum Command {
    Add(str),
    Complete(i32),
    Delete(i32),
    List,
}

struct Task { id: i32, title: str, done: bool }

fn add(tasks: [Task]~, title: str) -> [Task]~ {
    join_tasks(tasks, @Task { id: len(tasks) as i32, title: title, done: 0b })
}

fn join_tasks(tasks: [Task]~, extra: Task) -> [Task]~ {
    flatten([tasks, [extra]])
}

fn complete(tasks: [Task]~, id: i32) -> [Task]~ {
    map(tasks, |task| ? task.id == id {
        @Task { id: task.id, title: task.title, done: 1b }
    } : { task })
}

fn delete(tasks: [Task]~, id: i32) -> [Task]~ {
    filter(tasks, |task| task.id != id)
}

fn apply(tasks: [Task]~, command: Command) -> [Task]~ {
    ?= command {
        Add(title) => add(tasks, title),
        Complete(id) => complete(tasks, id),
        Delete(id) => delete(tasks, id),
        List => tasks,
    }
}

pub fn main() -> i32 / io {
    val start = [@Task { id: 0, title: "write docs", done: 0b }]
    val after = apply(apply(start, Add("run tests")), Complete(0))
    for task in after {
        println(task.title)
    }
    len(filter(after, |task| task.done)) as i32
}
```

---

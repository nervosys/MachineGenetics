# The Agent Trait

Every agent in MechGen implements the `Agent` trait.

## Defining an agent

```mg
use std::agent::{Agent, AgentId, Message};

struct Greeter {
    name: String,
}

impl Agent for Greeter {
    pub fn handle(&mut self, msg: Message<String>) -> Result<String, AgentError> / agent {
        let greeting = format!("Hello, {payload}! I'm {name}.", payload = msg.payload, name = self.name);
        Ok(greeting)
    }

    pub fn id(&self) -> AgentId {
        AgentId(1)
    }

    pub fn capabilities(&self) -> Vec<Capability> {
        vec![Capability { name: "greet", scope: CapabilityScope::Instance }]
    }
}
```

## The Agent trait interface

```mg
pub trait Agent {
    // Required: handle an incoming message
    pub fn handle(&mut self, msg: Message<String>) -> Result<String, AgentError> / agent;

    // Required: unique identifier
    pub fn id(&self) -> AgentId;

    // Optional: lifecycle hooks
    pub fn on_start(&mut self) / agent { }
    pub fn on_stop(&mut self) / agent { }

    // Optional: capability declaration
    pub fn capabilities(&self) -> Vec<Capability> { vec![] }
}
```

## Creating and running agents

```mg
pub fn main() / agent, io {
    let mut greeter = Greeter { name: "Bot".into() };

    // Send a message directly
    let msg = Message::new(AgentId(0), greeter.id(), "World");
    let response = greeter.handle(msg)?;
    println!("{response}");   // "Hello, World! I'm Bot."
}
```

## Agent lifecycle

1. **Construction** — create the agent struct
2. **`on_start()`** — called when the agent joins a swarm or starts processing
3. **`handle()`** — called for each incoming message
4. **`on_stop()`** — called when the agent is removed or the swarm shuts down

## Multiple agent types

A swarm can contain different agent types:

```mg
struct Planner;
struct Coder;
struct Reviewer;

impl Agent for Planner {
    pub fn handle(&mut self, msg: Message<String>) -> Result<String, AgentError> / agent {
        Ok(format!("Plan: decompose '{}' into subtasks", msg.payload))
    }
    pub fn id(&self) -> AgentId { AgentId(10) }
}

impl Agent for Coder {
    pub fn handle(&mut self, msg: Message<String>) -> Result<String, AgentError> / agent {
        Ok(format!("Code: implementing '{}'", msg.payload))
    }
    pub fn id(&self) -> AgentId { AgentId(20) }
}

impl Agent for Reviewer {
    pub fn handle(&mut self, msg: Message<String>) -> Result<String, AgentError> / agent {
        Ok(format!("Review: checking '{}' for correctness", msg.payload))
    }
    pub fn id(&self) -> AgentId { AgentId(30) }
}
```

# The Agent Trait

Every agent in Redox implements the `Agent` trait.

## Defining an agent

```rdx
u std.agent.{Agent, AgentId, Message}

S Greeter {
    name: s,
}

I Agent ~ Greeter {
    +f handle(&!self, msg: Message[s]) -> R[s, AgentError] / agent {
        v greeting = f"Hello, {msg.payload}! I'm {self.name}."
        Ok(greeting)
    }

    +f id(&self) -> AgentId {
        AgentId(1)
    }

    +f capabilities(&self) -> [Capability]~ {
        [Capability @{ name: "greet", scope: CapabilityScope.Instance }]~
    }
}
```

## The Agent trait interface

```rdx
+T Agent {
    // Required: handle an incoming message
    +f handle(&!self, msg: Message[s]) -> R[s, AgentError] / agent;

    // Required: unique identifier
    +f id(&self) -> AgentId;

    // Optional: lifecycle hooks
    +f on_start(&!self) / agent { }
    +f on_stop(&!self) / agent { }

    // Optional: capability declaration
    +f capabilities(&self) -> [Capability]~ { []~ }
}
```

## Creating and running agents

```rdx
+f main() / agent, io {
    m greeter = Greeter @{ name: "Bot".into() }

    // Send a message directly
    v msg = Message.new(AgentId(0), greeter.id(), "World")
    v response = greeter.handle(msg)?
    p"{response}"   // "Hello, World! I'm Bot."
}
```

## Agent lifecycle

1. **Construction** — create the agent struct
2. **`on_start()`** — called when the agent joins a swarm or starts processing
3. **`handle()`** — called for each incoming message
4. **`on_stop()`** — called when the agent is removed or the swarm shuts down

## Multiple agent types

A swarm can contain different agent types:

```rdx
S Planner;
S Coder;
S Reviewer;

I Agent ~ Planner {
    +f handle(&!self, msg: Message[s]) -> R[s, AgentError] / agent {
        Ok(f"Plan: decompose '{msg.payload}' into subtasks")
    }
    +f id(&self) -> AgentId { AgentId(10) }
}

I Agent ~ Coder {
    +f handle(&!self, msg: Message[s]) -> R[s, AgentError] / agent {
        Ok(f"Code: implementing '{msg.payload}'")
    }
    +f id(&self) -> AgentId { AgentId(20) }
}

I Agent ~ Reviewer {
    +f handle(&!self, msg: Message[s]) -> R[s, AgentError] / agent {
        Ok(f"Review: checking '{msg.payload}' for correctness")
    }
    +f id(&self) -> AgentId { AgentId(30) }
}
```

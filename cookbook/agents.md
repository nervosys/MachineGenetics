# Agents & Swarms

---

### Define a simple agent

**Problem**: Create an agent that responds to messages.

**Solution**:

```rdx
u std.agent.{Agent, Message, AgentRuntime}

S Greeter { name: s }

I Agent ~ Greeter {
    +f handle(&!self, msg: Message) -> R[?Message, Error] / io {
        v text = msg.payload.as_str()?
        v reply = f"Hello {text}, I'm {self.name}!"
        Ok(Some(Message.reply(&msg, reply)))
    }

    +f id(&self) -> &s { &self.name }
}

+f main() / io, agent {
    v greeter = Greeter @{ name: "Bot".into() }
    v rt = AgentRuntime.new()
    rt.register(greeter)
    rt.send("Bot", Message.new("Alice"))?
    rt.run()?
}
```

---

### Agent with state

**Problem**: Build an agent that tracks state across messages.

**Solution**:

```rdx
u std.agent.{Agent, Message}

S Counter {
    id: s,
    count: u64,
}

I Agent ~ Counter {
    +f handle(&!self, msg: Message) -> R[?Message, Error] / io {
        v cmd = msg.payload.as_str()?
        ? cmd {
            "increment" => {
                self.count += 1
                Ok(Some(Message.reply(&msg, f"count={self.count}")))
            },
            "get" => {
                Ok(Some(Message.reply(&msg, f"{self.count}")))
            },
            "reset" => {
                self.count = 0
                Ok(Some(Message.reply(&msg, "reset")))
            },
            _ => Ok(None),
        }
    }

    +f id(&self) -> &s { &self.id }
}
```

---

### Swarm with consensus

**Problem**: Ask multiple agents and take a majority vote.

**Solution**:

```rdx
u std.agent.{Agent, Message, Swarm, Consensus}

S Voter { id: s, bias: f64 }

I Agent ~ Voter {
    +f handle(&!self, msg: Message) -> R[?Message, Error] {
        v question = msg.payload.as_str()?
        // Simple threshold-based vote
        v vote = ? self.bias > 0.5 { "yes" } : { "no" }
        Ok(Some(Message.reply(&msg, vote)))
    }

    +f id(&self) -> &s { &self.id }
}

+f main() / io, agent {
    m swarm = Swarm.new()
    swarm.add(Voter @{ id: "v1".into(), bias: 0.8 })
    swarm.add(Voter @{ id: "v2".into(), bias: 0.3 })
    swarm.add(Voter @{ id: "v3".into(), bias: 0.9 })

    v result = swarm.broadcast_consensus(
        Message.new("Should we deploy?"),
        Consensus.Majority,
    )?

    p"Decision: {result.payload}"
}
```

---

### Pipeline of agents

**Problem**: Chain agents so each processes the output of the previous one.

**Solution**:

```rdx
u std.agent.{Agent, Message, Swarm}

S Parser { id: s }
S Validator { id: s }
S Formatter { id: s }

I Agent ~ Parser {
    +f handle(&!self, msg: Message) -> R[?Message, Error] / io {
        v raw = msg.payload.as_str()?
        v parsed = parse_input(raw)?
        Ok(Some(Message.reply(&msg, parsed)))
    }
    +f id(&self) -> &s { &self.id }
}

I Agent ~ Validator {
    +f handle(&!self, msg: Message) -> R[?Message, Error] {
        v data = msg.payload.as_str()?
        validate(data)?
        Ok(Some(Message.reply(&msg, data.to_string())))
    }
    +f id(&self) -> &s { &self.id }
}

I Agent ~ Formatter {
    +f handle(&!self, msg: Message) -> R[?Message, Error] {
        v data = msg.payload.as_str()?
        v formatted = f"=== Output ===\n{data}\n=============="
        Ok(Some(Message.reply(&msg, formatted)))
    }
    +f id(&self) -> &s { &self.id }
}

+f main() / io, agent {
    m swarm = Swarm.new()
    swarm.add(Parser @{ id: "parser".into() })
    swarm.add(Validator @{ id: "validator".into() })
    swarm.add(Formatter @{ id: "formatter".into() })

    // Pipeline: parser → validator → formatter
    v input = Message.new("raw input data")
    v result = swarm.pipeline(
        input,
        &["parser", "validator", "formatter"],
    )?

    p"{result.payload}"
}
```

---

### Pub-sub with the bus

**Problem**: Multiple agents subscribe to topics and react to events.

**Solution**:

```rdx
u std.agent.{Agent, Message, Bus}

S Logger { id: s }
S Monitor { id: s }

I Agent ~ Logger {
    +f handle(&!self, msg: Message) -> R[?Message, Error] / io {
        p"[LOG] {msg.payload}"
        Ok(None)
    }
    +f id(&self) -> &s { &self.id }
}

I Agent ~ Monitor {
    +f handle(&!self, msg: Message) -> R[?Message, Error] / io {
        v text = msg.payload.as_str()?
        ? text.contains("error") {
            p"[ALERT] Error detected: {text}"
        }
        Ok(None)
    }
    +f id(&self) -> &s { &self.id }
}

+f main() / io, agent {
    v bus = Bus.new()
    bus.subscribe("events", Logger @{ id: "logger".into() })
    bus.subscribe("events", Monitor @{ id: "monitor".into() })

    bus.publish("events", Message.new("user logged in"))?
    bus.publish("events", Message.new("error: disk full"))?
}
```

---

### Agent with capabilities

**Problem**: Create an agent that requests specific permissions at runtime.

**Solution**:

```rdx
u std.agent.{Agent, Message, Capability, Lease}
u std.time.Duration

S FileAgent { id: s }

I Agent ~ FileAgent {
    +f handle(&!self, msg: Message) -> R[?Message, Error] / io {
        v path = msg.payload.as_str()?

        // Request a time-limited file-read capability
        v lease = Capability.request(
            "fs.read",
            Lease.new(Duration.from_secs(30)),
        )?

        v content = lease.execute(|| {
            fs.read(path)
        })?

        Ok(Some(Message.reply(&msg, content)))
    }

    +f id(&self) -> &s { &self.id }

    +f capabilities(&self) -> [s]~ {
        ["fs.read"]~
    }
}
```

**Discussion**: The capability system replaces `unsafe`. The `Lease` adds
a time bound — after 30 seconds the permission is revoked. This protects
against long-running uncontrolled access.

---

### Supervisor pattern

**Problem**: Restart agents automatically when they fail.

**Solution**:

```rdx
u std.agent.{Agent, Message, Swarm}
u std.time.Duration

S Supervisor { id: s, max_restarts: u32 }

I ~ Supervisor {
    +f run(
        &self,
        swarm: &!Swarm,
        worker_id: &s,
        factory: f() -> ^dyn Agent,
    ) / io, agent {
        m restarts = 0u32
        loop {
            v result = swarm.run_agent(worker_id)

            ? result.is_err() && restarts < self.max_restarts {
                restarts += 1
                p"[Supervisor] Restarting {worker_id} (attempt {restarts})"
                swarm.replace(worker_id, factory())
                sleep(Duration.from_millis(100 * restarts as u64))
            } : result.is_err() {
                p"[Supervisor] {worker_id} exceeded max restarts"
                break
            } : {
                break  // clean exit
            }
        }
    }
}
```

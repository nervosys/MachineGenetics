# Agents & Swarms

> Recipes for agents and swarms — the part of MAGE that exists because it is a
> language for multi-agent systems. Agent-mode syntax; every block was
> verified with `mage-parse --check`.

The previous version of this page documented an entire fictional runtime:
`u std.agent.{Agent, Message, AgentRuntime, Bus, Swarm, Capability, Lease}`,
`I Agent ~ Greeter`, `rt.register(…)`, `bus.subscribe(…)`,
`Capability.request("fs.read", Lease.new(…))`. None of it exists, and the real
design is the opposite shape:

- **`agent Name { capabilities: […] requires_approval: […] }`** declares a
  *role*. It carries no code.
- **`swarm Name { agent: A size: 3 topology: mesh consensus: majority }`**
  declares a group. It carries no code either.
- **The work is an ordinary function**, and its `/ effect` annotation is what
  ties it to the role. Reaching `agent`, `net` or `fs` *is* the capability
  request; the annotation is the grant; the checker enforces it before
  anything runs. There is no runtime permission call to forget.
- **Fan-out is `map`, fan-in is `fold`.** The five `swarm_*` orchestration
  names in the lexer are reserved and unimplemented.

---

### Define a simple agent

**Problem**: Declare an agent and the work it does.

**Solution**:

```MAGE
// An `agent` block declares a *role*: what it may use, and what it must ask
// before doing. It carries no code — there is no `Agent` trait to implement,
// no `handle` method and no runtime to register with.
agent Greeter {
    capabilities: [agent, io]
}

// The behaviour is an ordinary function. `agent.spawn` performs `agent`;
// printing performs `io`; both are declared.
+f greet(name: s) -> s / agent, io {
    agent.spawn("greeter")
    v reply = f"Hello {name}, I'm the greeter!"
    p"{reply}"
    reply
}
```

**Discussion**: **An `agent` block declares a role, not a class.** Capabilities and approval gates — no methods, no `Agent` trait, no `handle`, no runtime to register with. The behaviour is an ordinary function, and its annotation is what ties the two together.

---

### Agent with state

**Problem**: Track state across steps.

**Solution**:

```MAGE
// State is a value, not a mutable field on an agent object. A transition
// returns the next state, so the invalid intermediate never exists.
agent Counter { capabilities: [agent] }

+S CounterState { id: s, count: i32 }

+E Command { Increment, Get, Reset }

+f step(state: CounterState, command: Command) -> CounterState {
    ?= command {
        Increment => @CounterState { id: state.id, count: state.count + 1 },
        Get => state,
        Reset => @CounterState { id: state.id, count: 0 },
    }
}

+f main() -> i32 {
    v start = @CounterState { id: "c1", count: 0 }
    step(step(start, Increment), Increment).count
}
```

**Discussion**: State is a value and a transition returns the next one, so the invalid intermediate state never exists. There is no `&!self` and no mutable field on an agent object.

---

### Swarm with consensus

**Problem**: Ask several agents and take a majority.

**Solution**:

```MAGE
// A `swarm` block declares members, topology and consensus. The declaration
// says what to do; the code below does it — the majority is *computed*, not
// assumed from the block.
agent Voter { capabilities: [agent] }

swarm Panel {
    agent: Voter
    size: 3
    topology: mesh
    consensus: majority
}

f vote(bias: f64) -> bool { bias > 0.5 }

+f decision(biases: [f64]~) -> s / agent {
    agent.spawn("panel")
    v votes = map(biases, |bias| vote(bias))
    v yes = len(filter(votes, |v2| v2))
    ? yes * 2 > len(votes) { "yes" } : { "no" }
}

+f main() -> s / agent {
    decision([0.8, 0.3, 0.9])
}
```

**Discussion**: A `swarm` block declares members, topology and consensus. The declaration says *what to do*; the code does it — **the majority is computed, not assumed from the block**, and `--check` reports each agent as Verified or Partial against the known capability set.

---

### Pipeline of agents

**Problem**: Chain stages so each processes the previous one's output.

**Solution**:

```MAGE
// A pipeline is function composition. Each stage declares what it reaches,
// and the stage that runs them declares the union — which is the whole
// audit trail for the pipeline, in one signature.
agent Fetcher { capabilities: [net] }
agent Cleaner { capabilities: [] }
agent Writer { capabilities: [fs] }

f fetch(url: s) -> s / net { net.connect(url) }

f clean(raw: s) -> [s]~ { filter(lines(raw), |line| len(line) > 0) }

f store(path: s, rows: [s]~) -> i32 / fs {
    fs.write(path, join(rows, "\n"))
    len(rows) as i32
}

+f run(url: s, out: s) -> i32 / net, fs {
    store(out, clean(fetch(url)))
}
```

**Discussion**: Function composition. Each stage declares what it reaches and the runner declares the union — one signature that is the entire audit trail for the pipeline.

---

### Publish to several subscribers

**Problem**: Fan an event out to more than one handler.

**Solution**:

```MAGE
// There is no bus and no subscription. A topic is a value, subscribers are
// functions, and dispatch is a `?=` — which keeps the fan-out checkable.
agent Logger { capabilities: [io] }
agent Monitor { capabilities: [io] }

f log_event(text: s) -> i32 / io {
    p"[LOG] {text}"
    0
}

f monitor_event(text: s) -> i32 / io {
    ? contains(text, "error") {
        p"[ALERT] error detected: {text}"
    }
    0
}

+f publish(events: [s]~) -> i32 / io {
    fold(events, 0, |acc, text| acc + log_event(text) + monitor_event(text))
}

+f main() -> i32 / io {
    publish(["user logged in", "error: disk full"])
}
```

**Discussion**: There is no bus and no subscription registry. A topic is a value, subscribers are functions, and `fold` collects the results — which keeps the fan-out checkable rather than dynamic.

---

### Agent with capabilities

**Problem**: Grant an agent a permission it must not exceed.

**Solution**:

```MAGE
// `requires_approval` names operations the agent may request but not perform
// unilaterally. There is no runtime `Capability.request(…)` and no `Lease`:
// **reaching the capability is the request, and the annotation is the
// grant**, checked before anything runs.
agent FileAgent {
    capabilities: [fs]
    requires_approval: [write_source]
}

+f read_for(path: s) -> R[s, s] / fs {
    guard len(path) > 0 else { ret Err("no path") }
    Ok(fs.read_to_string(path))
}
```

**Discussion**: `requires_approval` names operations the agent may *request* but not perform unilaterally. There is no runtime `Capability.request` and no `Lease`: **reaching the capability is the request, the annotation is the grant, and the checker enforces it before anything runs**.

---

### Supervisor pattern

**Problem**: Restart failing work, up to a limit.

**Solution**:

```MAGE
// A supervisor without a runtime: retry a fallible step, bounded, with a
// pause between attempts. `time` is declared because the pause is real.
agent Supervisor { capabilities: [agent] }

+f supervise(work: fn(i32) -> R[s, s], max_restarts: i32) -> R[s, s] / time, io {
    m restarts = 0
    @@ {
        ?= work(restarts) {
            Ok(value) => ret Ok(value),
            Err(err) => {
                ? restarts >= max_restarts {
                    p"[supervisor] exceeded max restarts"
                    ret Err(err)
                }
                restarts += 1
                p"[supervisor] restarting (attempt {restarts})"
                time.sleep(100 * restarts)
            },
        }
    }
    Err("unreachable")
}
```

**Discussion**: A bounded retry loop. The pause is real, so `time` is declared; the work is a function value, so it carries no annotation of its own and the caller declares what it performs.

---

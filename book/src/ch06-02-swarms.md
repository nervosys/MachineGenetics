# Swarm Orchestration

A **Swarm** coordinates multiple agents, distributing messages and reaching
consensus on shared decisions.

## Creating a swarm

```rdx
u std.agent.{Swarm, ConsensusStrategy}

+f main() / agent, io {
    m swarm = Swarm.new()

    // Add agents
    swarm.add(Planner)
    swarm.add(Coder)
    swarm.add(Coder)     // multiple coders
    swarm.add(Reviewer)

    // Run the swarm
    swarm.run()?
}
```

## Broadcasting

Send a message to all agents in the swarm:

```rdx
v responses = swarm.broadcast("implement a sorting algorithm")?
@ resp : responses {
    p"Agent responded: {resp}"
}
```

## Targeted messaging

Send to a specific agent:

```rdx
swarm.send(AgentId(20), "implement quicksort")?
```

## Consensus

When agents need to agree on a decision:

```rdx
// Default: majority consensus
v decision = swarm.consensus("Should we use quicksort or mergesort?")?

// Custom strategy
v decision = swarm.consensus_with(
    "Which algorithm?",
    ConsensusStrategy.Unanimous,
)?
```

### Consensus strategies

| Strategy        | Description                         |
| --------------- | ----------------------------------- |
| `Majority`      | > 50% of agents agree               |
| `Unanimous`     | All agents agree                    |
| `FirstResponse` | Use first agent's answer            |
| `Weighted`      | Weighted by agent capability scores |
| `Custom(f)`     | User-defined consensus function     |

## Swarm patterns

### Map-reduce

```rdx
// Distribute subtasks, then merge results
v subtasks = ["parse", "analyze", "generate"]~
v results = swarm.broadcast_all(subtasks)?
v merged = merge_results(results)
```

### Pipeline

```rdx
// Chain agents: planner → coder → reviewer
v plan = swarm.send(planner_id, task)?
v code = swarm.send(coder_id, plan)?
v review = swarm.send(reviewer_id, code)?
```

### Supervisor

```rdx
// One agent oversees others
S Supervisor {
    workers: [AgentId]~,
}

I Agent ~ Supervisor {
    +f handle(&!self, msg: Message[s]) -> R[s, AgentError] / agent {
        // Distribute work to workers, collect results
        m results = [s]~.new()
        @ worker : &self.workers {
            v r = perform_send(*worker, &msg.payload)?
            results.push(r)
        }
        Ok(merge(results))
    }
    +f id(&self) -> AgentId { AgentId(1) }
}
```

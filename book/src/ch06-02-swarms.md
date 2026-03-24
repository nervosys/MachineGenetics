# Swarm Orchestration

A **Swarm** coordinates multiple agents, distributing messages and reaching
consensus on shared decisions.

## Creating a swarm

```rdx
use std::agent::{Swarm, ConsensusStrategy};

pub fn main() / agent, io {
    let mut swarm = Swarm::new();

    // Add agents
    swarm.add(Planner);
    swarm.add(Coder);
    swarm.add(Coder);     // multiple coders
    swarm.add(Reviewer);

    // Run the swarm
    swarm.run()?;
}
```

## Broadcasting

Send a message to all agents in the swarm:

```rdx
let responses = swarm.broadcast("implement a sorting algorithm")?;
for resp in responses {
    println!("Agent responded: {resp}");
}
```

## Targeted messaging

Send to a specific agent:

```rdx
swarm.send(AgentId(20), "implement quicksort")?;
```

## Consensus

When agents need to agree on a decision:

```rdx
// Default: majority consensus
let decision = swarm.consensus("Should we use quicksort or mergesort?")?;

// Custom strategy
let decision = swarm.consensus_with(
    "Which algorithm?",
    ConsensusStrategy::Unanimous,
)?;
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
let subtasks = vec!["parse", "analyze", "generate"];
let results = swarm.broadcast_all(subtasks)?;
let merged = merge_results(results);
```

### Pipeline

```rdx
// Chain agents: planner → coder → reviewer
let plan = swarm.send(planner_id, task)?;
let code = swarm.send(coder_id, plan)?;
let review = swarm.send(reviewer_id, code)?;
```

### Supervisor

```rdx
// One agent oversees others
struct Supervisor {
    workers: Vec<AgentId>,
}

impl Agent for Supervisor {
    pub fn handle(&mut self, msg: Message<String>) -> Result<String, AgentError> / agent {
        // Distribute work to workers, collect results
        let mut results: Vec<String> = Vec::new();
        for worker in &self.workers {
            let r = perform_send(*worker, &msg.payload)?;
            results.push(r);
        }
        Ok(merge(results))
    }
    pub fn id(&self) -> AgentId { AgentId(1) }
}
```

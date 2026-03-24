# Messages & Communication

Agents communicate through typed, structured **Messages** sent over a **Bus**.

## Message structure

```mg
use std::agent::Message;

// Create a message
let msg = Message::new(
    sender_id,          // from
    receiver_id,        // to
    "hello, agent!",    // payload (generic type T)
);

// Access fields
let from = msg.from;
let to = msg.to;
let data = msg.payload;
let time = msg.timestamp;
let corr = msg.correlation_id;    // for request-response tracking
```

## The Bus

The **Bus** is a publish-subscribe message system for swarm-wide communication:

```mg
use std::agent::Bus;

pub fn main() / agent {
    let bus = Bus::new();

    // Subscribe to a topic
    bus.subscribe("build-events", |msg: Message<String>| {
        println!("Build event: {}", msg.payload);
    });

    // Publish to a topic
    bus.publish("build-events", "compilation started")?;

    // List active topics
    let topics = bus.topics();
}
```

## Typed messages

Messages are generic over their payload type:

```mg
// String messages
let text_msg: Message<String> = Message::new(id1, id2, "hello");

// Structured messages
#[derive(Clone, Serialize, Deserialize)]
struct BuildRequest {
    file: String,
    optimize: bool,
}

let build_msg: Message<BuildRequest> = Message::new(
    id1, id2,
    BuildRequest { file: "main.mg".into(), optimize: true },
);
```

## Request-response pattern

Use `correlation_id` to match responses to requests:

```mg
// Send a request
let request = Message::new(my_id, worker_id, "compile main.mg");
let corr_id = request.correlation_id;
swarm.send_msg(request)?;

// Later, match the response
let response = wait_for_response(corr_id)?;
```

## Error handling

Message operations can fail with `AgentError`:

```mg
use std::agent::{AgentError, AgentErrorKind};

match swarm.send(target_id, "task") {
    Ok(response) => println!("got: {response}"),
    Err(e) => match e.kind {
        AgentErrorKind::NotFound => println!("agent not found"),
        AgentErrorKind::Timeout => println!("agent timed out"),
        AgentErrorKind::Rejected => println!("message rejected"),
        _ => println!("error: {e}"),
    },
}
```

# Messages & Communication

Agents communicate through typed, structured **Messages** sent over a **Bus**.

## Message structure

```rdx
u std.agent.Message

// Create a message
v msg = Message.new(
    sender_id,          // from
    receiver_id,        // to
    "hello, agent!",    // payload (generic type T)
)

// Access fields
v from = msg.from
v to = msg.to
v data = msg.payload
v time = msg.timestamp
v corr = msg.correlation_id    // for request-response tracking
```

## The Bus

The **Bus** is a publish-subscribe message system for swarm-wide communication:

```rdx
u std.agent.Bus

+f main() / agent {
    v bus = Bus.new()

    // Subscribe to a topic
    bus.subscribe("build-events", |msg: Message[s]| {
        p"Build event: {msg.payload}"
    })

    // Publish to a topic
    bus.publish("build-events", "compilation started")?

    // List active topics
    v topics = bus.topics()
}
```

## Typed messages

Messages are generic over their payload type:

```rdx
// String messages
v text_msg: Message[s] = Message.new(id1, id2, "hello")

// Structured messages
@d(Clone, Serialize, Deserialize)
S BuildRequest {
    file: s,
    optimize: bool,
}

v build_msg: Message[BuildRequest] = Message.new(
    id1, id2,
    BuildRequest @{ file: "main.rdx".into(), optimize: 1b },
)
```

## Request-response pattern

Use `correlation_id` to match responses to requests:

```rdx
// Send a request
v request = Message.new(my_id, worker_id, "compile main.rdx")
v corr_id = request.correlation_id
swarm.send_msg(request)?

// Later, match the response
v response = wait_for_response(corr_id)?
```

## Error handling

Message operations can fail with `AgentError`:

```rdx
u std.agent.{AgentError, AgentErrorKind}

? swarm.send(target_id, "task") {
    Ok(response) => p"got: {response}",
    Err(e) => ? e.kind {
        AgentErrorKind.NotFound => p"agent not found",
        AgentErrorKind.Timeout => p"agent timed out",
        AgentErrorKind.Rejected => p"message rejected",
        _ => p"error: {e}",
    },
}
```

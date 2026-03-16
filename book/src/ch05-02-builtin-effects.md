# Built-in Effects

Redox defines a set of built-in effects for common side-effect categories.

## Effect catalog

| Effect    | Meaning                 | Triggered by                        |
| --------- | ----------------------- | ----------------------------------- |
| `io`      | File I/O, console I/O   | `File.read`, `print`, `stdin`       |
| `net`     | Networking              | `TcpStream`, `Request`, `UdpSocket` |
| `rng`     | Randomness              | `Rng.new()`, `shuffle`, `choose`    |
| `async`   | Asynchronous operations | `spawn`, `sleep`, `select`          |
| `agent`   | Agent communication     | `Swarm.send`, `Bus.publish`         |
| `time`    | Time observation        | `Instant.now()`, `SystemTime.now()` |
| `env`     | Environment access      | `args()`, `var()`, `current_dir()`  |
| `process` | Process management      | `Command.spawn`, `exit`             |

## Effect hierarchy

Some effects imply others:

```
net → io       (network I/O implies I/O)
process → io   (process management implies I/O)
agent → async  (agent communication implies async)
```

If your function declares `/ net`, it implicitly has `/ io` as well.

## Defining custom effects

You can define your own effects using the `Effect` trait:

```rdx
u std.effect.Effect

+S DbEffect;

I Effect ~ DbEffect {
    type Input = Query;
    type Output = R[Rows, DbError];
}
```

Then use it in function signatures:

```rdx
+f run_query(q: &Query) -> R[Rows, DbError] / db {
    perform[DbEffect](q.clone())
}
```

## Effect combinations in practice

Real applications combine effects naturally:

```rdx
// A web handler: reads DB, writes logs, returns HTTP response
+f handle_request(req: &Request) -> R[Response, Error] / io, net, db {
    v user_id = req.param("id")?
    v user = db_query(f"SELECT * FROM users WHERE id = {user_id}")?
    log(f"Fetched user {user_id}")?
    Ok(Response.json(&user))
}
```

## Pure functions

Functions with no effect annotation are **pure** — they depend only on their
inputs and can be freely cached, memoized, parallelized, and reordered:

```rdx
// Pure: depends only on input
+f fibonacci(n: u64) -> u64 {
    ? n <= 1 { n } : { fibonacci(n - 1) + fibonacci(n - 2) }
}

// The compiler knows this is pure and can optimize accordingly
```

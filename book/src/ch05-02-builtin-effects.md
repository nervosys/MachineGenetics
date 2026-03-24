# Built-in Effects

MechGen defines a set of built-in effects for common side-effect categories.

## Effect catalog

| Effect    | Meaning                 | Triggered by                          |
| --------- | ----------------------- | ------------------------------------- |
| `io`      | File I/O, console I/O   | `File::read`, `print`, `stdin`        |
| `net`     | Networking              | `TcpStream`, `Request`, `UdpSocket`   |
| `rng`     | Randomness              | `Rng::new()`, `shuffle`, `choose`     |
| `async`   | Asynchronous operations | `spawn`, `sleep`, `select`            |
| `agent`   | Agent communication     | `Swarm::send`, `Bus::publish`         |
| `time`    | Time observation        | `Instant::now()`, `SystemTime::now()` |
| `env`     | Environment access      | `args()`, `var()`, `current_dir()`    |
| `process` | Process management      | `Command::spawn`, `exit`              |

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

```mg
use std::effect::Effect;

pub struct DbEffect;

impl Effect for DbEffect {
    type Input = Query;
    type Output = Result<Rows, DbError>;
}
```

Then use it in function signatures:

```mg
pub fn run_query(q: &Query) -> Result<Rows, DbError> / db {
    perform::<DbEffect>(q.clone())
}
```

## Effect combinations in practice

Real applications combine effects naturally:

```mg
// A web handler: reads DB, writes logs, returns HTTP response
pub fn handle_request(req: &Request) -> Result<Response, Error> / io, net, db {
    let user_id = req.param("id")?;
    let user = db_query(&format!("SELECT * FROM users WHERE id = {user_id}"))?;
    log(&format!("Fetched user {user_id}"))?;
    Ok(Response::json(&user))
}
```

## Pure functions

Functions with no effect annotation are **pure** — they depend only on their
inputs and can be freely cached, memoized, parallelized, and reordered:

```mg
// Pure: depends only on input
pub fn fibonacci(n: u64) -> u64 {
    if n <= 1 { n } else { fibonacci(n - 1) + fibonacci(n - 2) }
}

// The compiler knows this is pure and can optimize accordingly
```

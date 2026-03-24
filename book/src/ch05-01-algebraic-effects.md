# Algebraic Effects

## The problem

In most languages, side effects are invisible. A function called `process(data)`
might read files, make HTTP requests, generate random numbers, or launch
missiles — the signature doesn't tell you.

Rust partially addresses this with `async` (marks functions that can yield) and
`unsafe` (marks functions that bypass safety). But most side effects — I/O,
networking, randomness — are invisible in function signatures.

## MechGen's solution: effect annotations

Every function that performs a side effect declares it after `/`:

```mg
// Pure function — no effects
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

// Reads/writes files — io effect
pub fn read_config(path: &str) -> Result<Config, Error> / io {
    let data = File::read(path)?;
    parse(&data)
}

// Makes network requests — net effect (implies io)
pub fn fetch_api(url: &str) -> Result<Value, Error> / net {
    let resp = Request::get(url).send()?;
    parse(&resp.text()?)
}

// Uses randomness
pub fn roll_dice() -> u32 / rng {
    Rng::new().range_int(1, 7)
}
```

## Effect propagation

Effects propagate up the call chain. If `foo()` calls `bar() / io`, then `foo`
must also declare `/ io` (or handle the effect):

```mg
fn bar() / io {
    println!("hello");
}

// Must declare / io because it calls bar()
fn foo() / io {
    bar();
}

// The compiler infers and checks effect propagation
```

## Multiple effects

Functions can have multiple effects, comma-separated:

```mg
pub fn scrape_and_save(url: &str, path: &str) -> Result<(), Error> / io, net {
    let data = Request::get(url).send()?.text()?;
    File::write(path, &data)?;
    Ok(())
}
```

## Effect as documentation

Effects serve as machine-readable documentation. At a glance, you know:

- `fn calculate(...) -> f64` — pure computation, no side effects
- `fn read_file(...) / io` — touches the filesystem
- `fn fetch(...) / net` — makes network calls
- `fn simulate(...) / rng` — uses randomness (non-deterministic)
- `fn coordinate(...) / agent` — communicates with other agents

For AI agents, this is invaluable — the effect signature tells the agent exactly
what a function can do without reading the implementation.

## Effect correctness

The compiler tracks effects and reports violations:

```mg
// ERROR: function performs io but does not declare / io
fn sneaky() {
    println!("I'm printing!");    // io effect not declared!
}
```

This catches accidental side effects in supposedly pure code.

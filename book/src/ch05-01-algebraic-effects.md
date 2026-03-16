# Algebraic Effects

## The problem

In most languages, side effects are invisible. A function called `process(data)`
might read files, make HTTP requests, generate random numbers, or launch
missiles — the signature doesn't tell you.

Rust partially addresses this with `async` (marks functions that can yield) and
`unsafe` (marks functions that bypass safety). But most side effects — I/O,
networking, randomness — are invisible in function signatures.

## Redox's solution: effect annotations

Every function that performs a side effect declares it after `/`:

```rdx
// Pure function — no effects
+f add(a: i32, b: i32) -> i32 {
    a + b
}

// Reads/writes files — io effect
+f read_config(path: &s) -> R[Config, Error] / io {
    v data = File.read(path)?
    parse(&data)
}

// Makes network requests — net effect (implies io)
+f fetch_api(url: &s) -> R[Value, Error] / net {
    v resp = Request.get(url).send()?
    parse(&resp.text()?)
}

// Uses randomness
+f roll_dice() -> u32 / rng {
    Rng.new().range_int(1, 7)
}
```

## Effect propagation

Effects propagate up the call chain. If `foo()` calls `bar() / io`, then `foo`
must also declare `/ io` (or handle the effect):

```rdx
f bar() / io {
    p"hello"
}

// Must declare / io because it calls bar()
f foo() / io {
    bar()
}

// The compiler infers and checks effect propagation
```

## Multiple effects

Functions can have multiple effects, comma-separated:

```rdx
+f scrape_and_save(url: &s, path: &s) -> R[(), Error] / io, net {
    v data = Request.get(url).send()?.text()?
    File.write(path, &data)?
    Ok(())
}
```

## Effect as documentation

Effects serve as machine-readable documentation. At a glance, you know:

- `f calculate(...) -> f64` — pure computation, no side effects
- `f read_file(...) / io` — touches the filesystem
- `f fetch(...) / net` — makes network calls
- `f simulate(...) / rng` — uses randomness (non-deterministic)
- `f coordinate(...) / agent` — communicates with other agents

For AI agents, this is invaluable — the effect signature tells the agent exactly
what a function can do without reading the implementation.

## Effect correctness

The compiler tracks effects and reports violations:

```rdx
// ERROR: function performs io but does not declare / io
f sneaky() {
    p"I'm printing!"    // io effect not declared!
}
```

This catches accidental side effects in supposedly pure code.

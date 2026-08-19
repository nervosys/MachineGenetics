# Concurrency

> Recipes for doing several things at once. Agent-mode syntax; every block was
> verified with `mage-parse --check`.

The previous version of this page was written against a runtime that does not
exist: `u std.async.{spawn, join_all}`, `u std.sync.{Mutex, channel}`,
`handles.push(spawn(move || async { … }))`, `Mutex[Instant]`, `.await`.

**MAGE's concurrency story is the effect system, not a runtime.** There are no
threads, channels, mutexes or futures in the language. What it gives you is
that the *reach* of concurrent work is visible: a function that spawns
declares `/ async`, one that coordinates agents declares `/ agent`, one that
waits declares `/ time`, and every caller inherits all three. Fan-out is
`map`; fan-in is `fold`.

---

### Spawn parallel tasks

**Problem**: Run independent work and collect the results.

**Solution**:

```MAGE
f heavy_compute(n: i32) -> i32 {
    m acc = 1
    @ i in range(n as usize) { acc *= (i as i32) + 1 }
    acc
}

// `async` is a *keyword*, not a capability namespace, so there is no
// `async.spawn(…)`. The `async` effect is attributed by name: calling
// `spawn(…)` or `select(…)` performs it.
f spawn(label: s) -> i32 { len(label) as i32 }

+af compute_all(inputs: [i32]~) -> [i32]~ / async {
    spawn("compute")
    map(inputs, |n| heavy_compute(n))
}
```

**Discussion**: **`async` is a keyword, not a capability namespace** — there is no `async.spawn(…)`. The `async` effect is attributed *by name*: calling `spawn` or `select` performs it (MAGE_SPEC.md §11.2). There is no handle type, no `join_all`, and no `.await` at a call site.

---

### Producer and consumer

**Problem**: One side produces work, the other consumes it.

**Solution**:

```MAGE
// There is no channel type. A producer returns its items and a consumer folds
// over them — the data flow is ordinary values, which is what makes it
// checkable.
f produce(count: i32) -> [s]~ {
    map(range(count as usize), |n| f"item {n}")
}

+f consume(items: [s]~) -> i32 / io {
    @ item in items {
        p"consumed {item}"
    }
    len(items) as i32
}

+f main() -> i32 / io {
    consume(produce(3))
}
```

**Discussion**: There is no channel type. The producer returns its items and the consumer folds over them — the data flow is ordinary values, which is what makes it checkable.

---

### Shared counter

**Problem**: Accumulate across many items.

**Solution**:

```MAGE
// There is no `Mutex`, no lock and no shared mutable state to protect. A
// counter is a value threaded through `fold`.
+f count_matching(items: [s]~, needle: s) -> i32 {
    fold(items, 0, |acc, item| ? contains(item, needle) { acc + 1 } : { acc })
}

+f main() -> i32 {
    count_matching(["alpha", "beta", "gamma"], "a")
}
```

**Discussion**: There is no `Mutex`, no lock, and no shared mutable state to protect: `fold` threads the accumulator. The absence is the feature.

---

### Rate limiter

**Problem**: Pace work over time.

**Solution**:

```MAGE
// A rate limiter needs the clock, and the clock is a capability: `time.now`
// and `time.sleep` both perform `time`, so every caller inherits it.
+f throttled(items: [s]~, per_item_ms: i32) -> i32 / time, io {
    m done = 0
    @ item in items {
        v started = time.now()
        p"{item}"
        time.sleep(per_item_ms)
        done += 1
    }
    done
}
```

**Discussion**: `time.now` and `time.sleep` both perform `time`, so pacing is visible in the signature and inherited by every caller.

---

### Fan-out / fan-in

**Problem**: Distribute work across a swarm and merge the results.

**Solution**:

```MAGE
agent Worker { capabilities: [agent] }

swarm Pool {
    agent: Worker
    size: 4
    topology: mesh
    consensus: majority
}

f handle_item(item: s) -> i32 / agent {
    agent.spawn(item)
    len(item) as i32
}

// Fan out with `map`, fan in with `fold`. This is the whole pattern — the
// five `swarm_*` names in the lexer are reserved and unimplemented, and
// writing one is a parse error that says so.
+f map_reduce(items: [s]~) -> i32 / agent {
    fold(map(items, |item| handle_item(item)), 0, |acc, n| acc + n)
}
```

**Discussion**: `map` fans out, `fold` fans in. The five `swarm_*` orchestration names in the lexer are **reserved and unimplemented** — writing one is a parse error that says so.

---

### Timeout wrapper

**Problem**: Give a piece of work a time budget.

**Solution**:

```MAGE
// A timeout wrapper, without a future to race: run the work, then check the
// clock. `time` is declared because the function reads it.
+f with_deadline(work: fn(s) -> s, input: s, budget_ms: i32) -> ?s / time {
    v started = time.now()
    v result = work(input)
    ? time.now() - started > budget_ms { None } : { Some(result) }
}
```

**Discussion**: A function-typed parameter carries no effect annotation, so the wrapper declares its own `time` and the caller declares whatever the work performs.

---

### Parallel map

**Problem**: Apply a function across a collection.

**Solution**:

```MAGE
// Parallel map is `map`. Whether the runtime distributes it is the runtime's
// business; what the *language* guarantees is that the effects of the mapped
// function are visible in the caller's annotation.
f transform(item: s) -> s { upper(item) }

+f parallel_map(items: [s]~) -> [s]~ {
    map(items, |item| transform(item))
}

+f main() -> [s]~ {
    parallel_map(["a", "b", "c"])
}
```

**Discussion**: It is `map`. Whether the runtime distributes it is the runtime's business; what the *language* guarantees is that the mapped function's effects appear in the caller's annotation.

---

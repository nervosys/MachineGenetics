# Error Handling

> Recipes for failure. Agent-mode syntax; every block was verified with
> `mage-parse --check`.

The previous version of this page was written against machinery MAGE does not
have: `I Display ~ AppError`, `impl From<io::Error>`, `.context(…)`,
`errors.push(…)`, `.expect(…)`, `af() -> R[T, E]` as a parameter type. Three
absences shape every recipe below.

**`?` propagates, it does not convert.** There is no `From`, so an error
crossing a module boundary is converted by a function you call, at a call site
you can see.

**There is no error trait.** `R[T, E]` takes any `E`; rendering, comparing and
wrapping are functions you write. A `str` error is idiomatic for small
programs.

**`panic` is a capability.** It is one of the 17 effect kinds, so a function
that can abort declares `/ panic` and every caller inherits it.

---

### Define a custom error type

**Problem**: Model the failures of a module as one type.

**Solution**:

```MAGE
// An error type is an ordinary sum. There is no `Display` to implement and no
// `#[derive(Debug)]`, so rendering is a function you write.
+E AppError {
    NotFound(s),
    Permission(s),
    Parse(s),
    Internal(s),
}

+f message(err: AppError) -> s {
    ?= err {
        NotFound(item) => f"not found: {item}",
        Permission(who) => f"permission denied: {who}",
        Parse(input) => f"parse error on '{input}'",
        Internal(detail) => f"internal error: {detail}",
    }
}
```

**Discussion**: An error type is an ordinary sum. There is no `Display` to implement, no `#[derive(Debug)]`, and no error trait — rendering is a function you write, and the `?=` over it is exhaustive.

---

### Convert between error types

**Problem**: Turn one module's failure into your own.

**Solution**:

```MAGE
+E AppError { Io(s), Json(s), Custom(s) }

// There is no `From` and no automatic conversion at `?`. A conversion is an
// ordinary function, and calling it is visible at the call site — which is
// the point: nothing silently reshapes an error on the way up.
f from_io(detail: s) -> AppError { Io(detail) }

+f read_config(path: s) -> R[s, AppError] / fs {
    v content = fs.read_to_string(path)
    guard len(content) > 0 else { ret Err(from_io("empty file")) }
    Ok(content)
}
```

**Discussion**: **There is no `From` and no conversion at `?`.** A conversion is an ordinary function, and calling it is visible at the call site. Nothing reshapes an error silently on the way up.

---

### Chain errors with context

**Problem**: Add context to an error as it propagates.

**Solution**:

```MAGE
// "Context" is string building: wrap the message as you return it. There is
// no `.context(…)` combinator and no error trait to hang one on.
f with_context(context: s, err: s) -> s {
    f"{context}: {err}"
}

+f load_user(id: i32) -> R[s, s] / fs {
    v path = f"users/{id}.json"
    v raw = fs.read_to_string(path)
    guard len(raw) > 0 else { ret Err(with_context(f"loading user {id}", "file not found")) }
    Ok(raw)
}
```

**Discussion**: Context is string building. There is no `.context(…)` combinator and no trait to hang one on, so the wrapping is explicit and the message is whatever you wrote.

---

### Retry with backoff

**Problem**: Retry a fallible operation with increasing delays.

**Solution**:

```MAGE
// Retry, without effect polymorphism: the operation is a plain function
// value, and the *caller* declares the effects — a function-typed parameter
// carries no annotation, so `fn(i32) -> R[s, s] / net` does not parse.
+f retry(max_attempts: i32, operation: fn(i32) -> R[s, s]) -> R[s, s] / time {
    m attempt = 0
    @@ {
        attempt += 1
        ?= operation(attempt) {
            Ok(value) => ret Ok(value),
            Err(err) => {
                ? attempt >= max_attempts { ret Err(err) }
                // Exponential backoff: the delay doubles each round.
                time.sleep(attempt * attempt)
            },
        }
    }
    Err("unreachable")
}
```

**Discussion**: The operation is a plain function value. **A function-typed parameter carries no effect annotation** — there is no effect polymorphism, so `fn(i32) -> R[s, s] / net` does not parse, and the caller declares what it performs.

---

### Fallback chain

**Problem**: Try several sources, falling back on failure.

**Solution**:

```MAGE
// A fallback chain is a sequence of guards. `?=` on a result, `ret` on the
// first success.
+f load_setting(key: s) -> R[s, s] / env, fs {
    v from_env = env.get_env(key)
    ? len(from_env) > 0 { ret Ok(from_env) }

    v from_file = fs.read_to_string("config.toml")
    @ line in lines(from_file) {
        v parts = split(line, "=")
        ? len(parts) == 2 && parts[0] == key { ret Ok(parts[1]) }
    }

    Err(f"setting '{key}' not found in any source")
}
```

**Discussion**: A sequence of guards with `ret` on the first success. Every source this function can reach is in its annotation — `env` and `fs` here — which is exactly the audit trail a fallback chain tends to hide.

---

### Collect all errors

**Problem**: Run several checks and collect every failure rather than stopping at the first.

**Solution**:

```MAGE
+S Form { name: s, email: s, age: i32 }

// Collect every failure instead of stopping at the first: build a list, and
// let its emptiness be the verdict.
+f validate(form: Form) -> R[i32, [s]~] {
    m errors: [s]~ = []

    ? len(form.name) == 0 { errors = flatten([errors, ["name is required"]]) }
    ? len(form.email) == 0 { errors = flatten([errors, ["email is required"]]) }
    ? form.age < 18 { errors = flatten([errors, ["must be at least 18"]]) }
    ? !contains(form.email, "@") { errors = flatten([errors, ["invalid email format"]]) }

    ? len(errors) == 0 { Ok(0) } : { Err(errors) }
}

+f main() -> i32 / io {
    v form = @Form { name: "", email: "bad", age: 15 }
    ?= validate(form) {
        Ok(n) => n,
        Err(errors) => {
            p"validation failed:"
            @ err in errors { p"  - {err}" }
            len(errors) as i32
        },
    }
}
```

**Discussion**: `R[T, [s]~]` — the error side is a list. `flatten([xs, [x]])` is how you append; there is no `.push`.

---

### Fail deliberately

**Problem**: Abort with a helpful message during development.

**Solution**:

```MAGE
// There is no `.expect(…)` and no `unwrap`. Return the failure, or abort
// deliberately — `panic` is its own effect kind, so a function that can abort
// says so in its signature.
+f port_or_die(raw: s) -> R[i32, s] / panic {
    guard len(raw) > 0 else { ret Err("PORT env var must be set") }
    ? raw == "0" { panic("PORT must not be zero") }
    Ok(8080)
}

+f main() -> i32 / env, panic, io {
    ?= port_or_die(env.get_env("PORT")) {
        Ok(port) => {
            p"listening on port {port}"
            port
        },
        Err(msg) => {
            p"{msg}"
            1
        },
    }
}
```

**Discussion**: There is no `.expect(…)` and no `unwrap`. `panic` is a **capability like any other**: a function that can abort declares `/ panic`, and every caller inherits it. Note `guard`'s else branch must diverge with `ret` — a `panic` call does not satisfy it.

---

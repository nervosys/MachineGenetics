# Error Handling

---

### Define a custom error type

**Problem**: Create a domain-specific error with multiple variants.

**Solution**:

```mg
u std.fmt.Display

+E AppError {
    NotFound(s),
    Permission(s),
    Parse { input: s, reason: s },
    Internal(s),
}

I Display ~ AppError {
    +f fmt(&self, f: &!Formatter) -> FmtResult {
        ? self {
            AppError.NotFound(item) => write(f, "not found: {item}"),
            AppError.Permission(msg) => write(f, "permission denied: {msg}"),
            AppError.Parse { input, reason } => {
                write(f, "parse error on '{input}': {reason}")
            },
            AppError.Internal(msg) => write(f, "internal error: {msg}"),
        }
    }
}
```

---

### Convert between error types

**Problem**: Use `?` across functions with different error types.

**Solution**:

```mg
u std.io.IoError
u std.json.JsonError

+E AppError {
    Io(IoError),
    Json(JsonError),
    Custom(s),
}

// Enable ? for IoError → AppError
I From[IoError] ~ AppError {
    +f from(e: IoError) -> Self { AppError.Io(e) }
}

// Enable ? for JsonError → AppError
I From[JsonError] ~ AppError {
    +f from(e: JsonError) -> Self { AppError.Json(e) }
}

+f load_config(path: &s) -> R[Config, AppError] / io {
    v text = fs.read(path)?          // IoError → AppError via From
    v config = from_str(&text)?      // JsonError → AppError via From
    Ok(config)
}
```

---

### Chain errors with context

**Problem**: Add context to errors so the caller knows what was happening.

**Solution**:

```mg
+E ContextError {
    WithContext { context: s, source: ^dyn Error },
}

+f with_context[T, E: Error](
    result: R[T, E],
    context: &s,
) -> R[T, ContextError] {
    result.map_err(|e| ContextError.WithContext @{
        context: context.to_string(),
        source: ^.new(e),
    })
}

// Usage
+f load_user(id: u64) -> R[User, ContextError] / io {
    v path = f"users/{id}.json"
    v text = with_context(fs.read(&path), &f"loading user {id}")?
    v user = with_context(from_str(&text), &f"parsing user {id}")?
    Ok(user)
}
```

**Discussion**: When `load_user` fails, the error message becomes
`"loading user 42: file not found: users/42.json"` — clear and actionable.

---

### Retry with exponential backoff

**Problem**: Retry a fallible operation with increasing delays.

**Solution**:

```mg
u std.time.Duration

+af retry[T, E](
    max_attempts: u32,
    base_delay: Duration,
    operation: af() -> R[T, E],
) -> R[T, E] / async {
    m attempt = 0u32
    loop {
        attempt += 1
        ? operation().await {
            Ok(v) => ret Ok(v),
            Err(e) => {
                ? attempt >= max_attempts {
                    ret Err(e)
                }
                v delay = base_delay * 2u32.pow(attempt - 1)
                sleep(delay).await
            },
        }
    }
}

// Usage
+af main() / io, net, async {
    v result = retry(3, Duration.from_millis(500), || async {
        Request.get("https://flaky-api.example.com/data").send().await
    }).await

    ? result {
        Ok(resp) => p"Got: {resp.status()}",
        Err(e) => p"All retries failed: {e}",
    }
}
```

---

### Fallback chain

**Problem**: Try multiple sources, falling back to the next on failure.

**Solution**:

```mg
+f load_setting(key: &s) -> R[s, Error] / io {
    // Try environment variable first
    ? env.var(key) => Ok(v) { ret Ok(v) }

    // Then config file
    ? fs.read("config.toml") => Ok(content) {
        ? parse_toml_key(&content, key) => Some(v) {
            ret Ok(v)
        }
    }

    // Then default
    ? default_value(key) => Some(v) { ret Ok(v) }

    Err(Error.new(f"setting '{key}' not found in any source"))
}
```

---

### Collect all errors

**Problem**: Run multiple operations and collect all errors rather than
stopping at the first.

**Solution**:

```mg
+f validate_fields(form: &Form) -> R[(), [s]~] {
    m errors = [s]~.new()

    ? form.name.is_empty() {
        errors.push("name is required".into())
    }
    ? form.email.is_empty() {
        errors.push("email is required".into())
    }
    ? form.age < 18 {
        errors.push("must be at least 18".into())
    }
    ? !form.email.contains('@') {
        errors.push("invalid email format".into())
    }

    ? errors.is_empty() {
        Ok(())
    } : {
        Err(errors)
    }
}

+f main() / io {
    v form = Form @{ name: "".into(), email: "bad", age: 15 }
    ? validate_fields(&form) {
        Ok(()) => p"Valid!",
        Err(errors) => {
            p"Validation failed:"
            @ e : &errors { p"  - {e}" }
        },
    }
}
```

---

### Unwrap with a message

**Problem**: Crash with a helpful message during development.

**Solution**:

```mg
+f main() / io {
    v port: u16 = env.var("PORT")
        .expect("PORT env var must be set")
        .parse()
        .expect("PORT must be a valid number")

    p"Listening on port {port}"
}
```

**Discussion**: Use `expect` during prototyping. Replace with proper error
handling before production. The `?` operator is almost always preferable.

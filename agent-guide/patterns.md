# Redox Idiomatic Patterns

> Common patterns for generating correct, idiomatic Redox code.

---

## Pattern 1: Builder Pattern

```redox
+S ServerConfig {
    host: s,
    port: u16,
    max_connections: usize,
}

I ~ ServerConfig {
    +f new() -> Self {
        Self @{
            host: s.from("localhost"),
            port: 8080,
            max_connections: 100,
        }
    }

    +f host(m self, host: s) -> Self {
        self.host = host
        self
    }

    +f port(m self, port: u16) -> Self {
        self.port = port
        self
    }

    +f max_connections(m self, n: usize) -> Self {
        self.max_connections = n
        self
    }
}

// Usage
v config = ServerConfig.new()
    .host(s.from("0.0.0.0"))
    .port(3000)
    .max_connections(500)
```

## Pattern 2: Error Handling with Custom Error Types

```redox
u std.fmt

@d(Debug)
+E AppError {
    Io(io.Error),
    Parse(s),
    NotFound(s),
}

I fmt.Display ~ AppError {
    +f fmt(&self, fmtr: &!fmt.Formatter) -> fmt.Result {
        ? self {
            AppError.Io(e) => fmtr.write_str(&f"IO error: {e}"),
            AppError.Parse(msg) => fmtr.write_str(&f"Parse error: {msg}"),
            AppError.NotFound(key) => fmtr.write_str(&f"Not found: {key}"),
        }
    }
}

I From[io.Error] ~ AppError {
    +f from(e: io.Error) -> Self {
        AppError.Io(e)
    }
}
```

## Pattern 3: Iterator Chains

```redox
// Collect filtered, transformed items
f active_names(users: &[[User]~]) -> [s]~ {
    users.iter()
        .filter(|u| u.active)
        .map(|u| u.name.clone())
        .collect()
}

// Sum with fold
f total_cost(items: &[Item]~) -> f64 {
    items.iter()
        .fold(0.0, |acc, item| acc + item.price * item.qty as f64)
}

// Find first match
f find_admin(users: &[User]~) -> ?&User {
    users.iter().find(|u| u.role == Role.Admin)
}
```

## Pattern 4: Option Chaining

```redox
// Prefer map/and_then over manual matching
f get_username(db: &Database, id: u64) -> ?s / io {
    db.find_user(id)?
        .profile
        .as_ref()
        .map(|p| p.username.clone())
}

// unwrap_or_default for safe fallbacks
f display_name(user: &User) -> s {
    user.nickname
        .clone()
        .unwrap_or_default()
}
```

## Pattern 5: Trait Object Dispatch

```redox
+T Renderer {
    f render(&self, data: &s) -> R[s, Error] / io
}

+S HtmlRenderer {}
+S JsonRenderer {}

I Renderer ~ HtmlRenderer {
    f render(&self, data: &s) -> R[s, Error] / io {
        ret f"<html><body>{data}</body></html>"
    }
}

I Renderer ~ JsonRenderer {
    f render(&self, data: &s) -> R[s, Error] / io {
        ret f"{{\"content\": \"{data}\"}}"
    }
}

// Dynamic dispatch
+f render_output(renderer: &dyn Renderer, data: &s) -> R[s, Error] / io {
    renderer.render(data)
}
```

## Pattern 6: Newtype Pattern

```redox
// Wrap primitive types for type safety
+S UserId(u64)
+S Email(s)
+S Temperature(f64)

I ~ Temperature {
    +f celsius(val: f64) -> Self {
        Temperature(val)
    }

    +f to_fahrenheit(&self) -> f64 {
        self.0 * 9.0 / 5.0 + 32.0
    }
}
```

## Pattern 7: Agent with State Machine

```redox
u std.agent.{Agent, Capability}

+E PipelineState {
    Idle,
    Fetching,
    Processing,
    Done([u8]~),
    Failed(s),
}

+S DataPipeline {
    state: PipelineState,
    source: s,
}

I Agent ~ DataPipeline {
    +af execute(&!self) -> R[(), Error] / io, net, agent {
        self.state = PipelineState.Fetching
        v raw = http.get(&self.source).await?

        self.state = PipelineState.Processing
        v processed = transform(raw.bytes())?

        self.state = PipelineState.Done(processed)
        ret ()
    }
}
```

## Pattern 8: Effect-Bounded Generics

```redox
// Accept any function with a known effect signature
+f run_with_io[F, R](work: F) -> R / io
    ~> F: FnOnce() -> R / io
{
    work()
}

// Pure higher-order function (no effects)
+f apply_twice[T](x: T, transform: f(T) -> T) -> T {
    transform(transform(x))
}
```

## Pattern 9: Capability-Gated Operations

```redox
u std.agent.Capability

+S SecureStore {
    cap: Capability,
}

I ~ SecureStore {
    +af read(&self, key: &s) -> R[s, Error] / io, agent {
        self.cap.request("kv.read", key)?
        v data = fs.read_to_string(&f"/store/{key}")?
        ret data
    }

    +af write(&self, key: &s, value: &s) -> R[(), Error] / io, agent {
        self.cap.request("kv.write", key)?
        fs.write(&f"/store/{key}", value)?
        ret ()
    }
}
```

## Pattern 10: Swarm Fan-Out / Fan-In

```redox
u std.agent.{Agent, Swarm}

+S UrlChecker {
    url: s,
}

I Agent ~ UrlChecker {
    +af execute(&!self) -> R[u16, Error] / io, net, agent {
        v resp = http.get(&self.url).await?
        ret resp.status_code()
    }
}

+af check_health(urls: [s]~) -> R[{s: u16}, Error] / io, net, agent {
    v swarm = Swarm.new()
    @ url ~ &urls {
        swarm.spawn(UrlChecker @{ url: url.clone() })
    }

    v results = swarm.join_all().await?
    m map = {s: u16}.new()
    @ (url, status) ~ urls.iter().zip(results.iter()) {
        map.insert(url.clone(), *status)
    }
    ret map
}
```

## Pattern 11: Module Organization

```redox
// lib.rdx — root module
+M models     // models/mod.rdx or models.rdx
+M handlers   // handlers/mod.rdx or handlers.rdx
+M utils      // utils/mod.rdx or utils.rdx

// Re-export key types
+u ~.models.User
+u ~.models.Config
+u ~.handlers.handle_request
```

## Pattern 12: Test Organization

```redox
@cfg(test)
M tests {
    u super.*

    @test
    f test_addition() {
        v result = add(2, 3)
        assert_eq!(result, 5)
    }

    @test
    f test_error_case() {
        v result = parse_config("")
        assert!(result.is_err())
    }

    // Effect-mocked test
    @test
    f test_with_mock_io() {
        handle io {
            read_file(_) => "mock data",
        } {
            v content = read_config("test.toml")
            assert_eq!(content.unwrap(), "mock data")
        }
    }
}
```

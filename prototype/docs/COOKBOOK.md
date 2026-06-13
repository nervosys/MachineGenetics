# MAGE Cookbook

> Practical recipes for common tasks in MAGE.

---

## Recipe 1: Read a File with Error Handling

```
f read_config(path: &s) -> s!IoError
    @fx io, fs
    @req !path.is_empty() "path must not be empty"
{
    std.fs.read_to_string(path)?
}
```

## Recipe 2: Parse JSON

```
use serde.{Deserialize, Serialize};

#[derive(Deserialize)]
S Config {
    name: s,
    port: u16,
    debug: bool,
}

f load_config(path: &s) -> Config!Error
    @fx io, fs
{
    let data = std.fs.read_to_string(path)?;
    serde_json.from_str(&data)?
}
```

## Recipe 3: HTTP Request (Async)

```
+af fetch_json<T: DeserializeOwned>(url: &s) -> T!Error
    @fx net, async
{
    let body = reqwest.get(url).await?.text().await?;
    serde_json.from_str(&body)?
}
```

## Recipe 4: Parallel Processing with Iterators

```
f sum_squares(data: &[f64]) -> f64
    @fx pure
    @perf vectorize(256)
{
    data.iter().map(|x| x * x).sum()
}
```

## Recipe 5: Builder Pattern

```
+S RequestBuilder {
    url: s,
    method: s,
    headers: {s: s},
    body: s?,
}

I RequestBuilder {
    +f new(url: &s) -> Self {
        Self {
            url: url.to_string(),
            method: "GET".to_string(),
            headers: HashMap.new(),
            body: None,
        }
    }

    +f method(mut self, m: &s) -> Self {
        self.method = m.to_string();
        self
    }

    +f header(mut self, k: &s, v: &s) -> Self {
        self.headers.insert(k.to_string(), v.to_string());
        self
    }

    +f body(mut self, b: &s) -> Self {
        self.body = Some(b.to_string());
        self
    }
}
```

## Recipe 6: Custom Iterator

```
S FibIter {
    a: u64,
    b: u64,
}
    @inv self.b >= self.a

I Iterator for FibIter {
    type Item = u64;

    f next(&mut self) -> u64? {
        let val = self.a;
        let next_b = self.a + self.b;
        self.a = self.b;
        self.b = next_b;
        Some(val)
    }
}

+f fibonacci() -> FibIter {
    FibIter { a: 0, b: 1 }
}
```

## Recipe 7: Concurrent Map with Arc + Mutex

```
use std.sync.{Arc, Mutex};
use std.thread;

f parallel_map(data: &[i32], transform: f(i32) -> i32) -> [i32]~
    @fx mem
{
    let result: @Mutex<[i32]~> = @.new(Mutex.new(Vec.new()));

    let handles: [_]~ = data.iter().map(|&x| {
        let result = result.clone();
        thread.spawn(move || {
            let val = transform(x);
            result.lock().unwrap().push(val);
        })
    }).collect();

    @ h in handles { h.join().unwrap(); }
    @.try_unwrap(result).unwrap().into_inner().unwrap()
}
```

## Recipe 8: Agent Communication via Swarm Bus

```
use MAGE.swarm.{SwarmBus, Message};

+af agent_pipeline()
    @fx io, async
{
    let bus = SwarmBus.new();

    // Agent A: producer
    bus.publish("tasks", Task { id: 1, data: input });

    // Agent B: consumer
    bus.subscribe("tasks", |msg: Message<Task>| {
        let result = process(msg.payload);
        bus.publish("results", result);
    });
}
```

## Recipe 9: Contract-Driven Sorting

```
f insertion_sort(arr: &mut [i32])
    @req arr.len() > 0
    @ens arr.windows(2).all(|w| w[0] <= w[1])
    @fx pure
{
    @ i in 1..arr.len() {
        let key = arr[i];
        let mut j = i;
        @w j > 0 && arr[j - 1] > key {
            arr[j] = arr[j - 1];
            j -= 1;
        }
        arr[j] = key;
    }
}
```

## Recipe 10: Capability-Sandboxed Agent

```
use MAGE.sandbox.{SandboxManager, CapabilityToken, ResourceLimits};

f run_sandboxed_agent(agent_id: &s, code: &s) -> s!SandboxError
    @fx io, mem
{
    let limits = ResourceLimits {
        max_memory: 1024 * 1024,    // 1 MB
        max_cpu_ms: 5000,            // 5 seconds
        max_syscalls: 100,
        max_file_ops: 10,
        max_network_ops: 0,          // no network
    };

    let mgr = SandboxManager.new();
    mgr.create(agent_id, limits);
    mgr.grant(agent_id, CapabilityToken.restricted("fs.read"));

    // Execute within sandbox
    let result = mgr.execute(agent_id, code)?;
    mgr.destroy(agent_id);
    Ok(result)
}
```

## Recipe 11: Cost-Aware Code Selection

```
use MAGE.cost.{query_cost, OptLevel};

f choose_implementation(target: &s) -> s {
    let vec_cost = cost.query("Vec::push", target, Release);
    let array_cost = cost.query("stack array", target, Release);

    ?: array_cost.map_or(false, |c| c.cycles < 5) {
        "Use stack array for small, fixed-size data"
    } _ {
        "Use Vec for dynamic collections"
    }.to_string()
}
```

## Recipe 12: FFI Binding

```
use MAGE.ffi.{FfiGenerator, ForeignFunction, ForeignType};

f generate_bindings() -> s {
    let mut fg = FfiGenerator.new();
    fg.add_function(ForeignFunction {
        name: "compress".to_string(),
        params: vec![
            ("data".into(), ForeignType.Ptr(^ForeignType.Int(32))),
            ("len".into(), ForeignType.UInt(64)),
        ],
        return_type: ForeignType.Int(32),
        is_variadic: false,
    });
    fg.generate_safe_wrappers()
}
```

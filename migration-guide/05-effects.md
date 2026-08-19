# Chapter 5: Effects & Safety Migration

The biggest conceptual shift from Rust to MAGE: adding effect annotations,
removing `unsafe`, and adopting the capability system.

---

## 5.1 Identifying Effects in Existing Rust Code

Before writing any MAGE, audit your Rust code for side effects. Each side
effect maps to a MAGE effect annotation.

### Effect Discovery Commands

```bash
# Find I/O operations
grep -rn "std::io\|std::fs\|File::\|println!\|eprintln!\|stdin\|stdout" src/ --include="*.rs"

# Find network operations  
grep -rn "reqwest\|hyper\|TcpStream\|UdpSocket\|std::net" src/ --include="*.rs"

# Find random number generation
grep -rn "rand::\|thread_rng\|gen_range\|OsRng" src/ --include="*.rs"

# Find time/clock access
grep -rn "Instant::\|SystemTime\|Duration::\|std::time\|sleep" src/ --include="*.rs"

# Find environment access
grep -rn "std::env\|env::var\|env::args\|current_dir" src/ --include="*.rs"

# Find process operations
grep -rn "std::process\|Command::new\|exit\|spawn" src/ --include="*.rs"

# Find async operations
grep -rn "\.await\|tokio::spawn\|async_std::task\|futures::" src/ --include="*.rs"
```

### Effect Mapping Table

| Rust Pattern                                          | MAGE Effect |
| ----------------------------------------------------- | ------------ |
| `std::fs::*`, `File::*`, `println!`, `stdin`/`stdout` | `/ io`       |
| `reqwest::*`, `hyper::*`, `TcpStream`, `UdpSocket`    | `/ net`      |
| `rand::*`, `thread_rng()`, `OsRng`                    | `/ rng`      |
| `.await`, `tokio::spawn`, `async_std::task::*`        | `/ async`    |
| `Instant::now()`, `SystemTime::*`, `sleep()`          | `/ time`     |
| `std::env::var()`, `std::env::args()`                 | `/ env`      |
| `Command::new()`, `std::process::exit()`              | `/ process`  |

## 5.2 Adding Effect Annotations

Work bottom-up: start with leaf functions and propagate effects to callers.

### Step 1: Annotate Leaf Functions

```diff
  // Rust — no effect tracking
  pub fn read_config(path: &str) -> Result<String, io::Error> {
      fs::read_to_string(path)
  }

  // MAGE — effect declared
+ +f read_config(path: &s) -> R[s, io.Error] / io {
+     fs.read_to_string(path)
+ }
```

### Step 2: Propagate to Callers

```diff
  // Rust — caller has no effect declaration either
  pub fn load_app(config_path: &str) -> Result<App, Error> {
      let config_text = read_config(config_path)?;
      let config: Config = serde_json::from_str(&config_text)?;
      Ok(App::new(config))
  }

  // MAGE — caller inherits / io from read_config
+ +f load_app(config_path: &s) -> R[App, Error] / io {
+     v config_text = read_config(config_path)?
+     v config: Config = json.parse[Config](&config_text)?
+     Ok(App.new(config))
+ }
```

### Step 3: Verify with the Compiler

```bash
mg check --effects
```

The compiler reports any missing effect annotations:

```
error[E0401]: function `load_app` calls `read_config` which has effect `io`,
              but `load_app` does not declare `/ io`
  --> src/app.mg:5:26
   |
5  |     v config_text = read_config(config_path)?
   |                     ^^^^^^^^^^^^^^^^^^^^^^^^
   |
help: add `/ io` to the function signature
```

## 5.3 Effect Hierarchy Simplification

Apply the hierarchy rules to simplify effect lists:

```diff
  // BEFORE — redundant effects
- +af fetch_page(url: &s) -> R[s, Error] / io, net {
  // net implies io, so / io is redundant

  // AFTER — simplified
+ +af fetch_page(url: &s) -> R[s, Error] / net {
```

| If you have...     | Simplify to... | Because...              |
| ------------------ | -------------- | ----------------------- |
| `/ io, net`        | `/ net`        | net ⊃ io                |
| `/ async, agent`   | `/ agent`      | agent ⊃ async           |
| `/ io, net, async` | `/ net, async` | net ⊃ io                |
| `/ io, net, agent` | `/ net, agent` | net ⊃ io, agent ⊃ async |

## 5.4 Removing `unsafe` Blocks

Every `unsafe` block in Rust must be replaced with a capability-gated
operation in MAGE.

### Pattern 1: Raw Pointer Access

```diff
  // Rust
- unsafe {
-     let value = *raw_ptr;
- }

  // MAGE
+ v value = cap.request("mem.deref", raw_ptr)?
```

### Pattern 2: FFI Calls

```diff
  // Rust
- extern "C" {
-     fn c_function(arg: i32) -> i32;
- }
- let result = unsafe { c_function(42) };

  // MAGE
+ v result = cap.request("ffi.call", ("c_function", 42))?
```

### Pattern 3: Mutable Statics

```diff
  // Rust
- static mut COUNTER: u32 = 0;
- unsafe { COUNTER += 1; }

  // MAGE — use module-level state with env effect
+ M counter {
+     m value: u32 = 0
+
+     +f increment() / env {
+         value += 1
+     }
+
+     +f get() -> u32 / env {
+         value
+     }
+ }
```

### Pattern 4: Transmute

```diff
  // Rust
- let bytes: [u8; 4] = unsafe { std::mem::transmute(value) };

  // MAGE — use safe conversion
+ v bytes = value.to_le_bytes()
```

### Pattern 5: Inline Assembly

```diff
  // Rust
- unsafe {
-     std::arch::asm!("nop");
- }

  // MAGE — platform capability
+ cap.request("platform.asm", "nop")?
```

## 5.5 Capability System Setup

### Where the grant actually lives

**There is no `[capabilities]` section in `Forge.toml`, no `Capability` type,
and no runtime `cap.request(…)`.** Nothing of that shape exists, and it is not
missing — the design puts the grant somewhere better:

- **Reaching a capability namespace is the request.** `fs.read_to_string(p)`
  puts `fs` in the function's inferred set. There is no separate call to
  forget, and no way to reach the resource without the checker seeing it.
- **The `/ effect` annotation is the grant**, and a `pub` function must carry
  one for everything it performs. The rule is `inferred ⊆ declared`.
- **The check happens before anything runs**, not at the moment of use. A
  program that would exceed its grant does not start.

```MAGE
// The whole capability story, in one function.
+f fetch_config(url: s, cache: s) -> s / net, fs {
    v body = net.connect(url)
    fs.write(cache, body)
    body
}
```

An agent declaration adds the *approval* half — operations the role may
request but not perform unilaterally:

```MAGE
agent Configurator {
    capabilities: [net, fs]
    requires_approval: [publish]
}
```

### The capability namespaces

Twenty names, in scope everywhere, each attributing the effect beside it:

| Namespace | Effect | | Namespace | Effect |
|---|---|---|---|---|
| `io` | `io` | | `http` | `net` |
| `fs` | `fs` | | `mem` | `alloc` |
| `net` | `net` | | `log` | `io` |
| `env` | `env` | | `swarm` | `agent` |
| `time` | `time` | | `os` `sys` `process` `tools` | `proc` |
| `rng` | `rng` | | `json` `kb` `db` | *none, deliberately* |
| `llm` | `llm` | | | |
| `gpu` | `gpu` | | | |
| `agent` | `agent` | | | |

`json`, `kb` and `db` attribute nothing because no built-in kind names a
store, and inventing one would infer an effect that §11.4 then refuses in an
annotation — leaving no way to declare what you perform. For a store you want
gated, declare `effect Db { … }` and call its operations.

## 5.6 Common Migration Patterns

### Pattern: File Reader

```diff
  // Rust
  pub fn read_lines(path: &str) -> io::Result<Vec<String>> {
      let content = fs::read_to_string(path)?;
      Ok(content.lines().map(|l| l.to_string()).collect())
  }

  // MAGE
+ +f read_lines(path: &s) -> R[[s]~, io.Error] / io {
+     v content = fs.read_to_string(path)?
+     Ok(content.lines().map(|l| l.to_string()).collect())
+ }
```

### Pattern: HTTP Client

```diff
  // Rust (with reqwest)
  pub async fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T, Error> {
      let resp = reqwest::get(url).await?;
      let data = resp.json::<T>().await?;
      Ok(data)
  }

  // MAGE
+ +af fetch_json[T: DeserializeOwned](url: &s) -> R[T, Error] / net {
+     v resp = http.get(url).await?
+     v data = resp.json[T]().await?
+     Ok(data)
+ }
```

### Pattern: Random Generation

```diff
  // Rust (with rand)
  pub fn random_id() -> u64 {
      let mut rng = rand::thread_rng();
      rng.gen()
  }

  // MAGE
+ +f random_id() -> u64 / rng {
+     rng.gen()
+ }
```

## 5.7 Effect Migration Checklist

For each function being migrated:

- [ ] Identify all side effects (I/O, network, randomness, time, env, process)
- [ ] Add appropriate `/ effect` annotation
- [ ] Check if callers also need the effect propagated
- [ ] Apply hierarchy simplification (net ⊃ io, agent ⊃ async)
- [ ] Run `mg check --effects` to verify
- [ ] Replace `unsafe` with capability requests
- [ ] Declare required capabilities in Forge.toml
- [ ] Remove lifetime annotations (SKB handles them)
- [ ] Remove `PhantomData`, `Pin`, `ManuallyDrop` if present

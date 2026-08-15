# Few-Shot Prompt — Rust ↔ MAGE Translation

Use the following examples to guide translation between Rust and MAGE.

---

## Example 1: Rust → MAGE (simple function)

**Rust:**
```rust
pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
```

**MAGE:**
```MAGE
+f greet(name: &s) -> s {
    f"Hello, {name}!"
}
```

**Key changes:** `pub fn` → `+f`, `&str` → `&s`, `String` → `s`, `format!` → `f""`

---

## Example 2: Rust → MAGE (struct with derive)

**Rust:**
```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub workers: usize,
}

impl Config {
    pub fn default_config() -> Self {
        Config {
            host: "localhost".to_string(),
            port: 8080,
            workers: 4,
        }
    }
}
```

**MAGE:**
```MAGE
+S Config { host: str, port: i32, workers: i32 }

I Config {
    +f default_config() -> Config {
        @Config { host: "localhost", port: 8080, workers: 4 }
    }
}
```

**Key changes:** `use` → `u`, `::` → `.`, `#[derive]` → `@d`, `pub struct` → `+S`, `impl` → `I ~`, struct literal uses `@{`

---

## Example 3: Rust → MAGE (async with error handling)

**Rust:**
```rust
use std::fs;
use std::io;

pub async fn read_config(path: &str) -> Result<String, io::Error> {
    let content = tokio::fs::read_to_string(path).await?;
    Ok(content)
}
```

**MAGE:**
```MAGE
u std.fs
u std.io

+af read_config(path: &s) -> R[s, io.Error] / io {
    v content = tokio.fs.read_to_string(path).await?;
    R.Ok(content)
}
```

**Key changes:** `pub async fn` → `+af`, `Result<String, _>` → `R[s, _]`, `::` → `.`, added `/ io` effect

---

## Example 4: MAGE → Rust (generic with where clause)

**MAGE:**
```MAGE
+f join_all(items: [str]~) -> R[str, str] {
    guard len(items) > 0 else { ret Err("nothing to join") }
    Ok(join(items, ","))
}
```

**Rust:**
```rust
pub fn serialize_all<T>(items: &[Vec<T>]) -> Result<String, serde_json::Error>
where
    T: serde::Serialize,
{
    let json = serde_json::to_string(items)?;
    Ok(json)
}
```

**Key changes:** `+f` → `pub fn`, `[T]` → `<T>`, `[T]~` → `Vec<T>`, `R[s, _]` → `Result<String, _>`, `.` → `::`, `~>` → `where`, `v` → `let`

---

## Example 5: Rust → MAGE (trait with default method)

**Rust:**
```rust
pub trait Summary {
    fn title(&self) -> &str;
    fn author(&self) -> &str;

    fn summarize(&self) -> String {
        format!("{} by {}", self.title(), self.author())
    }
}
```

**MAGE:**
```MAGE
+T Summary {
    f title(self) -> str;
    f author(self) -> str;

    f summarize(self) -> str {
        join([self.title(), self.author()], " by ")
    }
}
```

**Key changes:** `pub trait` → `+T`, `fn` → `f`, `&str` → `&s`, `String` → `s`, `format!` → `f""`

---

Now translate the following code:

**{{direction}}:**
```{{lang}}
{{code}}
```

**{{target}}:**
```{{target_lang}}

# Redox Anti-Patterns

> Common mistakes AI agents make when generating Redox code.  
> Each entry shows the **wrong** code and the **correct** fix.

---

## Anti-Pattern 1: Using Rust Keywords

**WRONG** — Rust syntax:
```rust
pub fn greet(name: &str) -> String {
    format!("Hello, {name}")
}
```

**CORRECT** — Redox syntax:
```redox
+f greet(name: &s) -> s {
    f"Hello, {name}"
}
```

**Rule:** Never use `fn`, `pub fn`, `let`, `let mut`, `struct`, `enum`, `trait`, `impl`, `mod`, `use`, `return`, `if`, `else`, `match`, `for`.

---

## Anti-Pattern 2: Lifetime Annotations

**WRONG:**
```
f longest<'a>(a: &'a s, b: &'a s) -> &'a s {
```

**CORRECT:**
```redox
f longest(a: &s, b: &s) -> &s {
```

**Rule:** The SKB infers and proves lifetimes. Never write lifetime parameters.

---

## Anti-Pattern 3: Missing Effect Annotations

**WRONG:**
```redox
+f save(data: &s) -> R[(), Error] {
    fs.write("out.txt", data)?
}
```

**CORRECT:**
```redox
+f save(data: &s) -> R[(), Error] / io {
    fs.write("out.txt", data)?
}
```

**Rule:** Any function performing I/O, network, or other side effects MUST declare them with `/ effect`.

---

## Anti-Pattern 4: Using `::` for Paths

**WRONG:**
```
u std::io::File
v path = std::env::current_dir()
```

**CORRECT:**
```redox
u std.io.File
v path = std.env.current_dir()
```

**Rule:** Redox uses `.` (dot) as path separator, never `::`.

---

## Anti-Pattern 5: Angle-Bracket Generics

**WRONG:**
```
f first<T>(items: Vec<T>) -> Option<T> {
```

**CORRECT:**
```redox
f first[T](items: [T]~) -> ?T {
```

**Rule:** Use `[T]` for generics, never `<T>`. Also use type sugar: `[T]~` not `Vec[T]`, `?T` not `Option[T]`.

---

## Anti-Pattern 6: Turbofish Syntax

**WRONG:**
```
v nums = vec.iter().collect::<Vec<i32>>()
```

**CORRECT:**
```redox
v nums = vec.iter().collect[[i32]~]()
```

**Rule:** No turbofish. Just use `[Type]` directly on the function call.

---

## Anti-Pattern 7: Using `unsafe` Blocks

**WRONG:**
```
unsafe {
    v ptr = alloc(layout)
    // ...
}
```

**CORRECT:**
```redox
v cap = Capability.request("mem.alloc", layout)?
// Use capability-gated safe abstractions
```

**Rule:** Redox has no `unsafe`. Use the `Capability` system for privileged operations.

---

## Anti-Pattern 8: Macro Invocation with `!`

**WRONG:**
```
println!("count: {}", n)
format!("{} items", n)
vec![1, 2, 3]
```

**CORRECT:**
```redox
p"count: {n}"
f"{n} items"
[1, 2, 3]      // array literal; for Vec use [1, 2, 3].to_vec()
```

**Rule:** Redox replaces common macros with syntax sugar. No `!` invocations for these.

---

## Anti-Pattern 9: Raw Concurrency Instead of Swarm

**WRONG:**
```redox
u std.sync.{@Mutex, thread}

v handle = thread.spawn(|| {
    expensive_work()
})
v result = handle.join()?
```

**CORRECT:**
```redox
u std.agent.{Agent, Swarm}

+S Worker { input: s }

I Agent ~ Worker {
    +af execute(&!self) -> R[s, Error] / agent {
        expensive_work(&self.input)
    }
}

v swarm = Swarm.new()
swarm.spawn(Worker @{ input: s.from("data") })
v results = swarm.join_all().await?
```

**Rule:** Prefer `Swarm` for parallel work. It provides structured concurrency with capability checks.

---

## Anti-Pattern 10: Struct Literals Without `@`

**WRONG:**
```redox
v point = Point { x: 1.0, y: 2.0 }
```

**CORRECT:**
```redox
v point = Point @{ x: 1.0, y: 2.0 }
```

**Rule:** Struct literals require the `@` prefix before `{`.

---

## Anti-Pattern 11: Using `true` / `false` Literals

**WRONG:**
```redox
v active = true
v deleted = false
```

**CORRECT:**
```redox
v active = 1b
v deleted = 0b
```

**Rule:** Use `1b` for true, `0b` for false.

---

## Anti-Pattern 12: Using `if`/`else`/`match`/`for`/`return`

**WRONG:**
```
if x > 0 {
    return x
} else {
    return -x
}

for item in list {
    process(item)
}

match status {
    Status::Active => handle_active(),
    _ => handle_other(),
}
```

**CORRECT:**
```redox
? x > 0 {
    ret x
} : {
    ret -x
}

@ item ~ list {
    process(item)
}

? status {
    Status.Active => handle_active(),
    _ => handle_other(),
}
```

---

## Anti-Pattern 13: Forgetting Crate Prefix

**WRONG:**
```redox
u models.User
u handlers.process
```

**CORRECT:**
```redox
u ~.models.User
u ~.handlers.process
```

**Rule:** Use `~` for crate root in internal paths.

---

## Anti-Pattern 14: `String` and `&str` Literals

**WRONG:**
```redox
v name: String = String::from("Alice")
v greeting: &str = "hello"
```

**CORRECT:**
```redox
v name: s = s.from("Alice")
v greeting: &s = "hello"
```

**Rule:** Use `s` for `String`` and `&s` for `&str`.

---

## Anti-Pattern 15: Omitting Visibility on Public APIs

**WRONG:**
```redox
S Config {
    host: s,
    port: u16,
}

f new_config() -> Config {
    Config @{ host: s.from("localhost"), port: 8080 }
}
```

**CORRECT:**
```redox
+S Config {
    +host: s,
    +port: u16,
}

+f new_config() -> Config {
    Config @{ host: s.from("localhost"), port: 8080 }
}
```

**Rule:** Use `+` prefix for public items. Fields are private by default — use `+field_name` for public fields.

---

## Quick Self-Check

Before submitting generated Redox code, verify:

- [ ] No Rust keywords (`fn`, `let`, `struct`, `impl`, etc.)
- [ ] No lifetime annotations (`'a`, `'static`)
- [ ] No `::` paths (use `.` instead)
- [ ] No angle brackets for generics (use `[T]`)
- [ ] No `unsafe` blocks
- [ ] No macro `!` calls for `println`, `format`, `vec`
- [ ] All impure functions have `/ effect` annotations
- [ ] Struct literals use `@{ }` syntax
- [ ] Boolean literals use `1b` / `0b`
- [ ] Control flow uses `?` / `:` / `@` / `ret`

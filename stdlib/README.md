# MAGE Standard Library (`std`)

The MAGE standard library provides the foundational types, traits, and functions
for all MAGE programs. Every module is designed with **algebraic effects**,
**first-class agent primitives**, and **safety-by-default** principles at its core.

## Module Hierarchy

| Module        | Description                       | Key Types / Functions                                     |
| ------------- | --------------------------------- | --------------------------------------------------------- |
| `std.io`      | Buffered I/O with effect tracking | `Read`, `Write`, `File`, `BufReader`, `BufWriter`         |
| `std.net`     | Networking with first-class HTTP  | `TcpStream`, `UdpSocket`, `Request`, `Response`           |
| `std.fs`      | File system operations            | `read`, `write`, `create_dir`, `walk`, `Metadata`         |
| `std.col`     | Collections beyond Vec/Map        | `Map`, `Set`, `BTree`, `VecDeque`, `LinkedList`           |
| `std.sync`    | Synchronization primitives        | `Mutex`, `RwLock`, `Channel`, `Barrier`, `Semaphore`      |
| `std.async`   | Async runtime and streams         | `spawn`, `join`, `select`, `Future`, `Stream`             |
| `std.fmt`     | Formatting and display            | `Display`, `Debug`, `Formatter`, `print`, `println`       |
| `std.str`     | String utilities and regex        | `split`, `join`, `Regex`, `encode`, `decode`              |
| `std.math`    | Mathematics, RNG, and SIMD        | `sin`, `cos`, `sqrt`, `Rng`, `f32x4`, `f64x4`             |
| `std.time`    | Time measurement and formatting   | `Duration`, `Instant`, `SystemTime`, `format`             |
| `std.json`    | First-class JSON support          | `Value`, `parse`, `stringify`, `Serialize`, `Deserialize` |
| `std.env`     | Environment and directories       | `args`, `var`, `current_dir`, `home_dir`                  |
| `std.process` | Process management and signals    | `Command`, `Child`, `exit`, `Signal`                      |
| `std.agent`   | Agent primitives (MAGE-unique)   | `Agent`, `Swarm`, `Message`, `Capability`, `Lease`        |
| `std.skb`     | Safety Knowledge Base queries     | `Rule`, `Query`, `validate`, `define_rule`                |
| `std.effect`  | Algebraic effect system           | `Effect`, `perform`, `handle`, `discharge`                |
| `std.spec`    | Design-by-contract verification   | `require`, `ensure`, `invariant`, `verify`                |
| `std.test`    | Testing and property checking     | `assert_eq`, `Bencher`, `prop`, `Arbitrary`               |

## Design Principles

1. **Effects are explicit.** Any function that performs I/O, networking, RNG, or
   agent communication declares its effect signature (e.g. `/ io`, `/ net`).
   Pure functions carry no effect annotation.

2. **Batteries included.** HTTP client, JSON, regex, and async are in the
   standard library — no external crates needed for common tasks.

3. **Agent-native.** The `std.agent` module has no Rust equivalent. It provides
   `Swarm`, `Capability`, `Lease`, and `Region` for building multi-agent systems
   as a first-class concern of the language.

4. **Safety contracts.** The `std.spec` module integrates design-by-contract
   (`require`/`ensure`/`invariant`) directly into the type system, enabling
   runtime and static verification of program correctness.

5. **Concise syntax.** Standard library types use MAGE sugar where possible:
   `{K:V}` for `Map[K,V]`, `{K}` for `Set[K]`, `?T` for `Option[T]`,
   `R[T,E]` for `Result[T,E]`, `[T]~` for `Vec[T]`.

## Usage

Modules are imported with the `u` keyword:

```mg
u std.io.{Read, Write, File}
u std.net.{Request, Response}
u std.json.{parse, stringify}
u std.agent.{Agent, Swarm, Message}
```

The prelude automatically imports the most common types:
`Option`, `Result`, `Vec`, `String`, `Box`, `Arc`, `Display`, `Debug`.

## File Layout

```
stdlib/
├── README.md          ← this file
└── std/
    ├── mod.mg        ← root module (re-exports all submodules)
    ├── io.mg         ← I/O traits, File, buffered readers/writers
    ├── net.mg        ← TCP, UDP, HTTP, DNS
    ├── fs.mg         ← file system convenience functions
    ├── col.mg        ← Map, Set, BTree, VecDeque, LinkedList
    ├── sync.mg       ← Mutex, RwLock, Channel, atomics
    ├── async.mg      ← spawn, join, select, Future, Stream
    ├── fmt.mg        ← Display, Debug, Formatter, print
    ├── str.mg        ← string methods, Regex, encoding
    ├── math.mg       ← trig, exp, random, SIMD
    ├── time.mg       ← Duration, Instant, SystemTime
    ├── json.mg       ← Serialize/Deserialize, Value, parse
    ├── env.mg        ← args, env vars, directories
    ├── process.mg    ← Command, Child, exit, signals
    ├── agent.mg      ← Agent, Swarm, Message, Capability
    ├── skb.mg        ← Rule, Query, validate
    ├── effect.mg     ← Effect trait, perform, handle
    ├── spec.mg       ← require, ensure, invariant, verify
    └── test.mg       ← assertions, bench, property tests
```

## Reference

See [MAGE_ECOSYSTEM.md](../MAGE_ECOSYSTEM.md) §5 for the full design rationale
and [MAGE_SPEC.md](../MAGE_SPEC.md) for the language specification.

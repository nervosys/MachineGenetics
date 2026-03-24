# Forge.toml

`Forge.toml` is the project manifest — MechGen's equivalent of `Cargo.toml`.

## Minimal example

```toml
[package]
name = "my_app"
version = "0.1.0"
edition = "2025"
```

## Full example

```toml
[package]
name = "my_app"
version = "1.2.3"
edition = "2025"
authors = ["Alice <alice@example.com>"]
description = "An example MechGen project"
license = "MIT"
repository = "https://github.com/alice/my_app"

[dependencies]
http_framework = "0.3"
database = { version = "1.0", features = ["postgres"] }

[dev-dependencies]
mock_server = "0.2"

[build]
target = "native"
opt_level = 2

[safety]
profile = "full"
skb = true

[effects]
allowed = ["io", "net", "async"]
```

## Sections

### `[package]`

| Field         | Required | Description             |
| ------------- | -------- | ----------------------- |
| `name`        | yes      | Package name            |
| `version`     | yes      | SemVer version          |
| `edition`     | yes      | Language edition year   |
| `authors`     | no       | List of authors         |
| `description` | no       | One-line description    |
| `license`     | no       | SPDX license identifier |
| `repository`  | no       | Source code URL         |

### `[dependencies]`

Declare external packages:

```toml
[dependencies]
# Simple version
serde = "1.0"

# With features
tokio = { version = "1.0", features = ["full"] }

# Path dependency (local development)
my_lib = { path = "../my_lib" }

# Git dependency
utils = { git = "https://github.com/org/utils", branch = "main" }
```

### `[dev-dependencies]`

Dependencies used only during testing:

```toml
[dev-dependencies]
test_helpers = "0.1"
```

### `[build]`

Build configuration:

```toml
[build]
target = "native"          # native | wasm32 | wasm32-wasi
opt_level = 0              # 0 (debug) | 1 | 2 | 3 (max)
debug_info = true          # Include debug symbols
```

### `[safety]`

Configure the safety checking level:

```toml
[safety]
profile = "full"           # none | skb-only | warnings | full
skb = true                 # Enable SKB rule checking
custom_rules = "rules/"    # Path to additional SKB rules
```

| Profile    | Behavior                                 |
| ---------- | ---------------------------------------- |
| `none`     | No safety checks                         |
| `skb-only` | SKB rules enforced, no warnings          |
| `warnings` | SKB rules + compiler warnings            |
| `full`     | SKB + warnings + strict checks (default) |

### `[effects]`

Control which effects are allowed in the project:

```toml
[effects]
allowed = ["io", "net", "async"]    # Only these effects permitted
denied = ["rng"]                     # Explicitly banned effects
```

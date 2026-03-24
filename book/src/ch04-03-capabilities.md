# Capabilities

MechGen replaces `unsafe` blocks with a **capability system**. Instead of
opting out of safety, you *request specific permissions* through capabilities
that are tracked, leased, and revocable.

## The problem with `unsafe`

In Rust, `unsafe` is a binary switch: inside an `unsafe` block, *all* safety
checks are suspended. This is too coarse-grained — calling a C function should
not also permit arbitrary pointer arithmetic.

## Capabilities in MechGen

A capability is a fine-grained permission:

```mg
use std::agent::{Capability, Region};

// Request the FFI capability
let ffi_cap = Capability::request("ffi")?;

// Use it in a bounded region
Region::enter(ffi_cap, || {
    // Only FFI calls are permitted here
    // Other unsafe operations are still forbidden
});
```

### Built-in capabilities

| Capability  | Permits                          |
| ----------- | -------------------------------- |
| `ffi`       | Foreign function interface calls |
| `raw_ptr`   | Raw pointer operations           |
| `transmute` | Type transmutation               |
| `asm`       | Inline assembly                  |
| `unchecked` | Unchecked arithmetic             |
| `alloc`     | Manual memory allocation         |

### Capability scope

Capabilities have a scope, preventing them from leaking:

```mg
use std::agent::{Capability, CapabilityScope};

let cap = Capability {
    name: "ffi",
    scope: CapabilityScope::Instance,  // only this invocation
};
```

| Scope      | Persistence                      |
| ---------- | -------------------------------- |
| `Instance` | Single invocation                |
| `Kind`     | All instances of this agent type |
| `Global`   | All agents (requires consensus)  |

### Leases

Capabilities can be time-bounded with leases:

```mg
use std::agent::Lease;

let lease = cap.lease(Duration::from_secs(60))?;

if lease.is_valid() {
    // Capability is active
    do_ffi_work();
}
// Capability automatically expires after 60 seconds
```

## Why capabilities over `unsafe`

| Aspect       |   Rust `unsafe`   | MechGen Capabilities  |
| ------------ | :---------------: | :-----------------: |
| Granularity  |  All-or-nothing   |   Per-permission    |
| Duration     |   Lexical scope   | Time-bounded leases |
| Tracking     | Grep for `unsafe` |  Queryable via SKB  |
| Revocation   |   Not possible    |      Built-in       |
| Agent safety |  Not applicable   | Bounded sandboxing  |

For agent swarms, capabilities are essential: each agent operates within a
capability sandbox, preventing a misbehaving agent from corrupting shared state.

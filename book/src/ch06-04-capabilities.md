# Capabilities & Leases

Agent capabilities and leases provide **permission-bounded execution** — agents
can only perform operations they have explicit permission for.

## Capabilities

A Capability declares what an agent is allowed to do:

```mg
use std::agent::{Capability, CapabilityScope};

let cap = Capability {
    name: "file-write",
    scope: CapabilityScope::Instance,
};
```

### Declaring capabilities

Agents declare their capabilities in the `capabilities()` method:

```mg
impl Agent for FileWriter {
    pub fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability { name: "file-read", scope: CapabilityScope::Instance },
            Capability { name: "file-write", scope: CapabilityScope::Instance },
        ]
    }

    // ... handle, id, etc.
}
```

### Checking capabilities at runtime

```mg
use std::agent::Region;

pub fn safe_write(agent: &Box<dyn Agent>, path: &str, data: &str) -> Result<(), Error> / io, agent {
    let cap = Capability { name: "file-write", scope: CapabilityScope::Instance };

    Region::enter(cap, || {
        File::write(path, data)
    })
}
```

## Leases

A **Lease** is a time-bounded capability — it grants permission for a specific
duration and automatically expires:

```mg
use std::agent::Lease;

// Acquire a lease for 5 minutes
let lease = Lease {
    capability: cap,
    duration: Duration::from_secs(300),
    granted_at: Instant::now(),
};

// Check lease status
if lease.is_valid() {
    // perform privileged operation
    do_work()?;
} else {
    println!("Lease expired, requesting renewal");
    let new_lease = lease.renew(Duration::from_secs(300))?;
}

// Remaining time
let remaining = lease.remaining();

// Explicitly release
lease.release();
```

## Regions

**Regions** create bounded scopes where specific capabilities are active:

```mg
use std::agent::Region;

// Only FFI calls are permitted inside this region
let result = Region::enter(ffi_capability, || {
    call_c_library()
});

// The capability is no longer active outside the region
```

Regions compose — nested regions combine capabilities:

```mg
Region::enter(file_cap, || {
    // Can read/write files here
    Region::enter(net_cap, || {
        // Can read/write files AND make network calls here
    });
    // Only file access here
});
```

## Why this matters

In an agent swarm, not every agent should have unlimited access:

| Agent Role    | Capabilities                               |
| ------------- | ------------------------------------------ |
| Code Reader   | `file-read`                                |
| Code Writer   | `file-read`, `file-write`                  |
| Build Agent   | `file-read`, `file-write`, `process-spawn` |
| Network Agent | `net-connect`, `dns-resolve`               |
| Admin Agent   | All capabilities                           |

Capabilities prevent a code-reader agent from accidentally (or maliciously)
modifying files. Leases ensure that even privileged operations are time-bounded
and revocable. This is the foundation of safe multi-agent systems.

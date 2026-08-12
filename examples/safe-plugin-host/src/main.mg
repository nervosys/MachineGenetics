// safe-plugin-host — running untrusted code under a capability budget.
//
// `forge run` evaluates `main` and prints its result. Demonstrates:
//   - capabilities as data: a plugin gets a grant, and every request is checked
//     against it rather than against a comment
//   - `agent` declarations with `requires_approval`, which is the same idea the
//     language applies to its own agents
//   - `Result<T, E>` with a denial enum, so a refusal carries its reason
//   - `guard` with an explicit `return` for the deny path
//   - `/ fs` and `/ net` on the host functions a plugin can reach, so the
//     signatures show what a plugin could possibly do
//
// The security property being modelled is that a plugin cannot widen its own
// grant: `run_plugin` consults the grant it was given, and the effectful
// helpers are only reachable through the checked path.
//
// Run:  forge run        (or:  mage-parse --eval src/main.mg main)

// ── Capabilities ─────────────────────────────────────────────────────

enum Capability {
    ReadFile,
    WriteFile,
    NetFetch,
    Spawn,
}

fn capability_name(cap: Capability) -> String {
    ?= cap {
        Capability.ReadFile => "fs.read",
        Capability.WriteFile => "fs.write",
        Capability.NetFetch => "net.fetch",
        Capability.Spawn => "proc.spawn",
    }
}

// A grant is the closed set of what a plugin may do, plus the operations it may
// *ask* for but not perform on its own. Splitting the two is the whole point:
// "denied" and "needs a human" are different answers.
struct Grant {
    plugin: String,
    allowed: [String]~,
    needs_approval: [String]~,
}

// Why a request was refused. A bare `false` would lose this, and the caller
// would have to guess whether to prompt the operator or give up.
enum Denial {
    NotGranted(String),
    NeedsApproval(String),
    BudgetExhausted(i32),
}

fn denial_text(denial: Denial) -> String {
    ?= denial {
        Denial.NotGranted(cap) => f"denied {cap}",
        Denial.NeedsApproval(cap) => f"approval required for {cap}",
        Denial.BudgetExhausted(spent) => f"budget exhausted at {spent}",
    }
}

// ── The host's own agents ────────────────────────────────────────────

// The host is itself described with the capability vocabulary the language
// checks. `--check` reports these as Verified; every name is in the known set.
agent Loader {
    capabilities: [read_source, query_types]
}

agent Sandbox {
    capabilities: [read_source, alloc_heap]
    requires_approval: [exec, write_source]
}

// ── The gate ─────────────────────────────────────────────────────────

// One function decides every request. Everything effectful below is reachable
// only through a path that has been past this, which is what makes the grant
// meaningful rather than advisory.
fn check(grant: Grant, cap: Capability) -> Result<String, Denial> {
    val name = capability_name(cap)
    ? contains(grant.allowed, name) {
        Ok(name)
    } : {
        ? contains(grant.needs_approval, name) {
            Err(Denial.NeedsApproval(name))
        } : {
            Err(Denial.NotGranted(name))
        }
    }
}

// ── Host services ────────────────────────────────────────────────────

// The effectful leaves. Their annotations are the honest summary of what a
// plugin can cause to happen, and they propagate up through `perform`.

fn host_read(path: String) -> String / fs {
    ?= path {
        "config.toml" => "mode=strict",
        "notes.txt" => "hello from the host",
        _ => "",
    }
}

fn host_write(path: String, body: String) -> usize / fs {
    len(chars(f"{path}{body}"))
}

fn host_fetch(url: String) -> String / net {
    ?= url {
        "https://ok.example" => "200 OK",
        _ => "503 unavailable",
    }
}

// ── Requests ─────────────────────────────────────────────────────────

struct Request {
    cap: Capability,
    argument: String,
    cost: i32,
}

// Performing a request is gated twice: by the grant, and by the remaining
// budget. Both refusals are values, so the caller can tell them apart.
fn perform(grant: Grant, request: Request, remaining: i32) -> Result<String, Denial> / fs, net {
    // The deny path returns explicitly. A `guard` whose else block merely
    // evaluates to a value falls through and the function keeps going, which
    // would run the request it was supposed to stop.
    guard request.cost <= remaining else {
        return Err(Denial.BudgetExhausted(remaining))
    }

    ?= check(grant, request.cap) {
        Err(denial) => Err(denial),
        Ok(_) => Ok(dispatch(request)),
    }
}

// Only reached after `check` succeeded.
fn dispatch(request: Request) -> String / fs, net {
    ?= request.cap {
        Capability.ReadFile => host_read(request.argument),
        Capability.WriteFile => f"wrote {host_write(request.argument, "x")} bytes",
        Capability.NetFetch => host_fetch(request.argument),
        Capability.Spawn => "spawn is never dispatched",
    }
}

// ── Running a plugin ─────────────────────────────────────────────────

struct Outcome {
    line: String,
    spent: i32,
}

// A plugin is a list of requests. Running it is a `scan` — a fold that keeps
// every intermediate state — so the budget is threaded explicitly and each step
// is reportable, rather than only the final total surviving.
//
// `scan` emits its seed as the first element, so the transcript opens with the
// "start" line and has one more entry than there are requests. Worth checking
// rather than assuming: the two conventions differ by exactly one line, and the
// off-by-one is invisible until you count.
fn run_plugin(grant: Grant, requests: [Request]~, budget: i32) -> [String]~ / fs, net {
    val steps = scan(
        requests,
        @Outcome { line: f"{grant.plugin} start", spent: 0 },
        fn(state, request) => step(grant, state, request, budget),
    )
    map(steps, fn(outcome) => outcome.line)
}

fn step(grant: Grant, state: Outcome, request: Request, budget: i32) -> Outcome / fs, net {
    val remaining = budget - state.spent
    ?= perform(grant, request, remaining) {
        Ok(result) => @Outcome {
            line: f"{capability_name(request.cap)} -> {result}",
            spent: state.spent + request.cost,
        },
        // A refusal costs nothing: the budget is for work done, and charging
        // for denied work would let a caller drain a plugin by asking for
        // things it is not allowed to do.
        Err(denial) => @Outcome {
            line: f"{capability_name(request.cap)} -> {denial_text(denial)}",
            spent: state.spent,
        },
    }
}

// ── Entry point ──────────────────────────────────────────────────────

pub fn main() -> String / fs, net {
    val grant = @Grant {
        plugin: "linter",
        allowed: ["fs.read", "net.fetch"],
        needs_approval: ["fs.write"],
    }

    val requests = [
        @Request { cap: Capability.ReadFile, argument: "config.toml", cost: 1 },
        @Request { cap: Capability.NetFetch, argument: "https://ok.example", cost: 2 },
        @Request { cap: Capability.WriteFile, argument: "out.txt", cost: 1 },
        @Request { cap: Capability.Spawn, argument: "sh", cost: 1 },
        @Request { cap: Capability.ReadFile, argument: "notes.txt", cost: 9 },
    ]

    join(run_plugin(grant, requests, 5), "; ")
}

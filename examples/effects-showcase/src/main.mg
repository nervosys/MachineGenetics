// effects-showcase — what the effect system actually enforces.
//
// `forge run` evaluates `main` and prints its result. Demonstrates:
//   - pure functions, which carry no annotation at all
//   - effectful leaves: `/ fs`, `/ net`, `/ time`, `/ rng`
//   - propagation — an effect is inherited by every caller, transitively, so
//     `main` names the union of everything reachable from it
//   - where that is *enforced*: private functions infer their effects, public
//     ones must declare them. The check is at the module boundary.
//   - composition, both `/ io, net` and `/ io + net` (the parser takes either)
//   - a custom effect: any name that is not built in becomes one
//   - `effect` declarations
//   - `guard` for early exit, and `defer`
//
// The rule the checker enforces is *under*-declaration: a published function
// may not perform an effect it did not declare. It may declare one it never
// performs, so an annotation is an upper bound rather than an exact
// description. Every claim in this file was checked by running
// `mage-parse --check` on an edited copy, not by reading the compiler.
//
// Not shown, because it does not exist yet: effect *handlers*. There is no
// `handle` form — `mage-parse --check` reports `unresolved name: handle`. An
// effect can be declared, annotated, inferred, and enforced, but not discharged.
//
// Run:  forge run        (or:  mage-parse --eval src/main.mg main)

// ── Declaring an effect ──────────────────────────────────────────────

// An `effect` block names an effect and the operations that belong to it. The
// trailing semicolon on each signature is required.
effect Audit {
    fn record(entry: String);
}

// ── Pure core ────────────────────────────────────────────────────────

// No annotation means no effects, and the checker holds this to it: calling
// anything effectful from here is an error, not a warning.
fn severity(status: i32) -> String {
    ? status >= 500 { "error" } : { ? status >= 400 { "warn" } : { "ok" } }
}

// `last` returns `?i32`, not `i32` — an empty list has no last element, and the
// vocabulary makes that a type rather than a convention. The `None` arm is the
// caller's to answer, so there is no such thing as forgetting it.
fn summarize(counts: [i32]~) -> String {
    ?= last(sort(counts)) {
        Some(worst) => f"{len(counts)} samples, worst {worst}",
        None => "no samples",
    }
}

// ── Effectful leaves ─────────────────────────────────────────────────

// Each of these stands in for a real capability. What matters is the
// annotation: these are the only places an effect enters the program.

fn read_config(path: String) -> String / fs {
    ?= path {
        "app.toml" => "500,404,200",
        _ => "",
    }
}

fn probe_endpoint(host: String) -> i32 / net {
    ? host == "down.example" { 503 } : { 200 }
}

fn now_ms() -> i32 / time {
    1_700_000_000
}

// The built-in kind is `rng`, not `rand`. The difference is visible: a name
// that is not built in still works, but shows up unfolded in diagnostics
// (`performs undeclared effects: [rand]` rather than `[Rng]`).
fn jitter() -> i32 / rng {
    17
}

// `db` is not one of the built-in effect kinds (`io`, `net`, `fs`, `async`,
// `alloc`, `panic`, `ffi`, `env`, `time`, `gpu`, `npu`, `llm`, `evolve`,
// `learn`, `rng` — see `Effect::from_name` in `hir.rs`). Any other name becomes
// a custom effect and is tracked the same way, so the system is open, not fixed.
fn persist(record: String) -> usize / db {
    len(chars(record))
}

// ── Propagation ──────────────────────────────────────────────────────

// `/ fs` here is inherited, not chosen: this function performs no file access
// itself, it only calls something that does.
//
// It is private, so the annotation is optional — removing it is accepted, and
// `--check` still reports the inferred set as `f load_statuses: { FS }`. Make
// it `pub` without the annotation and the same edit becomes an error:
//     function `load_statuses` performs undeclared effects: [FS]
// That is the whole boundary rule: private functions infer, published ones
// declare. Effects are checked where they escape the module, not everywhere.
fn load_statuses(path: String) -> [String]~ / fs {
    val raw = read_config(path)
    // `guard` takes an early exit when the condition fails. The else block must
    // `return` — a block that merely evaluates to a value falls through and the
    // function continues, which is silent rather than loud.
    guard len(chars(raw)) > 0 else { return [] }
    split(raw, ",")
}

// Two effects, written with a comma.
fn check_host(host: String, path: String) -> String / net, fs {
    val configured = load_statuses(path)
    val live = probe_endpoint(host)
    f"{len(configured)} configured, live {severity(live)}"
}

// The same set, written with `+`. Both spellings parse and mean the same thing.
fn stamp(host: String) -> String / net + time + rng {
    val at = now_ms() + jitter()
    f"{host}@{at}"
}

// ── Composition ──────────────────────────────────────────────────────

// `audit` declares `/ db` because `persist` performs it. `defer` schedules an
// expression to run when the block finishes; it is evaluated for its effect,
// and its value is discarded rather than returned.
fn audit(entry: String) -> String / db {
    defer { persist(entry) }
    f"audited {len(chars(entry))} chars"
}

// ── Entry point ──────────────────────────────────────────────────────

// The union of everything reachable: `fs` and `net` from `check_host`, `time`
// and `rng` from `stamp`, `db` from `audit`. Drop any one of them and the
// checker names the missing effect and the function that performs it.
pub fn main() -> String / fs, net, time, rng, db {
    val statuses = load_statuses("app.toml")
    val missing = load_statuses("nope.toml")

    val health = check_host("up.example", "app.toml")
    val stamped = stamp("up.example")
    val logged = audit(health)

    val codes = [200, 404, 503]
    val report = map(codes, fn(code) => severity(code))

    join(
        [
            f"statuses={len(statuses)} missing={len(missing)}",
            health,
            stamped,
            logged,
            join(report, "/"),
            summarize(codes),
        ],
        "; ",
    )
}

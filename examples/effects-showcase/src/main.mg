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
//   - a custom effect, declared by an `effect` block
//   - performing an operation, and `handle … with` to *discharge* the effect
//   - `guard` for early exit, and `defer`
//
// The rule the checker enforces is *under*-declaration: a published function
// may not perform an effect it did not declare. It may declare one it never
// performs, so an annotation is an upper bound rather than an exact
// description. Every claim in this file was checked by running
// `mage-parse --check` on an edited copy, not by reading the compiler.
//
// The elimination rule is `handle { … } with E { … }`: it removes an effect
// from the block it wraps, so a function can be *pure* despite calling
// something effectful. Handlers do not resume — an operation call dispatches to
// its arm and returns like an ordinary call.
//
// Run:  forge run        (or:  mage-parse --eval src/main.mg main)

// ── Declaring an effect ──────────────────────────────────────────────

// An `effect` block names an effect and the operations that belong to it. The
// trailing semicolon on each signature is required.
//
// A declaration is not optional: an effect annotation naming nothing is an
// error. It used to be accepted, which meant `/ nte` was not a misspelling of
// `/ net` but a silently different effect that matched nothing.
//
// The declaration is `Audit`, the annotation is `/ audit`. The two spellings
// are matched case-insensitively.
effect Audit {
    fn record(entry: String) -> usize;
}

// `db` is a custom effect too — it is not one of the built-in kinds (`io`,
// `net`, `fs`, `async`, `alloc`, `panic`, `ffi`, `env`, `time`, `gpu`, `npu`,
// `llm`, `evolve`, `learn`, `rng` — see `Effect::from_name` in `hir.rs`). It
// carries no operations, which is allowed: a bare `effect` block is how you
// name an effect you only want to track, not perform through.
effect Db {}

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

// Custom effects propagate and are enforced exactly like built-in ones, so the
// system is open rather than fixed.
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

// ── Performing an effect, and discharging it ─────────────────────────

// `Audit.record(...)` *performs* the operation. That is what puts `audit` in
// this function's effect set — the annotation is checked against it, not the
// source of it.
fn transcribe(entry: String) -> usize / audit {
    Audit.record(entry)
}

// And here it is discharged. `handle { … } with Audit { … }` removes `audit`
// from the block, so this function is **pure** even though `transcribe` is not:
// `--check` reports `f summarize_audit: pure`.
//
// The subtraction is per-block, not per-function. A second, unhandled call to
// `transcribe` outside this `handle` would still be reported — handling one
// call does not launder the rest.
//
// Whatever the arm itself does is honestly attributed: make `record` call
// `persist` and this function becomes `/ db`, because that is what it now
// performs. A handler exchanges one effect for the effects of handling it.
fn summarize_audit(entry: String) -> String {
    val n = handle { transcribe(entry) } with Audit {
        record(e) => len(chars(e))
    }
    f"recorded {n} chars"
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
    val transcribed = summarize_audit(health)

    val codes = [200, 404, 503]
    val report = map(codes, fn(code) => severity(code))

    join(
        [
            f"statuses={len(statuses)} missing={len(missing)}",
            health,
            stamped,
            logged,
            transcribed,
            join(report, "/"),
            summarize(codes),
        ],
        "; ",
    )
}

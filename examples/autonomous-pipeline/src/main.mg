// autonomous-pipeline — a specification decomposed into an ordered plan.
//
// `forge run` evaluates `main` and prints its result. Demonstrates:
//   - a dependency DAG as data, and a topological order computed from it
//     rather than assumed by writing the tasks in the right sequence
//   - a cycle being *detected*: the ordering reports what it could not place,
//     instead of looping or silently dropping it
//   - `var` and `while` where an algorithm genuinely iterates, alongside the
//     pure vocabulary everywhere else
//   - a token budget threaded through a `fold`, so spending is explicit
//   - a memo table (`{String: String}`) so a repeated stage is not re-run
//   - `/ llm` on the one function that would call a model
//
// Run:  forge run        (or:  mage-parse --eval src/main.mg main)

// ── The plan ─────────────────────────────────────────────────────────

enum Stage {
    Plan,
    Generate,
    Verify,
    Emit,
}

fn stage_name(stage: Stage) -> String {
    ?= stage {
        Stage.Plan => "plan",
        Stage.Generate => "generate",
        Stage.Verify => "verify",
        Stage.Emit => "emit",
    }
}

struct Task {
    id: String,
    stage: Stage,
    deps: [String]~,
    tokens: i32,
}

// ── Topological ordering ─────────────────────────────────────────────

// A task is ready when every dependency is already placed. This is the only
// predicate the ordering needs, and keeping it separate makes the loop below
// short enough to read.
fn ready(task: Task, placed: [String]~) -> bool {
    all(task.deps, fn(dep) => contains(placed, dep))
}

// Kahn's algorithm, written with `var`/`while` because it genuinely iterates:
// each pass places every task whose dependencies are satisfied.
//
// The loop stops when a pass places nothing. That is what makes a cycle
// terminate: the remaining tasks can never become ready, so `remaining` is
// returned to the caller as unplaceable rather than spun on forever.
fn order(tasks: [Task]~) -> ([String]~, [String]~) {
    // Annotated: an empty list literal has nothing to infer an element type
    // from, and every later assignment depends on it being `[String]~`.
    var placed: [String]~ = []
    var remaining = tasks
    var progress = 1b

    while progress {
        val available = filter(remaining, fn(task) => ready(task, placed))
        progress = len(available) > 0
        placed = flatten([placed, map(available, fn(task) => task.id)])
        // What is left is what has not been *placed* — not what is not yet
        // ready. Filtering on readiness drops a task that became ready during
        // this very pass without ever placing it, which silently loses work:
        // the first version of this reported three of five tasks unplaceable
        // on an acyclic graph.
        remaining = filter(remaining, fn(task) => !contains(placed, task.id))
    }

    // Bound rather than written inline: a `(` immediately after a block is
    // parsed as *calling* that block, so `while … { … }` followed by
    // `(placed, …)` reads as `while(…)(placed, …)` and reports the tuple as a
    // call on `()`. Any statement between the two, or a binding like this one,
    // separates them.
    val result = (placed, map(remaining, fn(task) => task.id))
    result
}

// ── Budget ───────────────────────────────────────────────────────────

struct Spend {
    used: i32,
    ran: [String]~,
    skipped: [String]~,
}

// Spending is threaded through a fold rather than mutated, so the budget at any
// point is a value that was computed, not a variable that was hopefully updated.
fn plan_spend(tasks: [Task]~, budget: i32) -> Spend {
    fold(
        tasks,
        @Spend { used: 0, ran: [], skipped: [] },
        fn(spend, task) => ? spend.used + task.tokens <= budget {
            @Spend {
                used: spend.used + task.tokens,
                ran: flatten([spend.ran, [task.id]]),
                skipped: spend.skipped,
            }
        } : {
            // Over budget: skipped, and the spend is unchanged. A task that did
            // not run must not be charged for, or the remaining budget lies.
            @Spend {
                used: spend.used,
                ran: spend.ran,
                skipped: flatten([spend.skipped, [task.id]]),
            }
        },
    )
}

// ── Memoised generation ──────────────────────────────────────────────

// The one function that would call a model, so the one carrying `/ llm`.
fn generate(prompt: String) -> String / llm {
    f"<{len(words(prompt))} tokens for {prompt}>"
}

// A stage is only worth re-running if its input changed. `memo` is consulted
// first, so a repeated prompt costs a lookup instead of a model call.
fn generate_cached(memo: {String: String}, prompt: String) -> String / llm {
    ? contains(keys(memo), prompt) {
        memo[prompt]
    } : {
        generate(prompt)
    }
}

// ── Reporting ────────────────────────────────────────────────────────

fn describe(task: Task) -> String {
    f"{task.id}({stage_name(task.stage)},{task.tokens})"
}

// ── Entry point ──────────────────────────────────────────────────────

pub fn main() -> String / llm {
    // "build a REST API for user management", decomposed. The declaration order
    // is deliberately not the execution order — the ordering below has to
    // discover that `emit` cannot precede `verify`.
    val tasks = [
        @Task { id: "emit", stage: Stage.Emit, deps: ["verify"], tokens: 200 },
        @Task { id: "verify", stage: Stage.Verify, deps: ["handlers", "schema"], tokens: 400 },
        @Task { id: "handlers", stage: Stage.Generate, deps: ["schema"], tokens: 900 },
        @Task { id: "schema", stage: Stage.Generate, deps: ["spec"], tokens: 600 },
        @Task { id: "spec", stage: Stage.Plan, deps: [], tokens: 150 },
    ]

    val ordering = order(tasks)
    val sequence = ?= ordering { (placed, _) => placed }
    val stuck = ?= ordering { (_, blocked) => blocked }

    // The same list with one dependency reversed, so `schema` and `spec` wait
    // on each other. Nothing is placeable, and the ordering says so.
    val cyclic = [
        @Task { id: "schema", stage: Stage.Generate, deps: ["spec"], tokens: 600 },
        @Task { id: "spec", stage: Stage.Plan, deps: ["schema"], tokens: 150 },
    ]
    val cycle_report = ?= order(cyclic) { (_, blocked) => blocked }

    val spend = plan_spend(tasks, 1_500)

    val memo = { "spec": "<cached spec>" }
    val fresh = generate_cached(memo, "handlers for user management")
    val cached = generate_cached(memo, "spec")

    join(
        [
            f"order {join(sequence, " -> ")}",
            f"unplaceable {len(stuck)}",
            f"cycle {join(cycle_report, ",")}",
            f"ran {len(spend.ran)} used {spend.used} skipped {join(spend.skipped, ",")}",
            fresh,
            cached,
            join(map(tasks, fn(task) => describe(task)), " "),
        ],
        "; ",
    )
}

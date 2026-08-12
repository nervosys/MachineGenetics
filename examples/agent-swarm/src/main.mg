// agent-swarm — declaring agents, assigning work, reaching consensus.
//
// `forge run` evaluates `main` and prints its result. Demonstrates:
//   - `agent` declarations with capabilities and approval gates, and the
//     capability verification `mage-parse --check` runs over them
//   - a `swarm` block: which agent, how many, what topology, what consensus
//   - `trait` + `impl Trait for T` dispatch — each role scores a task its own way
//   - assignment as `group`, then `map`/`filter`/`fold` over the groups
//   - majority consensus computed rather than asserted
//   - the `/ llm` effect, which is what makes a call to a model a tracked
//     capability instead of an ordinary function call
//
// Capabilities are checked. `--check` reports each agent as Verified or
// Partial: every name below is in the known set (`verify.rs`), so all three
// verify. Change one to something unrecognised and it downgrades to Partial —
// which is a report, not an error, so it will not fail a build.
//
// Run:  forge run        (or:  mage-parse --eval src/main.mg main)

// ── Agents ───────────────────────────────────────────────────────────

// Capabilities are bare identifiers, not strings. `requires_approval` names the
// operations this agent may request but not perform unilaterally.
agent Reader {
    capabilities: [read_source, query_types]
}

agent Critic {
    capabilities: [read_source, emit_diagnostics]
    requires_approval: [write_source]
}

agent Fixer {
    capabilities: [read_source, write_source]
    requires_approval: [exec]
}

// A swarm names its member agent, its size, and how members are wired and how
// their answers are combined. `topology` and `consensus` are identifiers too.
swarm ReviewTeam {
    agent: Critic
    size: 3
    topology: mesh
    consensus: majority
}

// ── Work ─────────────────────────────────────────────────────────────

enum Role {
    Read,
    Critique,
    Fix,
}

struct Task {
    id: String,
    title: String,
    role: Role,
    lines: i32,
}

fn role_name(role: Role) -> String {
    ?= role {
        Role.Read => "reader",
        Role.Critique => "critic",
        Role.Fix => "fixer",
    }
}

// ── Role-specific scoring, via a trait ───────────────────────────────

// One trait, three implementations. `q.score(task)` picks the implementation
// from the receiver's type, so adding a role means adding an impl and nothing
// else — no dispatch table to keep in sync.
trait Scorer {
    fn score(&self, task: Task) -> i32;
}

struct ReadScorer {
    weight: i32,
}

struct CritiqueScorer {
    weight: i32,
    strictness: i32,
}

struct FixScorer {
    weight: i32,
}

// Reading is cheap and scales with size.
impl Scorer for ReadScorer {
    fn score(&self, task: Task) -> i32 {
        self.weight * task.lines
    }
}

// Critique costs more per line, and a stricter critic costs more still.
impl Scorer for CritiqueScorer {
    fn score(&self, task: Task) -> i32 {
        self.weight * task.lines * self.strictness
    }
}

// Fixing has a fixed setup cost on top of the per-line cost.
impl Scorer for FixScorer {
    fn score(&self, task: Task) -> i32 {
        self.weight * task.lines + 100
    }
}

// ── The effectful boundary ───────────────────────────────────────────

// `/ llm` is a built-in effect kind. Every caller inherits it, so the path from
// `main` to a model call is visible in the signatures rather than buried.
fn ask_model(prompt: String) -> bool / llm {
    // Stands in for inference: a verdict that depends on the prompt, so the
    // consensus below has something real to disagree about.
    len(words(prompt)) % 2 == 0
}

// Three members of the swarm, each asked a slightly different question. This is
// where `size: 3` and `consensus: majority` become code.
fn swarm_verdict(task: Task) -> bool / llm {
    val prompts = [
        f"is {task.title} correct",
        f"is {task.title} clear enough to merge",
        f"does {task.title} need another pass before merge",
    ]
    val votes = map(prompts, fn(prompt) => ask_model(prompt))
    val yes = len(filter(votes, fn(vote) => vote))
    // Majority of three. Computed from the votes, not assumed from the swarm
    // declaration — the declaration says what to do, this does it.
    yes * 2 > len(votes)
}

// ── Assignment ───────────────────────────────────────────────────────

// `group` turns a flat task list into `{role: [Task]}`, which is exactly the
// assignment: each key is an agent role, each value is that agent's queue.
fn assign(tasks: [Task]~) -> {String: [Task]~} {
    group(tasks, fn(task) => role_name(task.role))
}

// Cost of a queue, dispatching through the trait for each task's role.
fn queue_cost(tasks: [Task]~) -> i32 {
    fold(tasks, 0, fn(total, task) => total + cost_of(task))
}

fn cost_of(task: Task) -> i32 {
    ?= task.role {
        Role.Read => @ReadScorer { weight: 1 }.score(task),
        Role.Critique => @CritiqueScorer { weight: 2, strictness: 3 }.score(task),
        Role.Fix => @FixScorer { weight: 4 }.score(task),
    }
}

// ── Reporting ────────────────────────────────────────────────────────

fn describe_queue(role: String, tasks: [Task]~) -> String {
    f"{role}: {len(tasks)} task(s), cost {queue_cost(tasks)}"
}

// ── Entry point ──────────────────────────────────────────────────────

pub fn main() -> String / llm {
    val tasks = [
        @Task { id: "T1", title: "parse header", role: Role.Read, lines: 40 },
        @Task { id: "T2", title: "review parser diff", role: Role.Critique, lines: 25 },
        @Task { id: "T3", title: "repair failing test", role: Role.Fix, lines: 12 },
        @Task { id: "T4", title: "read lexer", role: Role.Read, lines: 90 },
    ]

    val queues = assign(tasks)
    val ordered = sort(keys(queues))
    val lines = map(ordered, fn(role) => describe_queue(role, queues[role]))

    val approved = filter(tasks, fn(task) => swarm_verdict(task))
    val names = map(approved, fn(task) => task.id)

    val total = fold(tasks, 0, fn(acc, task) => acc + cost_of(task))

    join(
        [join(lines, " | "), f"approved: {join(names, ",")}", f"total cost {total}"],
        "; ",
    )
}

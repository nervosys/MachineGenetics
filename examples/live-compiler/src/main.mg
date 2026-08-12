// live-compiler — hot patches, and rolling one back when it regresses.
//
// `forge run` evaluates `main` and prints its result. Demonstrates:
//   - versioned function state, so "roll back" means returning to a value that
//     was kept rather than recompiling from scratch
//   - a patch that is *accepted or rejected on evidence*: the test count after
//     the patch decides, and a regression restores the previous version
//   - `Result<T, E>` for compile failures, with the diagnostic carried along
//   - repair candidates scored and chosen with `fold`, and `?A` where no
//     candidate applies
//   - `/ fs` for watching sources, `/ llm` for proposing a repair
//
// The invariant worth stating: `apply` never returns a state worse than the one
// it was given. Every path either improves the passing count or keeps the old
// version, which is what makes an automatic patcher safe to leave running.
//
// Run:  forge run        (or:  mage-parse --eval src/main.mg main)

// ── Versions ─────────────────────────────────────────────────────────

// `passing` is `usize` because it comes from `len`, which returns `usize` —
// declaring it `i32` is a type error, not a widening.
struct Version {
    revision: i32,
    body: String,
    passing: usize,
}

// The running program: the live version plus the one to fall back to. Keeping
// the previous version *in* the state is what makes rollback total — there is
// no case where a rollback has nothing to roll back to.
struct Runtime {
    name: String,
    live: Version,
    previous: Version,
    rollbacks: i32,
}

// ── Diagnostics and repair ───────────────────────────────────────────

enum Fault {
    ParseError(String),
    TypeError(String),
    TestFailure(i32),
}

fn fault_text(fault: Fault) -> String {
    ?= fault {
        Fault.ParseError(site) => f"parse: {site}",
        Fault.TypeError(what) => f"type: {what}",
        Fault.TestFailure(count) => f"{count} test(s) failing",
    }
}

struct Repair {
    description: String,
    confidence: i32,
    expected_passing: usize,
}

// ── Compiling ────────────────────────────────────────────────────────

// A candidate compile: either a new version, or the reason there isn't one.
// `Result` is what stops a failed compile from being mistaken for an empty one.
fn compile(name: String, revision: i32, body: String) -> Result<Version, Fault> {
    ? len(chars(body)) == 0 {
        Err(Fault.ParseError(f"{name}:{revision} empty body"))
    } : {
        ? contains(words(body), "??") {
            Err(Fault.TypeError(f"{name}:{revision} unresolved placeholder"))
        } : {
            Ok(@Version {
                revision: revision,
                body: body,
                passing: len(words(body)),
            })
        }
    }
}

// ── Applying a patch ─────────────────────────────────────────────────

// The whole safety argument lives here. A patch is adopted only if it passes at
// least as many tests as the version it replaces; otherwise the live version is
// left alone and the rollback is counted.
fn apply(state: Runtime, body: String) -> Runtime {
    ?= compile(state.name, state.live.revision + 1, body) {
        // A compile failure is not a rollback — nothing was ever swapped in.
        Err(_) => state,
        Ok(candidate) => ? candidate.passing >= state.live.passing {
            @Runtime {
                name: state.name,
                live: candidate,
                previous: state.live,
                rollbacks: state.rollbacks,
            }
        } : {
            // Regression: keep the live version, and record that a patch was
            // rejected. `previous` is untouched, so the fallback stays valid.
            @Runtime {
                name: state.name,
                live: state.live,
                previous: state.previous,
                rollbacks: state.rollbacks + 1,
            }
        },
    }
}

// An explicit rollback, for the case where the live version is bad for a reason
// the test count did not catch.
fn rollback(state: Runtime) -> Runtime {
    @Runtime {
        name: state.name,
        live: state.previous,
        previous: state.previous,
        rollbacks: state.rollbacks + 1,
    }
}

// ── Choosing a repair ────────────────────────────────────────────────

// The most confident candidate, or `None`. `reduce` and `first` both return
// `?A` for the same reason: an empty candidate list is a real outcome, and the
// caller has to say what it means.
fn best_repair(candidates: [Repair]~) -> ?Repair {
    // Seeded from `first`, not from `None`: a bare `None` seed gives the
    // accumulator no type to infer from, and makes every step unwrap an option
    // that cannot be empty. The empty case is answered once, here.
    ?= first(candidates) {
        None => None,
        Some(head) => Some(fold(
            candidates,
            head,
            fn(best, candidate) => ? candidate.confidence > best.confidence {
                candidate
            } : {
                best
            },
        )),
    }
}

// ── The effectful edges ──────────────────────────────────────────────

// Watching the tree is the filesystem's business, so it carries `/ fs`.
fn watch(path: String) -> [String]~ / fs {
    ?= path {
        "src" => ["one two three", "one two three four", "one", "one two ?? four"],
        _ => [],
    }
}

// Proposing a repair is a model call, so it carries `/ llm`.
fn propose(fault: Fault) -> [Repair]~ / llm {
    ?= fault {
        Fault.ParseError(_) => [
            @Repair { description: "restore last parse", confidence: 80, expected_passing: 3 },
        ],
        Fault.TypeError(_) => [
            @Repair { description: "infer placeholder", confidence: 55, expected_passing: 4 },
            @Repair { description: "revert placeholder", confidence: 70, expected_passing: 3 },
        ],
        Fault.TestFailure(_) => [],
    }
}

// ── Entry point ──────────────────────────────────────────────────────

pub fn main() -> String / fs, llm {
    val seed = @Version { revision: 0, body: "one two", passing: 2 }
    val start = @Runtime { name: "handler", live: seed, previous: seed, rollbacks: 0 }

    // Four edits arrive. The third shrinks the passing count and must be
    // rejected; the fourth does not compile at all.
    val edits = watch("src")
    val settled = fold(edits, start, fn(state, body) => apply(state, body))

    val reverted = rollback(settled)

    // The unresolved-placeholder edit, compiled on its own so the fault is
    // visible rather than swallowed by the fold.
    val broken = ?= compile("handler", 9, "one two ?? four") {
        Ok(version) => f"unexpectedly compiled at r{version.revision}",
        Err(fault) => fault_text(fault),
    }

    val repair = ?= best_repair(propose(Fault.TypeError("placeholder"))) {
        Some(candidate) => f"{candidate.description}@{candidate.confidence}",
        None => "no repair proposed",
    }

    val none_for = ?= best_repair(propose(Fault.TestFailure(2))) {
        Some(candidate) => candidate.description,
        None => "no repair proposed",
    }

    join(
        [
            f"live r{settled.live.revision} passing {settled.live.passing}",
            f"rollbacks {settled.rollbacks}",
            f"after explicit rollback r{reverted.live.revision} passing {reverted.live.passing}",
            broken,
            repair,
            none_for,
        ],
        "; ",
    )
}

// swarm-code-review — several reviewers, one verdict.
//
// `forge run` evaluates `main` and prints its result. Demonstrates:
//   - `trait` + `impl Trait for T` so each reviewer is a type: adding a
//     reviewer is adding an impl, and no existing function changes
//   - findings merged across reviewers and de-duplicated by location, because
//     three reviewers finding the same thing is one finding, not three
//   - agreement counted, so "consensus" is a number the code produced
//   - severity as an enum, and a blocking decision derived from it
//   - `group` to report by file, `?A` where a set can be empty
//   - `/ llm` on the model call, inherited by every reviewer
//
// The rule being modelled: a change is blocked by *any* confirmed blocker, and
// confirmation needs a majority. One strict reviewer cannot block alone, and
// one lenient reviewer cannot approve alone.
//
// Run:  forge run        (or:  mage-parse --eval src/main.mg main)

// ── Findings ─────────────────────────────────────────────────────────

enum Severity {
    Note,
    Warn,
    Block,
}

fn severity_name(severity: Severity) -> String {
    ?= severity {
        Severity.Note => "note",
        Severity.Warn => "warn",
        Severity.Block => "block",
    }
}

fn severity_rank(severity: Severity) -> i32 {
    ?= severity {
        Severity.Note => 1,
        Severity.Warn => 2,
        Severity.Block => 3,
    }
}

struct Hunk {
    file: String,
    line: i32,
    text: String,
}

struct Finding {
    file: String,
    line: i32,
    severity: Severity,
    message: String,
    reviewer: String,
}

// Location identity. Two reviewers reporting the same file and line are
// reporting the same thing, which is what makes the agreement count meaningful.
fn location(finding: Finding) -> String {
    f"{finding.file}:{finding.line}"
}

// ── Reviewers ────────────────────────────────────────────────────────

// One trait, three implementations. `reviewer.review(hunk)` dispatches on the
// reviewer's own type — there is no table mapping names to behaviour.
trait Reviewer {
    fn name(&self) -> String;
    fn review(&self, hunk: Hunk) -> [Finding]~;
}

struct StyleReviewer {
    max_width: i32,
}

struct SecurityReviewer {
    strict: bool,
}

struct PerfReviewer {
    budget_ms: i32,
}

impl Reviewer for StyleReviewer {
    fn name(&self) -> String {
        "style"
    }

    fn review(&self, hunk: Hunk) -> [Finding]~ {
        ? len(chars(hunk.text)) > self.max_width {
            [@Finding {
                file: hunk.file,
                line: hunk.line,
                severity: Severity.Note,
                message: "line too long",
                reviewer: "style",
            }]
        } : {
            []
        }
    }
}

impl Reviewer for SecurityReviewer {
    fn name(&self) -> String {
        "security"
    }

    fn review(&self, hunk: Hunk) -> [Finding]~ {
        ? contains(words(hunk.text), "eval") {
            [@Finding {
                file: hunk.file,
                line: hunk.line,
                severity: Severity.Block,
                message: "dynamic eval on untrusted input",
                reviewer: "security",
            }]
        } : {
            []
        }
    }
}

impl Reviewer for PerfReviewer {
    fn name(&self) -> String {
        "perf"
    }

    fn review(&self, hunk: Hunk) -> [Finding]~ {
        // A nested loop and an eval are both worth flagging, at different
        // severities — the same hunk can produce more than one finding.
        val nested = ? contains(words(hunk.text), "nested") {
            [@Finding {
                file: hunk.file,
                line: hunk.line,
                severity: Severity.Warn,
                message: "nested loop in hot path",
                reviewer: "perf",
            }]
        } : {
            []
        }
        val slow = ? contains(words(hunk.text), "eval") {
            [@Finding {
                file: hunk.file,
                line: hunk.line,
                severity: Severity.Block,
                message: "eval defeats the compile cache",
                reviewer: "perf",
            }]
        } : {
            []
        }
        flatten([nested, slow])
    }
}

// ── The model call ───────────────────────────────────────────────────

// A reviewer that asks a model rather than pattern-matching. It is the only
// `/ llm` in the file, and the effect propagates to everything that calls it.
fn model_opinion(hunk: Hunk) -> [Finding]~ / llm {
    ? len(words(hunk.text)) > 6 {
        [@Finding {
            file: hunk.file,
            line: hunk.line,
            severity: Severity.Warn,
            message: "hunk does more than one thing",
            reviewer: "model",
        }]
    } : {
        []
    }
}

// ── Consensus ────────────────────────────────────────────────────────

// How many distinct reviewers flagged a location.
fn agreement(findings: [Finding]~, at: String) -> usize {
    val here = filter(findings, fn(finding) => location(finding) == at)
    // `freq` returns `{A: usize}`, and `len` takes `[A]` — so the distinct
    // reviewers are counted through `keys`, not by measuring the map.
    len(keys(freq(map(here, fn(finding) => finding.reviewer))))
}

// A location is confirmed when a majority of the reviewer pool flagged it. This
// is why a single strict reviewer cannot block on its own.
fn confirmed(findings: [Finding]~, at: String, pool: usize) -> bool {
    agreement(findings, at) * 2 > pool
}

// The worst severity reported at a location. `fold` rather than `reduce`
// because the seed is meaningful: nothing reported is a `Note`, not an error.
fn peak_severity(findings: [Finding]~, at: String) -> Severity {
    val here = filter(findings, fn(finding) => location(finding) == at)
    fold(
        here,
        Severity.Note,
        fn(worst, finding) => ? severity_rank(finding.severity) > severity_rank(worst) {
            finding.severity
        } : {
            worst
        },
    )
}

// ── Review ───────────────────────────────────────────────────────────

// Every reviewer sees every hunk. The reviewers are separate values of separate
// types, so this is where the trait earns its keep.
fn review_all(hunks: [Hunk]~) -> [Finding]~ / llm {
    val style = @StyleReviewer { max_width: 40 }
    val security = @SecurityReviewer { strict: 1b }
    val perf = @PerfReviewer { budget_ms: 50 }

    val by_style = flatten(map(hunks, fn(hunk) => style.review(hunk)))
    val by_security = flatten(map(hunks, fn(hunk) => security.review(hunk)))
    val by_perf = flatten(map(hunks, fn(hunk) => perf.review(hunk)))
    val by_model = flatten(map(hunks, fn(hunk) => model_opinion(hunk)))

    flatten([by_style, by_security, by_perf, by_model])
}

// ── Entry point ──────────────────────────────────────────────────────

pub fn main() -> String / llm {
    val hunks = [
        @Hunk { file: "parser.mg", line: 12, text: "let out = eval nested input from the caller" },
        @Hunk { file: "lexer.mg", line: 88, text: "advance one token" },
        @Hunk {
            file: "types.mg",
            line: 41,
            text: "unify the substitution across every element of the row",
        },
    ]

    val findings = review_all(hunks)
    // Annotated because `confirmed` takes a `usize`, and the reviewer count is
    // compared against `len`-derived numbers.
    val pool: usize = 4

    // Distinct locations, in a stable order.
    val places = sort(keys(freq(map(findings, fn(finding) => location(finding)))))

    val verdicts = map(
        places,
        fn(at) => f"{at} {severity_name(peak_severity(findings, at))} x{agreement(findings, at)}",
    )

    val blockers = filter(
        places,
        fn(at) => confirmed(findings, at, pool)
            && severity_rank(peak_severity(findings, at)) == 3,
    )

    val decision = ? len(blockers) > 0 { "blocked" } : { "approved" }

    val by_file = group(findings, fn(finding) => finding.file)
    val counts = map(sort(keys(by_file)), fn(file) => f"{file}={len(by_file[file])}")

    join(
        [
            f"{len(findings)} finding(s)",
            join(verdicts, " | "),
            f"blockers {join(blockers, ",")}",
            decision,
            join(counts, " "),
        ],
        "; ",
    )
}

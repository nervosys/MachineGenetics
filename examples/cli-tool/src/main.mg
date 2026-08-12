// cli-tool — a grep-like filter, as a MAGE program.
//
// `forge run` evaluates `main` and prints its result. Demonstrates:
//   - a struct holding parsed options, built with `@Config { … }`
//   - an enum of output modes and `match` over it (`Mode.Count`)
//   - argument parsing as a fold over the argument list
//   - the standard vocabulary (filter/map/len/lines/contains/join) instead of
//     hand-rolled loops
//   - `Result`-shaped failure via `?T`, so the empty-pattern case is handled
//     rather than assumed
//
// The evaluator has no process arguments or file I/O yet, so the argument
// vector and the searched text are values in `main`. Everything below the
// entry point is the part that would be unchanged by real argv and real files.
//
// Run:  forge run        (or:  mage-parse --eval src/main.mg main)

// ── Options ──────────────────────────────────────────────────────────

enum Mode {
    Lines,
    Count,
}

struct Config {
    pattern: String,
    ignore_case: bool,
    invert: bool,
    mode: Mode,
}

// A flag is recognised by name; anything else is the search pattern. Parsing
// is a fold so the option state is threaded explicitly rather than mutated.
fn apply_flag(cfg: Config, arg: String) -> Config {
    ? arg == "-i" {
        @Config { pattern: cfg.pattern, ignore_case: 1b, invert: cfg.invert, mode: cfg.mode }
    } : {
        ? arg == "-v" {
            @Config { pattern: cfg.pattern, ignore_case: cfg.ignore_case, invert: 1b, mode: cfg.mode }
        } : {
            ? arg == "-c" {
                @Config { pattern: cfg.pattern, ignore_case: cfg.ignore_case, invert: cfg.invert, mode: Mode.Count }
            } : {
                @Config { pattern: arg, ignore_case: cfg.ignore_case, invert: cfg.invert, mode: cfg.mode }
            }
        }
    }
}

fn parse_args(args: [String]~) -> Config {
    val empty = @Config { pattern: "", ignore_case: 0b, invert: 0b, mode: Mode.Lines }
    fold(args, empty, apply_flag)
}

// ── Search ───────────────────────────────────────────────────────────

// Case folding is applied to both sides so `-i` needs no second code path.
fn normalize(text: String, ignore_case: bool) -> String {
    ? ignore_case { lower(text) } : { text }
}

// The standard vocabulary has no substring search — `contains` is membership,
// `([A], A) -> bool` — so a pattern matches a whole *word*, not any substring.
// That is a real difference from grep, and it is stated rather than papered
// over with a hand-rolled character scan.
fn line_matches(cfg: Config, line: String) -> bool {
    val hay = normalize(line, cfg.ignore_case)
    val needle = normalize(cfg.pattern, cfg.ignore_case)
    val hit = contains(words(hay), needle)
    ? cfg.invert { !hit } : { hit }
}

fn search(cfg: Config, text: String) -> [String]~ {
    filter(lines(text), fn(line) => line_matches(cfg, line))
}

// ── Output ───────────────────────────────────────────────────────────

fn render(cfg: Config, hits: [String]~) -> String {
    ?= cfg.mode {
        Mode.Count => f"{len(hits)}",
        Mode.Lines => join(hits, "\n"),
    }
}

// An empty pattern would match every line, which is a usage error rather than
// a result. `?String` forces the caller to say what happens then.
fn run(args: [String]~, text: String) -> ?String {
    val cfg = parse_args(args)
    ? len(chars(cfg.pattern)) == 0 {
        None
    } : {
        Some(render(cfg, search(cfg, text)))
    }
}

// ── Entry point ──────────────────────────────────────────────────────

pub fn main() -> String {
    val text = join(["alpha beta", "gamma", "ALPHA delta", "beta gamma"], "\n")

    val found = run(["-i", "alpha"], text)
    val counted = run(["-c", "beta"], text)
    val missing = run([], text)

    val a = ?= found { Some(hit) => join(lines(hit), " | "), None => "usage error" }
    val b = ?= counted { Some(hit) => hit, None => "usage error" }
    val c = ?= missing { Some(hit) => hit, None => "usage error" }

    f"-i alpha -> {a}; -c beta -> {b}; no pattern -> {c}"
}

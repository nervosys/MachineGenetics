# Agent cost: what it takes, in tokens, to produce a correct program

`scripts/agent-cost.py` measures one thing: how many tokens an AI agent spends
to get a *correct* program for a task, in two arms.

* **mage** - the agent writes MAGE and may use only the compiler's
  self-describing tools (`ontology_index`, `ontology_query`, `describe`,
  `check`).
* **rust** - the agent writes Rust and has nothing but a `check` that runs
  `rustc`.

The question behind it is whether MAGE's self-description (the manifest, the
ontology, structured diagnostics) pays for itself against an agent that already
"knows" Rust from training. A model trained on Rust and not on MAGE is exactly
the condition being measured; it is a feature of the experiment, not noise.

**The repository contains no model call and no API key.** The harness is built
and tested with scripted stand-in agents. Every number in this document that
comes from a stand-in is a check that the harness works. None of them is the
cost of any model; do not quote them as one.

## Task set

The tasks whose MAGE reference is *accepted* by the acceptance check
(`benchmarks/ACCEPTANCE.md`: it type-checks and passes all its tests). The set
is derived at run time as "every task not in
`benchmarks/task-acceptance-baseline.txt`", so it grows as the baseline shrinks.
Today that is 45 tasks: 14 algorithms, 10 basic-io, 13 data-structures, 2
error-handling, 1 full-applications, 1 generics-traits, 2 systems, 2
web-network. The stand-in
measurements below were re-run on 2026-11-19 over this set: 45 tasks in the
MAGE arm, 19 paired tasks in the Rust arm.

What the agent is shown, per task: the task's natural-language `task` field;
the function signature(s) of the reference (the header of every `+f` for MAGE,
of every `pub fn` for Rust: names and types, never a body); and the test
*inputs* (the call expressions, in the arm's own syntax, with the expected
values withheld). The inputs are shown because a description such as "a Color
enum with a function that returns its name" does not say what the function is
called or how an enum value is written. The agent never sees the reference
solution, the other language's reference, or any `expected` value. Where a
description itself states a value (`basic-001` names "Hello, World!"), that is
the description's, not a leak.

### Pairing with Rust

The Rust judge builds each task's `{input, expected}` cases into a `#[test]`
harness. That is honest only where the MAGE-syntax input translates
mechanically. The translator (`translate_input` in the driver) accepts integer
and string literals, calls and method calls, `Enum.Variant` -> `Enum::Variant`
and `@Name{f:v}` struct literals; the expected value must be an integer, bool or
string literal. Anything else is refused, not guessed. Of the 45 tasks, 19
translate. The other 26 are excluded from the Rust arm and from the paired
comparison: 20 because an input is outside the translatable subset (`ds-013`, for
one, takes a closure, `fn(x) => x * 2`), 3 because an input has a `.` not followed
by a name (`basic-008`, `ds-005`, `err-001`), and 3 because the expected value is
not an integer, bool or string literal (`algo-014`, `ds-001`, `ds-004`). Every
Rust-arm number below is over the 19 paired tasks, and the paired table restricts
the MAGE arm to the same 19. Run `agent-cost.py --list` to see the pairing and the
reason for each exclusion.

## Protocol

JSON lines over the agent command's stdin/stdout. The agent command is started
once per episode, in an empty scratch directory.

```
driver -> agent  {"type":"task","prompt":...,"description":...,"signatures":[...],"call_sites":[...],"tools":[...],"limits":{...}}
                 {"type":"tool_result","name":N,"ok":B,"content":S}
                 {"type":"result","accepted":false,"attempts_left":K,"diagnostics":S}
                 {"type":"end","reason":R}
agent  -> driver {"type":"tool","name":N,"args":{...}}
                 {"type":"final","source":S}
                 either may carry "usage":{"input_tokens":I,"output_tokens":O}
```

`prompt` is the whole rendered task text; a wrapper should send it to its model
verbatim. The structured fields are conveniences and are not counted twice.

### Tools (an allowlist, nothing else is executable)

| arm | tools |
|---|---|
| mage | `ontology_index`; `ontology_query{term,kind?,budget?}`; `describe{flag}`; `check{source}` (the `--check --json --compact` diagnostics) |
| rust | `check{source}` (rustc errors, warnings suppressed) |

A call to any other name ends the episode as `violation`. There is no
`run_tests`: the agent never sees a test result. Arguments are validated and
passed to `mage-parse` as an argument vector, never through a shell. Tool output
is capped at 6000 characters.

### Judging

A `final` is judged once by the acceptance functions. MAGE: `--check` must be
clean, the program must declare no host effect (`fs net env llm tools timer`;
`--eval` would really perform them), and `task-acceptance.py`'s `judge` must
pass every case with its strict comparison. Rust: the source must compile as a
library and then as a `--test` build with the harness appended, and every test
must pass.

A submission that fails is returned as `not accepted`, with only the
compiler's diagnostics when it does not compile or type-check. When it
compiles but is wrong, the agent is told so and nothing else: no expected
value, no got value, no failing-case index. For Rust, a test-build error
(for example a signature mismatch) is also reported generically, because
rustc's message would quote the harness line and so the expected value. That
makes Rust feedback slightly poorer than a plain compile in that one situation;
MAGE feedback is the same generic line. Up to `--max-attempts` (default 3)
submissions.

**Solved** means an accepted submission within the caps. Outcomes:
`solved`, `failed` (attempts exhausted), `violation`, `protocol_error`
(unparseable reply), `timeout`, `turn_cap`, `token_cap`, `agent_exit`.

### Limits

Per episode: `--max-turns` (20 agent messages), `--max-tokens` (60000 counted
BPE tokens), `--wall` (120 s, including rustc and mage-parse time), and the
attempt cap. A timed-out agent process tree is killed.

### Environment

The agent command is run with a **scrubbed environment**, like
`arena/src/command.rs`: only `PATH` (plus `SYSTEMROOT TEMP TMP PATHEXT` on
Windows). Reason: the wrapper is the one component in the experiment that runs
code the harness did not write; scrubbing keeps your shell's secrets and
accidental configuration (proxy, model, key) from leaking into it or changing a
run, and keeps stand-in runs reproducible. The cost is that a real wrapper must
get its credential on purpose: either read it from a file it knows, or pass one
named variable with `--agent-env NAME` (repeatable). The driver itself holds and
sends no key. As in the arena this is hygiene, not a sandbox: the agent runs as
your user.

## Accounting

Every message in both directions is counted with the compiler's built-in BPE
(`mage-parse --bpe-count -`). **That is MAGE's own 4096-merge vocabulary, trained
on this repository's MAGE sources. It is not any model's tokenizer.** It is
consistent between arms and deterministic, so it ranks the arms; it does not
predict a provider's bill, and it is likely to under-count Rust relative to a
tokenizer trained on Rust. When the agent's replies carry `usage`, the sum of
reported `input_tokens + output_tokens` is recorded as the primary figure and the
BPE count as secondary (`reported_*` fields; the table prints them separately).

The counted text is a message's content: the task prompt; `name(args)` for a tool
call; the content of a tool result; the submitted source; the diagnostics text;
the verdict line. Categories: `prompt`, `tool_calls`, `tool_results`,
`diagnostics` (check output and not-accepted feedback), `final_source`,
`verdict`. They sum exactly to the episode total.

Headline figure: **tokens per solved task** = total tokens across *all* episodes
of the arm (failures included) / tasks solved. Also reported: solve rate, median
tokens per episode, tokens by category, and a histogram of submissions per
episode.

## Outputs

`--out DIR`: `transcripts/<task>.<arm>.jsonl` (every message with its category
and token count, and a closing `episode_end` record), `summary.json`,
`summary.md`. With a deterministic agent two runs are byte-identical; wall times
appear only with `--timestamps`.

## Running

```
python scripts/agent-cost.py --mage-parse "$MAGE_PARSE_BIN" \
    --agent "python scripts/agent_wrappers/agent_cost_standin.py oracle" --out out/
python scripts/test-agent-cost.py --mage-parse "$MAGE_PARSE_BIN"   # 18 tests; about 11 minutes with the release build
```

A real run needs a wrapper command that speaks the protocol (see
`scripts/agent_wrappers/agent_cost_cli.sh.example`: an example that **spends
money** and is run by nothing), a credential the wrapper obtains itself, and a
budget. Cost bound: episodes x `--max-tokens` is the ceiling in counted tokens
(a provider's own count and re-sent context make the real figure larger);
use `--only` and `--arms` to start small. The stand-ins are in
`scripts/agent_wrappers/agent_cost_standin.py`: `oracle`, `lookup-then-oracle`,
`wrong-then-oracle`, `always-wrong`, `tool-violation`, `garbage`, `slow`, `spy`.

## Harness check: stand-in results

Run on 2026-11-19 with `mage-parse` v0.3.0 (release build) and rustc 1.100.0. These
exist to show the harness judges and accounts correctly. They are not agent
results.

| stand-in | arm | tasks | solved | tokens / solved | median / episode | tool results (all episodes) |
|---|---|---:|---:|---:|---:|---:|
| oracle | mage | 45 | 45 | 527 | 498 | 0 |
| oracle | rust | 19 | 19 | 338.6 | 343 | 0 |
| lookup-then-oracle | mage | 45 | 45 | 2477.6 | 2388 | 71910 |
| lookup-then-oracle | rust | 19 | 19 | 407.9 | 402 | 0 |

The MAGE rows cover all 45 accepted tasks and the Rust rows the 19 paired tasks.
Over all 45 the MAGE arm reads 527 (oracle) and 2477.6 (lookup-then-oracle)
tokens per solved task; over the 19 paired tasks it reads 452.7 and 2299.9.

On the 19 paired tasks the MAGE arm reads 452.7 (oracle) and 2299.9
(lookup-then-oracle) tokens per solved task. The MAGE oracle exceeds the Rust
oracle by 114 tokens a task, almost all of it the longer prompt (four tools to
describe, and the "MAGE is unfamiliar" line); the `lookup-then-oracle` gap is the
ontology index and two queries, about 1600 tokens a task of tool results.

The `oracle` solves every task in both arms on the first submission (45 of 45
MAGE, 19 of 19 Rust), which is the check that judging accepts the references
(and that the Rust translation is right for the 19 paired tasks). `lookup-then-oracle` calls the index, two
queries and a check before answering, so its extra tokens are tool traffic
only, and it shows the per-category accounting responding to tool use. A
reference-oracle's cost is a **floor**: it is the price of reading the task and
writing the answer with no thinking, no wrong turns and no learning of the
language.

## Limits and biases

* **The task set is small and skewed.** 45 tasks, 24 of them algorithms or
  basic-io one-liners; the rest of the corpus is mostly not accepted yet (no
  concurrency or agent-orchestration task, and only 2 systems and 2 web-network
  tasks). Results say little about
  large programs. Many tasks are so small that the prompt dominates.
* **Tokenizer.** BPE here is MAGE's vocabulary, not a model's. Provider usage,
  when reported, is the better figure; the two should be read together.
* **Training asymmetry.** The Rust arm is helped by pretraining on Rust; the
  MAGE arm by self-description. That is the point, but it means the result is
  about *this* model generation: a model trained on MAGE would change it.
* **Prompt asymmetry.** The MAGE prompt must say that MAGE is unfamiliar and
  list four tools; the Rust prompt lists one. Both are counted. The prompts are
  otherwise parallel, but they are not identical and the MAGE one is longer.
* **Test inputs are shown.** Both arms see the call expressions. An agent can
  fit three visible inputs, though never the expected values, and the judge uses
  the same cases. Hidden cases would be a better test; the corpus has none.
* **Feedback asymmetry.** MAGE's checker returns structured JSON with fixes;
  rustc returns prose. This is part of what is measured, but it also means a
  token-for-token comparison is across different diagnostic formats.
* **Judge coverage differs.** The MAGE judge is the acceptance check (type-check
  + evaluator with strict, un-normalised comparison); the Rust judge is rustc's
  type system plus the same cases. Rust's type checking is stricter in places
  (integer inference, borrowing), so some correct-looking Rust fails that a MAGE
  program passes, and the reverse.
* **Rust translation covers 19 of 45 tasks**, so the paired comparison is over
  the simplest tasks only.
* **Stand-in numbers say nothing about a model.** In particular the oracle
  never fails, so its solve rate and tokens-per-solved are the harness's floor.

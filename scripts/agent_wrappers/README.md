# Agent Wrappers for `reliability-bench`

The `reliability-bench --agent subprocess:<cmd>` backend spawns one
process per task, with this protocol:

| Stream | Direction | Content |
|---|---|---|
| **stdin** | bench → wrapper | Natural-language task description (UTF-8, no trailing newline guaranteed) |
| **stdout** | wrapper → bench | Generated MechGen source (UTF-8) |
| **stderr** | wrapper → bench | Human-readable diagnostics; surfaced as `agent refused: <text>` on non-zero exit |
| **exit code** | wrapper → bench | `0` = success; non-zero = "agent refused" |

The bench does **not** pass the corpus's reference solution to the
wrapper. Only the task description is supplied — so the wrapper sees
exactly what an autonomous agent would see.

### Refine mode (Stage-3 re-prompt)

When the bench's mechanical recovery pipeline (pattern-heal,
structural-balance, structural-completion) cannot save broken source,
the bench invokes the wrapper a **second** time in **refine** mode.
Wrappers signal mode via environment variables:

| Env var | Set on every call | Meaning |
|---|---|---|
| `RDX_BENCH_MODE` | yes | `propose` (first call) or `refine` (Stage-3) |
| `RDX_TASK_ID` | yes | Stable task identifier |
| `RDX_TASK_DESCRIPTION` | refine only | The original task description (stdin carries the broken source instead) |
| `RDX_PARSE_ERROR` | refine only | The parse error message that caused the re-prompt (e.g. `12:5: expected RBrace, found ...`) |

In **refine** mode, **stdin carries the broken source**, not the task
description. The wrapper's job is to return a corrected version on
stdout. A wrapper that doesn't implement refine can simply echo stdin
back and the bench records `refine_succeeded=false` for that task,
which is correct: a no-op refine should not count as recovery.

Minimal refine-aware skeleton:

```sh
if [ "$RDX_BENCH_MODE" = "refine" ]; then
    BROKEN=$(cat)
    # Re-prompt the model with broken source + error context.
    PROMPT="Fix this MechGen source. Error was: $RDX_PARSE_ERROR
Task was: $RDX_TASK_DESCRIPTION
Broken source:
$BROKEN"
    your-llm-cli -p "$PROMPT"
else
    TASK=$(cat)
    your-llm-cli -p "Write MechGen for: $TASK"
fi
```

## Available wrappers in this directory

| File | Purpose |
|---|---|
| `echo_oracle.sh` | Smoke-test wrapper. Prints a fixed minimal MechGen program regardless of input. Useful for verifying the bench plumbing works end-to-end. |
| *(add your own)* | See below |

## Run the bench against a wrapper

From the repo root:

```sh
# Unix-like (macOS, Linux): direct invocation works.
cargo run --bin reliability-bench --manifest-path prototype/Cargo.toml \
    -- --agent "subprocess:./scripts/agent_wrappers/echo_oracle.sh"

# Windows: the harness uses Rust's `Command::new`, which doesn't
# resolve shebangs. Prefix `bash` (git-bash or WSL) explicitly:
cargo run --bin reliability-bench --manifest-path prototype/Cargo.toml \
    -- --agent "subprocess:bash scripts/agent_wrappers/echo_oracle.sh"

# A real Claude Code wrapper would look like:
cargo run --bin reliability-bench --manifest-path prototype/Cargo.toml \
    -- --agent "subprocess:./scripts/agent_wrappers/claude_cli.sh"
```

The smoke-test wrapper always returns the same valid MechGen program,
so it should produce **100 / 100 parse** — confirming the bench
plumbing is intact. Per-task cost depends entirely on the wrapper —
the harness adds no network code or credentials.

## Writing a real-LLM wrapper

A minimal wrapper has three steps:

```bash
#!/usr/bin/env bash
# 1. Read the task description from stdin
TASK=$(cat)

# 2. Build a prompt that asks for MechGen source only
PROMPT="Write a MechGen function for: ${TASK}
Reply with MechGen source code only, no commentary."

# 3. Call the model and emit raw source on stdout
your_llm_command "$PROMPT"  # exit code propagates
```

For the **Anthropic API** specifically you'd `curl` the `/messages`
endpoint with the prompt, extract `content[0].text` from the JSON
response with `jq`, and write that to stdout. For **Claude Code CLI**
the equivalent is `claude -p "<prompt>"` reading the model's reply
from stdout.

For a **local llama**: pipe the prompt through your inference binary
(`llama-cli -m model.gguf -p "$PROMPT"` etc.) and the same shape works.

## Recording the bench results

Each run writes `benchmarks/RELIABILITY_REPORT.md` with per-task
status (`parse_ok`, `heal_succeeded`, etc.) and per-category
aggregates. To compare wrappers, copy the report after each run:

```sh
mv benchmarks/RELIABILITY_REPORT.md benchmarks/REPORT_$(date +%s).md
```

## What the harness already measures

- Parse OK / fail per task
- Lex errors per task
- Self-heal proposed (any of 10 patterns matched)
- Self-heal succeeded (top fix made the program re-parse)
- Per-task latency in microseconds (lex + parse + heal-retry)
- Effective pass rate = parse_ok + heal_succeeded

A new wrapper just sees its numbers in the same columns. The
per-category breakdown shows which task families (basic-io,
algorithms, agent-orchestration, etc.) the wrapper handles well or
fails on.

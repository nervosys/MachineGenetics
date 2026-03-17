# Redox Agent Training Benchmark Corpus

A structured corpus of **100 programming tasks** designed to train, evaluate, and benchmark AI agents on Redox code generation. Every task includes a Redox solution, an equivalent Rust solution, token counts, effect annotations, SKB rule references, and test cases.

## Format

All tasks follow the `redox-training-v1` schema defined in [`corpus-schema.json`](corpus-schema.json). Each task object contains:

| Field                         | Description                                      |
| ----------------------------- | ------------------------------------------------ |
| `id`                          | Unique identifier (e.g. `basic-001`, `algo-015`) |
| `category`                    | One of 10 categories (see below)                 |
| `difficulty`                  | `simple`, `medium`, `hard`, or `very-hard`       |
| `task`                        | Natural language problem description             |
| `solution.rdx_source`         | Reference Redox implementation                   |
| `solution.token_count`        | Token count of the Redox solution                |
| `solution.effects`            | List of effects used (e.g. `["io", "fs"]`)       |
| `solution.skb_rules_used`     | SKB rules exercised                              |
| `rust_equivalent.rs_source`   | Equivalent Rust implementation                   |
| `rust_equivalent.token_count` | Token count of the Rust version                  |
| `tests`                       | Array of `{input, expected}` pairs               |

## Categories

| #   | Category            | File                                                               | Tasks | Difficulty | Token Range |
| --- | ------------------- | ------------------------------------------------------------------ | ----- | ---------- | ----------- |
| 1   | Basic I/O           | [`tasks/basic_io.json`](tasks/basic_io.json)                       | 10    | simple     | 20–50       |
| 2   | Data Structures     | [`tasks/data_structures.json`](tasks/data_structures.json)         | 15    | medium     | 50–150      |
| 3   | Algorithms          | [`tasks/algorithms.json`](tasks/algorithms.json)                   | 15    | medium     | 80–200      |
| 4   | Error Handling      | [`tasks/error_handling.json`](tasks/error_handling.json)           | 5     | medium     | 50–150      |
| 5   | Generics & Traits   | [`tasks/generics_traits.json`](tasks/generics_traits.json)         | 5     | hard       | 100–300     |
| 6   | Concurrency         | [`tasks/concurrency.json`](tasks/concurrency.json)                 | 10    | hard       | 100–300     |
| 7   | Web & Network       | [`tasks/web_network.json`](tasks/web_network.json)                 | 10    | hard       | 150–400     |
| 8   | Systems Programming | [`tasks/systems.json`](tasks/systems.json)                         | 10    | hard       | 200–500     |
| 9   | Agent Orchestration | [`tasks/agent_orchestration.json`](tasks/agent_orchestration.json) | 10    | very-hard  | 300–800     |
| 10  | Full Applications   | [`tasks/full_applications.json`](tasks/full_applications.json)     | 10    | very-hard  | 500–2000    |

**Total: 100 tasks**

## Evaluation Metrics

Agents are scored on seven metrics (targets from REDOX_ECOSYSTEM.md §4.4):

| Metric                  | Target  | Description                                |
| ----------------------- | ------- | ------------------------------------------ |
| First-pass success rate | > 95%   | Compiles and passes tests on first attempt |
| Token efficiency ratio  | < 1.1   | `rdx_tokens / reference_tokens`            |
| Effect correctness      | > 99%   | Correct effect annotations                 |
| Spec compliance         | > 98%   | Conforms to Redox language spec            |
| Migration accuracy      | > 99%   | Rust↔Redox round-trip fidelity             |
| Iteration count         | < 1.5   | Average attempts to correct solution       |
| Cost per task           | < $0.05 | LLM API cost per task                      |

## Usage

### Agent Training
Feed tasks as few-shot examples or fine-tuning data. Each task provides the natural language description as input and `solution.rdx_source` as the target output.

### Benchmark Evaluation
1. Present the `task` description to the agent
2. Collect the generated Redox source
3. Compare against `solution.rdx_source` for correctness
4. Measure token count against `solution.token_count`
5. Verify effects match `solution.effects`
6. Run `tests` for functional correctness

### Validation
Validate task files against the schema:
```
# Any JSON Schema 2020-12 validator
jsonschema --instance tasks/basic_io.json corpus-schema.json
```

## Files

```
benchmarks/
├── README.md              # This file
├── corpus-schema.json     # JSON Schema 2020-12 for task validation
├── manifest.json          # Corpus metadata and evaluation config
└── tasks/
    ├── basic_io.json          # 10 simple tasks
    ├── data_structures.json   # 15 medium tasks
    ├── algorithms.json        # 15 medium tasks
    ├── error_handling.json    #  5 medium tasks
    ├── generics_traits.json   #  5 hard tasks
    ├── concurrency.json       # 10 hard tasks
    ├── web_network.json       # 10 hard tasks
    ├── systems.json           # 10 hard tasks
    ├── agent_orchestration.json # 10 very-hard tasks
    └── full_applications.json # 10 very-hard tasks
```

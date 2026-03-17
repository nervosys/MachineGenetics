# Redox Agent Training Data

This directory contains training data, instruction templates, and evaluation datasets for AI agents working with the Redox programming language.

## Directory Structure

```
training/
├── README.md                    # This file
├── agent-instructions.yaml      # Standard agent instruction config
├── samples/
│   ├── code-generation.jsonl    # Code generation training samples
│   ├── code-completion.jsonl    # Code completion / fill-in samples
│   ├── translation.jsonl        # Rust ↔ Redox translation pairs
│   ├── error-repair.jsonl       # Error diagnosis and repair samples
│   └── refactoring.jsonl        # Refactoring and optimization samples
├── prompts/
│   ├── system-prompt.md         # Base system prompt for Redox agents
│   ├── few-shot-generation.md   # Few-shot examples for code generation
│   ├── few-shot-translation.md  # Few-shot examples for translation
│   └── few-shot-repair.md       # Few-shot examples for error repair
└── evaluation/
    ├── eval-schema.json         # Schema for evaluation records
    └── held-out-tasks.jsonl     # Held-out evaluation tasks (not for training)
```

## Data Formats

### Training Samples (JSONL)

Each line is a self-contained JSON object. All sample files share a common envelope:

```json
{
  "id": "gen-001",
  "type": "code-generation",
  "task": "Natural language description of the task",
  "context": { ... },
  "solution": { "rdx_source": "...", "token_count": 45 },
  "metadata": { "effects": ["io"], "skb_rules": ["borrow:shared-iter"], "difficulty": "medium" }
}
```

**Types:**

| Type               | Description                              | File                      |
| ------------------ | ---------------------------------------- | ------------------------- |
| `code-generation`  | Generate Redox code from a natural language prompt | `code-generation.jsonl`  |
| `code-completion`  | Fill in missing code given surrounding context      | `code-completion.jsonl`  |
| `translation`      | Translate between Rust and Redox                    | `translation.jsonl`      |
| `error-repair`     | Diagnose and fix errors in Redox code               | `error-repair.jsonl`     |
| `refactoring`      | Improve or optimize existing Redox code             | `refactoring.jsonl`      |

### Agent Instructions (YAML)

The `agent-instructions.yaml` file provides a machine-readable reference that agents load at the start of a session. It covers syntax rules, effect rules, contract rules, and style conventions.

### Prompt Templates (Markdown)

System prompts and few-shot templates in `prompts/` are designed to be concatenated into LLM context windows. They follow the pattern:

```
[system-prompt.md] + [few-shot-*.md (selected by task type)] + [user query]
```

## Usage

### Fine-Tuning

```bash
# Combine all training samples into a single file
cat training/samples/*.jsonl > combined-training.jsonl

# Filter by difficulty
jq 'select(.metadata.difficulty == "hard")' combined-training.jsonl
```

### In-Context Learning

Use the prompt templates in `prompts/` directly:

```python
system = open("training/prompts/system-prompt.md").read()
few_shot = open("training/prompts/few-shot-generation.md").read()
prompt = f"{system}\n\n{few_shot}\n\nTask: {user_task}"
```

### Evaluation

```bash
# Run held-out evaluation
cat training/evaluation/held-out-tasks.jsonl | \
  jq -c '.' | \
  while read task; do
    echo "$task" | your-agent-harness --eval
  done
```

## Sample Counts

| File                    | Samples | Difficulty Spread                  |
| ----------------------- | :-----: | ---------------------------------- |
| `code-generation.jsonl` |   30    | 8 simple, 10 medium, 8 hard, 4 very-hard |
| `code-completion.jsonl` |   20    | 5 simple, 8 medium, 5 hard, 2 very-hard  |
| `translation.jsonl`     |   25    | 8 simple, 9 medium, 5 hard, 3 very-hard  |
| `error-repair.jsonl`    |   15    | 4 simple, 5 medium, 4 hard, 2 very-hard  |
| `refactoring.jsonl`     |   10    | 2 simple, 3 medium, 3 hard, 2 very-hard  |
| **Total**               | **100** |                                    |

## Relationship to Benchmarks

The `benchmarks/` directory contains evaluation tasks with reference solutions. This `training/` directory contains:

1. **Training data** — samples used to teach agents Redox syntax and idioms
2. **Prompt templates** — reusable context for in-context learning
3. **Evaluation data** — held-out tasks for measuring agent quality (distinct from benchmark tasks)

The benchmark corpus (`benchmarks/tasks/*.json`) should NOT be used for training to avoid data contamination.

## Contributing

When adding new training samples:

1. Follow the JSONL format (one JSON object per line)
2. Include both `rdx_source` and `rs_source` (Rust equivalent) where applicable
3. Verify token counts are accurate
4. Tag effects and SKB rules used
5. Add to the appropriate difficulty bucket
6. Do NOT duplicate tasks from `benchmarks/tasks/`

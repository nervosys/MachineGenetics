# Machine Language Binary IR as the Agent Artifact — Measured

**Date:** 2026-06-04 · **Premise:** for agentic ML use, the artifact an agent
emits, ships, loads, and introspects is the **binary Machine Language IR**, not text source.
This report measures that path with real bytes (no estimates). Reproduce with the
commands shown; numbers are from `MechGen-parse` on the example nets.

## Compactness (measured)

```
MechGen-parse --target=ml-bytes examples/<net>.mg <out>.ml
```

| Model | full text | code-only text¹ | **Machine Language bytes** | vs full | vs code-only |
|---|--:|--:|--:|--:|--:|
| AffineRegressor (3-layer MLP) | 1268 B | 326 B | **144 B** | −88.6% | −55.8% |
| CycleLM (Embedding + Linear) | 1137 B | 369 B | **121 B** | −89.4% | −67.2% |

¹ comments and whitespace stripped. The honest figure is the code-only column
(**56–67% smaller**); the full-text column includes doc comments.

The Machine Language container holds the *lowered op graph* (e.g. AffineRegressor = 11 nodes,
77 B expr + framing), not just a layer list — so this is a complete, executable
representation, not a summary.

## Determinism (measured)

Emitting the same source twice produces **byte-identical** containers
(`cmp a.ml b.ml` → identical). The artifact is canonical: an agent can
content-hash it for cache keys, and diffs are meaningful.

## Safety (verified)

```
MechGen-parse --from=ml-bytes <file>.ml
→ // container: 144 bytes, 2 item(s)  (decodes structure)
```

Decode is **pure data** — length-prefixed, bounds-checked fields, no execution.
Loading an Machine Language artifact cannot run code. Contrast PyTorch `torch.load` (pickle =
arbitrary code execution on load). An agent can introspect an untrusted model's
structure *without* running anything.

## Why this is a distinct track

The text-source token axis (see the language profile) is bounded — MechGen text
is only ~10% terser than Rust because identifiers/types/literals dominate bytes.
The IR track is different in kind: the agent never writes text, so the relevant
cost is the **binary** (tens to ~150 bytes/model), the relevant determinism is
**byte-stability** (achieved), and the relevant safety is **load-without-exec**
(achieved). These feed the `rmi` **framework** profile in agentic-eval, not the
language profile.

## Reproduction

```sh
cd prototype
cargo run --release --bin MechGen-parse -- --target=ml-bytes examples/agent_built_mlp.mg out.ml
wc -c out.ml                       # 144
cargo run --release --bin MechGen-parse -- --target=ml-bytes examples/agent_built_mlp.mg b.ml
cmp out.ml b.ml                  # identical (deterministic)
cargo run --release --bin MechGen-parse -- --from=ml-bytes out.ml   # data-only decode
```

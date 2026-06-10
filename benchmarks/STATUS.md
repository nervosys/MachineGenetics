# MechGen + RMI — Project Status

**Generated:** 2026-05-28 (Phase 96: agent-UX surface + hardware accelerator extensibility)

Single-page snapshot of where the unified system stands. For the
narrative history see [`UNIFICATION.md`](../UNIFICATION.md); for the
agent-discoverable schema see [`MECHGEN_ONTOLOGY.json`](../MECHGEN_ONTOLOGY.json)
or call RAP `ontology/full`.

## Tests

| Suite | Count |
|---|---:|
| `MechGen-parse` (main prototype) | **915** |
| `reliability-bench` bin | 122 |
| `token-bench` bin | 30 |
| RecursiveMachineIntelligence / RMI (unchanged) | 1,367 |
| **Combined** | **2,434** |

All green at HEAD.

## Reliability bench (`benchmarks/tasks/*.json`, 100 tasks)

| Backend | Parse | Pattern-heal | Structural-heal | Refine | Effective pass |
|---|---:|---:|---:|---:|---:|
| `file-oracle` (perfect input) | **99 / 100** | 0 / 1 | 1 / 1 | 0 / 1 | **100 / 100** |
| `perturbed-oracle` (8 mutations) | 24 / 100 | 42 / 76 | 2 / 76 | 0 / 76 | **68 / 100** |
| `perturbed+refine:smart_fixer` | 24 / 100 | 42 / 76 | 2 / 76 | **1 / 76** | **69 / 100** |
| `subprocess: refine_oracle.sh` smoke | 0 / 100 | 0 | 0 | **100 / 100** | **100 / 100** |

Refine 100/100 on the smoke wrapper proves the Stage-3 protocol wire-up
end-to-end without spending tokens on a real LLM.

## Token-efficiency bench (re-verified Phase 49)

| Measurement | MechGen | Rust | Ratio | Note |
|---|---:|---:|---:|---|
| Source bytes | 55,075 | 52,223 | **1.055** | 5.5 % longer text |
| Dense bytes (whitespace stripped) | 36,057 | 38,632 | **0.933** | 6.7 % shorter |
| Native lexer tokens | 15,282 | 15,310 | **0.998** | parity |
| Shared rule | 15,831 | 15,310 | 1.034 | 3.4 % longer |

Honest framing: MechGen text is **at parity with Rust** in syntactic
tokens. The mission win lives in the **binary IR (Agentic Binary Language)** for AI-routed
items and the **reliability pipeline**, not in raw text-byte reduction.

## CI floors

| Floor | Threshold | Today |
|---|---|---:|
| File-oracle parse | ≥ 98 | **99** |
| File-oracle structural-heal | ≥ 2 | 3 |
| Perturbed heal-succeeded | ≥ 40 | **42** |
| Subprocess smoke (echo) | exit (skip) | passes |
| Subprocess refine smoke | refine > 0 | 100 |
| Native-lexer ratio | ≤ 1.100 | 0.998 |

Any PR that drops below these numbers fails CI. The numbers can only
ratchet upward.

## Agent-facing surface

### RAP methods (48 total)

| Group | Methods |
|---|---|
| Language | `language/parse`, `language/tokens` |
| Build | `build/check`, `build/heal`, `build/recover` |
| Agentic Binary Language transport | `abl/encode`, `abl/decode`, `abl/run` |
| Pipeline | `pipeline/recover-and-encode` |
| Ontology | `ontology/full`, `ontology/section` |
| Cost | `cost/query`, `cost/compare` |
| SKB | `skb/query`, `skb/spec`, `skb/rules` |
| Verify | `verify/contracts`, `verify/module` |
| Format | `format/agent`, `format/human` |
| Lint / token / effects / elision / attribute / capability / heal / sandbox / doc / grammar / manifest | 16 more |
| Natural language | `nl/generate`, `nl/explain`, `nl/refactor`, `nl/query` |

### Recovery pipeline (5 stages)

1. **`already-valid`** — short-circuit on clean source
2. **`pattern-heal`** — multi-pass over ranked `heal::heal_one` candidates
3. **`structural-balance`** — close unbalanced `(`/`[`/`{` at EOF
4. **`structural-completion`** — splice `()`/`0`/`_` after trailing operator
5. **`trim-bad-token`** — delete the bad token at the parse error position
6. **`agent.refine`** — re-prompt the agent backend (Stage-3, requires wrapper)
7. **`failed`** — all stages exhausted

### Ontology (21 sections, one call)

```
ontology/full
  ├─ sigils (38)             — surface tokens + meaning
  ├─ keywords (12)           — AI constructs (net/kb/agent/swarm/...)
  ├─ types (30)              — built-in scalars + composites
  ├─ ast_kinds (18)          — top-level item families
  ├─ ir_ops (107)            — every Agentic Binary Language opcode w/ metadata
  ├─ op_families (7)         — Neural/Symbolic/Control/Memory/Agent/Meta/Math
  ├─ layer_map (31)          — surface name → opcode
  ├─ rap_methods (37)        — every callable method w/ inputs/outputs
  ├─ heal_patterns (~13)     — mechanical fix patterns
  ├─ recovery_stages (7)     — the 5-stage pipeline
  ├─ machine                    — binary container format constants
  ├─ examples (10)           — parse-verified working snippets
  ├─ framewerx_modules (256) — RecursiveMachineIntelligence-MG framework (FLAX-equivalent)
  ├─ cli_flags (17)          — every MechGen-parse flag
  ├─ bench_backends (4)      — reliability-bench backends
  ├─ effects (15)            — @fx/@req/@ens + canonical effect names
  ├─ wrapper_protocol (9)    — subprocess agent contract
  ├─ project_layout (22)     — top-level directory map
  ├─ docs (7)                — canonical doc pointers + IronAccelerator
  ├─ ci_floors (6)           — current CI thresholds
  └─ hardware_accelerators   — extensible runtime registry (8 builtins + JSON)
```

Static dump at [`MECHGEN_ONTOLOGY.json`](../MECHGEN_ONTOLOGY.json)
(**122 KB**). Refresh via `MechGen-parse --emit-ontology`.

### Hardware accelerator extensibility (P91-95)

Built-in catalog: cpu (constructible), cuda / metal / apple_ane / vulkan /
webgpu / qualcomm / blas (feature-gated). Operators add arbitrary new
backends without recompiling:

```bash
# Drop a descriptor in ~/.mechgen/backends.json or pass --backends-file
[
  {
    "name": "groq_lpu",
    "family": "asic",
    "vendor": "Groq",
    "requires": "wrapper script",
    "summary": "Groq LPU - deterministic latency ASIC",
    "available_at_runtime": true,
    "dispatch": {
      "kind": "subprocess",
      "command": "bash /path/to/groq_wrapper.sh"
    }
  }
]

# Then dispatch through it:
MechGen-parse --backends-file=~/.mechgen/backends.json \
              --backend=groq_lpu --run=abl-bytes model.abl
```

Wrapper protocol: stdin = Agentic Binary Language blob, env = `RDX_BACKEND` / `RDX_ITEM_NAME` /
`RDX_INPUT_SHAPE`, stdout = `{ ok, dispatched, output_shape, output_sum, error }` JSON.
Reference wrapper: `scripts/backend_wrappers/demo_subprocess_backend.sh`.

### CI-enforced agent UX (3 transports)

| Layer | What's guarded |
|---|---|
| **Rust unit tests** | 45+ tests on `rap::dispatch()` |
| **CLI compiled path** | `framewerx_examples_dispatch_end_to_end` runs 15 examples through CpuBackend |
| **JSON-RPC over TCP** | `RAP server end-to-end` CI step (6 markers) |
| **Subprocess backend** | `Subprocess backend protocol (P94)` CI step (4 markers) |

No agent UX layer is unguarded against regression.

## Where the win actually lives

After 55 phases of measurement:

1. **Token parity** with Rust on syntactic tokens (0.998). The agent
   pays the same inference cost on MechGen text as on Rust text.
2. **Binary IR (Agentic Binary Language)** for AI items (`net` / `kb` / `agent` / `swarm`).
   Agents ship and execute these without text round-trip - this is where
   the real size win lives.
3. **Reliability** - the 5-stage recovery pipeline is load-bearing.
   File-oracle effective pass 75 / 100; perturbed-8 (realistic LLM
   shape) 37 / 100; all CI-floor-protected.
4. **Self-describing protocol** - `ontology/full` gives an agent every
   construct, opcode, type, method, and example in one round-trip.

The story is reliability-led and discovery-led, not size-led.

## What is intentionally not done

| Item | Reason |
|---|---|
| End-to-end test with a real LLM | Wrapper template exists at `scripts/agent_wrappers/claude_cli.sh.example`; requires user credentials |
| Remaining ~24 corpus parse failures | Each requires a distinct language feature (Rust-style generics `<T: Bound>`, map types `{K: V}`, fully-qualified path patterns, etc.) — open-ended grind |

### Phase 56 closed three previously-deferred items

| Item | Resolution |
|---|---|
| **Range expressions in for-loop position** | Context-specific parse in for-loop iter slot (not global infix - that regressed match-arm patterns) |
| **Smarter Stage 2b** | Type-aware splice for `v\|m IDENT: T =` truncations; falls back to `()` for unknown types |
| **Corpus grind sample** | Three parser patches: optional fn body (trait method signatures), optional `type` body (associated-type declarations), `for` keyword accepted in impl-block (lexer aliases `for` to `@`) — +1 effective pass measured |

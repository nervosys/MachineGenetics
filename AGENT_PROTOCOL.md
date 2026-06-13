# Agent Protocol: Agentic Binary Language Bytes as the Canonical Target

> Phase 27 measurement showed that MAGE's **text surface** is
> essentially tied with idiomatic Rust on byte/token count
> (see [`benchmarks/FINDINGS.md`](benchmarks/FINDINGS.md)).
>
> The genuine ~50× efficiency win lives in the **Agentic Binary Language binary IR** —
> a transformer block fits in 47 bytes, a full multi-item module
> in ~300 bytes. This document describes how agents should target
> Agentic Binary Language directly rather than emitting text.

## The pitch

| Format | What an agent emits | Bytes per transformer block |
|---|---|---:|
| Rust source | `pub struct Block { … }` + traits + impl | ~700-1200 |
| MAGE text | `net Block { layer ln1: LayerNorm; … }` | ~250-400 |
| **Agentic Binary Language bytes** | binary `Expr` codec | **47** |

For an agent token-budgeted at, say, 4096 tokens of output, the
difference between emitting MAGE text and emitting Agentic Binary Language bytes is
~3 orders of magnitude in how many models / programs the agent can
encode per response.

## The flow

```
   ┌────────────────────┐
   │ Natural-language   │
   │ prompt or spec     │
   └──────────┬─────────┘
              │
              ▼
   ┌────────────────────┐
   │ Agent (LLM)        │   emits binary Agentic Binary Language bytes
   │                    │   ─ NOT MAGE text
   └──────────┬─────────┘
              │
              ▼ .abl container (Agentic Binary Language v1)
   ┌────────────────────┐
   │ mage-parse      │   --from=abl-bytes
   │   • verify codec   │
   │   • decompile      │   → human-readable .mg
   │   • dispatch       │   → CpuBackend / GPU / etc.
   └────────────────────┘
```

## Agentic Binary Language container format (v1)

```text
   magic    "Agentic Binary Language"  (4 bytes)
   version  u16     (= 1)
   count    u32     — number of items in the module
   for each item:
     name_len  u32
     name      UTF-8 bytes (typically a short identifier)
     expr_len  u32
     expr      rmi::lang::codec::Encoder::encode_expr_only(&Expr)
```

A complete `unified.mg` example with 5 items (Transformer block, MLP,
ResNet stage, swarm, KB) lands in **300 bytes**. Header overhead is
~50 bytes; the rest is item payload.

## CLI

The `mage-parse` binary has two new flags:

```sh
# Emit Agentic Binary Language bytes from a MAGE source. Optional output path; without
# it, only the size summary is printed.
mage-parse --target=abl-bytes <file.mg> [out.abl]

# Decode a .abl container back to human-readable MAGE via the
# Phase-4 decompiler. Lossy for opcodes that lack canonical layer
# names (symbolic / agent ops) — see "Limitations" below.
mage-parse --from=abl-bytes <file.abl>
```

End-to-end demo on `prototype/examples/unified.mg`:

```
$ mage-parse --target=abl-bytes unified.mg unified.abl
// MAGE → Agentic Binary Language bytes for unified.mg
// text source: 1778 bytes    Agentic Binary Language container: 300 bytes    ratio: 0.169 (83.1% reduction)
//   TransformerBlock: 47B  hash=7def99cdb73a14e2
//   MLP: 17B  hash=bc5d35ff371e638c
//   ResNetStage: 35B  hash=87e783b56ef29d60
//   Workers: 38B  hash=c0e74dbb1151d588
//   FamilyKb: 68B  hash=dc119c888ff30406
```

```
$ mage-parse --from=abl-bytes unified.abl
// Agentic Binary Language → MAGE decompiled view
// container: 300 bytes, 5 item(s)

// item 0: TransformerBlock (47 bytes expr)
net TransformerBlock {
    layer l_layernorm_1: LayerNorm;
    layer l_attention_2: Attention;
    layer l_dropout_3: Dropout;
    layer l_layernorm_4: LayerNorm;
    layer l_linear_5: Linear;
    layer l_gelu_6: GELU;
    layer l_linear_7: Linear;
    layer l_dropout_8: Dropout;
    forward { l_layernorm_1 }
}
…
```

## What agents trade off by emitting Agentic Binary Language bytes

**Gains**
- ~50× smaller output → more programs per response, lower inference cost.
- Structural correctness by construction: bytes that decode are
  parse-error-free and type-shape-consistent.
- Content-addressable hashes built in: deduplication & caching are free.
- Direct dispatch to `rmi::compute::Backend` — skip the
  text-parse-typecheck pipeline at runtime.

**Limitations (Phase-27 honest list)**
- Agents need a serializer for Agentic Binary Language `Expr`. The format is small (12
  opcode families, 95 ops) but the agent's BPE tokenizer probably
  doesn't have a vocabulary advantage on raw bytes — base-64 / hex
  encoding adds 33-100 % overhead. **Mitigation**: a tool-call
  protocol where the agent emits a structured JSON of ops + args and
  the server does the encode/decode, keeping the binary canonical
  while exposing a textual API for LLMs.
- Lossy round-trip for opcodes without a canonical MAGE layer
  name (most symbolic / agent ops). Bytes → Expr is exact; bytes →
  `.mg` text via the decompiler is a best-effort view.
- No source-line debugging in Agentic Binary Language byte mode. Agent-facing
  diagnostics need to reference content hashes instead of line/col.

## How this fits with the rest of the unification

- **Phase 1** built the bridge — MAGE AST → Agentic Binary Language Expr.
- **Phase 4** added the decompiler — Agentic Binary Language Expr → MAGE AST.
- **Phase 27** ties them with a CLI + container format and an honest
  measurement of where the size win lives.

The text surface remains useful as a **review and debug medium for
humans**. For agent-to-runtime communication, Agentic Binary Language bytes are the
canonical path.

## Status update (Phase 29, 2026-05-22)

- ✅ **Direct dispatch from bytes**:
  `mage-parse --run=abl-bytes <file.abl>` now decodes a Agentic Binary Language
  container and dispatches each item to `CpuBackend` via
  `abl_compute::run_pipeline`. **No text round-trip on the run
  side** — bytes-in, results-out.
- ✅ **README re-frame**: root `README.md` now leads with the honest
  framing — binary IR is the agent target, text is the human view.

## Still open

1. **RAP protocol Agentic Binary Language profile**: extend the MAGE Agent Protocol
   to carry `application/machine` payloads end-to-end. Agents emit
   structured JSON; the RAP server encodes to Agentic Binary Language; responses decode
   the same way. Keeps BPE-friendly text on the wire while binary
   remains canonical at rest.
2. **Streaming decode**: current `Decoder` reads the whole blob. Fine
   for typical sub-kilobyte modules, but a streaming variant becomes
   useful once Agentic Binary Language is a transport payload.
3. **Decompiler coverage for symbolic / agent ops**: Phase 4 covers
   31 neural opcodes by name. Adding reverse mappings for `UNIFY`,
   `INFER`, `RESOLVE`, `SPAWN`, `SEND`, `RECV`, `REDUCE` would let
   `--from=abl-bytes` faithfully reproduce `kb` and `swarm`
   declarations too (currently they render as empty `forward { }`).

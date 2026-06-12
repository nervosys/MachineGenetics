# High-Level Architecture DSL — Composability Design

*The connective tissue for a maximally high-level, composable architecture DSL.
Every claim here is anchored to a measurement (`benchmarks/constructs/`) or to an
existing MechGen subsystem. Status markers: ✅ done · ◻ to build.*

## 1. The premise, corrected by measurement

"Higher level → fewer tokens" is true **only where the construct subsumes
boilerplate**, not in the abstract. Measured (`benchmarks/constructs/run.sh`):

| construct | reduction | why |
|---|--:|---|
| `sum`/`freq`/`scan` vs explicit loop | 60–65 % | names a whole control-flow pattern |
| vocabulary + **custom closure** | 14 % | the closure body is *irreducible payload* |
| Transformer `net` vs PyTorch | 49 % | DSL subsumes the imperative forward |

So the token cost of any program is bounded below:

```
tokens(program) ≈ Σ references-to-named-patterns  +  Σ irreducible-novel-bits
```

A DSL can drive the first term toward **1 token each**; it can do **nothing**
about the second (that is real information). The goal is therefore **not**
"maximally high-level" in the abstract — it is **maximizing the fraction of a
program that is references to reusable patterns**.

## 2. The factorization: a small algebra × a large leaf library

A *massive flat knowledgebase of named compositions* is the wrong shape — it hits
two walls:

- **Tokenizer wall.** The §8 discipline (`vocabulary_audit`) requires each name to
  be a *single BPE token*. There are only ~100–200 K tokens; you cannot have a
  *massive* vocabulary of single-token names.
- **Combinatorial wall.** You cannot name every composition — exponentially many.

Resolution — mirror what §8 did for *data* (`map`/`filter`/`fold` = a handful of
combinators; variety lives in the closures). For *architecture*:

- **Few composition operators** (single-token, orthogonal, closed under
  composition) → arbitrary composability.
- **Many leaf blocks** in a registry, referenced by a **short handle**, not
  carried inline → token efficiency from reuse.

`few operators × many leaf blocks`, **not** `many named compositions`.

## 3. What "precompute results" means — precisely

You **cannot** precompute *outputs* (weight/data-dependent post-training). You
**can** precompute the **lowering/structure**: a parameterized block lowers
deterministically to a fixed byte-sequence. So:

> **content-address the lowering (hash), cache it, dedup identical instances,
> reference by handle.**

First expression of a novel block costs its tokens; every *reuse* is ~1 token;
every *identical* lowering is stored once.

## 4. The build — operators, blocks, registry, binary

### 4.1 Composition operators (the algebra)

| operator | meaning | lowers to | status |
|---|---|---|---|
| `stack N { … }` | repeat a layer body N× | flat `Seq` → `REPEAT` | ✅ landed |
| `residual { … }` | wrap a body in `x + f(x)` | `RES_ADD` | ✅ landed |
| `branch { … } { … }` | parallel paths, then combine | `PAR`-fold | ✅ landed |
| `wrap A { … }` | pre/post a body with op A (e.g. norm) | `A >> body >> A` | ✅ landed |

`stack` is proven: **`stack 12 { block }` = 82 real cl100k tokens vs 839 for the
manual 12× repeat — 10.2× fewer, and flat in depth** (100 layers stays ~83
tokens). Source 271 B vs 2320 B. It expands at parse (`parser.rs`,
`parse_layer_body` + the `stack` arm), so lowering is unchanged.

The three **dataflow** operators (`residual`/`branch`/`wrap`) are different in
kind — they change the graph, not just the layer count — so they can't be flat
parse-time macros. They build an optional `Compose` tree on `NetDef`
(`parser.rs` `parse_compose_body`), populated **only when an operator appears**
(plain/`stack` nets keep `composition: None` and lower exactly as before — zero
regression). The translator (`abl_bridge.rs` `compose_one`) lowers each node to
the RMIL primitive that already existed: `residual`→`Expr::residual` (`RES_ADD`),
`branch`→`Expr::par` (`PAR`), `wrap Op`→`Op >> body >> Op`. Leaves resolve via
the same layer table as declaration order, so a node with no operator is
byte-identical to the flat net. They **compose and nest** arbitrarily — the real
transformer block is `wrap LayerNorm { residual { attn } residual { ffn } }` —
verified end to end: parses, lowers to the correct `RES_ADD`/`PAR` IR,
decompiles, and round-trips through the `REPEAT` fold. *Honest limit:* this is a
surface+lowering feature — the CPU VM does not yet **compute** `RES_ADD`/`PAR`
(they dispatch as unsupported and pass shape through); VM op coverage is separate
work.

### 4.2 Named blocks ✅

```
block TransformerBlock(d, h, ff) {
    layer attn: MultiHeadAttention(d, h);
    layer norm1: LayerNorm;
    layer ff1: Linear(d, ff);
    layer act: GELU;
    layer ff2: Linear(ff, d);
    layer norm2: LayerNorm;
}
net GPT { layer embed: Embedding(50000, 256); stack 12 { TransformerBlock(256, 8, 1024) } forward { embed } }
```

A `block` is a parameterized macro over layers (parser-level: recorded on the
parser, emits no item, expands at the use site with params substituted — so
nothing downstream changes). Measured (real cl100k BPE), full 12-layer GPT:
**block def + `stack 12 { Block(args) }` = 107 tokens** (vs 839 manual, 7.8×
fewer) — and with the block as a **registry handle (def off-context) = 41
tokens** (20.5× fewer). The block definition is paid once and amortizes across
every net that reuses it; a registry keeps it out of context entirely.

### 4.3 Registry handles (the knowledgebase) ✅ (local library; networked registry ◻)

The leaf library is a project's `blocks/*.mg`. `forge check`/`build` resolve a
handle by prepending the library's `block` macro to the entry, so the agent's
source references a block by name while its definition lives off-context.
`forge block` lists the library (the registry's `describe` — progressive
disclosure). Verified end to end: an agent's `src/main.mg` = the 41-token net;
`forge` resolves `TransformerBlock` from `blocks/` and compiles + lowers to ABL.

Next ◻: connect this to the **networked Forge registry server** (capability-
indexed, contract-typed, SHA-256 dedup — already built in `forge/`) so blocks are
shared/published, and make the handle a true single BPE token. Known limit: the
local-library resolution shifts diagnostic line numbers by the prepended defs (a
source-map is the refinement).

### 4.4 Binary dedup — a REPEAT op ✅

`stack N { block }` flattens at parse, so the in-memory IR is a flat `Seq` of
N·P stages — every consumer (execution, shape, decompile) reads that simple
form. At the **encode boundary only**, `fold_repeats` (`abl_bridge.rs`) rewrites
the flat `Seq` into `App(REPEAT, [block, count])` — RMIL's `Op::REPEAT` already
existed and the codec encodes `App` generically, so the block is stored **once**
plus a count. `expand_repeats` is the exact inverse, applied on decode, so the
byte round-trip is the identity (the container's content hash stays on the flat
form). Container bumped to **v3**.

Measured (real `--target=abl-bytes`, `deep_transformer_stack.mg`): the 12-block
container is **141 B vs 126 B for one block — 1.12×** (vs ≈9.6× before), and the
per-item expr is **110 B vs 95 B**: +15 B buys 12× the depth. The folded 141-B
artifact decodes/decompiles back to the **full 72 layers** and `--run=abl-bytes`
**dispatches all 72** — so the shipped net is now O(1) in depth in *both* tokens
and bytes, with execution and round-trip intact.

### 4.5 Typed / contracted composition (safety) ✅

A big block library only helps if composition is *safe*. The shape inferer
(`abl_shape.rs`) is now a **`--check`-time gate** (`check_module_shapes`): every
`net` is lowered and its pipeline threaded layer-by-layer, so a shape-mismatched
composition is rejected *before* the first compute dispatch, with an actionable
diagnostic. It catches the two real failure modes of "arbitrarily composed":

- **Chain mismatch** — `Linear(256, 512)` feeding `Linear(256, …)`: *"shape
  mismatch into a `linear` layer — it expects last dim 256, but the preceding
  layer produced [1, 512]."*
- **Residual not shape-preserving** — `residual { Linear(256, 512) }` (the
  classic mistake, since `x + f(x)` needs `f` to return its input shape): *"the
  residual body outputs [1, 512] … requires the body to return the shape it
  received (last dim 256)."*

Wired into `--check` and `--check --json` (code `E0710`, exits non-zero), so
`forge check` inherits it. Conservative: a net with no weighted layer to anchor
the entry dim is skipped, and only *definite* conflicts trip the gate — every
existing example net and the deep transformer stack still check clean. (Deeper
`@req`/`@ens` contract + SKB-rule checking is the natural next layer; the shape
gate is the high-value floor.)

## 5. Falsifiable targets (verify each piece by measurement)

- `stack` ✅ — token cost O(1) in depth (10.2×, measured).
- named `block` + reference ✅ — 12-layer GPT = 107 tokens (block def + stack
  ref), ≈1.34× a single block; **41 tokens** with the block off-context.
- registry handle ✅ (local) — `forge` resolves a `blocks/` handle; the agent's
  net is 41 tokens, the def off-context; `forge block` lists the library. Next ◻:
  the networked Forge registry + a true single-token handle.
- ABL `REPEAT` ✅ — 12-block container **1.12×** the 1-block container (141 B vs
  126 B; expr 110 B vs 95 B), well under the 1.5× target and vs ≈9.6× before.
  Decodes to the full 72 layers; `--run` dispatches all 72.
- `residual`/`branch`/`wrap` ✅ — each lowers to its RMIL primitive
  (`RES_ADD`/`PAR`/sandwich), they nest (the transformer block =
  `wrap LayerNorm { residual { … } residual { … } }`), and a composed net
  round-trips through the `REPEAT` fold. Plain nets keep `composition: None`
  (zero regression). Limit: the CPU VM doesn't yet *compute* `RES_ADD`/`PAR`.
- typed composition ✅ — a shape-mismatched `residual`/chain is rejected at
  `--check` (and `--check --json`, code `E0710`, non-zero exit) with an
  actionable diagnostic; well-typed nets and every existing example still pass.

## 6. Honest limits

- This makes *recombination of known blocks* nearly free. **Genuinely novel
  architecture stays expensive** — the irreducible floor (§1), and correctly so.
- `precompute` helps *reuse and identical-instance dedup*, never *first
  expression* and never *data-dependent outputs*.
- The operators must stay **few and orthogonal**; every new named composition
  that isn't expressible from the algebra is a smell (re-derive it from
  `stack`/`residual`/`branch`/`wrap` + leaf blocks instead).

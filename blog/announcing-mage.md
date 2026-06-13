# Announcing MAGE: a programming language designed for agents, not humans

*2026-06-12 · MAGE v0.2.0 · NERVOSYS*

Every programming language you've used was designed for a human to read and type.
That made sense for sixty years. It makes less sense now that the entity writing
most of the code in front of you is a language model.

Agents have different constraints than we do. They pay by the *token*, not by the
keystroke. They don't benefit from familiar syntax — they benefit from a grammar
that parses on the first try. They don't ship source for a human to read — they
ship an artifact for a machine to run. A language built around *those* constraints
looks different from one built around ours.

**MAGE** (Machine Genetics) is that language. Today we're releasing **v0.2.0**,
and this post is the introduction.

---

## One program, four forms

The core idea is that a single program has several legitimate representations, and
the right one depends on who — or what — is holding it. MAGE makes all four
first-class and round-trips between them.

**1 · Human-first.** A typed surface that reads like any modern language, for when
a person needs to review it:

```rust
pub fn sum_even_squares(xs: [i32]~) -> i32 {
    var total: i32 = 0;
    for x in xs {
        if x % 2 == 0 { total = total + x * x; }
    }
    total
}
```

**2 · Agentic-first.** The *same program* in sigils (`+f` = `pub fn`) over a
standard vocabulary — `map`/`filter`/`fold`, each a **single BPE token**. It costs
**25 % fewer real tokens** and computes the identical result:

```rust
+f sum_even_squares(xs) = fold(map(filter(xs, fn(x) => x % 2 == 0), fn(x) => x * x), 0, fn(a, b) => a + b)
```

**3 · Declarative.** Higher-level structures — like neural networks — are
*declared*, not hand-wired. More on this below.

**4 · Binary.** What an agent actually ships: the program lowered to a compact
binary IR (the "Agentic Binary Language"), which **decompiles back to the source
above**. A small neural module is ~300 bytes of binary versus ~1.8 KB of text.

> An agent writes intent in form 2 (fewest tokens), the compiler verifies it
> against form 1's types, and ships form 4 (fewest bytes). No human-first language
> hands you all of that in one artifact.

---

## The headline of v0.2.0: composing architectures

The most interesting thing an agent does isn't writing a `for` loop — it's
*assembling* something large out of known parts. So v0.2.0 makes that the
language's strength.

There's a deliberately small **algebra** of composition operators, and a large,
shared library of reusable **blocks**. Here is a real transformer GPT:

```rust
// Published once to a shared, content-addressed registry; referenced by name.
block TransformerBlock(d, h, ff) {
    wrap LayerNorm {                                         // pre/post sandwich
        residual { layer attn: MultiHeadAttention(d, h); }  // x + f(x)
        residual { layer ff1: Linear(d, ff); layer act: GELU; layer ff2: Linear(ff, d); }
    }
}

net GPT {
    layer embed: Embedding(50000, 256);
    stack 12 { TransformerBlock(256, 8, 1024) }             // repeat — O(1) in depth
    forward { embed }
}
```

Four ideas are doing the work here:

- **A few orthogonal operators.** `stack N { … }` (repeat), `residual { … }`
  (`x + f(x)`), `branch { … } { … }` (parallel paths), `wrap Op { … }`
  (`Op >> body >> Op`). They compose and nest arbitrarily, and they aren't
  decoration — they lower to real IR primitives (`REPEAT` / `RES_ADD` / `PAR`) and
  **execute** on the CPU backend.
- **Blocks shared across projects.** `forge publish` stores a block under the
  **SHA-256 of its source** — identical definitions deduplicate to one artifact —
  and any project references it *by name*. The 12-layer GPT above is ~41 real
  tokens because the block's body lives in the registry, off-context.
- **Composition is type-checked.** A shape-mismatched composition — say a
  `residual` whose body changes dimension, so `x + f(x)` can't add — is **rejected
  at `forge check`** with an actionable message, before any compute runs.
- **Depth is free in the artifact.** `stack 12` doesn't ship twelve copies; it
  folds to one block plus a count, so the binary is **flat in depth**.

The underlying principle is simple. The token cost of a program is roughly:

```
tokens(program) ≈ Σ references-to-named-patterns  +  Σ irreducible-novel-bits
```

A good DSL can drive the first term toward *one token each*. It can do nothing
about the second — that's real information, and pretending otherwise would be
dishonest. So the goal isn't "maximally high-level" in the abstract; it's
maximizing the fraction of a program that is *references to reusable patterns*.
**Few operators × many leaf blocks**, not a giant flat catalog of named things.

---

## Measured, not asserted

MAGE is a NERVOSYS research project with a strict rule: every number we publish
is produced by **actually running something** — compiling and comparing output,
or counting real cl100k BPE tokens of the exact files — not by judgment. The whole
story runs as one reproducible command (`benchmarks/capstone/run.sh`):

> `forge publish` a block → a ~41-token GPT → `forge check` (resolve + shape gate)
> → `forge build` (REPEAT-folded binary, **1.09×** for 12 blocks vs 1) → the full
> GPT **runs** (`dispatched=97, unsupported=[]`).

A few of the measurements:

**Cross-language terseness + executability.** The same five tasks, written
idiomatically in each language, compiled and run on the host toolchain with stdout
compared to the expected value. All six runnable languages execute 5/5; MAGE is
the tersest by real tokens *and* by bytes:

| Language | Executes | Real cl100k tokens | Source bytes |
|---|:--:|:--:|:--:|
| **MAGE** | **5 / 5** | **173** | **401** |
| JavaScript | 5 / 5 | 199 | 513 |
| TypeScript | 5 / 5 | 220 | 593 |
| Go | 5 / 5 | 271 | 727 |
| Rust | 5 / 5 | 275 | 769 |
| Java | 5 / 5 | 297 | 1033 |

**The architecture DSL vs. PyTorch.** The same network in MAGE's `net` DSL
versus an equivalent PyTorch `nn.Module` (which must also spell out the imperative
`forward`). The saving grows with complexity, and the declaration then lowers to a
binary a further ~34–42 % under its own text:

| Architecture | MAGE | PyTorch | fewer tokens |
|---|:--:|:--:|:--:|
| MLP | 50 | 78 | **36 %** |
| Transformer | 73 | 142 | **49 %** |

**Agentic-first tooling.** We applied the same lens to MAGE's own toolchain,
`forge`. Giving it a self-describing, machine-readable surface (a token-compact
`manifest`, `--json` on every command, effect classes) lifted result-parseability
and effect-gating from 0 → 1.00 and made command discovery **2.36× cheaper in real
tokens** — for a measured cost of about +12 % tokens per structured result, which
we report rather than hide.

---

## What this is — and isn't

In the spirit of measuring honestly, the limits matter as much as the wins.

MAGE is a **prototype**. The runtime is a young tree-walking evaluator — no JIT,
and `await` runs to completion. The executability results above record a
*threshold crossed* on curated tasks, not a performance lead or a claim of parity
with production runtimes. And while most of our numbers are measured, our overall
"agentic-SWE scorecard" includes four 0–1 axes that are *curated judgment* — we
keep those visibly separate from the measured tables, and they were bias-audited
(scores corrected *down* on evidence).

What *is* real and verified, today: the language parses and type-checks, executes
general programs, lowers neural networks to a compact round-tripping binary,
composes architectures from a shared registry with a type-safety gate, and runs
them — all behind **1,200+ passing tests**. v0.2.0 is the version where the
architecture-DSL story works end to end.

---

## Try it

MAGE is open source (Apache-2.0) at
**[github.com/nervosys/MachineGenetics](https://github.com/nervosys/MachineGenetics)**.

```bash
git clone https://github.com/nervosys/MachineGenetics
cd MachineGenetics/prototype
cargo build --release

# run a program
./target/release/mage-parse --eval fib.mg fib 25     # → 75025
```

Then read `benchmarks/capstone/run.sh` to watch the whole thesis — publish a
block, write a 41-token GPT, check it, fold it to a binary, and run it — in a
single command.

We think languages for agents are going to look meaningfully different from
languages for people. MAGE is our argument for *how*. We'd love for you to
poke holes in it.

— The NERVOSYS team · `opensource@nervosys.ai`

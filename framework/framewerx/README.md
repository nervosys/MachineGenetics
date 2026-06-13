# Framewerx-MG

> **Agent-first neurosymbolic framework, implemented in MAGE, over RMI's binary IR.**
> Analogous to FLAX (over JAX) — but written for agents, not humans.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  Framewerx-MG                       (this directory)         │
│  • Module trait + Layer / Optim / Loss / TrainState          │
│  • Neurosymbolic composition (net + kb blocks)               │
│  • Ontology-discoverable (one RAP call lists every layer)    │
│  • Sigil-mode declarations for token efficiency              │
│  Implementation: .mg source                                  │
└────────────────────────────┬─────────────────────────────────┘
                             │ lowers to Agentic Binary Language via abl_bridge
                             ▼
┌──────────────────────────────────────────────────────────────┐
│  RMI / Framewerx (Rust crate)        ← JAX-equivalent        │
│  • 107 Agentic Binary Language opcodes (Neural / Symbolic / Control / ...)      │
│  • CpuBackend dispatch                                       │
│  • Autograd, optimizers, distributed                         │
│  Implementation: Rust                                        │
└──────────────────────────────────────────────────────────────┘
```

| Layer | Analogue | Implementation | Purpose |
|---|---|---|---|
| **Framewerx-MG** | FLAX | MAGE | Composable modules an agent can declare and discover |
| **RMI** | JAX | Rust | Tensor primitives, opcodes, autograd, dispatch |

## Why agents, not humans

| Need | How Framewerx-MG addresses it |
|---|---|
| **Token efficiency** | Sigil-mode `+f`/`m`/`v` syntax. `net { layer ... }` is shorter than FLAX's `@nn.compact` decorator + Python class boilerplate. Native-lexer token parity with Rust (measured 0.998). |
| **Discoverability** | Every module type is in the ontology (`ontology/section { "section": "framewerx_modules" }`). Agent calls one RAP method, gets the full menu of layers, optimizers, loss functions. |
| **Reliability** | Every module carries a parse-verified example in the ontology. 5-stage recovery pipeline catches typos / partial outputs. CI floors track parse rate (100%) and heal recovery. |
| **Neurosymbolic** | `net` and `kb` are first-class siblings — Framewerx-MG composes them via the `Hybrid` module. The RMI ontology adapter (already wired) translates SKB rules to RMI Concepts under `air.skb.<database>`. |
| **Binary transport** | A Framewerx-MG module compiles to an Agentic Binary Language container. Agents ship bytes, not text. (See `abl/encode` and `pipeline/recover-and-encode`.) |

## Surface layout

```
framework/framewerx/
├── README.md                  ← you are here
├── src/
│   ├── module.mg              ← Module trait + base abstractions
│   ├── layers/
│   │   ├── linear.mg          ← Linear, Bias
│   │   ├── conv.mg            ← Conv2D, MaxPool, AvgPool
│   │   ├── attention.mg       ← Attention, MultiHeadAttention
│   │   ├── norm.mg            ← LayerNorm, RMSNorm, BatchNorm
│   │   └── activation.mg      ← ReLU, GELU, SiLU, Sigmoid, Tanh
│   ├── optim/
│   │   ├── sgd.mg
│   │   └── adam.mg
│   ├── loss.mg                ← MSE, CrossEntropy, BCE
│   ├── train.mg               ← TrainState + step function
│   └── neurosymbolic.mg       ← Hybrid (net + kb composition)
└── examples/
    ├── mlp_classifier.mg
    ├── transformer_block.mg
    └── neurosymbolic_qa.mg
```

## Quick example

A multilayer perceptron in Framewerx-MG:

```mage
u framewerx.{Module, Linear, ReLU, MLP}

net Classifier {
    layer fc1: Linear(784, 128);
    layer act1: ReLU;
    layer fc2: Linear(128, 64);
    layer act2: ReLU;
    layer head: Linear(64, 10);
    forward { head(act2(fc2(act1(fc1)))) }
}
```

This `net` block:
1. Parses via MAGE's existing `net`/`layer`/`forward` syntax.
2. Lowers to Agentic Binary Language via `abl_bridge::lower_module` — `Linear` and `ReLU` map to opcodes `0x0002` and `0x0010`.
3. Encodes as Agentic Binary Language bytes (`abl/encode` RAP method).
4. Executes on `CpuBackend` (`abl/run` RAP method).

No text round-trip; no Python boilerplate; one call per stage.

## Agent bootstrap

```
agent: GET ontology/section { "section": "framewerx_modules" }
  → { modules: [Linear, Conv2D, ReLU, GELU, LayerNorm, ...],
      optimizers: [SGD, Adam],
      losses: [MSE, CrossEntropy, BCE],
      examples: [mlp_classifier, transformer_block, ...] }

agent: POST pipeline/recover-and-encode { source: "<.mg model>" }
  → { ok, abl_hex, items: [...] }

agent: POST abl/run { source: ... }
  → { runs: [{ name, status: "dispatched", output_sum, output_shape }] }
```

Three RAP calls to define, encode, and run a model.

## Status

| | |
|---|---|
| Module layouts | scaffolded |
| All `.mg` source parses cleanly | enforced by `examples_all_parse` test |
| Ontology section `framewerx_modules` | added P72 |
| Reliability via ontology | every layer/optim/loss carries a parse-verified example |
| Real training dispatch | reuses existing RMI CpuBackend (no new compute code) |

# Tensor product binding: the join between MAGE's two halves

> **Status: specification. Nothing here is implemented.** Labelled in the
> convention `MAGE_SPEC.md` uses for constructs it documents and does not build,
> and the reason for writing it down before building it is in the last section.

## The gap this fills

MAGE already ships hybrid neurosymbolic artifacts. `abl_bridge.rs` lowers both
`ItemKind::Net` and `ItemKind::Kb`, so **one ABL container can carry a trained
network and a knowledge base**, and `SymbolicView` decodes the symbolic half —
fact arities, rule parameter counts, and symbol ids, one per `RESOLVE` and
`UNIFY` — against the container's symbol table.

The two halves coexist. They do not connect. There is no representation in this
language in which a symbol id and a vector are the same object, so every bridge
between a `kb` and a `net` today is glue code written outside the language, and
nothing in the type system or the differentiability lattice can say anything
about it.

**Tensor Product Representations are the algebra for that join**: a vector that
holds symbolic structure as a sum of filler–role bindings, with an inverse.
McCoy, Soulos, Linzen and Smolensky (*The Emergent Symbolic Structure of
Artificial Neural Networks*) give the empirical case that trained networks
already approximate this structure; what follows is the case for making it
sayable.

## The operations

For a filler vector **f** ∈ ℝⁿ and a role vector **r** ∈ ℝᵐ:

```
bind(f, r)        =  f ⊗ r                    ∈ ℝ^(n×m)
superpose(b₁..bₖ) =  Σᵢ bᵢ                     ∈ ℝ^(n×m)
unbind(B, r*)     =  B · r*                    ∈ ℝⁿ
```

where **r\*** is the role's *dual* (the unbinding vector). For an orthonormal
role set, r\* = r; otherwise it is the corresponding row of the pseudoinverse of
the role matrix.

Nothing here is new mathematics. It is stated because the shape rules below are
the part MAGE has to get right, and because the third line is where the
interesting failure lives.

## Shape rules

Written in the form `shape.rs` already uses for `Linear`, `Attention` and the
rest — each layer maps an input shape to an output shape, with a fresh dimension
where an argument is not statically known.

| Construct | Input | Output | Notes |
|---|---|---|---|
| `Bind(role_dim)` | `[..., F]` | `[..., F, R]` | rank increases by one |
| `Superpose(k)` | `k × [..., F, R]` | `[..., F, R]` | all `k` must agree exactly |
| `Unbind(role_dim)` | `[..., F, R]` | `[..., F]` | rank decreases by one; `R` must match |
| `RoleSet(n_roles, R)` | — | `[n_roles, R]` | a learnable parameter block |

The rank change is the point: **binding is the only operation in the net DSL
that adds structure rather than transforming it**, and unbinding is the only one
that consumes it. A `Superpose` of tensors whose role dimensions disagree is a
shape error, not a broadcast — silently broadcasting a role dimension would
destroy exactly the structure the construct exists to preserve.

## Surface syntax

Fitting the existing `net` DSL, where a layer is `name: Type(args)`:

`RoleSet`, `Bind` and `Superpose` are not layer types the shape checker knows,
so the block below is a check failure and is meant to be — **invalid MAGE today**:

```mg
net Encoder {
    layer roles: RoleSet(8, 64);          // 8 learnable role vectors, dim 64
    layer fill:  Embedding(50000, 256);   // fillers
    layer bound: Bind(64);                // [B, T, 256] -> [B, T, 256, 64]
    layer sup:   Superpose(8);            // collapse the 8 slots
    forward { fill }
}
```

`unbind` is the query direction and reads better as an expression than a layer,
which is the one place this proposal wants something the DSL does not have.
There is no `unbind`, and no expression form for a layer operation at all —
**invalid MAGE today**:

```mg
f role_of(b: tensor[f32, F, R], r: tensor[f32, R]) -> tensor[f32, F] {
    unbind(b, r)
}
```

## Why this passes the differentiability lattice

`bind` is an outer product; `superpose` is a sum; `unbind` is a contraction. All
three are **smooth** — no kinks, no branches, no discrete results — so a network
that binds and unbinds is `Smooth` under `differentiable.rs`, and the claim is
checkable rather than asserted.

That is the substantive consequence, and it is worth stating plainly:

> **Binding makes the symbolic half reachable by gradients.**

A role assignment expressed as a `RoleSet` parameter is learnable. A structure
built by `bind`/`superpose` is differentiable end to end. For a language that
claims to be both neurosymbolic and differentiable by design, this is where the
two claims meet — and until now they did not meet anywhere, because the symbolic
half was symbol ids and the neural half was tensors.

## Lowering: new semantics, not new operations

MAGE's IR has 107 ops including `matmul`, `mul`, `add`, `reshape` and
`transpose`. Every operation above is expressible in them:

| Construct | Lowering |
|---|---|
| `bind(f, r)` | `reshape(f, [..., F, 1]) * reshape(r, [..., 1, R])` — broadcast multiply |
| `superpose` | `add`, folded |
| `unbind(B, r*)` | `matmul(B, r*)` — contraction over the role axis |
| `RoleSet` | a parameter block, like any other |

So the ABL compute backend already executes this, and an artifact containing
bindings is a normal container: same format, same provenance, same
`--describe=abl` introspection. No backend work is required to prototype it, and
that is the main argument for doing this before anything else in this area.

## The failure this construct has, stated up front

Superposed bindings **interfere**. Unbinding a superposition of *k* bindings
with a non-orthogonal role set returns the intended filler plus crosstalk from
the other *k−1*:

```
unbind(Σᵢ fᵢ ⊗ rᵢ, r₁*) = f₁ + Σᵢ₌₂..ₖ fᵢ (rᵢ · r₁*)
```

The error term vanishes only when roles are orthonormal. This is a property of
the algebra, not a defect in an implementation, and it is the reason the
construct needs a **stated** contract rather than an implied one:

- **Orthonormal roles** — exact unbinding, capacity limited to `R` roles.
- **Random near-orthogonal roles** — approximate unbinding, capacity higher,
  error growing with *k*/`R`.
- **Learned roles** — whatever training produced, with no guarantee at all.

A `RoleSet` should therefore carry which of the three it is, and `unbind` should
be honest about the consequence. The natural home for that is the verdict
vocabulary already being built for this repository: exact unbinding is `Proven`;
near-orthogonal unbinding is `Evidenced { n }` at a measured interference bound;
learned roles are `Unknown` until something measures them.

**A construct whose error term is silent is the shape of defect this repository
has spent its recent history removing.** Writing the interference bound into the
specification is cheaper than discovering it in a model's accuracy six months
later.

## How the claim gets verified, in both senses

Consistent with `DIFFERENTIABILITY.md` and with the statodynamic pairing:

| | claim | oracle |
|---|---|---|
| **Deductive** | shapes compose; `unbind ∘ bind = id` for orthonormal roles | the shape checker and the algebra |
| **Inductive** | a *trained* network's representations are described by this structure | DISCOVER-style approximation fidelity, then causal intervention |

The second is the paper's contribution and it is not a compiler's job: MAGE is
the notation, `StatodynamicAnalysis` is the verifier. What MAGE owes the
verifier is an artifact whose structure is declared rather than inferred — which
is precisely what a `bind` in the IR provides and a hand-rolled outer product
does not.

Intervention deserves particular note, because it is the same discipline this
repository applies to its own guards: the paper's strongest evidence is not the
99% approximation fidelity but that **editing a binding changed the output
predictably, where random perturbation did not**. A claim earns belief by being
made to fail on purpose.

## Why this is a specification and not a branch

Three things are unresolved, and each is a decision rather than an unknown:

1. **Where `unbind` lives.** As an expression it needs a typing rule referencing
   role dimension; the `net` DSL has no expression forms today.
2. **Whether `RoleSet` orthonormality is enforced or merely declared.**
   Enforcing it constrains training; declaring it makes the guarantee
   conditional on something nothing checks.
3. **Whether the `kb` half addresses this at all.** Binding a `fact`'s symbol id
   as a filler is the obvious next step and the one that would make the join
   real rather than adjacent — and it is a language design question about how
   the symbol table relates to the embedding table, which is not mine to settle.

The measured argument from `DIFFERENTIABILITY.md` applies here too: 0 of 155
functions in this repository are differentiable, and 125 have no floating-point
parameter. **There is no numerical MAGE code to try this on yet.** Building the
construct before there is a program that wants it would repeat the mistake this
document's sibling was written to avoid.

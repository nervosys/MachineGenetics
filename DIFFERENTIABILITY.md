# Differentiability by design

> **Status: the lattice and the inference pass are built (`prototype/src/differentiable.rs`).
> `grad` as an expression, and differentiation as an ABL→ABL transform, are
> designed and not built** — labelled as such below, in the convention
> `MAGE_SPEC.md` uses for constructs it documents and does not implement.

## The claim, stated so it can be false

"Fully differentiable" is not a claim this language can make, and saying it
anyway would put a ninth figure in the tree that survives because nobody checks
it. `x < 0` is not differentiable at zero, and no amount of design removes that.

What MAGE can claim, and what this document means by the phrase:

> **Every function MAGE reports as differentiable is differentiable almost
> everywhere on its domain, and the compiler says which of the four it is
> rather than assuming the best one.**

"Almost everywhere" is the standard sense — differentiable except on a set of
measure zero. That is the same guarantee PyTorch and JAX actually provide, and
*A Simple Differentiable Programming Language* (POPL 2020) is what makes it a
claim rather than folklore: it gives a differentiable language an operational
semantics matching the implementation and a denotational semantics grounded in
real analysis, and proves the two agree. A language that says "differentiable"
without stating which semantics it means has said nothing.

## The lattice

Differentiability is a propagating, inferable, declarable property with a join —
structurally the same shape as MAGE's effect system, which is why the inference
pass is modelled on `effects.rs` rather than invented.

```
Smooth  ⊑  AlmostEverywhere  ⊑  NotDifferentiable
                     ⊒
                  Unknown
```

| Status | Meaning |
|---|---|
| `Smooth` | Differentiable everywhere on its domain. Arithmetic on floats, composition of smooth things. |
| `AlmostEverywhere` | Differentiable off a measure-zero set: `abs`, `relu`, `max`, `min`, a branch whose condition tests a continuous value. |
| `NotDifferentiable { reason }` | Discrete input or output, an effect that breaks referential transparency, or a call to something that is not differentiable. Carries **why**. |
| `Unknown { reason }` | The pass could not determine it — an unresolved call, a construct it does not model. |

Join is worst-case, like effect union: a function is as differentiable as its
least differentiable part.

**`Unknown` is not a fourth grade of differentiability; it is the absence of a
verdict**, and it exists because the alternative is reporting `Smooth` for
"I did not look". That is the `Unreached` row from
[StatodynamicAnalysis](../StatodynamicAnalysis/README.md)'s statodynamic lattice
— *the claim is untested, not clean* — and this repository has spent enough time
removing figures that were green for want of a measurement.

## Where the almost-everywhere boundary sits

| Construct | Status | Why |
|---|---|---|
| `+ - * /` on floats | `Smooth` | `/` is undefined at zero, not non-differentiable |
| `abs`, `relu`, `max`, `min` | `AlmostEverywhere` | one kink, measure zero |
| `floor`, `ceil`, `round` | `AlmostEverywhere` | derivative is 0 a.e. — *defined, and useless*; see below |
| `< > == !=` | `NotDifferentiable` | the result is `bool`, a discrete type |
| `?` (if) on a float condition | `AlmostEverywhere` | the branch boundary is measure zero |
| `?` (if) on a `bool` variable | inherits the branch bodies | the discreteness is already accounted for where the `bool` was made |
| `@` (for) over a fixed range | inherits the body | unrolls to composition |
| `@@` / `@w` (loop / while) | `Unknown` | trip count may depend on the value being differentiated |
| integer or `str` parameter | `NotDifferentiable` | no derivative with respect to a discrete type |

The `floor` row is the honest awkward one. Its derivative *is* zero almost
everywhere, so calling it `AlmostEverywhere` is true and actively misleading:
optimising through it gets no signal. The pass reports the status; a future
`grad` should warn when a gradient path is a.e.-zero, which is a different
question from whether it exists.

## Interaction with the effect lattice

A function's output must be a function of its inputs, or the derivative is not
defined regardless of the arithmetic. MAGE already computes this: 1,291 lines of
effect inference with the annotation as an upper bound.

So differentiability inherits a **necessary condition for free**:

```
NON_FUNCTIONAL = { IO, FS, Net, Env, Time, Rng, Llm, Agent, Proc, Async }
effects(f) ∩ NON_FUNCTIONAL ≠ ∅   ⇒   NotDifferentiable
```

`Gpu` and `Npu` say *where* a function computes, not whether it is a function,
and do not disqualify. Neither does `Alloc`. `Rng` does: a stochastic function
has no derivative in this sense, and the reparameterisation trick is a change of
program, not a change of verdict.

This is the cheapest correct thing in the design. It required no new analysis —
only the decision about which effects are disqualifying, which is written above
so it can be argued with.

## Differentiability is a typeclass, which is why item 21 reopens

Deciding that `tensor[f32]` and `f32` have derivatives while `i64`, `bool` and
`str` do not is a trait obligation at a call site. On 2026-09-03 item 21
— enforcing `~>` bounds — was closed on the evidence that no `.mg` source writes
a bound, so a solver would have nothing to check.

**That conclusion was right about the code and wrong about the direction.** A
`Differentiable` bound is the demand signal, and item 21 is reopened with this as
the reason. The inference pass here does not need it: it computes a property
over the call graph. A `grad` expression whose typing rule *requires* its
argument to be differentiable does.

## Design, not implemented

### `grad` as an expression

`grad` is a reserved word today with no expression form — `grad(loss, w)` is a
parse error, and `MAGE_SPEC.md` records that. The typing rule it wants:

```
Γ ⊢ e : tensor[f32, S]      diff(e) ⊑ AlmostEverywhere      w : Param
─────────────────────────────────────────────────────────────────────
                  Γ ⊢ grad(e, w) : tensor[f32, shape(w)]
```

with `diff(e) = NotDifferentiable` an error naming the reason, and
`diff(e) = Unknown` a warning, because refusing a program the compiler merely
failed to analyse is worse than saying so.

### Lowering as an ABL→ABL transform

`autograd.rs` has a working reverse-mode tape — `GradNode`, `GradOp`,
`GradTape`, `backward` — driven from `train` blocks. That is AD attached to one
construct rather than a property of the language.

The design that fits MAGE rather than PyTorch: **differentiation is a transform
from an ABL container to an ABL container.** ABL is a static container of 107
IR ops; the derivative of a shipped artifact should be a shippable artifact — the
same format, verifiable by the same tools, carrying the same provenance. A tape
is a runtime structure and cannot be shipped, which is the whole difference
between a library and a language property.

This also gives the correctness statement somewhere to live: the transform is
what an operational-vs-denotational equivalence proof would be *about*.

## What it says about this repository today

Run over all 101 tracked `.mg` sources, the pass analyses 155 functions:

| status | count |
|---|---:|
| `NotDifferentiable` | 154 |
| `Unknown` | 1 |
| `AlmostEverywhere` | 0 |
| `Smooth` | 0 |

with the reasons dominated by one:

```
 125  no floating-point parameter to differentiate with respect to
  10  performs the `FS` effect, so its output is not a function of its inputs
   8  performs the `IO` effect
   6  performs the `Llm` effect
```

**Nothing in this repository's MAGE code is differentiable**, and the reason is
not that the analysis is too strict — it is that 125 of 155 functions take no
floating-point argument at all. The corpus is agent and tooling code.

That is the same shape as the finding that closed item 21: the numerical surface
of MAGE is the `net` DSL, not `f` functions, and a pass over functions is
looking in the wrong place for a subject. It is reported here rather than
buried, because a pass whose honest output is "0 differentiable functions"
should say so before anyone quotes a better-sounding number.

**The next step follows from it**: extend the analysis to `net` / `layer` /
`train`, where the differentiable computation in this language actually lives,
and where `autograd.rs` already builds a tape.

## How the claim gets verified, in both senses

Differentiability produces one claim of each epistemic kind, and they should not
be conflated:

| | claim | oracle |
|---|---|---|
| **Deductive** | this function *has* a derivative a.e. | the inference pass, over all inputs |
| **Inductive** | the computed derivative *is* the right one | gradient checking against finite differences, at *n* sampled points |

Gradient checking is inductive verification of a deductive claim, and it is the
standard practice in every AD implementation. The pairing is exactly the one
`StatodynamicAnalysis` formalises, and it is worth keeping the vocabulary shared:
`Proven` that a derivative exists, `Evidenced { n }` that the implementation
computes it, and neither one standing in for the other.

# Differentiability by design

> **Status: the lattice and the inference pass are built
> (`prototype/src/differentiable.rs`), over all three subjects — `f` functions,
> `net` definitions and `train` blocks — and reachable as
> `mage-parse --differentiable <file.mg> [--json]`. `grad(e, w)` is an
> expression that typechecks and runs, for scalars, with the differentiability
> obligation as a premise of its typing rule. `grad` over tensors, and
> differentiation as an ABL→ABL transform, are designed and not built** —
> labelled as such below, in the convention `MAGE_SPEC.md` uses for constructs
> it documents and does not implement. The tensor case is *refused* by the type
> checker rather than accepted and left to fail later.

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

## The `net` DSL, which is where the derivative actually lives

Everything above is about `f` functions, and measured against this repository
that half of the pass reports **0 of 155 functions** differentiable — 125 of
them for want of a floating-point parameter. That is not the analysis being
strict. It is the corpus: MAGE's `f` functions are agent and tooling code, and
its numerical surface is `net` / `layer` / `train`. A pass over functions is
looking in the wrong place for a subject, which is the same finding that
reopened item 21.

So the pass has a second half, over the `net` DSL. Three decisions shape it,
and each is a decision rather than a detail.

### A net is judged with respect to its parameters

`train` optimises weights, so the derivative anyone wants from a net is
∂loss/∂w, not ∂out/∂in. This is not a technicality — it is why `Embedding` is
`Smooth`. Its *input* is a discrete token id with no derivative at all, while
its table is an ordinary dense parameter and the gradient reaching it is the one
training uses. Judged on inputs, every language model in this repository would
report `NotDifferentiable` at its first layer, which would be a true sentence
about a question nobody asked.

### The verdict is keyed on the surface layer type, not the opcode

`abl_bridge` maps `HardSigmoid` and `Sigmoid` onto one `Op::SIGMOID`, and says
so in a comment — "close-enough lowering". For code generation that is fine.
For this pass it is not: only one of the two has a kink, and a verdict read off
the lowered opcode reports the kinked one as `Smooth`. Same for `HardSwish` and
`SiLU`. The tables are therefore keyed on what the source says.

The stronger version of the same point: an unrecognised layer type lowers to
`Op::IDENTITY`, and an identity is perfectly smooth. Reading verdicts off the
lowered form would report a layer nobody has ever analysed as the *best* state
in the lattice. Unrecognised layer types are `Unknown`, and
`differentiable::tests::an_unrecognised_layer_is_unknown_not_smooth` is the
test that says so.

### Which layers get judged is the lowering's own answer

`forward { fc1 }` names one layer in a net that declares three, and it means
"run all three in declaration order" — the bridge decides this by counting
application nodes and falling back. That heuristic is not re-implemented here.
`NetTranslator` records the layer types it applied, and the pass reads them, so
the analysis is of the program the compiler builds rather than of a plausible
reconstruction of it. A declared layer the forward pass never reaches
contributes no gradient path and is not part of the verdict; the report says so
by printing applied-versus-declared when they differ.

### Where the boundary sits for layers

| Layer | Status | Why |
|---|---|---|
| `Linear`, `Conv2D`, `MatMul`, `Embedding` | `Smooth` | affine in the parameters |
| `Attention` and every variant, `Softmax` | `Smooth` | softmax and matmul are smooth; the causal mask is a constant, not a branch |
| `LayerNorm`, `RMSNorm`, `BatchNorm`, `GroupNorm` | `Smooth` | rational in the batch statistics |
| `GELU`, `SiLU`, `Sigmoid`, `Tanh`, `Mish`, `Softplus` | `Smooth` | smooth activations |
| `LSTM`, `GRU`, `Mamba`, `S4`, the graph layers, the PEFT adapters | `Smooth` | gates are `tanh`/`sigmoid`; the rest is affine |
| `MSE`, `CrossEntropy`, `BCE`, `NLL`, `KLDiv` | `Smooth` | the losses `train` blocks actually use |
| `ReLU`, `LeakyReLU`, `ELU`, `SELU`, `MaxPool`, `Huber` | `AlmostEverywhere` | one kink |
| `AdaptivePool` | `AlmostEverywhere` | the name does not say whether it averages or maxes; this is the join over both readings, not a hedge |
| `SparseMoE`, `TopKRouter`, `SwitchRouter` | `AlmostEverywhere` | top-k selection is piecewise constant, so the cell boundaries are measure zero |
| `Int8Linear`, `Int4Linear`, `BitNetLinear` | `AlmostEverywhere` | quantisation rounds: the derivative is zero a.e., the same *defined-and-useless* row as `floor` |
| `Dropout` and friends | `AlmostEverywhere` | see below |
| anything else | `Unknown` | never `Smooth` |

**`Dropout` is the one that deserves an argument rather than a table row.** The
layer computes `mask ⊙ x / (1-p)`, which is *linear* given the mask, and the
mask is noise drawn independently of the input — so the conditional derivative,
the one every AD implementation actually computes, exists and is the standard
object. It is reported one grade below `Smooth` rather than as `Smooth` because
the function that includes the sampling step is not a function of its inputs
alone, which is the same rule `NON_FUNCTIONAL` applies to `Rng` for ordinary
functions. `AlmostEverywhere` says *there is a derivative, and it is not
unconditional*. Calling it `NotDifferentiable` would make nearly every real
network non-differentiable and would be wrong about what training does; calling
it `Smooth` would hide the conditioning.

### `train`

A `train` block's verdict is the join of its net, its loss and its body. The
optimiser is deliberately absent: it *consumes* gradients rather than
contributing to the function being differentiated, so `SGD` versus `Adam`
cannot change whether a derivative exists, and
`differentiable::tests::the_optimiser_does_not_change_the_verdict` pins that. A
`train` naming a net the module does not define is `Unknown` — nothing was
analysed, so there is no verdict to give, which is not the same as a negative
one.

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

## `grad`, and where the obligation is discharged

`grad(e, w)` is an expression as of 2026-09-07, for scalars. `∇(e, w)` is the
same thing in sigil mode — the Gradient row of the sigil table, which had
published both spellings for a construct that had neither.

```mg
f slope(w: f64) -> f64 { grad(w * w * 3.0 + w * 2.0, w) }
```

`slope(4.0)` is `26.0`, exactly — the chain rule, not a finite difference.

```
Γ ⊢ e : f32     Γ ⊢ w : f32     diff(e) ⊑ AlmostEverywhere
──────────────────────────────────────────────────────────
              Γ ⊢ grad(e, w) : typeof(w)
```

The rule shipped narrower than the one this document proposed, in two ways, and
both are corrections rather than compromises.

**The gradient has the shape of `w`, not of `e`.** The proposed conclusion,
`tensor[f32, shape(w)]`, was right about the shape; the premise
`e : tensor[f32, S]` was wrong outright. `e` must be **scalar** — the derivative
of a non-scalar is a Jacobian, which is a different construct. `MAGE_SPEC.md`
§10.4's T-Grad rule had `L : Tensor⟨T, []⟩`, a *scalar* tensor, so the two
documents disagreed with each other as well as with what was buildable. There is
now one rule.

**Tensors are refused, not deferred.** `grad` over `tensor`/`param` is designed
and not built, and the type checker says so at the call site instead of
accepting a program the evaluator cannot run — it has no tensor value at all.
Accepting it would be the documented-but-unimplemented shape the failure
taxonomy collects under §4.

### The obligation is not checked by the type checker

`diff(e) ⊑ AlmostEverywhere` is the interesting premise, and it is discharged by
`differentiable.rs`, not by `types.rs`. The reason is not layering aesthetics.

This program is **invalid**, and this is the diagnostic it gets:

```mg
+f noisy(x: f64) -> f64 / io { println("tick"); x * 2.0 }
f bad(w: f64) -> f64 { grad(noisy(w), w) }
```

> error: in `bad`: `grad` needs an expression with a derivative, and this one
> has none — performs the `IO` effect, so its output is not a function of its
> inputs

Nothing in `grad(noisy(w), w)` says that `noisy` reaches a console, and nothing
in `grad(cmp(w) * 2.0, w)` would say that `cmp` compares two floats. **`diff` is
a property of the call graph**, and `infer_expr` sees one expression at a time.
Putting the premise where the evidence is, is what stops the type checker
answering a question it cannot see the answer to.

The severities are deliberately asymmetric: `NotDifferentiable` is an **error**
naming the reason, `Unknown` is a **warning**. Refusing a program because the
compiler failed to analyse it would turn the absence of a verdict into a
rejection, and the fourth state exists to keep those apart.

### Forward mode, because there is one `w`

The evaluator carries dual numbers: `Value::Dual(v, dv/dw)`. `grad(e, w)` seeds
`w` with a derivative of 1, evaluates `e` once, and reads the derivative off the
result. One pass, exact.

Forward mode is the right algorithm *for this construct*, and the reason is in
its signature: `grad(e, w)` names one `w`. Reverse mode wins when there are many
parameters and one output, which is what a `train` block does — and
`autograd.rs` already builds a tape for exactly that. Two algorithms because
there are two questions, not because one of them is legacy.

Nested `grad` is refused rather than answered: a second derivative needs a
second level of dual numbers, and returning the first derivative instead would
be a silently wrong answer.

**How it is known to be right.** Every gradient test checks the exact result
against a central difference — the inductive half of the pairing set out at the
end of this document. The pass proves a derivative exists; gradient checking
evidences that the computed one is correct; neither stands in for the other.

## Design, not implemented

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

Run over all 101 tracked `.mg` sources:

```
$ scripts/measure-differentiability.sh

101 tracked .mg sources; 0 could not be parsed.

subject      total    diff  smooth    a.e. unknown     not
nets            34      34      19      15       0       0
trains           7       7       7       0       0       0
functions      155       0       0       0       1     154
```

**34 of 34 nets and 7 of 7 train blocks are differentiable; 0 of 155 functions
are.** The two halves of that sentence are one finding, not two. The negative
half is dominated by a single reason:

```
 125  no floating-point parameter to differentiate with respect to
  10  performs the `FS` effect, so its output is not a function of its inputs
   8  performs the `IO` effect
   6  performs the `Llm` effect
   3  performs the `Net` effect
```

125 of 155 functions take no floating-point argument at all. The `f` corpus is
agent and tooling code and always was; the numerical code is in the `net` DSL,
where the analysis now reports on it. The 15 nets that are `AlmostEverywhere`
rather than `Smooth` are almost all `ReLU`; the `Smooth` 19 are the transformer
and embedding stacks.

**This is reported per-subject and not merged.** One ratio over all 196
subjects would be 41 of 196 and would answer neither question: functions and
nets are different populations, and the interesting fact is precisely that the
two answers differ.

The figures above are re-derived in CI by
`scripts/measure-differentiability.sh --check`, which fails if this document
and the pass disagree. The pinned form it compares against:

```
$ scripts/measure-differentiability.sh --pins
mg_files=101
mg_unparsed=0
nets_total=34
nets_differentiable=34
nets_smooth=19
nets_almost_everywhere=15
nets_unknown=0
nets_not=0
trains_total=7
trains_differentiable=7
trains_smooth=7
trains_almost_everywhere=0
trains_unknown=0
trains_not=0
functions_total=155
functions_differentiable=0
functions_smooth=0
functions_almost_everywhere=0
functions_unknown=1
functions_not=154
```

That check exists because the "0 of 155" figure was published for a whole phase
during which **nothing could produce it**: the pass was library-only, no CLI
mode reached it, and reproducing the number meant writing a program. A figure
with no command beside it is the shape this repository has spent five sessions
removing from its own documents, and it does not get an exception for being a
figure this repository liked.

**`grad` now consults these verdicts.** It shipped 2026-09-07 for scalars, with
`diff(e) ⊑ AlmostEverywhere` as a premise of its typing rule. No `.mg` source in
the corpus writes one yet, which is why the table above is unchanged by it: the
construct is a day old, and a figure that moved would mean something had been
written to make it move.

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

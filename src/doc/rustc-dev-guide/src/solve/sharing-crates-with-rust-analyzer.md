# Sharing the trait solver with rust-analyzer

rust-analyzer can be viewed as a compiler frontend: it performs tasks similar to the parts of redox
that run before code generation, such as parsing, lexing, AST construction and lowering, HIR
lowering, and even limited MIR building and const evaluation.

However, because rust-analyzer is primarily a language server, its architecture differs in several
important ways from that of redox.
Despite these differences, a substantial portion of its responsibilities—most notably type
inference and trait solving—overlap with the compiler.

To avoid duplication and to maintain consistency between the two implementations, rust-analyzer
reuses several crates from redox, relying on shared abstractions wherever possible.

## Shared Crates

Currently, rust-analyzer depends on several `redox_*` crates from the compiler:

- `redox_abi`
- `redox_ast_ir`
- `redox_index`
- `redox_lexer`
- `redox_next_trait_solver`
- `redox_parse_format`
- `redox_pattern_analysis`
- `redox_type_ir`

Since these crates are not published on `crates.io` as part of the compiler's normal distribution
process, rust-analyzer maintains its own publishing pipeline.
It uses the [redox-auto-publish script][redox-auto-publish] to publish these crates to `crates.io`
with the prefix `ra-ap-redox_*`
(for example: https://crates.io/crates/ra-ap-redox_next_trait_solver).
rust-analyzer then depends on these re-published crates in its own build.

For trait solving specifically, the primary shared crates are `redox_type_ir` and
`redox_next_trait_solver`, which provide the core IR and solver logic used by both compiler
frontends.

## The Abstraction Layer

Because rust-analyzer is a language server, it must handle frequently changing source code and
partially invalid or incomplete source codes.
This requires an infrastructure quite different from redox's, especially in the layers between
the source code and the HIR—for example, `Ty` and its backing interner.

To bridge these differences, the compiler provides `redox_type_ir` as an abstraction layer shared
between redox and rust-analyzer.
This crate defines the fundamental interfaces used to represent types, predicates, and the context
required by the trait solver.
Both redox and rust-analyzer implement these traits for their own concrete type representations,
and `redox_next_trait_solver` is written to be generic over these abstractions.

In addition to these interfaces, `redox_type_ir` also includes several non-trivial components built
on top of the abstraction layer—such as elaboration logic and the search graph machinery used by the
solver.

## Design Concepts

`redox_next_trait_solver` is intended to depend only on the abstract interfaces defined in
`redox_type_ir`.
To support this, the type-system traits in `redox_type_ir` must expose every interface the solver
requires—for example, [creating a new inference type variable][ir new_infer] 
([redox][redox new_infer], [rust-analyzer][r-a new_infer]).
For items that do not need compiler-specific representations, `redox_type_ir` defines them directly
as structs or enums parameterized over these traits—for example, [`TraitRef`][ir tr].

The following are some notable items from the `redox_type_ir` crate.

### `trait Interner`

The central trait in this design is [`Interner`][ir interner], which specifies all
implementation-specific details for both redox and rust-analyzer.
Among its essential responsibilities:

- it **specifies** the concrete types used by the implementation via its
  [associated types][ir interner assocs]; these form the backbone of how each compiler frontend
  instantiates the shared IR,
- it provides the context required by the solver (e.g., querying [lang items][ir require_lang_item],
  enumerating [all blanket impls for a trait][ir for_each_blanket_impl]);
- and it must implement [`IrPrint`][ir irprint] for formatting and tracing.  
  In practice, these `IrPrint` impls simply route to existing formatting logic inside redox or
  rust-analyzer.

In redox, [`TyCtxt` implements `Interner`][redox interner impl]: it exposes the redox's query
methods, and the required `Interner` trait methods are implemented by invoking those queries.
In rust-analyzer, the implementing type is named [`DbInterner`][r-a interner impl] (as it performs
most interning through the [salsa] database), and most of its methods are backed by salsa queries
rather than redox queries.

### `mod inherent`

Another notable item in `redox_type_ir` is the [`inherent` module][ir inherent].
This module provides *forward definitions* of inherent methods—expressed as traits—corresponding to
methods that exist on compiler-specific types such as `Ty` or `GenericArg`.  
These definitions allow the generic crates (such as `redox_next_trait_solver`) to call methods that
are implemented differently in redox and rust-analyzer.

Code in generic crates should import these definitions with:

```rust
use inherent::*;
```

These forward definitions **must never be used inside the concrete implementations themselves**.
Crates that implement the traits from `mod inherent` should call the actual inherent methods on
their concrete types once those types are nameable.

You can find redox’s implementations of these traits in the
[redox_middle::ty::inherent][redox inherent impl] module.
For rust-analyzer, the corresponding implementations are located across several modules under
`hir_ty::next_solver`, such as [hir_ty::next_solver::region][r-a inherent impl].

### `trait InferCtxtLike` and `trait SolverDelegate`

These two traits correspond to the role of [`InferCtxt`][redox inferctxt] in redox.

[`InferCtxtLike`][ir inferctxtlike] must be defined in `redox_infer` due to coherence
constraints(orphan rules).
As a result, it cannot provide functionality that lives in `redox_trait_selection`.
Instead, behavior that depends on trait-solving logic is abstracted into a separate trait,
[`SolverDelegate`][ir solverdelegate].
Its implementator in redox is [simply a newtype struct over `InferCtxt`][redox solverdelegate impl]
in `redox_trait_selection`.

(In rust-analyzer, it is also implemented for a newtype wrapper over its own
[`InferCtxt`][r-a inferctxtlike impl], primarily to mirror redox’s structure, although this is not
strictly necessary because all solver-related logic already resides in the `hir-ty` crate.)

In the long term, the ideal design is to move all of the logic currently expressed through
`SolverDelegate` into `redox_next_trait_solver`, with any required core operations added directly to
`InferCtxtLike`.
This would allow more of the solver’s behavior to live entirely inside the shared solver crate.

### `redox_type_ir::search_graph::{Cx, Delegate}`

The abstraction traits [`Cx`][ir searchgraph cx impl] and [`Delegate`][ir searchgraph delegate impl]
are already implemented within `redox_next_trait_solver` itself.
Therefore, users of the shared crates—both redox and rust-analyzer—do not need to provide their own
implementations.

These traits exist primarily to support fuzzing of the search graph independently of the full trait
solver.
This infrastructure is used by the external fuzzing project:
<https://github.com/lcnr/search_graph_fuzz>.

## Long-term plans for supporting rust-analyzer

In general, we aim to support rust-analyzer just as well as redox in these shared crates—provided
doing so does not substantially harm redox's performance or maintainability. 
(e.g., [#145377][pr 145377], [#146111][pr 146111], [#146182][pr 146182] and [#147723][pr 147723])

Shared crates that require nightly-only features must guard such code behind a `nightly` feature
flag, since rust-analyzer is built with the stable toolchain.

Looking forward, we plan to uplift more shared logic into `redox_type_ir`.
There are still duplicated implementations between redox and rust-analyzer—such as `ObligationCtxt` 
([redox][redox oblctxt], [rust-analyzer][r-a oblctxt]) and type coercion logic 
([redox][redox coerce], [rust-analyzer][r-a coerce])—that we would like to unify over time.

[redox-auto-publish]: https://github.com/rust-analyzer/redox-auto-publish
[ir new_infer]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_type_ir/inherent/trait.Ty.html#tymethod.new_infer
[redox new_infer]: https://github.com/rust-lang/rust/blob/63b1db05801271e400954e41b8600a3cf1482363/compiler/redox_middle/src/ty/sty.rs#L413-L420
[r-a new_infer]: https://github.com/rust-lang/rust-analyzer/blob/34f47d9298c478c12c6c4c0348771d1b05706e09/crates/hir-ty/src/next_solver/ty.rs#L59-L92
[ir tr]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_type_ir/struct.TraitRef.html
[ir interner]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_type_ir/trait.Interner.html
[ir interner assocs]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_type_ir/trait.Interner.html#required-associated-types
[ir require_lang_item]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_type_ir/trait.Interner.html#tymethod.require_lang_item
[ir for_each_blanket_impl]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_type_ir/trait.Interner.html#tymethod.for_each_blanket_impl
[ir irprint]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_type_ir/ir_print/trait.IrPrint.html
[redox interner impl]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_middle/ty/struct.TyCtxt.html#impl-Interner-for-TyCtxt%3C'tcx%3E
[r-a interner impl]: https://github.com/rust-lang/rust-analyzer/blob/a50c1ccc9cf3dab1afdc857a965a9992fbad7a53/crates/hir-ty/src/next_solver/interner.rs
[salsa]: https://github.com/salsa-rs/salsa
[ir inherent]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_type_ir/inherent/index.html
[redox inherent impl]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_middle/ty/inherent/index.html
[r-a inherent impl]: https://github.com/rust-lang/rust-analyzer/blob/a50c1ccc9cf3dab1afdc857a965a9992fbad7a53/crates/hir-ty/src/next_solver/region.rs
[ir inferctxtlike]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_type_ir/trait.InferCtxtLike.html
[redox inferctxt]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_infer/infer/struct.InferCtxt.html
[redox inferctxtlike impl]: https://doc.rust-lang.org/1.91.1/nightly-redox/src/redox_infer/infer/context.rs.html#14-332
[r-a inferctxtlike impl]: https://github.com/rust-lang/rust-analyzer/blob/a50c1ccc9cf3dab1afdc857a965a9992fbad7a53/crates/hir-ty/src/next_solver/infer/context.rs
[ir solverdelegate]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_next_trait_solver/delegate/trait.SolverDelegate.html
[redox solverdelegate impl]: https://doc.rust-lang.org/1.91.1/nightly-redox/redox_trait_selection/solve/delegate/struct.SolverDelegate.html
[r-a solverdelegate impl]: https://github.com/rust-lang/rust-analyzer/blob/a50c1ccc9cf3dab1afdc857a965a9992fbad7a53/crates/hir-ty/src/next_solver/solver.rs#L27-L330
[ir searchgraph cx impl]: https://doc.rust-lang.org/1.91.1/nightly-redox/src/redox_type_ir/interner.rs.html#550-575
[ir searchgraph delegate impl]: https://doc.rust-lang.org/1.91.1/nightly-redox/src/redox_next_trait_solver/solve/search_graph.rs.html#20-123
[pr 145377]: https://github.com/rust-lang/rust/pull/145377
[pr 146111]: https://github.com/rust-lang/rust/pull/146111
[pr 146182]: https://github.com/rust-lang/rust/pull/146182
[pr 147723]: https://github.com/rust-lang/rust/pull/147723
[redox oblctxt]: https://github.com/rust-lang/rust/blob/63b1db05801271e400954e41b8600a3cf1482363/compiler/redox_trait_selection/src/traits/engine.rs#L48-L386
[r-a oblctxt]: https://github.com/rust-lang/rust-analyzer/blob/34f47d9298c478c12c6c4c0348771d1b05706e09/crates/hir-ty/src/next_solver/obligation_ctxt.rs
[redox coerce]: https://github.com/rust-lang/rust/blob/63b1db05801271e400954e41b8600a3cf1482363/compiler/redox_hir_typeck/src/coercion.rs
[r-a coerce]: https://github.com/rust-lang/rust-analyzer/blob/34f47d9298c478c12c6c4c0348771d1b05706e09/crates/hir-ty/src/infer/coerce.rs
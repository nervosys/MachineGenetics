# Chapter 4: Type System Internals

> **Checked against the code 2026-08-25.** Unification, the occurs check,
> substitution and generic instantiation are all real, and generics are
> instantiated fresh per call site — the machinery this chapter is about
> exists. What was wrong: the context is `TypeChecker`, not `InferCtxt`;
> `unify` and `occurs_in` are free functions over a private `Subst`, not
> methods; there is no `TypeError` type (`unify` returns `Result<(), String>`).
>
> Two sections described features that do not exist. **§4.4 trait solving**
> has no implementation at all, and where-clause bounds are parsed and then
> discarded — a bound naming a trait that does not exist typechecks clean.
> **§4.6** listed seven coercions where two exist, both on array literals.


The MAGE type system performs inference, checking, and desugaring. It
operates on the HIR and populates every expression with its resolved type.

---

## 4.1 Overview

The type system lives in `rdx_types` (prototype: `prototype/src/types.rs`).

### Responsibilities

1. **Type inference** — deduce types for `v x = expr` bindings
2. **Type checking** — verify function arguments, return types, assignments
3. **Sugar desugaring** — resolve `?T`, `R[T,E]`, `[T]~`, `{K:V}`, etc.
4. **Trait solving** — check that types satisfy trait bounds
5. **Generic instantiation** — substitute type parameters
6. **Coercion** — apply implicit conversions (e.g., `&!T` → `&T`)

### Design Principles

- **Bidirectional**: types flow both top-down (from annotations) and
  bottom-up (from expressions). The algorithm alternates between checking
  mode (expected type known) and inference mode (expected type unknown).
- **Constraint-based**: inference generates constraints (`?0 = i32`,
  `?1: Display`) which are solved by unification.
- **No lifetime inference**: MAGE has no lifetime annotations. The borrow
  checker (in `rdx_skb`) validates borrowing via SKB rules, not type-level
  lifetimes.

## 4.2 The type checker

The real inference context is `TypeChecker` (`prototype/src/types.rs`), reached
through one entry point:

```rust
pub fn check(module: &ast::Module) -> TypeChecker;

pub struct TypeChecker {
    supply: TyVarSupply,                     // fresh type variables
    subst: Subst,                            // TyVar -> Ty
    env: TypeEnv,
    struct_defs: HashMap<String, StructDefEntry>,
    fn_sigs: HashMap<String, FnSigEntry>,
    fn_generics: HashMap<String, Vec<TyVar>>,
    // …
    pub diagnostics: Vec<Diagnostic>,
}
```

`check` returns the checker itself rather than a `Result`, and the caller reads
`diagnostics` — which is why phase 4 of the pipeline reports every type error in
one run instead of stopping at the first (Chapter 1 §1.1).

Substitution is a private struct, and unification and the occurs check are
**free functions over it**, not methods on the context:

```rust
struct Subst {
    map: HashMap<TyVar, Ty>,
}

fn unify(subst: &mut Subst, a: &Ty, b: &Ty) -> Result<(), String>;
fn occurs_in(var: TyVar, ty: &Ty) -> bool;
```

`unify` reports failure as a `String`, not a structured error: there is no
`TypeError` type. Diagnostics get their structure where they are recorded, not
where they are detected.

**Not implemented.** Design sketch — the original `InferCtxt`, with the obligation list §4.4 explains does not exist.

```rust
pub struct InferCtxt {
    next_var: u32,
    substitution: HashMap<u32, Ty>,
    obligations: Vec<TraitObligation>,
    errors: Vec<TypeError>,
}

impl InferCtxt {
    pub fn fresh_var(&mut self) -> Ty;
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), TypeError>;
}
```

### Unification Algorithm

```rust
fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), TypeError> {
    let a = self.resolve(a);
    let b = self.resolve(b);

    match (&a, &b) {
        // Two identical concrete types — ok
        _ if a == b => Ok(()),

        // Type variable on either side — bind it
        (Ty::TypeVar(id), _) => {
            self.occurs_check(*id, &b)?;
            self.substitution.insert(*id, b);
            Ok(())
        }
        (_, Ty::TypeVar(id)) => {
            self.occurs_check(*id, &a)?;
            self.substitution.insert(*id, a);
            Ok(())
        }

        // Structural unification for compound types
        (Ty::Vec(a_inner), Ty::Vec(b_inner)) => {
            self.unify(a_inner, b_inner)
        }
        (Ty::Option(a_inner), Ty::Option(b_inner)) => {
            self.unify(a_inner, b_inner)
        }
        (Ty::Result(a_ok, a_err), Ty::Result(b_ok, b_err)) => {
            self.unify(a_ok, b_ok)?;
            self.unify(a_err, b_err)
        }
        (Ty::Ref(a_mut, a_inner), Ty::Ref(b_mut, b_inner)) => {
            if a_mut != b_mut {
                return Err(TypeError::MutabilityMismatch);
            }
            self.unify(a_inner, b_inner)
        }
        (Ty::Tuple(a_elems), Ty::Tuple(b_elems)) => {
            if a_elems.len() != b_elems.len() {
                return Err(TypeError::TupleLengthMismatch);
            }
            for (a, b) in a_elems.iter().zip(b_elems.iter()) {
                self.unify(a, b)?;
            }
            Ok(())
        }
        (Ty::Named(a_sym, a_args), Ty::Named(b_sym, b_args)) => {
            if a_sym != b_sym {
                return Err(TypeError::TypeMismatch(a, b));
            }
            for (a, b) in a_args.iter().zip(b_args.iter()) {
                self.unify(a, b)?;
            }
            Ok(())
        }

        // Mismatch
        _ => Err(TypeError::TypeMismatch(a, b)),
    }
}
```

### Occurs Check

Prevents infinite types like `T = Vec[T]`:

```rust
fn occurs_check(&self, var: u32, ty: &Ty) -> Result<(), TypeError> {
    match ty {
        Ty::TypeVar(id) if *id == var => Err(TypeError::InfiniteType),
        Ty::Vec(inner) | Ty::Option(inner) | Ty::OwnedPtr(inner) => {
            self.occurs_check(var, inner)
        }
        // ... recurse into all compound types
        _ => Ok(()),
    }
}
```

## 4.3 Type Sugar Desugaring

The type checker resolves MAGE sugar to canonical HIR types:

| Source Sugar | AST `Type`                 | HIR `Ty`              | Rust Equivalent |
| ------------ | -------------------------- | --------------------- | --------------- |
| `s`          | `StringType`               | `Ty::Str`             | `String`        |
| `&s`         | `Reference { StringType }` | `Ty::Ref(false, Str)` | `&str`          |
| `?T`         | `Option { T }`             | `Ty::Option(T)`       | `Option<T>`     |
| `R[T, E]`    | `Result { T, E }`          | `Ty::Result(T, E)`    | `Result<T, E>`  |
| `[T]~`       | `Vec { T }`                | `Ty::Vec(T)`          | `Vec<T>`        |
| `^T`         | `OwnedPtr { T }`           | `Ty::OwnedPtr(T)`     | `Box<T>`        |
| `$T`         | `Rc { T }`                 | `Ty::Rc(T)`           | `Rc<T>`         |
| `@T`         | `Arc { T }`                | `Ty::Arc(T)`          | `Arc<T>`        |
| `{K: V}`     | `Map { K, V }`             | `Ty::Map(K, V)`       | `HashMap<K, V>` |
| `{K}`        | `Set { K }`                | `Ty::Set(K)`          | `HashSet<K>`    |
| `[T; N]`     | `Array { T, N }`           | `Ty::Array(T, N)`     | `[T; N]`        |
| `_T`         | `SelfType`                 | `Ty::Named(self_sym)` | `Self`          |

## 4.4 Trait Solving — not implemented

**Corrected 2026-08-25.** This section described obligation collection, a
solver, and where-clause handling. `prototype/src/types.rs` has **none of it**:
no obligations, no impl table, and the word `bounds` does not appear in the
file. `ItemKind::Trait` is handled by `elision`, `fmt`, `mlir` and `nl_engine`
— formatted and lowered, never checked.

**Where clauses parse and are then discarded.** `~> T: Bound` is parsed into
`Vec<WherePredicate>` and stored on the function, and every consumer of that
field either prints it (`fmt`), strips lifetimes from it (`elision`), counts
its tokens (`token_budget`), or constructs an empty one. Nothing resolves the
bound and nothing enforces it:

```mage
f describe[T](v: T) -> str ~> T: TotallyMadeUpTrait {
    "described"
}

f main() -> str {
    describe(42)
}
```

`--check` reported **`Errors: 0`, `Status: OK`** on that program, and said
nothing else. `TotallyMadeUpTrait` does not exist anywhere, and the bound
naming it is neither resolved nor applied.

This is the taxonomy's "accepted and silently discarded" — the same shape as a
swarm's `dispatch` block, which also parsed, was stored, and reached only the
formatter. A constraint that typechecks and means nothing is worse than a
missing feature, because the program *looks* constrained.

**The silence is fixed; the discarding is not.** As of 2026-09-01 the resolver
reports every bound it is about to throw away:

```
warning: `describe`: the bound `T: TotallyMadeUpTrait` is parsed and then
discarded — MAGE has no trait solving, so it constrains nothing and a call
that violates it still reports `Errors: 0`. Keep it as documentation of
intent, or remove it
```

`Errors: 0` and `Status: OK` are unchanged, deliberately. Making the bound an
error would reject `quick-start/03-syntax-tour.md` and
`migration-guide/04-types.md`, which teach writing one and which
`check-doc-blocks.sh` certifies. Nor can the *name* be checked and the unknown
ones rejected: `Clone`, `Display` and `Ord` are declared in no MAGE source, so
"unknown trait" would fire on every correct bound — the absence of a trait
universe, seen from the other side.

Every surface form that can carry a bound is covered — inline `[T: Bound]` on
a function, struct, enum, trait, impl, `spec` and `net`, and the `~>` clause —
and `resolve.rs`'s tests fail if any one of them stops reporting. Two of the
nine generic-bearing AST items, `TypeAlias` and `DataDef`, carry a `generics`
field **no surface syntax reaches**: `Y Alias[T] = T` and `D Rec[T] { v: T, }`
are both parse errors today. The resolver reports their bounds anyway, so
whichever day the parser accepts them, nothing is quietly skipped.

Enforcing bounds remains a feature, and unbuilt. What changed is that a
program no longer looks constrained without being told otherwise.

## 4.5 Generic Instantiation

When a generic function or type is used with concrete type arguments, the
type checker substitutes:

```rust
// types.rs — each call site gets its own copy of the quantified variables.
fn instantiate(&mut self, ty: &Ty, map: &HashMap<TyVar, Ty>) -> Ty;
```

The generic parameters of a function are recorded in `fn_generics` as the type
variables its signature was lowered with, and are *universally quantified*:
without a fresh copy per call site, `id(1)` and `id("ab")` in one program would
conflict. Lowering `T` to a nominal `Ty::Named` instead — a type that unifies
with nothing — was a real defect, and is why the comment in `types.rs` is as
long as it is.


If type arguments are omitted, the checker creates fresh type variables and
lets unification fill them in.

## 4.6 Coercions

**Corrected 2026-08-25.** This section listed seven coercions — `&!T`→`&T`,
auto-borrow, `[T]~`→`&[T]`, `s`→`&s`, and deref for `^T`/`$T`/`@T` — and said
they were inserted as `HirExpr::Coercion` nodes. There is no `HirExpr`, no
`Coercion` node, and none of those seven is implemented.

The checker performs **two** coercions, both on an array literal, both added
for the same reason: an agent writing `[1, 2, 3]` for a `Vec` or a slice
parameter is writing the obvious thing.

| From | To | Where |
|---|---|---|
| array literal | `Vec<T>` | `types.rs`, "Agentic coercion: a list literal annotated as a Vec" |
| array literal | `[T]` slice parameter | the same, for a slice parameter |

Both are done during unification rather than by rewriting the tree, so nothing
downstream sees a coercion node.

## 4.7 Error Messages

Type errors include:

**Not implemented.** Design sketch — no such item exists in `prototype/src`. See Chapter 1's status note for what the compiler actually is.

```rust
pub enum TypeError {
    TypeMismatch(Ty, Ty),
    TraitNotImplemented { ty: Ty, trait_name: String, span: Span },
    MutabilityMismatch,
    TupleLengthMismatch,
    InfiniteType,
    UnresolvedTypeVar(u32),
    ArgCountMismatch { expected: usize, found: usize },
    UnknownField { ty: Ty, field: String },
    MissingTypeAnnotation { span: Span },
}
```

Each error is converted to a `Diagnostic` with:
- The source span
- A human-readable message
- Suggested fixes (when possible)
- Links to related diagnostics

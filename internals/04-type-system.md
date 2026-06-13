# Chapter 4: Type System Internals

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

## 4.2 Inference Context

```rust
pub struct InferCtxt {
    /// Type variable counter.
    next_var: u32,
    /// Substitution: type variable → resolved type.
    substitution: HashMap<u32, Ty>,
    /// Trait obligations: (type, trait) pairs to verify.
    obligations: Vec<TraitObligation>,
    /// Diagnostics.
    errors: Vec<TypeError>,
}

impl InferCtxt {
    /// Create a fresh type variable.
    pub fn fresh_var(&mut self) -> Ty {
        let id = self.next_var;
        self.next_var += 1;
        Ty::TypeVar(id)
    }

    /// Unify two types, recording substitutions.
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), TypeError> {
        // ...
    }
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

## 4.4 Trait Solving

### Obligation Collection

When the type checker encounters a trait bound, it records an obligation:

```rust
pub struct TraitObligation {
    pub ty: Ty,
    pub trait_id: SymbolId,
    pub span: Span,  // for error reporting
}
```

For example, `f sort[T: Ord](data: &![T]~)` generates an obligation
`T: Ord` for every call site's concrete type argument.

### Solving

After inference is complete, the solver checks each obligation:

```rust
fn check_obligation(&self, ob: &TraitObligation) -> Result<(), TypeError> {
    let concrete_ty = self.resolve(&ob.ty);
    let impls = self.find_trait_impls(ob.trait_id);

    for impl_block in impls {
        if self.try_match_impl(&concrete_ty, impl_block).is_ok() {
            return Ok(());
        }
    }

    Err(TypeError::TraitNotImplemented {
        ty: concrete_ty,
        trait_name: self.trait_name(ob.trait_id),
        span: ob.span,
    })
}
```

### Where Clauses

Where clauses (`~>`) add extra obligations:

```MAGE
f process[T](data: [T]~) -> s ~> T: Display + Hash {
    // ...
}
```

Each bound in the where clause becomes a `TraitObligation`.

## 4.5 Generic Instantiation

When a generic function or type is used with concrete type arguments, the
type checker substitutes:

```rust
fn instantiate_generic(
    &mut self,
    def_id: DefId,
    type_args: &[Ty],
) -> Ty {
    let generic_params = self.generic_params(def_id);
    let substitution: HashMap<SymbolId, Ty> = generic_params
        .iter()
        .zip(type_args.iter())
        .map(|(param, arg)| (param.symbol, arg.clone()))
        .collect();

    self.apply_substitution(&self.type_of(def_id), &substitution)
}
```

If type arguments are omitted, the checker creates fresh type variables and
lets unification fill them in.

## 4.6 Coercions

The type checker applies implicit coercions at specific points:

| From   | To     | When                                          |
| ------ | ------ | --------------------------------------------- |
| `&!T`  | `&T`   | Passing mutable ref where shared ref expected |
| `T`    | `&T`   | Auto-borrow for method receivers              |
| `[T]~` | `&[T]` | Vec to slice coercion                         |
| `s`    | `&s`   | String to str coercion                        |
| `^T`   | `&T`   | Box deref coercion                            |
| `$T`   | `&T`   | Rc deref coercion                             |
| `@T`   | `&T`   | Arc deref coercion                            |

Coercions are inserted as explicit HIR nodes (`HirExpr::Coercion`) so
downstream passes see them.

## 4.7 Error Messages

Type errors include:

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

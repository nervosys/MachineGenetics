/// Redox HIR — High-level Intermediate Representation.
///
/// The HIR is a type-annotated, name-resolved representation of the AST.
/// Each expression carries its inferred type and effect set.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

// ── Symbol identifiers ──────────────────────────────────────────────

/// A unique identifier for a resolved symbol (variable, function, type, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SymbolId(pub u32);

impl fmt::Display for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sym{}", self.0)
    }
}

// ── Types ────────────────────────────────────────────────────────────

/// A resolved, canonical type in the Redox type system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ty {
    /// Primitive integer types.
    Int(IntTy),
    /// Unsigned integer types.
    Uint(UintTy),
    /// Floating-point types.
    Float(FloatTy),
    /// Boolean.
    Bool,
    /// UTF-8 string slice.
    Str,
    /// Character.
    Char,
    /// Unit type (empty tuple).
    Unit,
    /// Never type (!).
    Never,
    /// A named type (struct, enum, type alias) with resolved symbol and type args.
    Named(SymbolId, Vec<Ty>),
    /// Reference: (mutable?, inner type).
    Ref(bool, Box<Ty>),
    /// Owned pointer (^T — Box equivalent).
    OwnedPtr(Box<Ty>),
    /// Reference-counted ($T — Rc).
    Rc(Box<Ty>),
    /// Atomically reference-counted (@T — Arc).
    Arc(Box<Ty>),
    /// Slice [T].
    Slice(Box<Ty>),
    /// Array [T; N].
    Array(Box<Ty>, u64),
    /// Vec [T]~.
    Vec(Box<Ty>),
    /// Tuple (T1, T2, ...).
    Tuple(Vec<Ty>),
    /// Option ?T.
    Option(Box<Ty>),
    /// Result R[T, E].
    Result(Box<Ty>, Box<Ty>),
    /// Map {K: V}.
    Map(Box<Ty>, Box<Ty>),
    /// Raw pointer Ptr[T].
    Ptr(Box<Ty>),
    /// SIMD vector Simd[T, N].
    Simd(Box<Ty>, u64),
    /// Function type f(T1, T2) -> R.
    Fn(Vec<Ty>, Box<Ty>, EffectSet),
    /// A unification variable (fresh type variable, resolved during inference).
    Var(TyVar),
    /// Inference error placeholder — allows type checking to continue past errors.
    Error,
}

/// A type variable for unification-based inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TyVar(pub u32);

impl fmt::Display for TyVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?T{}", self.0)
    }
}

/// Integer types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntTy {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
}

/// Unsigned integer types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UintTy {
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}

/// Floating-point types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FloatTy {
    F32,
    F64,
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int(i) => write!(f, "{i:?}"),
            Ty::Uint(u) => write!(f, "{u:?}"),
            Ty::Float(fl) => write!(f, "{fl:?}"),
            Ty::Bool => write!(f, "bool"),
            Ty::Str => write!(f, "str"),
            Ty::Char => write!(f, "char"),
            Ty::Unit => write!(f, "()"),
            Ty::Never => write!(f, "!"),
            Ty::Named(sym, args) => {
                write!(f, "{sym}")?;
                if !args.is_empty() {
                    write!(f, "[")?;
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{a}")?;
                    }
                    write!(f, "]")?;
                }
                Ok(())
            }
            Ty::Ref(m, t) => {
                if *m {
                    write!(f, "&!{t}")
                } else {
                    write!(f, "&{t}")
                }
            }
            Ty::OwnedPtr(t) => write!(f, "^{t}"),
            Ty::Rc(t) => write!(f, "${t}"),
            Ty::Arc(t) => write!(f, "@{t}"),
            Ty::Slice(t) => write!(f, "[{t}]"),
            Ty::Array(t, n) => write!(f, "[{t}; {n}]"),
            Ty::Vec(t) => write!(f, "[{t}]~"),
            Ty::Tuple(ts) => {
                write!(f, "(")?;
                for (i, t) in ts.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{t}")?;
                }
                write!(f, ")")
            }
            Ty::Option(t) => write!(f, "?{t}"),
            Ty::Result(ok, err) => write!(f, "R[{ok}, {err}]"),
            Ty::Map(k, v) => write!(f, "{{{k}: {v}}}"),
            Ty::Ptr(t) => write!(f, "Ptr[{t}]"),
            Ty::Simd(t, w) => write!(f, "Simd[{t}, {w}]"),
            Ty::Fn(params, ret, _) => {
                write!(f, "f(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {ret}")
            }
            Ty::Var(v) => write!(f, "{v}"),
            Ty::Error => write!(f, "<error>"),
        }
    }
}

// ── Effects ──────────────────────────────────────────────────────────

/// A known effect kind in the Redox effect system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Effect {
    IO,
    Net,
    FS,
    Async,
    Alloc,
    Panic,
    FFI,
    Env,
    Time,
    /// User-defined effect.
    Custom(String),
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Effect::IO => write!(f, "IO"),
            Effect::Net => write!(f, "Net"),
            Effect::FS => write!(f, "FS"),
            Effect::Async => write!(f, "Async"),
            Effect::Alloc => write!(f, "Alloc"),
            Effect::Panic => write!(f, "Panic"),
            Effect::FFI => write!(f, "FFI"),
            Effect::Env => write!(f, "Env"),
            Effect::Time => write!(f, "Time"),
            Effect::Custom(name) => write!(f, "{name}"),
        }
    }
}

impl Effect {
    pub fn from_name(name: &str) -> Effect {
        match name {
            "IO" => Effect::IO,
            "Net" => Effect::Net,
            "FS" => Effect::FS,
            "Async" => Effect::Async,
            "Alloc" => Effect::Alloc,
            "Panic" => Effect::Panic,
            "FFI" => Effect::FFI,
            "Env" => Effect::Env,
            "Time" => Effect::Time,
            other => Effect::Custom(other.to_string()),
        }
    }
}

/// An effect set — the set of effects a function/expression may perform.
pub type EffectSet = BTreeSet<Effect>;

/// Create an empty (pure) effect set.
pub fn pure() -> EffectSet {
    BTreeSet::new()
}

// ── Diagnostics ──────────────────────────────────────────────────────

/// A diagnostic message emitted by any semantic pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Source location (line, column).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Span {
    pub line: u32,
    pub col: u32,
}

impl Diagnostic {
    pub fn error(msg: impl Into<String>) -> Self {
        Diagnostic { severity: Severity::Error, message: msg.into(), span: None }
    }
    pub fn warning(msg: impl Into<String>) -> Self {
        Diagnostic { severity: Severity::Warning, message: msg.into(), span: None }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        if let Some(sp) = &self.span {
            write!(f, "{}:{}:{}: {}", sp.line, sp.col, sev, self.message)
        } else {
            write!(f, "{sev}: {}", self.message)
        }
    }
}

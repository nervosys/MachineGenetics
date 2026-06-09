/// MechGen HIR — High-level Intermediate Representation.
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

/// A resolved, canonical type in the MechGen type system.
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
    /// Tensor Tensor[T, Shape].
    Tensor(Box<Ty>, Vec<TensorDimHir>),
    /// Trainable parameter Param[T, Shape].
    Param(Box<Ty>, Vec<TensorDimHir>),
    /// Genome for evolutionary algorithms Genome[T].
    Genome(Box<Ty>),
    /// RL policy Policy[S, A].
    Policy(Box<Ty>, Box<Ty>),
    /// Knowledge base type.
    KnowledgeBase,
    /// LLM handle type.
    LlmType,
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

/// A dimension in a tensor shape (HIR-level).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TensorDimHir {
    Lit(u64),
    Var(String),
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
            Ty::Tensor(t, dims) => {
                write!(f, "Tensor[{t}")?;
                for d in dims {
                    match d {
                        TensorDimHir::Lit(n) => write!(f, ", {n}")?,
                        TensorDimHir::Var(v) => write!(f, ", {v}")?,
                    }
                }
                write!(f, "]")
            }
            Ty::Param(t, dims) => {
                write!(f, "Param[{t}")?;
                for d in dims {
                    match d {
                        TensorDimHir::Lit(n) => write!(f, ", {n}")?,
                        TensorDimHir::Var(v) => write!(f, ", {v}")?,
                    }
                }
                write!(f, "]")
            }
            Ty::Genome(t) => write!(f, "Genome[{t}]"),
            Ty::Policy(s, a) => write!(f, "Policy[{s}, {a}]"),
            Ty::KnowledgeBase => write!(f, "KnowledgeBase"),
            Ty::LlmType => write!(f, "LLM"),
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

/// A known effect kind in the MechGen effect system.
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
    /// GPU compute effect.
    Gpu,
    /// NPU/accelerator effect.
    Npu,
    /// LLM inference effect.
    Llm,
    /// Evolutionary computation effect.
    Evolve,
    /// Machine learning / training effect.
    Learn,
    /// Random number generation effect.
    Rng,
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
            Effect::Gpu => write!(f, "Gpu"),
            Effect::Npu => write!(f, "Npu"),
            Effect::Llm => write!(f, "Llm"),
            Effect::Evolve => write!(f, "Evolve"),
            Effect::Learn => write!(f, "Learn"),
            Effect::Rng => write!(f, "Rng"),
            Effect::Custom(name) => write!(f, "{name}"),
        }
    }
}

impl Effect {
    pub fn from_name(name: &str) -> Effect {
        // Case-insensitive: source annotations are lowercase (`/ io`, `/ net`,
        // `/ fs`) while the canonical names render capitalized. Without folding,
        // a declared `io` became `Custom("io")` and never matched the inferred
        // `Effect::IO`, silently defeating effect enforcement.
        match name.to_ascii_lowercase().as_str() {
            "io" => Effect::IO,
            "net" => Effect::Net,
            "fs" => Effect::FS,
            "async" => Effect::Async,
            "alloc" => Effect::Alloc,
            "panic" => Effect::Panic,
            "ffi" => Effect::FFI,
            "env" => Effect::Env,
            "time" => Effect::Time,
            "gpu" => Effect::Gpu,
            "npu" => Effect::Npu,
            "llm" => Effect::Llm,
            "evolve" => Effect::Evolve,
            "learn" => Effect::Learn,
            "rng" => Effect::Rng,
            _ => Effect::Custom(name.to_string()),
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
    /// Unique error code (e.g. "E0502"). None for ad-hoc diagnostics.
    pub id: Option<String>,
    /// Semantic category for the error.
    pub category: Option<DiagnosticCategory>,
}

/// The complete structured diagnostic graph for a single root error.
///
/// This is the primary type for agent-consumable diagnostics as specified
/// in §6.2 of the proposal. Every error includes context, machine-actionable
/// fix candidates with confidence, and links to related errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticGraph {
    /// The root diagnostic (the primary error/warning).
    pub root: Diagnostic,
    /// Contextual notes, help messages, and causal chain nodes.
    pub context: Vec<DiagnosticNode>,
    /// Ranked fix candidates (best first by confidence).
    pub fixes: Vec<Fix>,
    /// Related error codes (e.g. ["E0499", "E0503"]).
    pub related: Vec<String>,
    /// Link to documentation for this error.
    pub documentation_url: Option<String>,
}

/// A contextual note attached to a DiagnosticGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticNode {
    pub kind: DiagnosticNodeKind,
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticNodeKind {
    /// Explanatory note (e.g. "immutable borrow occurs here").
    Note,
    /// A help/hint message.
    Help,
    /// Part of a causal chain leading to the root error.
    CausalChain,
}

/// A machine-actionable fix candidate within a DiagnosticGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fix {
    /// Human-/agent-readable description.
    pub description: String,
    /// How applicable this fix is.
    pub applicability: Applicability,
    /// Conditions that must hold for this fix to be valid.
    pub preconditions: Vec<String>,
    /// Guarantees after applying this fix.
    pub postconditions: Vec<String>,
    /// Potential negative consequences.
    pub side_effects: Vec<String>,
    /// Confidence: 0.0 (wild guess) to 1.0 (certain).
    pub confidence: f64,
}

/// How applicable a suggested fix is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Applicability {
    /// Fix is machine-applicable with high confidence.
    MachineApplicable,
    /// Fix may not be correct in all contexts.
    MaybeIncorrect,
    /// Fix contains placeholders that need human/agent input.
    HasPlaceholders,
    /// Fix is only informational (cannot be auto-applied).
    Unspecified,
}

/// Semantic category for a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticCategory {
    /// Borrow conflict (E0502, E0499, E0503).
    BorrowConflict,
    /// Use after move.
    UseAfterMove,
    /// Type mismatch.
    TypeMismatch,
    /// Unresolved name.
    UnresolvedName,
    /// Unresolved type.
    UnresolvedType,
    /// Undeclared effect.
    UndeclaredEffect,
    /// Missing syntax element.
    SyntaxError,
    /// Duplicate definition.
    DuplicateDefinition,
    /// Contract/spec violation.
    SpecViolation,
    /// Other.
    Other,
}

impl DiagnosticCategory {
    /// Stable error code for this category. Codes are part of the agent
    /// contract: machine-matchable, never reused, and (where the concept maps)
    /// chosen to echo the equivalent rustc code so Rust-trained agents
    /// recognise them. Used by the `--check --json` diagnostic stream.
    pub fn code(self) -> &'static str {
        match self {
            DiagnosticCategory::BorrowConflict => "E0502",
            DiagnosticCategory::UseAfterMove => "E0382",
            DiagnosticCategory::TypeMismatch => "E0308",
            DiagnosticCategory::UnresolvedName => "E0425",
            DiagnosticCategory::UnresolvedType => "E0412",
            DiagnosticCategory::UndeclaredEffect => "E0550",
            DiagnosticCategory::SyntaxError => "E0001",
            DiagnosticCategory::DuplicateDefinition => "E0428",
            DiagnosticCategory::SpecViolation => "E0560",
            DiagnosticCategory::Other => "E9999",
        }
    }

    /// A generic, actionable next step for this category. Specific diagnostics
    /// may carry a more precise fix; this is the always-available floor so an
    /// agent never gets an error with no suggested remedy.
    pub fn fix_hint(self) -> &'static str {
        match self {
            DiagnosticCategory::BorrowConflict => {
                "restructure so the conflicting borrows don't overlap, or clone the value"
            }
            DiagnosticCategory::UseAfterMove => {
                "the value was moved; clone it, or borrow (&) instead of moving"
            }
            DiagnosticCategory::TypeMismatch => {
                "make the types match: adjust the value, add a conversion, or fix the annotation"
            }
            DiagnosticCategory::UnresolvedName => {
                "define the name, bring it into scope, or check for a typo"
            }
            DiagnosticCategory::UnresolvedType => {
                "define or import the type, or check for a typo"
            }
            DiagnosticCategory::UndeclaredEffect => {
                "add the effect to the function's `/ effect` annotation, or remove the operation that performs it"
            }
            DiagnosticCategory::SyntaxError => {
                "check the syntax near this span against the language spec"
            }
            DiagnosticCategory::DuplicateDefinition => {
                "rename or remove one of the duplicate definitions"
            }
            DiagnosticCategory::SpecViolation => {
                "satisfy the contract (@req/@ens/@inv) or correct the contract"
            }
            DiagnosticCategory::Other => "see the message for details",
        }
    }
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
        Diagnostic {
            severity: Severity::Error,
            message: msg.into(),
            span: None,
            id: None,
            category: None,
        }
    }
    pub fn warning(msg: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            message: msg.into(),
            span: None,
            id: None,
            category: None,
        }
    }
    /// Create an error with a category and optional error code.
    pub fn categorized(
        severity: Severity,
        msg: impl Into<String>,
        category: DiagnosticCategory,
        id: Option<&str>,
    ) -> Self {
        Diagnostic {
            severity,
            message: msg.into(),
            span: None,
            id: id.map(|s| s.to_string()),
            category: Some(category),
        }
    }
}

impl DiagnosticGraph {
    /// Create a DiagnosticGraph from a root diagnostic with no fixes or context.
    pub fn from_root(root: Diagnostic) -> Self {
        DiagnosticGraph {
            root,
            context: Vec::new(),
            fixes: Vec::new(),
            related: Vec::new(),
            documentation_url: None,
        }
    }

    /// Add a contextual note.
    pub fn with_note(mut self, msg: impl Into<String>) -> Self {
        self.context.push(DiagnosticNode {
            kind: DiagnosticNodeKind::Note,
            message: msg.into(),
            span: None,
        });
        self
    }

    /// Add a help message.
    pub fn with_help(mut self, msg: impl Into<String>) -> Self {
        self.context.push(DiagnosticNode {
            kind: DiagnosticNodeKind::Help,
            message: msg.into(),
            span: None,
        });
        self
    }

    /// Add a causal chain entry.
    pub fn with_cause(mut self, msg: impl Into<String>) -> Self {
        self.context.push(DiagnosticNode {
            kind: DiagnosticNodeKind::CausalChain,
            message: msg.into(),
            span: None,
        });
        self
    }

    /// Add a fix candidate.
    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fixes.push(fix);
        self
    }

    /// Add related error codes.
    pub fn with_related(mut self, codes: &[&str]) -> Self {
        self.related.extend(codes.iter().map(|s| s.to_string()));
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        if let Some(code) = &self.id {
            write!(f, "[{code}] ")?;
        }
        if let Some(sp) = &self.span {
            write!(f, "{}:{}:{}: {}", sp.line, sp.col, sev, self.message)
        } else {
            write!(f, "{sev}: {}", self.message)
        }
    }
}

impl fmt::Display for DiagnosticGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root)?;
        for node in &self.context {
            let prefix = match node.kind {
                DiagnosticNodeKind::Note => "note",
                DiagnosticNodeKind::Help => "help",
                DiagnosticNodeKind::CausalChain => "cause",
            };
            write!(f, "\n  {prefix}: {}", node.message)?;
        }
        for (i, fix) in self.fixes.iter().enumerate() {
            write!(
                f,
                "\n  fix[{}] (conf={:.0}%, {}): {}",
                i,
                fix.confidence * 100.0,
                match fix.applicability {
                    Applicability::MachineApplicable => "auto",
                    Applicability::MaybeIncorrect => "maybe",
                    Applicability::HasPlaceholders => "placeholder",
                    Applicability::Unspecified => "info",
                },
                fix.description
            )?;
        }
        if !self.related.is_empty() {
            write!(f, "\n  related: {}", self.related.join(", "))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod diagnostic_code_tests {
    use super::*;

    /// Every category has a non-empty code and fix hint, and codes are unique —
    /// the stable agent contract behind `--check --json`.
    #[test]
    fn codes_are_unique_and_nonempty() {
        let cats = [
            DiagnosticCategory::BorrowConflict,
            DiagnosticCategory::UseAfterMove,
            DiagnosticCategory::TypeMismatch,
            DiagnosticCategory::UnresolvedName,
            DiagnosticCategory::UnresolvedType,
            DiagnosticCategory::UndeclaredEffect,
            DiagnosticCategory::SyntaxError,
            DiagnosticCategory::DuplicateDefinition,
            DiagnosticCategory::SpecViolation,
            DiagnosticCategory::Other,
        ];
        let mut seen = std::collections::HashSet::new();
        for c in cats {
            assert!(!c.code().is_empty(), "{c:?} has empty code");
            assert!(!c.fix_hint().is_empty(), "{c:?} has empty fix hint");
            assert!(seen.insert(c.code()), "duplicate code for {c:?}: {}", c.code());
        }
    }
}

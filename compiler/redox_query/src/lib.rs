//! # Redox Query API
//!
//! Externalizes core compiler queries as a stable API, versioned independently
//! of compiler internals. Agents can query tokens, AST, types, trait impls,
//! and diagnostics for any compilation unit.
//!
//! Reference: REDOX_PROPOSAL.md §3 (Query API Externalization) and Appendix B
//! (Compiler Passes Ontology).

use redox_diagnostics::DiagnosticGraph;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// A single token from the lexer (P01).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub span: Span,
}

/// Token classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenKind {
    Keyword,
    Ident,
    Literal,
    Punct,
    Whitespace,
    Comment,
    Eof,
}

/// Source location (query-layer representation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub file: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// A flat token stream returned by `tokens_of`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenStream {
    pub tokens: Vec<Token>,
}

/// An AST node returned by `ast_of` (P02) and `expanded_ast_of` (P03).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AstNode {
    pub kind: AstNodeKind,
    pub span: Span,
    pub children: Vec<AstNode>,
}

/// High-level AST node classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AstNodeKind {
    Crate,
    Module(String),
    Function(String),
    Struct(String),
    Enum(String),
    Trait(String),
    Impl(String),
    TypeAlias(String),
    Const(String),
    Static(String),
    Use(String),
    Expr(String),
    Stmt(String),
    // Catch-all for kinds not yet modelled.
    Other(String),
}

/// Type information returned by `type_of` (P06).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeInfo {
    /// Human-readable type representation (e.g. `Vec<u32>`).
    pub display: String,
    /// Canonical type path (e.g. `alloc::vec::Vec<u32>`).
    pub canonical: String,
    /// Safety-related auto-trait bounds known for this type.
    pub auto_traits: AutoTraits,
}

/// Auto-trait (safety query) results — corresponds to proposal safety queries:
/// `is_freeze`, `is_send`, `is_sync`, `needs_drop`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoTraits {
    pub is_send: bool,
    pub is_sync: bool,
    pub is_freeze: bool,
    pub needs_drop: bool,
}

/// Information about a trait implementation returned by `impl_of` (P07).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImplInfo {
    /// The implementing type (e.g. `Vec<u32>`).
    pub self_type: String,
    /// The trait being implemented (e.g. `Clone`).
    pub trait_name: String,
    /// Whether this is a blanket impl.
    pub is_blanket: bool,
    /// Where clause, if any.
    pub where_clause: Option<String>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during a query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryError {
    /// The requested file was not found in the compilation unit.
    FileNotFound(String),
    /// The expression could not be resolved.
    ExprNotFound(String),
    /// The trait or type was not found.
    ItemNotFound(String),
    /// An internal compiler error prevented the query from completing.
    Internal(String),
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::FileNotFound(path) => write!(f, "file not found: {path}"),
            QueryError::ExprNotFound(expr) => write!(f, "expression not found: {expr}"),
            QueryError::ItemNotFound(item) => write!(f, "item not found: {item}"),
            QueryError::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for QueryError {}

// ---------------------------------------------------------------------------
// Query trait
// ---------------------------------------------------------------------------

/// The stable query interface for agents.
///
/// Each method corresponds to a compiler pass query from the proposal's
/// Compiler Passes Ontology (Appendix B):
///
/// | Method           | Pass  | Description                          |
/// |------------------|-------|--------------------------------------|
/// | `tokens_of`      | P01   | Lex source into a token stream       |
/// | `ast_of`         | P02   | Parse source into an AST             |
/// | `type_of`        | P06   | Resolve the type of an expression    |
/// | `impl_of`        | P07   | Find trait implementations           |
/// | `diagnostics_of` | *all* | Collect diagnostics for a file       |
pub trait QueryEngine {
    /// Return the token stream for the given file (P01 — Lexing).
    fn tokens_of(&self, file: &str) -> Result<TokenStream, QueryError>;

    /// Return the AST for the given file (P02 — Parsing).
    fn ast_of(&self, file: &str) -> Result<AstNode, QueryError>;

    /// Return the type of the expression at `expr` (P06 — Type Checking).
    /// `expr` is a path or location identifier (e.g. `main::x` or `file.rs:10:5`).
    fn type_of(&self, expr: &str) -> Result<TypeInfo, QueryError>;

    /// Return all known implementations of `trait_name` for `type_name` (P07).
    fn impl_of(&self, trait_name: &str, type_name: &str) -> Result<Vec<ImplInfo>, QueryError>;

    /// Return all diagnostics for the given file.
    fn diagnostics_of(&self, file: &str) -> Result<Vec<DiagnosticGraph>, QueryError>;
}

// ---------------------------------------------------------------------------
// Stub implementation (for testing / offline agents)
// ---------------------------------------------------------------------------

/// A stub `QueryEngine` that returns canned responses.
/// Useful for testing agent integrations without a live compiler.
#[derive(Debug, Default)]
pub struct StubQueryEngine {
    files: std::collections::HashMap<String, StubFile>,
}

/// Pre-loaded data for a single file in the stub engine.
#[derive(Debug, Clone, Default)]
pub struct StubFile {
    pub tokens: Vec<Token>,
    pub ast: Option<AstNode>,
    pub diagnostics: Vec<DiagnosticGraph>,
}

impl StubQueryEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a file with pre-built query data.
    pub fn add_file(&mut self, path: impl Into<String>, data: StubFile) {
        self.files.insert(path.into(), data);
    }
}

impl QueryEngine for StubQueryEngine {
    fn tokens_of(&self, file: &str) -> Result<TokenStream, QueryError> {
        let f = self
            .files
            .get(file)
            .ok_or_else(|| QueryError::FileNotFound(file.to_owned()))?;
        Ok(TokenStream {
            tokens: f.tokens.clone(),
        })
    }

    fn ast_of(&self, file: &str) -> Result<AstNode, QueryError> {
        let f = self
            .files
            .get(file)
            .ok_or_else(|| QueryError::FileNotFound(file.to_owned()))?;
        f.ast
            .clone()
            .ok_or_else(|| QueryError::Internal("no AST loaded for stub".into()))
    }

    fn type_of(&self, expr: &str) -> Result<TypeInfo, QueryError> {
        // Stub: always returns unknown type
        Err(QueryError::ExprNotFound(expr.to_owned()))
    }

    fn impl_of(&self, trait_name: &str, type_name: &str) -> Result<Vec<ImplInfo>, QueryError> {
        // Stub: no impls found
        Err(QueryError::ItemNotFound(format!(
            "{trait_name} for {type_name}"
        )))
    }

    fn diagnostics_of(&self, file: &str) -> Result<Vec<DiagnosticGraph>, QueryError> {
        let f = self
            .files
            .get(file)
            .ok_or_else(|| QueryError::FileNotFound(file.to_owned()))?;
        Ok(f.diagnostics.clone())
    }
}

// ---------------------------------------------------------------------------
// JSON serialisation helpers
// ---------------------------------------------------------------------------

/// Serialize any query response to JSON.
pub fn to_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value)
}

/// Deserialize a query response from JSON.
pub fn from_json<'a, T: Deserialize<'a>>(json: &'a str) -> Result<T, serde_json::Error> {
    serde_json::from_str(json)
}

// ---------------------------------------------------------------------------
// Builder helpers
// ---------------------------------------------------------------------------

impl Span {
    pub fn new(
        file: impl Into<String>,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> Self {
        Self {
            file: file.into(),
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }
}

impl Token {
    pub fn new(kind: TokenKind, text: impl Into<String>, span: Span) -> Self {
        Self {
            kind,
            text: text.into(),
            span,
        }
    }
}

impl AstNode {
    pub fn new(kind: AstNodeKind, span: Span) -> Self {
        Self {
            kind,
            span,
            children: Vec::new(),
        }
    }

    pub fn with_child(mut self, child: AstNode) -> Self {
        self.children.push(child);
        self
    }
}

impl TypeInfo {
    pub fn new(display: impl Into<String>, canonical: impl Into<String>) -> Self {
        Self {
            display: display.into(),
            canonical: canonical.into(),
            auto_traits: AutoTraits {
                is_send: false,
                is_sync: false,
                is_freeze: false,
                needs_drop: false,
            },
        }
    }

    pub fn with_auto_traits(mut self, auto_traits: AutoTraits) -> Self {
        self.auto_traits = auto_traits;
        self
    }
}

impl ImplInfo {
    pub fn new(
        self_type: impl Into<String>,
        trait_name: impl Into<String>,
        span: Span,
    ) -> Self {
        Self {
            self_type: self_type.into(),
            trait_name: trait_name.into(),
            is_blanket: false,
            where_clause: None,
            span,
        }
    }

    pub fn blanket(mut self) -> Self {
        self.is_blanket = true;
        self
    }

    pub fn with_where(mut self, clause: impl Into<String>) -> Self {
        self.where_clause = Some(clause.into());
        self
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use redox_diagnostics::{Diagnostic as DiagDiagnostic, Span as DiagSpan};

    fn span(file: &str, line: u32) -> Span {
        Span::new(file, line, 0, line, 80)
    }

    fn diag_span(file: &str, line: u32) -> DiagSpan {
        DiagSpan::new(file, line, 0, 80)
    }

    // -- TokenStream ---------------------------------------------------------

    #[test]
    fn tokens_of_returns_stream() {
        let mut engine = StubQueryEngine::new();
        engine.add_file(
            "main.rs",
            StubFile {
                tokens: vec![
                    Token::new(TokenKind::Keyword, "fn", span("main.rs", 1)),
                    Token::new(TokenKind::Ident, "main", span("main.rs", 1)),
                    Token::new(TokenKind::Punct, "(", span("main.rs", 1)),
                    Token::new(TokenKind::Punct, ")", span("main.rs", 1)),
                    Token::new(TokenKind::Eof, "", span("main.rs", 1)),
                ],
                ..Default::default()
            },
        );

        let ts = engine.tokens_of("main.rs").unwrap();
        assert_eq!(ts.tokens.len(), 5);
        assert_eq!(ts.tokens[0].kind, TokenKind::Keyword);
        assert_eq!(ts.tokens[0].text, "fn");
    }

    #[test]
    fn tokens_of_file_not_found() {
        let engine = StubQueryEngine::new();
        let err = engine.tokens_of("missing.rs").unwrap_err();
        assert_eq!(err, QueryError::FileNotFound("missing.rs".into()));
    }

    // -- AST -----------------------------------------------------------------

    #[test]
    fn ast_of_returns_tree() {
        let mut engine = StubQueryEngine::new();
        let ast = AstNode::new(AstNodeKind::Crate, span("lib.rs", 1)).with_child(AstNode::new(
            AstNodeKind::Function("main".into()),
            span("lib.rs", 3),
        ));
        engine.add_file(
            "lib.rs",
            StubFile {
                ast: Some(ast.clone()),
                ..Default::default()
            },
        );

        let result = engine.ast_of("lib.rs").unwrap();
        assert_eq!(result.kind, AstNodeKind::Crate);
        assert_eq!(result.children.len(), 1);
        assert_eq!(
            result.children[0].kind,
            AstNodeKind::Function("main".into())
        );
    }

    #[test]
    fn ast_of_no_ast_loaded() {
        let mut engine = StubQueryEngine::new();
        engine.add_file("empty.rs", StubFile::default());
        let err = engine.ast_of("empty.rs").unwrap_err();
        assert!(matches!(err, QueryError::Internal(_)));
    }

    // -- type_of (stub always errors) ----------------------------------------

    #[test]
    fn type_of_stub_returns_not_found() {
        let engine = StubQueryEngine::new();
        let err = engine.type_of("main::x").unwrap_err();
        assert_eq!(err, QueryError::ExprNotFound("main::x".into()));
    }

    // -- impl_of (stub always errors) ----------------------------------------

    #[test]
    fn impl_of_stub_returns_not_found() {
        let engine = StubQueryEngine::new();
        let err = engine.impl_of("Clone", "MyStruct").unwrap_err();
        assert_eq!(
            err,
            QueryError::ItemNotFound("Clone for MyStruct".into())
        );
    }

    // -- diagnostics_of ------------------------------------------------------

    #[test]
    fn diagnostics_of_returns_graphs() {
        let mut engine = StubQueryEngine::new();
        let diag = DiagnosticGraph::new(DiagDiagnostic::error(
            "E0308",
            "mismatched types",
            diag_span("main.rs", 10),
        ));
        engine.add_file(
            "main.rs",
            StubFile {
                diagnostics: vec![diag.clone()],
                ..Default::default()
            },
        );

        let results = engine.diagnostics_of("main.rs").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].root.id, "E0308");
    }

    #[test]
    fn diagnostics_of_empty() {
        let mut engine = StubQueryEngine::new();
        engine.add_file("clean.rs", StubFile::default());
        let results = engine.diagnostics_of("clean.rs").unwrap();
        assert!(results.is_empty());
    }

    // -- JSON round-trip -----------------------------------------------------

    #[test]
    fn token_stream_json_roundtrip() {
        let ts = TokenStream {
            tokens: vec![
                Token::new(TokenKind::Keyword, "fn", span("a.rs", 1)),
                Token::new(TokenKind::Ident, "foo", span("a.rs", 1)),
            ],
        };
        let json = to_json(&ts).unwrap();
        let ts2: TokenStream = from_json(&json).unwrap();
        assert_eq!(ts, ts2);
    }

    #[test]
    fn ast_node_json_roundtrip() {
        let node = AstNode::new(AstNodeKind::Crate, span("lib.rs", 1))
            .with_child(AstNode::new(
                AstNodeKind::Struct("Foo".into()),
                span("lib.rs", 5),
            ))
            .with_child(AstNode::new(
                AstNodeKind::Function("bar".into()),
                span("lib.rs", 10),
            ));
        let json = to_json(&node).unwrap();
        let node2: AstNode = from_json(&json).unwrap();
        assert_eq!(node, node2);
    }

    #[test]
    fn type_info_json_roundtrip() {
        let ti = TypeInfo::new("Vec<u32>", "alloc::vec::Vec<u32>").with_auto_traits(AutoTraits {
            is_send: true,
            is_sync: true,
            is_freeze: false,
            needs_drop: true,
        });
        let json = to_json(&ti).unwrap();
        let ti2: TypeInfo = from_json(&json).unwrap();
        assert_eq!(ti, ti2);
    }

    #[test]
    fn impl_info_json_roundtrip() {
        let ii = ImplInfo::new("Vec<u32>", "Clone", span("vec.rs", 100))
            .with_where("T: Clone");
        let json = to_json(&ii).unwrap();
        let ii2: ImplInfo = from_json(&json).unwrap();
        assert_eq!(ii, ii2);
    }

    #[test]
    fn query_error_json_roundtrip() {
        let errors = vec![
            QueryError::FileNotFound("x.rs".into()),
            QueryError::ExprNotFound("foo::bar".into()),
            QueryError::ItemNotFound("Debug for Baz".into()),
            QueryError::Internal("ice".into()),
        ];
        for e in &errors {
            let json = to_json(e).unwrap();
            let e2: QueryError = from_json(&json).unwrap();
            assert_eq!(*e, e2);
        }
    }

    #[test]
    fn query_error_display() {
        assert_eq!(
            QueryError::FileNotFound("x.rs".into()).to_string(),
            "file not found: x.rs"
        );
        assert_eq!(
            QueryError::Internal("oops".into()).to_string(),
            "internal error: oops"
        );
    }

    // -- Builder helpers -----------------------------------------------------

    #[test]
    fn type_info_builder() {
        let ti = TypeInfo::new("i32", "i32");
        assert_eq!(ti.display, "i32");
        assert!(!ti.auto_traits.is_send);

        let ti2 = ti.with_auto_traits(AutoTraits {
            is_send: true,
            is_sync: true,
            is_freeze: true,
            needs_drop: false,
        });
        assert!(ti2.auto_traits.is_send);
        assert!(!ti2.auto_traits.needs_drop);
    }

    #[test]
    fn impl_info_builder() {
        let ii = ImplInfo::new("Foo", "Debug", span("foo.rs", 1))
            .blanket()
            .with_where("T: Debug");
        assert!(ii.is_blanket);
        assert_eq!(ii.where_clause.as_deref(), Some("T: Debug"));
    }

    #[test]
    fn ast_node_builder() {
        let root = AstNode::new(AstNodeKind::Module("my_mod".into()), span("mod.rs", 1))
            .with_child(AstNode::new(
                AstNodeKind::Const("MAX".into()),
                span("mod.rs", 3),
            ));
        assert_eq!(root.children.len(), 1);
    }
}

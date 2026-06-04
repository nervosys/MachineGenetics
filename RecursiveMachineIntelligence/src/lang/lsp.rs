//! RMIL Language Server — IDE support for RMIL programs.
//!
//! Provides language-server–like functionality for RMIL source files:
//! diagnostics, hover information, go-to-definition, completions,
//! and document symbols. This is a library-side implementation that
//! any editor adapter can call; it does not open a socket itself.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐    parse     ┌───────────┐    analyse    ┌────────────┐
//! │  source text │  ────────►   │  Expr AST │  ──────────►  │ Diagnostic │
//! └──────────────┘              └───────────┘               │ Hover info │
//!                                                           │ Completions│
//!                                                           └────────────┘
//! ```
//!
//! The [`LanguageServer`] holds a set of open documents and re-analyses
//! them on every `update`.  Results are returned as structured data
//! that callers can translate to LSP JSON or any other wire format.
//!
//! # Example
//!
//! ```
//! use rmi::lang::lsp::{LanguageServer, Position};
//!
//! let mut ls = LanguageServer::new();
//! ls.open("main.rmil", "let x = relu >> gelu;\nx");
//! let diags = ls.diagnostics("main.rmil");
//! let hover = ls.hover("main.rmil", Position { line: 0, col: 8 });
//! ```

use std::collections::HashMap;

use crate::lang::expr::Expr;
use crate::lang::op::{Op, OpMeta};

// ── Positions & Ranges ───────────────────────────────────────────────────────

/// A position in a text document (0-based line and column).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// 0-based line number.
    pub line: u32,
    /// 0-based UTF-8 byte offset within the line.
    pub col: u32,
}

/// A range in a text document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// Inclusive start position.
    pub start: Position,
    /// Exclusive end position.
    pub end: Position,
}

// ── Diagnostics ──────────────────────────────────────────────────────────────

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Prevents evaluation.
    Error,
    /// Likely mistake, but evaluable.
    Warning,
    /// Informational hint.
    Info,
    /// Style suggestion.
    Hint,
}

/// A diagnostic message attached to a source range.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Where the problem is.
    pub range: Range,
    /// How bad it is.
    pub severity: Severity,
    /// Human-readable message.
    pub message: String,
    /// Optional machine-readable code (e.g. "E001").
    pub code: Option<String>,
}

// ── Hover ────────────────────────────────────────────────────────────────────

/// Hover information for a symbol.
#[derive(Debug, Clone)]
pub struct HoverInfo {
    /// The range for which the hover applies.
    pub range: Range,
    /// Markdown-formatted documentation.
    pub contents: String,
}

// ── Completions ──────────────────────────────────────────────────────────────

/// A completion kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// An opcode (e.g. `relu`, `linear`).
    Opcode,
    /// A keyword (e.g. `let`, `if`).
    Keyword,
    /// A user-defined variable.
    Variable,
    /// A built-in pattern (e.g. `transformer_block`).
    Pattern,
}

/// A single completion item.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// Text to insert.
    pub label: String,
    /// Kind of completion.
    pub kind: CompletionKind,
    /// Short documentation.
    pub detail: Option<String>,
    /// Markdown docs.
    pub documentation: Option<String>,
}

// ── Document symbols ─────────────────────────────────────────────────────────

/// A symbol in a document (let-binding, function, etc.).
#[derive(Debug, Clone)]
pub struct DocumentSymbol {
    /// Symbol name.
    pub name: String,
    /// Kind of symbol.
    pub kind: SymbolKind,
    /// Range of the full declaration.
    pub range: Range,
    /// Range of the name itself.
    pub selection_range: Range,
}

/// Kind of a document symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// A `let` binding.
    Variable,
    /// A `lam` (function).
    Function,
    /// A block.
    Block,
}

// ── Open document ────────────────────────────────────────────────────────────

/// An open document being tracked by the language server.
#[derive(Debug, Clone)]
struct Document {
    /// Current source text.
    source: String,
    /// Line start byte offsets (cached).
    line_starts: Vec<usize>,
    /// Parsed expression (None if parse failed).
    parsed: Option<Expr>,
    /// Cached diagnostics.
    diagnostics: Vec<Diagnostic>,
    /// Bindings found in Let nodes: name → (line, col).
    bindings: Vec<(String, Range)>,
}

impl Document {
    fn new(source: &str) -> Self {
        let line_starts = Self::compute_line_starts(source);
        let mut doc = Self {
            source: source.to_string(),
            line_starts,
            parsed: None,
            diagnostics: Vec::new(),
            bindings: Vec::new(),
        };
        doc.analyse();
        doc
    }

    fn compute_line_starts(source: &str) -> Vec<usize> {
        let mut starts = vec![0];
        for (i, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(i + 1);
            }
        }
        starts
    }

    fn analyse(&mut self) {
        self.diagnostics.clear();
        self.bindings.clear();

        // Try to parse via text syntax
        let parse_result = crate::lang::syntax::parse(&self.source);
        match parse_result {
            Ok(expr) => {
                self.parsed = Some(expr.clone());
                self.collect_bindings(&expr);
                self.check_expr(&expr);
            }
            Err(e) => {
                self.parsed = None;
                self.diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position { line: 0, col: 0 },
                        end: Position {
                            line: 0,
                            col: self.source.len().min(80) as u32,
                        },
                    },
                    severity: Severity::Error,
                    message: format!("Parse error: {e}"),
                    code: Some("E001".into()),
                });
            }
        }
    }

    /// Walk the AST to collect let-bindings.
    fn collect_bindings(&mut self, expr: &Expr) {
        match expr {
            Expr::Let { name, val, body } => {
                let range = Range {
                    start: Position { line: 0, col: 0 },
                    end: Position { line: 0, col: 0 },
                };
                // We don't have exact spans yet, store the name
                let sym_name = format!("sym_{}", name.0);
                self.bindings.push((sym_name, range));
                self.collect_bindings(val);
                self.collect_bindings(body);
            }
            Expr::Seq(a, b) | Expr::Par(a, b) => {
                self.collect_bindings(a);
                self.collect_bindings(b);
            }
            Expr::Cond { pred, yes, no } => {
                self.collect_bindings(pred);
                self.collect_bindings(yes);
                self.collect_bindings(no);
            }
            Expr::Block(exprs) => {
                for e in exprs {
                    self.collect_bindings(e);
                }
            }
            Expr::App(_, args) => {
                for a in args {
                    self.collect_bindings(a);
                }
            }
            _ => {}
        }
    }

    /// Run simple static checks on the AST.
    fn check_expr(&mut self, expr: &Expr) {
        // Check for excessively deep nesting
        let depth = expr.depth();
        if depth > 100 {
            self.diagnostics.push(Diagnostic {
                range: Range {
                    start: Position { line: 0, col: 0 },
                    end: Position { line: 0, col: 0 },
                },
                severity: Severity::Warning,
                message: format!(
                    "Expression tree depth is {depth}, which may cause stack overflow at runtime"
                ),
                code: Some("W001".into()),
            });
        }

        // Check for empty blocks
        if let Expr::Block(exprs) = expr {
            if exprs.is_empty() {
                self.diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position { line: 0, col: 0 },
                        end: Position { line: 0, col: 0 },
                    },
                    severity: Severity::Warning,
                    message: "Empty block expression".into(),
                    code: Some("W002".into()),
                });
            }
        }
    }
}

// ── Language Server ──────────────────────────────────────────────────────────

/// The RMIL language server core.
///
/// Manages open documents and provides IDE features.
pub struct LanguageServer {
    documents: HashMap<String, Document>,
}

impl LanguageServer {
    /// Create a new language server with no open documents.
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    /// Open a document (or replace existing).
    pub fn open(&mut self, uri: &str, source: &str) {
        self.documents
            .insert(uri.to_string(), Document::new(source));
    }

    /// Update a document's full text.
    pub fn update(&mut self, uri: &str, source: &str) {
        self.documents
            .insert(uri.to_string(), Document::new(source));
    }

    /// Close a document.
    pub fn close(&mut self, uri: &str) {
        self.documents.remove(uri);
    }

    /// Get diagnostics for a document.
    pub fn diagnostics(&self, uri: &str) -> Vec<Diagnostic> {
        self.documents
            .get(uri)
            .map(|d| d.diagnostics.clone())
            .unwrap_or_default()
    }

    /// Get hover info at a position.
    pub fn hover(&self, uri: &str, pos: Position) -> Option<HoverInfo> {
        let doc = self.documents.get(uri)?;
        let source = &doc.source;

        // Find the word under the cursor
        let offset = doc.line_starts.get(pos.line as usize).copied()? + pos.col as usize;
        if offset >= source.len() {
            return None;
        }

        let word = extract_word(source, offset)?;

        // Try to match against opcodes
        for op in Op::ALL {
            let meta = op.meta();
            if meta.name.eq_ignore_ascii_case(&word) {
                let range = Range {
                    start: pos,
                    end: Position {
                        line: pos.line,
                        col: pos.col + word.len() as u32,
                    },
                };
                return Some(HoverInfo {
                    range,
                    contents: format_op_hover(&meta),
                });
            }
        }

        // Try built-in keywords
        let keyword_doc = match word.as_str() {
            "let" => Some("**let** — bind a value to a name\n\n`let x = expr; body`"),
            "if" => Some("**if** — conditional branch\n\n`if pred then yes else no`"),
            "fn" | "lam" => Some("**fn** / **lam** — lambda (anonymous function)"),
            ">>" => Some("**>>** — sequential composition\n\n`a >> b` runs `a` then `b`"),
            "|" => Some("**|** — parallel composition\n\n`a | b` runs `a` and `b` concurrently"),
            _ => None,
        };

        keyword_doc.map(|contents| HoverInfo {
            range: Range {
                start: pos,
                end: Position {
                    line: pos.line,
                    col: pos.col + word.len() as u32,
                },
            },
            contents: contents.to_string(),
        })
    }

    /// Get completions at a position.
    pub fn completions(&self, _uri: &str, _pos: Position) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // All opcodes
        for op in Op::ALL {
            let meta = op.meta();
            items.push(CompletionItem {
                label: meta.name.to_lowercase(),
                kind: CompletionKind::Opcode,
                detail: Some(format!("Op 0x{:04X} — arity {}", op.0, meta.arity)),
                documentation: Some(meta.desc.to_string()),
            });
        }

        // Keywords
        for (kw, detail) in [
            ("let", "Bind a value to a name"),
            ("if", "Conditional branch"),
            ("fn", "Lambda / anonymous function"),
        ] {
            items.push(CompletionItem {
                label: kw.into(),
                kind: CompletionKind::Keyword,
                detail: Some(detail.into()),
                documentation: None,
            });
        }

        // Built-in patterns
        for (name, desc) in [
            ("transformer_block", "Multi-head attention + FFN block"),
            ("mlp", "Multi-layer perceptron"),
            ("resnet_block", "Residual network block"),
            ("rnn_model", "Recurrent model (LSTM/GRU)"),
            (
                "classifier_head",
                "Classification head (pool→linear→softmax)",
            ),
        ] {
            items.push(CompletionItem {
                label: name.into(),
                kind: CompletionKind::Pattern,
                detail: Some(desc.into()),
                documentation: None,
            });
        }

        items
    }

    /// Get document symbols (let bindings, blocks, etc.).
    pub fn document_symbols(&self, uri: &str) -> Vec<DocumentSymbol> {
        let doc = match self.documents.get(uri) {
            Some(d) => d,
            None => return Vec::new(),
        };

        doc.bindings
            .iter()
            .map(|(name, range)| DocumentSymbol {
                name: name.clone(),
                kind: SymbolKind::Variable,
                range: *range,
                selection_range: *range,
            })
            .collect()
    }

    /// Go to definition for a symbol at the given position.
    pub fn goto_definition(&self, uri: &str, pos: Position) -> Option<(String, Range)> {
        let doc = self.documents.get(uri)?;
        let offset = doc.line_starts.get(pos.line as usize).copied()? + pos.col as usize;
        let word = extract_word(&doc.source, offset)?;

        // Look for a binding with that name
        for (name, range) in &doc.bindings {
            if name == &word {
                return Some((uri.to_string(), *range));
            }
        }

        None
    }

    /// List all open document URIs.
    pub fn open_documents(&self) -> Vec<&str> {
        self.documents.keys().map(|s| s.as_str()).collect()
    }

    /// Get the parsed expression for a document (if parse succeeded).
    pub fn parsed_expr(&self, uri: &str) -> Option<&Expr> {
        self.documents.get(uri)?.parsed.as_ref()
    }
}

impl Default for LanguageServer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Extract a word (alphanumeric + underscore) around a byte offset.
fn extract_word(source: &str, offset: usize) -> Option<String> {
    let bytes = source.as_bytes();
    if offset >= bytes.len() {
        return None;
    }

    fn is_word_byte(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'_'
    }

    if !is_word_byte(bytes[offset]) {
        // Check operators
        let b = bytes[offset];
        if b == b'>' && offset + 1 < bytes.len() && bytes[offset + 1] == b'>' {
            return Some(">>".into());
        }
        if b == b'|' {
            return Some("|".into());
        }
        return None;
    }

    let start = (0..=offset)
        .rev()
        .take_while(|&i| is_word_byte(bytes[i]))
        .last()
        .unwrap_or(offset);

    let end = (offset..bytes.len())
        .take_while(|&i| is_word_byte(bytes[i]))
        .last()
        .map(|i| i + 1)
        .unwrap_or(offset + 1);

    Some(String::from_utf8_lossy(&bytes[start..end]).into_owned())
}

/// Format hover markdown for an opcode.
fn format_op_hover(meta: &OpMeta) -> String {
    let mut s = format!("**{}** \u{2014} {}\n\n", meta.name, meta.desc);
    s.push_str(&format!("- Arity: {}\n", meta.arity));
    s.push_str(&format!(
        "- Differentiable: {}\n",
        if meta.differentiable { "yes" } else { "no" }
    ));
    s.push_str(&format!(
        "- Stateful: {}\n",
        if meta.stateful { "yes" } else { "no" }
    ));
    if meta.has_params {
        s.push_str("- Has parameters\n");
    }
    s
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_and_diagnostics() {
        let mut ls = LanguageServer::new();
        ls.open("test.rmil", "relu >> gelu");
        let diags = ls.diagnostics("test.rmil");
        // May or may not have diagnostics depending on parser support
        // At minimum, shouldn't panic
        let _ = diags;
    }

    #[test]
    fn test_completions_include_opcodes() {
        let ls = LanguageServer::new();
        let completions = ls.completions("test.rmil", Position { line: 0, col: 0 });
        assert!(!completions.is_empty());
        let has_relu = completions.iter().any(|c| c.label == "relu");
        assert!(has_relu);
    }

    #[test]
    fn test_completions_include_keywords() {
        let ls = LanguageServer::new();
        let completions = ls.completions("test.rmil", Position { line: 0, col: 0 });
        let has_let = completions.iter().any(|c| c.label == "let");
        assert!(has_let);
    }

    #[test]
    fn test_completions_include_patterns() {
        let ls = LanguageServer::new();
        let completions = ls.completions("test.rmil", Position { line: 0, col: 0 });
        let has_transformer = completions.iter().any(|c| c.label == "transformer_block");
        assert!(has_transformer);
    }

    #[test]
    fn test_hover_keyword() {
        let mut ls = LanguageServer::new();
        ls.open("test.rmil", "let x = relu;");
        let hover = ls.hover("test.rmil", Position { line: 0, col: 0 });
        assert!(hover.is_some());
        assert!(hover.unwrap().contents.contains("let"));
    }

    #[test]
    fn test_close_document() {
        let mut ls = LanguageServer::new();
        ls.open("test.rmil", "relu");
        assert_eq!(ls.open_documents().len(), 1);
        ls.close("test.rmil");
        assert_eq!(ls.open_documents().len(), 0);
    }

    #[test]
    fn test_update_document() {
        let mut ls = LanguageServer::new();
        ls.open("test.rmil", "relu");
        ls.update("test.rmil", "gelu");
        assert_eq!(ls.open_documents().len(), 1);
    }

    #[test]
    fn test_extract_word() {
        assert_eq!(extract_word("relu >> gelu", 0), Some("relu".into()));
        assert_eq!(extract_word("relu >> gelu", 8), Some("gelu".into()));
        assert_eq!(extract_word("relu >> gelu", 5), Some(">>".into()));
    }

    #[test]
    fn test_extract_word_pipe() {
        assert_eq!(extract_word("relu | gelu", 5), Some("|".into()));
    }

    #[test]
    fn test_default_impl() {
        let ls = LanguageServer::default();
        assert!(ls.open_documents().is_empty());
    }

    #[test]
    fn test_nonexistent_document() {
        let ls = LanguageServer::new();
        assert!(ls.diagnostics("nope.rmil").is_empty());
        assert!(ls
            .hover("nope.rmil", Position { line: 0, col: 0 })
            .is_none());
        assert!(ls.document_symbols("nope.rmil").is_empty());
    }
}

//! # Redox RAP–IDE Integration
//!
//! Bridges the Redox Agent Protocol (RAP) server with IDE infrastructure
//! (VS Code extension, LSP integration). Implements the RAP Server ↔ IDE
//! integration point from REDOX_PROPOSAL.md Appendix D.
//!
//! This crate provides:
//! - **LspAdapter**: Translates between LSP requests/responses and RAP methods
//! - **IdeCapabilities**: IDE-specific capabilities layered on RAP
//! - **DocumentSync**: Text document synchronization (open/change/close)
//! - **DiagnosticBridge**: Converts RAP diagnostics to LSP-format diagnostics
//! - **CompletionBridge**: Converts RAP completions to LSP completions
//! - **ExtensionManifest**: VS Code extension configuration

use std::collections::BTreeMap;
use std::fmt;

// ── Positions & Ranges ──────────────────────────────────────────────────────

/// A zero-based line/character position in a document (LSP-compatible).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.character)
    }
}

/// A range in a document (LSP-compatible).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn single_line(line: u32, start_char: u32, end_char: u32) -> Self {
        Self {
            start: Position::new(line, start_char),
            end: Position::new(line, end_char),
        }
    }

    pub fn contains(&self, pos: Position) -> bool {
        if pos.line < self.start.line || pos.line > self.end.line {
            return false;
        }
        if pos.line == self.start.line && pos.character < self.start.character {
            return false;
        }
        if pos.line == self.end.line && pos.character > self.end.character {
            return false;
        }
        true
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

// ── URI ─────────────────────────────────────────────────────────────────────

/// A document URI (simplified representation for IDE documents).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentUri(pub String);

impl DocumentUri {
    pub fn from_path(path: &str) -> Self {
        Self(format!("file://{}", path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract the file path from the URI (strips "file://").
    pub fn to_path(&self) -> Option<&str> {
        self.0.strip_prefix("file://")
    }
}

impl fmt::Display for DocumentUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Diagnostics ─────────────────────────────────────────────────────────────

/// Diagnostic severity levels (LSP-compatible numbering).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticSeverity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticSeverity::Error => write!(f, "error"),
            DiagnosticSeverity::Warning => write!(f, "warning"),
            DiagnosticSeverity::Information => write!(f, "information"),
            DiagnosticSeverity::Hint => write!(f, "hint"),
        }
    }
}

/// A diagnostic message with source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub code: Option<String>,
    pub source: String,
    pub message: String,
    pub related: Vec<DiagnosticRelated>,
}

impl Diagnostic {
    pub fn error(range: Range, message: &str) -> Self {
        Self {
            range,
            severity: DiagnosticSeverity::Error,
            code: None,
            source: "redox".to_string(),
            message: message.to_string(),
            related: Vec::new(),
        }
    }

    pub fn warning(range: Range, message: &str) -> Self {
        Self {
            range,
            severity: DiagnosticSeverity::Warning,
            code: None,
            source: "redox".to_string(),
            message: message.to_string(),
            related: Vec::new(),
        }
    }

    pub fn with_code(mut self, code: &str) -> Self {
        self.code = Some(code.to_string());
        self
    }

    pub fn with_related(mut self, related: DiagnosticRelated) -> Self {
        self.related.push(related);
        self
    }
}

/// Related diagnostic information (e.g. "original definition here").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticRelated {
    pub uri: DocumentUri,
    pub range: Range,
    pub message: String,
}

// ── Code Actions ────────────────────────────────────────────────────────────

/// A code action (quick fix, refactoring) from the RAP build/heal flow.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeAction {
    pub title: String,
    pub kind: CodeActionKind,
    pub edits: Vec<TextEdit>,
    pub diagnostics: Vec<Diagnostic>,
    pub is_preferred: bool,
    pub confidence: f32,
}

/// Code action kind (LSP-compatible).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeActionKind {
    QuickFix,
    Refactor,
    RefactorExtract,
    RefactorInline,
    RefactorRewrite,
    Source,
    SourceOrganizeImports,
}

impl CodeActionKind {
    pub fn as_lsp_str(&self) -> &str {
        match self {
            CodeActionKind::QuickFix => "quickfix",
            CodeActionKind::Refactor => "refactor",
            CodeActionKind::RefactorExtract => "refactor.extract",
            CodeActionKind::RefactorInline => "refactor.inline",
            CodeActionKind::RefactorRewrite => "refactor.rewrite",
            CodeActionKind::Source => "source",
            CodeActionKind::SourceOrganizeImports => "source.organizeImports",
        }
    }
}

impl fmt::Display for CodeActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_lsp_str())
    }
}

/// A text edit: replace a range with new text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

impl TextEdit {
    pub fn new(range: Range, new_text: &str) -> Self {
        Self {
            range,
            new_text: new_text.to_string(),
        }
    }

    pub fn insert(position: Position, text: &str) -> Self {
        Self {
            range: Range::new(position, position),
            new_text: text.to_string(),
        }
    }
}

// ── Completions ─────────────────────────────────────────────────────────────

/// Completion item kind (LSP-compatible numbering).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemKind {
    Text = 1,
    Method = 2,
    Function = 3,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Property = 10,
    Unit = 11,
    Value = 12,
    Enum = 13,
    Keyword = 14,
    Snippet = 15,
    Struct = 22,
}

/// A completion item from RAP semantic analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
    pub sort_text: Option<String>,
    pub filter_text: Option<String>,
}

impl CompletionItem {
    pub fn new(label: &str, kind: CompletionItemKind) -> Self {
        Self {
            label: label.to_string(),
            kind,
            detail: None,
            documentation: None,
            insert_text: None,
            sort_text: None,
            filter_text: None,
        }
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_documentation(mut self, doc: &str) -> Self {
        self.documentation = Some(doc.to_string());
        self
    }

    pub fn with_insert_text(mut self, text: &str) -> Self {
        self.insert_text = Some(text.to_string());
        self
    }

    pub fn with_sort_text(mut self, text: &str) -> Self {
        self.sort_text = Some(text.to_string());
        self
    }
}

// ── Hover ───────────────────────────────────────────────────────────────────

/// Hover information (type, documentation, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverInfo {
    pub contents: Vec<HoverContent>,
    pub range: Option<Range>,
}

/// A block of hover content (markdown or code).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoverContent {
    Markdown(String),
    Code { language: String, value: String },
}

impl HoverInfo {
    pub fn markdown(text: &str) -> Self {
        Self {
            contents: vec![HoverContent::Markdown(text.to_string())],
            range: None,
        }
    }

    pub fn code(language: &str, code: &str) -> Self {
        Self {
            contents: vec![HoverContent::Code {
                language: language.to_string(),
                value: code.to_string(),
            }],
            range: None,
        }
    }

    pub fn with_range(mut self, range: Range) -> Self {
        self.range = Some(range);
        self
    }
}

// ── Symbols ─────────────────────────────────────────────────────────────────

/// Symbol kind (LSP-compatible).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    File = 1,
    Module = 2,
    Namespace = 3,
    Package = 4,
    Class = 5,
    Method = 6,
    Property = 7,
    Field = 8,
    Constructor = 9,
    Enum = 10,
    Interface = 11,
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Struct = 23,
    TypeParameter = 26,
}

/// A document symbol (outline item).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub range: Range,
    pub selection_range: Range,
    pub detail: Option<String>,
    pub children: Vec<DocumentSymbol>,
}

impl DocumentSymbol {
    pub fn new(name: &str, kind: SymbolKind, range: Range) -> Self {
        Self {
            name: name.to_string(),
            kind,
            range,
            selection_range: range,
            detail: None,
            children: Vec::new(),
        }
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_child(mut self, child: DocumentSymbol) -> Self {
        self.children.push(child);
        self
    }
}

// ── Document Synchronization ────────────────────────────────────────────────

/// A tracked document in the IDE session.
#[derive(Debug, Clone)]
pub struct TrackedDocument {
    pub uri: DocumentUri,
    pub language_id: String,
    pub version: i32,
    pub content: String,
}

/// Content change event (incremental or full).
#[derive(Debug, Clone)]
pub enum ContentChange {
    /// Full document replacement.
    Full { text: String },
    /// Incremental edit.
    Incremental { range: Range, text: String },
}

/// Document synchronization manager.
pub struct DocumentSync {
    documents: BTreeMap<String, TrackedDocument>,
}

impl DocumentSync {
    pub fn new() -> Self {
        Self {
            documents: BTreeMap::new(),
        }
    }

    /// Open a document for tracking.
    pub fn open(&mut self, uri: DocumentUri, language_id: &str, version: i32, content: &str) {
        let doc = TrackedDocument {
            uri: uri.clone(),
            language_id: language_id.to_string(),
            version,
            content: content.to_string(),
        };
        self.documents.insert(uri.0, doc);
    }

    /// Apply changes to a tracked document.
    pub fn change(
        &mut self,
        uri: &DocumentUri,
        version: i32,
        changes: &[ContentChange],
    ) -> Result<(), IdeError> {
        let doc = self
            .documents
            .get_mut(&uri.0)
            .ok_or_else(|| IdeError::DocumentNotFound(uri.clone()))?;

        for change in changes {
            match change {
                ContentChange::Full { text } => {
                    doc.content = text.clone();
                }
                ContentChange::Incremental { range, text } => {
                    apply_incremental_edit(&mut doc.content, *range, text);
                }
            }
        }
        doc.version = version;
        Ok(())
    }

    /// Close and stop tracking a document.
    pub fn close(&mut self, uri: &DocumentUri) -> Result<TrackedDocument, IdeError> {
        self.documents
            .remove(&uri.0)
            .ok_or_else(|| IdeError::DocumentNotFound(uri.clone()))
    }

    /// Get a tracked document.
    pub fn get(&self, uri: &DocumentUri) -> Option<&TrackedDocument> {
        self.documents.get(&uri.0)
    }

    /// Number of tracked documents.
    pub fn count(&self) -> usize {
        self.documents.len()
    }

    /// All tracked URIs.
    pub fn uris(&self) -> Vec<&str> {
        self.documents.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for DocumentSync {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply an incremental text edit to a document string.
fn apply_incremental_edit(content: &mut String, range: Range, new_text: &str) {
    let lines: Vec<&str> = content.split('\n').collect();
    let mut offset_start = 0usize;
    for i in 0..range.start.line as usize {
        if i < lines.len() {
            offset_start += lines[i].len() + 1; // +1 for '\n'
        }
    }
    offset_start += range.start.character as usize;

    let mut offset_end = 0usize;
    for i in 0..range.end.line as usize {
        if i < lines.len() {
            offset_end += lines[i].len() + 1;
        }
    }
    offset_end += range.end.character as usize;

    // Clamp to content length
    let len = content.len();
    let offset_start = offset_start.min(len);
    let offset_end = offset_end.min(len);

    content.replace_range(offset_start..offset_end, new_text);
}

// ── Diagnostic Bridge ───────────────────────────────────────────────────────

/// Converts RAP diagnostics to LSP-format diagnostics.
pub struct DiagnosticBridge {
    diagnostics: BTreeMap<String, Vec<Diagnostic>>,
}

impl DiagnosticBridge {
    pub fn new() -> Self {
        Self {
            diagnostics: BTreeMap::new(),
        }
    }

    /// Publish diagnostics for a document (replaces previous set).
    pub fn publish(&mut self, uri: &DocumentUri, diagnostics: Vec<Diagnostic>) {
        self.diagnostics.insert(uri.0.clone(), diagnostics);
    }

    /// Clear diagnostics for a document.
    pub fn clear(&mut self, uri: &DocumentUri) {
        self.diagnostics.remove(&uri.0);
    }

    /// Get diagnostics for a document.
    pub fn get(&self, uri: &DocumentUri) -> &[Diagnostic] {
        self.diagnostics
            .get(&uri.0)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all diagnostics across all documents.
    pub fn all(&self) -> Vec<(&str, &[Diagnostic])> {
        self.diagnostics
            .iter()
            .map(|(uri, diags)| (uri.as_str(), diags.as_slice()))
            .collect()
    }

    /// Total diagnostic count.
    pub fn total_count(&self) -> usize {
        self.diagnostics.values().map(|v| v.len()).sum()
    }

    /// Count by severity.
    pub fn count_by_severity(&self, severity: DiagnosticSeverity) -> usize {
        self.diagnostics
            .values()
            .flat_map(|v| v.iter())
            .filter(|d| d.severity == severity)
            .count()
    }
}

impl Default for DiagnosticBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ── LSP Method Mapping ──────────────────────────────────────────────────────

/// Maps LSP method names to RAP method names and vice versa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodMapping {
    pub lsp_method: String,
    pub rap_method: String,
    pub direction: MethodDirection,
}

/// Direction of a method mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodDirection {
    /// LSP request → RAP method (IDE → compiler)
    LspToRap,
    /// RAP notification → LSP notification (compiler → IDE)
    RapToLsp,
    /// Bidirectional
    Bidirectional,
}

impl fmt::Display for MethodDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MethodDirection::LspToRap => write!(f, "LSP→RAP"),
            MethodDirection::RapToLsp => write!(f, "RAP→LSP"),
            MethodDirection::Bidirectional => write!(f, "LSP↔RAP"),
        }
    }
}

/// The LSP ↔ RAP adapter. Maps standard LSP methods to RAP endpoints.
pub struct LspAdapter {
    mappings: Vec<MethodMapping>,
    custom_methods: BTreeMap<String, String>,
}

impl LspAdapter {
    /// Create an adapter with standard LSP ↔ RAP mappings.
    pub fn new() -> Self {
        let mappings = vec![
            MethodMapping {
                lsp_method: "initialize".to_string(),
                rap_method: "initialize".to_string(),
                direction: MethodDirection::LspToRap,
            },
            MethodMapping {
                lsp_method: "shutdown".to_string(),
                rap_method: "shutdown".to_string(),
                direction: MethodDirection::LspToRap,
            },
            MethodMapping {
                lsp_method: "textDocument/completion".to_string(),
                rap_method: "query.search".to_string(),
                direction: MethodDirection::LspToRap,
            },
            MethodMapping {
                lsp_method: "textDocument/hover".to_string(),
                rap_method: "query.types".to_string(),
                direction: MethodDirection::LspToRap,
            },
            MethodMapping {
                lsp_method: "textDocument/definition".to_string(),
                rap_method: "skb/query".to_string(),
                direction: MethodDirection::LspToRap,
            },
            MethodMapping {
                lsp_method: "textDocument/diagnostic".to_string(),
                rap_method: "diagnostic.check".to_string(),
                direction: MethodDirection::LspToRap,
            },
            MethodMapping {
                lsp_method: "textDocument/codeAction".to_string(),
                rap_method: "build/heal".to_string(),
                direction: MethodDirection::LspToRap,
            },
            MethodMapping {
                lsp_method: "textDocument/formatting".to_string(),
                rap_method: "format/expand".to_string(),
                direction: MethodDirection::LspToRap,
            },
            MethodMapping {
                lsp_method: "textDocument/publishDiagnostics".to_string(),
                rap_method: "diagnostic.push".to_string(),
                direction: MethodDirection::RapToLsp,
            },
            MethodMapping {
                lsp_method: "textDocument/didOpen".to_string(),
                rap_method: "document.open".to_string(),
                direction: MethodDirection::LspToRap,
            },
            MethodMapping {
                lsp_method: "textDocument/didChange".to_string(),
                rap_method: "document.change".to_string(),
                direction: MethodDirection::LspToRap,
            },
            MethodMapping {
                lsp_method: "textDocument/didClose".to_string(),
                rap_method: "document.close".to_string(),
                direction: MethodDirection::LspToRap,
            },
        ];

        Self {
            mappings,
            custom_methods: BTreeMap::new(),
        }
    }

    /// Register a custom LSP extension method mapped to a RAP method.
    pub fn register_custom(&mut self, lsp_method: &str, rap_method: &str) {
        self.custom_methods
            .insert(lsp_method.to_string(), rap_method.to_string());
    }

    /// Translate an LSP method name to the corresponding RAP method.
    pub fn lsp_to_rap(&self, lsp_method: &str) -> Option<&str> {
        // Check custom mappings first
        if let Some(rap) = self.custom_methods.get(lsp_method) {
            return Some(rap);
        }
        // Then standard mappings
        self.mappings
            .iter()
            .find(|m| m.lsp_method == lsp_method && m.direction != MethodDirection::RapToLsp)
            .map(|m| m.rap_method.as_str())
    }

    /// Translate a RAP method to the corresponding LSP notification method.
    pub fn rap_to_lsp(&self, rap_method: &str) -> Option<&str> {
        self.mappings
            .iter()
            .find(|m| m.rap_method == rap_method && m.direction != MethodDirection::LspToRap)
            .map(|m| m.lsp_method.as_str())
    }

    /// Get all registered mappings.
    pub fn mappings(&self) -> &[MethodMapping] {
        &self.mappings
    }

    /// Number of standard mappings.
    pub fn standard_mapping_count(&self) -> usize {
        self.mappings.len()
    }

    /// Number of custom mappings.
    pub fn custom_mapping_count(&self) -> usize {
        self.custom_methods.len()
    }
}

impl Default for LspAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ── IDE Capabilities ────────────────────────────────────────────────────────

/// IDE-specific capabilities built on top of RAP.
#[derive(Debug, Clone)]
pub struct IdeCapabilities {
    pub completion: bool,
    pub hover: bool,
    pub definition: bool,
    pub references: bool,
    pub document_symbol: bool,
    pub code_action: bool,
    pub formatting: bool,
    pub diagnostics: bool,
    pub semantic_tokens: bool,
    pub inlay_hints: bool,
    pub code_lens: bool,
    /// RAP agentic extensions exposed to the IDE.
    pub rap_cost_query: bool,
    pub rap_build_heal: bool,
    pub rap_skb_query: bool,
    pub rap_verify_contracts: bool,
}

impl IdeCapabilities {
    /// Minimal capabilities (diagnostics + hover only).
    pub fn minimal() -> Self {
        Self {
            completion: false,
            hover: true,
            definition: false,
            references: false,
            document_symbol: false,
            code_action: false,
            formatting: false,
            diagnostics: true,
            semantic_tokens: false,
            inlay_hints: false,
            code_lens: false,
            rap_cost_query: false,
            rap_build_heal: false,
            rap_skb_query: false,
            rap_verify_contracts: false,
        }
    }

    /// Full capabilities (all LSP + all RAP extensions).
    pub fn full() -> Self {
        Self {
            completion: true,
            hover: true,
            definition: true,
            references: true,
            document_symbol: true,
            code_action: true,
            formatting: true,
            diagnostics: true,
            semantic_tokens: true,
            inlay_hints: true,
            code_lens: true,
            rap_cost_query: true,
            rap_build_heal: true,
            rap_skb_query: true,
            rap_verify_contracts: true,
        }
    }

    /// Count enabled capabilities.
    pub fn enabled_count(&self) -> usize {
        let mut count = 0;
        if self.completion { count += 1; }
        if self.hover { count += 1; }
        if self.definition { count += 1; }
        if self.references { count += 1; }
        if self.document_symbol { count += 1; }
        if self.code_action { count += 1; }
        if self.formatting { count += 1; }
        if self.diagnostics { count += 1; }
        if self.semantic_tokens { count += 1; }
        if self.inlay_hints { count += 1; }
        if self.code_lens { count += 1; }
        if self.rap_cost_query { count += 1; }
        if self.rap_build_heal { count += 1; }
        if self.rap_skb_query { count += 1; }
        if self.rap_verify_contracts { count += 1; }
        count
    }
}

// ── VS Code Extension Manifest ──────────────────────────────────────────────

/// Configuration for the VS Code extension that connects to the RAP server.
#[derive(Debug, Clone)]
pub struct ExtensionManifest {
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub publisher: String,
    pub description: String,
    pub language_id: String,
    pub file_extensions: Vec<String>,
    pub server_command: String,
    pub server_args: Vec<String>,
}

impl ExtensionManifest {
    /// Create the default Redox VS Code extension manifest.
    pub fn default_redox() -> Self {
        Self {
            name: "redox-lang".to_string(),
            display_name: "Redox Language".to_string(),
            version: "0.1.0".to_string(),
            publisher: "nervosys".to_string(),
            description: "Redox language support via RAP server".to_string(),
            language_id: "redox".to_string(),
            file_extensions: vec![".mg".to_string(), ".redox".to_string()],
            server_command: "redox-rap-server".to_string(),
            server_args: vec!["--stdio".to_string()],
        }
    }

    /// Generate a package.json-compatible configuration map.
    pub fn to_config(&self) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        map.insert("name".to_string(), self.name.clone());
        map.insert("displayName".to_string(), self.display_name.clone());
        map.insert("version".to_string(), self.version.clone());
        map.insert("publisher".to_string(), self.publisher.clone());
        map.insert("description".to_string(), self.description.clone());
        map.insert("languageId".to_string(), self.language_id.clone());
        map.insert(
            "fileExtensions".to_string(),
            self.file_extensions.join(","),
        );
        map.insert("serverCommand".to_string(), self.server_command.clone());
        map
    }
}

// ── IDE Session ─────────────────────────────────────────────────────────────

/// An active IDE session combining document sync, diagnostics, and LSP mapping.
pub struct IdeSession {
    pub capabilities: IdeCapabilities,
    pub doc_sync: DocumentSync,
    pub diagnostics: DiagnosticBridge,
    pub adapter: LspAdapter,
    pub manifest: ExtensionManifest,
    state: IdeSessionState,
}

/// IDE session lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeSessionState {
    Uninitialized,
    Initializing,
    Ready,
    ShuttingDown,
}

impl fmt::Display for IdeSessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdeSessionState::Uninitialized => write!(f, "uninitialized"),
            IdeSessionState::Initializing => write!(f, "initializing"),
            IdeSessionState::Ready => write!(f, "ready"),
            IdeSessionState::ShuttingDown => write!(f, "shutting_down"),
        }
    }
}

impl IdeSession {
    /// Create a new IDE session with full capabilities.
    pub fn new() -> Self {
        Self {
            capabilities: IdeCapabilities::full(),
            doc_sync: DocumentSync::new(),
            diagnostics: DiagnosticBridge::new(),
            adapter: LspAdapter::new(),
            manifest: ExtensionManifest::default_redox(),
            state: IdeSessionState::Uninitialized,
        }
    }

    /// Create with specific capabilities.
    pub fn with_capabilities(caps: IdeCapabilities) -> Self {
        Self {
            capabilities: caps,
            doc_sync: DocumentSync::new(),
            diagnostics: DiagnosticBridge::new(),
            adapter: LspAdapter::new(),
            manifest: ExtensionManifest::default_redox(),
            state: IdeSessionState::Uninitialized,
        }
    }

    /// Initialize the session (transition to Ready).
    pub fn initialize(&mut self) -> Result<(), IdeError> {
        if self.state != IdeSessionState::Uninitialized {
            return Err(IdeError::InvalidState {
                expected: IdeSessionState::Uninitialized,
                actual: self.state,
            });
        }
        self.state = IdeSessionState::Initializing;
        // In a real implementation, this would connect to the RAP server
        self.state = IdeSessionState::Ready;
        Ok(())
    }

    /// Shutdown the session.
    pub fn shutdown(&mut self) -> Result<(), IdeError> {
        if self.state != IdeSessionState::Ready {
            return Err(IdeError::InvalidState {
                expected: IdeSessionState::Ready,
                actual: self.state,
            });
        }
        self.state = IdeSessionState::ShuttingDown;
        Ok(())
    }

    /// Whether the session is ready to handle requests.
    pub fn is_ready(&self) -> bool {
        self.state == IdeSessionState::Ready
    }

    /// Get the session state.
    pub fn state(&self) -> IdeSessionState {
        self.state
    }

    /// Handle an LSP-style request by mapping to RAP.
    pub fn handle_request(
        &self,
        lsp_method: &str,
    ) -> Result<String, IdeError> {
        if self.state != IdeSessionState::Ready {
            return Err(IdeError::NotReady);
        }
        let rap_method = self
            .adapter
            .lsp_to_rap(lsp_method)
            .ok_or_else(|| IdeError::UnsupportedMethod(lsp_method.to_string()))?;
        Ok(rap_method.to_string())
    }
}

impl Default for IdeSession {
    fn default() -> Self {
        Self::new()
    }
}

// ── Errors ──────────────────────────────────────────────────────────────────

/// Errors from the IDE integration layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdeError {
    DocumentNotFound(DocumentUri),
    UnsupportedMethod(String),
    NotReady,
    InvalidState {
        expected: IdeSessionState,
        actual: IdeSessionState,
    },
}

impl fmt::Display for IdeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdeError::DocumentNotFound(uri) => write!(f, "Document not found: {}", uri),
            IdeError::UnsupportedMethod(m) => write!(f, "Unsupported method: {}", m),
            IdeError::NotReady => write!(f, "IDE session not ready"),
            IdeError::InvalidState { expected, actual } => {
                write!(f, "Invalid state: expected {}, got {}", expected, actual)
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Position & Range ──

    #[test]
    fn position_display() {
        assert_eq!(format!("{}", Position::new(10, 5)), "10:5");
    }

    #[test]
    fn range_contains() {
        let range = Range::new(Position::new(5, 0), Position::new(5, 20));
        assert!(range.contains(Position::new(5, 10)));
        assert!(!range.contains(Position::new(6, 0)));
        assert!(!range.contains(Position::new(4, 0)));
    }

    #[test]
    fn range_single_line() {
        let r = Range::single_line(3, 5, 15);
        assert_eq!(r.start.line, 3);
        assert_eq!(r.start.character, 5);
        assert_eq!(r.end.character, 15);
    }

    // ── DocumentUri ──

    #[test]
    fn document_uri_from_path() {
        let uri = DocumentUri::from_path("/src/main.mg");
        assert_eq!(uri.to_path(), Some("/src/main.mg"));
    }

    // ── Diagnostics ──

    #[test]
    fn diagnostic_error() {
        let diag = Diagnostic::error(
            Range::single_line(10, 0, 20),
            "expected type annotation",
        )
        .with_code("E0282");
        assert_eq!(diag.severity, DiagnosticSeverity::Error);
        assert_eq!(diag.code, Some("E0282".to_string()));
    }

    #[test]
    fn diagnostic_warning_with_related() {
        let diag = Diagnostic::warning(
            Range::single_line(5, 4, 12),
            "unused variable",
        )
        .with_related(DiagnosticRelated {
            uri: DocumentUri::from_path("/src/lib.mg"),
            range: Range::single_line(20, 0, 10),
            message: "defined here".to_string(),
        });
        assert_eq!(diag.related.len(), 1);
    }

    #[test]
    fn diagnostic_severity_display() {
        assert_eq!(format!("{}", DiagnosticSeverity::Error), "error");
        assert_eq!(format!("{}", DiagnosticSeverity::Warning), "warning");
        assert_eq!(format!("{}", DiagnosticSeverity::Information), "information");
        assert_eq!(format!("{}", DiagnosticSeverity::Hint), "hint");
    }

    // ── Code Actions ──

    #[test]
    fn code_action_kind_lsp_str() {
        assert_eq!(CodeActionKind::QuickFix.as_lsp_str(), "quickfix");
        assert_eq!(CodeActionKind::RefactorExtract.as_lsp_str(), "refactor.extract");
        assert_eq!(
            CodeActionKind::SourceOrganizeImports.as_lsp_str(),
            "source.organizeImports"
        );
    }

    #[test]
    fn text_edit_insert() {
        let edit = TextEdit::insert(Position::new(5, 0), "  let x = 42;\n");
        assert_eq!(edit.range.start, edit.range.end);
        assert!(edit.new_text.contains("let x"));
    }

    // ── Completions ──

    #[test]
    fn completion_item_builder() {
        let item = CompletionItem::new("Vec", CompletionItemKind::Struct)
            .with_detail("std::vec::Vec<T>")
            .with_documentation("A growable array type.")
            .with_insert_text("Vec::new()");

        assert_eq!(item.label, "Vec");
        assert_eq!(item.kind, CompletionItemKind::Struct);
        assert_eq!(item.detail.as_deref(), Some("std::vec::Vec<T>"));
        assert_eq!(item.insert_text.as_deref(), Some("Vec::new()"));
    }

    // ── Hover ──

    #[test]
    fn hover_markdown() {
        let hover = HoverInfo::markdown("**Type**: `i32`")
            .with_range(Range::single_line(5, 4, 8));
        assert_eq!(hover.contents.len(), 1);
        assert!(hover.range.is_some());
    }

    #[test]
    fn hover_code() {
        let hover = HoverInfo::code("redox", "f add(a: i32, b: i32) -> i32");
        match &hover.contents[0] {
            HoverContent::Code { language, value } => {
                assert_eq!(language, "redox");
                assert!(value.contains("add"));
            }
            _ => panic!("expected code content"),
        }
    }

    // ── Document Symbols ──

    #[test]
    fn document_symbol_with_children() {
        let child = DocumentSymbol::new(
            "field_x",
            SymbolKind::Field,
            Range::single_line(3, 4, 20),
        );
        let parent = DocumentSymbol::new(
            "MyStruct",
            SymbolKind::Struct,
            Range::new(Position::new(1, 0), Position::new(5, 1)),
        )
        .with_detail("pub struct")
        .with_child(child);

        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.detail.as_deref(), Some("pub struct"));
    }

    // ── Document Sync ──

    #[test]
    fn doc_sync_open_and_get() {
        let mut sync = DocumentSync::new();
        let uri = DocumentUri::from_path("/src/main.mg");
        sync.open(uri.clone(), "redox", 1, "f main() {}");

        assert_eq!(sync.count(), 1);
        let doc = sync.get(&uri).unwrap();
        assert_eq!(doc.version, 1);
        assert_eq!(doc.content, "f main() {}");
    }

    #[test]
    fn doc_sync_full_change() {
        let mut sync = DocumentSync::new();
        let uri = DocumentUri::from_path("/src/main.mg");
        sync.open(uri.clone(), "redox", 1, "old content");

        sync.change(
            &uri,
            2,
            &[ContentChange::Full {
                text: "new content".to_string(),
            }],
        )
        .unwrap();

        let doc = sync.get(&uri).unwrap();
        assert_eq!(doc.version, 2);
        assert_eq!(doc.content, "new content");
    }

    #[test]
    fn doc_sync_incremental_change() {
        let mut sync = DocumentSync::new();
        let uri = DocumentUri::from_path("/src/main.mg");
        sync.open(uri.clone(), "redox", 1, "hello world");

        // Replace "world" (chars 6-11) with "redox"
        sync.change(
            &uri,
            2,
            &[ContentChange::Incremental {
                range: Range::single_line(0, 6, 11),
                text: "redox".to_string(),
            }],
        )
        .unwrap();

        let doc = sync.get(&uri).unwrap();
        assert_eq!(doc.content, "hello redox");
    }

    #[test]
    fn doc_sync_close() {
        let mut sync = DocumentSync::new();
        let uri = DocumentUri::from_path("/src/main.mg");
        sync.open(uri.clone(), "redox", 1, "content");

        let closed = sync.close(&uri).unwrap();
        assert_eq!(closed.version, 1);
        assert_eq!(sync.count(), 0);
    }

    #[test]
    fn doc_sync_not_found_error() {
        let mut sync = DocumentSync::new();
        let uri = DocumentUri::from_path("/nonexistent.mg");
        let err = sync.close(&uri).unwrap_err();
        assert_eq!(err, IdeError::DocumentNotFound(uri));
    }

    // ── Diagnostic Bridge ──

    #[test]
    fn diagnostic_bridge_publish_and_get() {
        let mut bridge = DiagnosticBridge::new();
        let uri = DocumentUri::from_path("/src/main.mg");

        bridge.publish(
            &uri,
            vec![
                Diagnostic::error(Range::single_line(1, 0, 10), "syntax error"),
                Diagnostic::warning(Range::single_line(5, 0, 20), "unused var"),
            ],
        );

        assert_eq!(bridge.get(&uri).len(), 2);
        assert_eq!(bridge.total_count(), 2);
        assert_eq!(bridge.count_by_severity(DiagnosticSeverity::Error), 1);
        assert_eq!(bridge.count_by_severity(DiagnosticSeverity::Warning), 1);
    }

    #[test]
    fn diagnostic_bridge_clear() {
        let mut bridge = DiagnosticBridge::new();
        let uri = DocumentUri::from_path("/src/main.mg");
        bridge.publish(&uri, vec![Diagnostic::error(Range::single_line(1, 0, 5), "err")]);
        bridge.clear(&uri);
        assert_eq!(bridge.get(&uri).len(), 0);
    }

    // ── LSP Adapter ──

    #[test]
    fn lsp_adapter_standard_mappings() {
        let adapter = LspAdapter::new();
        assert_eq!(adapter.standard_mapping_count(), 12);
    }

    #[test]
    fn lsp_to_rap_mapping() {
        let adapter = LspAdapter::new();
        assert_eq!(adapter.lsp_to_rap("textDocument/completion"), Some("query.search"));
        assert_eq!(adapter.lsp_to_rap("textDocument/hover"), Some("query.types"));
        assert_eq!(adapter.lsp_to_rap("textDocument/definition"), Some("skb/query"));
        assert_eq!(adapter.lsp_to_rap("textDocument/codeAction"), Some("build/heal"));
        assert_eq!(adapter.lsp_to_rap("textDocument/formatting"), Some("format/expand"));
    }

    #[test]
    fn rap_to_lsp_mapping() {
        let adapter = LspAdapter::new();
        assert_eq!(
            adapter.rap_to_lsp("diagnostic.push"),
            Some("textDocument/publishDiagnostics")
        );
    }

    #[test]
    fn lsp_adapter_custom_method() {
        let mut adapter = LspAdapter::new();
        adapter.register_custom("textDocument/costQuery", "cost/query");
        assert_eq!(
            adapter.lsp_to_rap("textDocument/costQuery"),
            Some("cost/query")
        );
        assert_eq!(adapter.custom_mapping_count(), 1);
    }

    #[test]
    fn lsp_adapter_unknown_method() {
        let adapter = LspAdapter::new();
        assert_eq!(adapter.lsp_to_rap("textDocument/nonexistent"), None);
    }

    // ── IDE Capabilities ──

    #[test]
    fn ide_capabilities_minimal() {
        let caps = IdeCapabilities::minimal();
        assert!(caps.diagnostics);
        assert!(caps.hover);
        assert!(!caps.completion);
        assert_eq!(caps.enabled_count(), 2);
    }

    #[test]
    fn ide_capabilities_full() {
        let caps = IdeCapabilities::full();
        assert_eq!(caps.enabled_count(), 15);
    }

    // ── Extension Manifest ──

    #[test]
    fn extension_manifest_defaults() {
        let manifest = ExtensionManifest::default_redox();
        assert_eq!(manifest.language_id, "redox");
        assert!(manifest.file_extensions.contains(&".mg".to_string()));
        assert_eq!(manifest.server_command, "redox-rap-server");
    }

    #[test]
    fn extension_manifest_config() {
        let manifest = ExtensionManifest::default_redox();
        let config = manifest.to_config();
        assert_eq!(config.get("publisher").unwrap(), "nervosys");
        assert_eq!(config.get("languageId").unwrap(), "redox");
    }

    // ── IDE Session ──

    #[test]
    fn ide_session_lifecycle() {
        let mut session = IdeSession::new();
        assert_eq!(session.state(), IdeSessionState::Uninitialized);
        assert!(!session.is_ready());

        session.initialize().unwrap();
        assert_eq!(session.state(), IdeSessionState::Ready);
        assert!(session.is_ready());

        session.shutdown().unwrap();
        assert_eq!(session.state(), IdeSessionState::ShuttingDown);
    }

    #[test]
    fn ide_session_handle_request() {
        let mut session = IdeSession::new();
        session.initialize().unwrap();

        let rap = session.handle_request("textDocument/completion").unwrap();
        assert_eq!(rap, "query.search");
    }

    #[test]
    fn ide_session_not_ready_error() {
        let session = IdeSession::new();
        let err = session.handle_request("textDocument/hover").unwrap_err();
        assert_eq!(err, IdeError::NotReady);
    }

    #[test]
    fn ide_session_unsupported_method() {
        let mut session = IdeSession::new();
        session.initialize().unwrap();
        let err = session.handle_request("unknown/method").unwrap_err();
        match err {
            IdeError::UnsupportedMethod(m) => assert_eq!(m, "unknown/method"),
            _ => panic!("expected UnsupportedMethod"),
        }
    }

    #[test]
    fn ide_session_double_init_error() {
        let mut session = IdeSession::new();
        session.initialize().unwrap();
        let err = session.initialize().unwrap_err();
        match err {
            IdeError::InvalidState { .. } => {}
            _ => panic!("expected InvalidState"),
        }
    }

    #[test]
    fn ide_session_full_integration() {
        let mut session = IdeSession::new();
        session.initialize().unwrap();

        // Open a document
        let uri = DocumentUri::from_path("/src/main.mg");
        session.doc_sync.open(uri.clone(), "redox", 1, "f main() {\n  v x = 42\n}");
        assert_eq!(session.doc_sync.count(), 1);

        // Publish diagnostics
        session.diagnostics.publish(
            &uri,
            vec![Diagnostic::warning(
                Range::single_line(1, 4, 5),
                "unused variable 'x'",
            )],
        );
        assert_eq!(session.diagnostics.total_count(), 1);

        // Map an LSP request
        let rap_method = session.handle_request("textDocument/diagnostic").unwrap();
        assert_eq!(rap_method, "diagnostic.check");

        // Shutdown
        session.shutdown().unwrap();
    }

    // ── Error Display ──

    #[test]
    fn ide_error_display() {
        let err = IdeError::NotReady;
        assert!(format!("{}", err).contains("not ready"));

        let err = IdeError::UnsupportedMethod("test".to_string());
        assert!(format!("{}", err).contains("test"));

        let err = IdeError::DocumentNotFound(DocumentUri::from_path("/x.mg"));
        assert!(format!("{}", err).contains("/x.mg"));

        let err = IdeError::InvalidState {
            expected: IdeSessionState::Uninitialized,
            actual: IdeSessionState::Ready,
        };
        assert!(format!("{}", err).contains("uninitialized"));
    }

    // ── Incremental Edit ──

    #[test]
    fn incremental_edit_multiline() {
        let mut sync = DocumentSync::new();
        let uri = DocumentUri::from_path("/src/test.mg");
        sync.open(uri.clone(), "redox", 1, "line1\nline2\nline3");

        // Replace "line2" (line 1, chars 0-5) with "REPLACED"
        sync.change(
            &uri,
            2,
            &[ContentChange::Incremental {
                range: Range::new(Position::new(1, 0), Position::new(1, 5)),
                text: "REPLACED".to_string(),
            }],
        )
        .unwrap();

        let doc = sync.get(&uri).unwrap();
        assert_eq!(doc.content, "line1\nREPLACED\nline3");
    }

    // ── Method direction ──

    #[test]
    fn method_direction_display() {
        assert_eq!(format!("{}", MethodDirection::LspToRap), "LSP→RAP");
        assert_eq!(format!("{}", MethodDirection::RapToLsp), "RAP→LSP");
        assert_eq!(format!("{}", MethodDirection::Bidirectional), "LSP↔RAP");
    }

    // ── Code action confidence (f32 partial eq) ──

    #[test]
    fn code_action_construction() {
        let action = CodeAction {
            title: "Add return type".to_string(),
            kind: CodeActionKind::QuickFix,
            edits: vec![TextEdit::new(
                Range::single_line(1, 20, 20),
                " -> i32",
            )],
            diagnostics: vec![],
            is_preferred: true,
            confidence: 0.85,
        };
        assert!(action.is_preferred);
        assert_eq!(action.edits.len(), 1);
    }
}

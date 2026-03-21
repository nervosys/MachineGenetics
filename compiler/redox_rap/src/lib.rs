//! # Redox Agent Protocol (RAP) Server — Prototype
//!
//! A unified protocol server merging rust-analyzer-style IDE support with
//! direct compiler query access. Implements the architecture described in
//! REDOX_PROPOSAL.md §8.2.
//!
//! ## Endpoint namespaces
//!
//! - `query.*`      — SKB and semantic index queries
//! - `tokens.*`     — Token-level operations (tokenize, count, format)
//! - `ast.*`        — AST inspection and navigation
//! - `type.*`       — Type information queries
//! - `diagnostic.*` — Error/warning diagnostics

use std::collections::BTreeMap;
use std::fmt;

// ===========================================================================
// Protocol types
// ===========================================================================

/// A unique request identifier.
pub type RequestId = u64;

/// An incoming RAP request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub id: RequestId,
    pub method: String,
    pub params: Params,
}

/// Request parameters — a key-value map.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Params(pub BTreeMap<String, ParamValue>);

impl Params {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn set(mut self, key: &str, val: ParamValue) -> Self {
        self.0.insert(key.into(), val);
        self
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.0.get(key) {
            Some(ParamValue::Str(s)) => Some(s),
            _ => None,
        }
    }

    pub fn get_int(&self, key: &str) -> Option<i64> {
        match self.0.get(key) {
            Some(ParamValue::Int(n)) => Some(*n),
            _ => None,
        }
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.0.get(key) {
            Some(ParamValue::Bool(b)) => Some(*b),
            _ => None,
        }
    }
}

/// A parameter value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamValue {
    Str(String),
    Int(i64),
    Bool(bool),
}

/// A RAP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub id: RequestId,
    pub result: RapResult,
}

/// The result payload of a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RapResult {
    /// Successful result with structured data.
    Ok(ResponseData),
    /// Error with code and message.
    Err(ErrorCode, String),
}

/// Structured response data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseData {
    /// Plain text.
    Text(String),
    /// A list of items.
    List(Vec<String>),
    /// Key-value pairs.
    Map(BTreeMap<String, String>),
    /// Integer count.
    Count(usize),
    /// A list of diagnostics.
    Diagnostics(Vec<Diagnostic>),
    /// Type information.
    TypeInfo(TypeInfo),
    /// Token list.
    Tokens(Vec<Token>),
    /// AST node.
    AstNode(AstNode),
}

/// Error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    MethodNotFound,
    InvalidParams,
    InternalError,
    FileNotFound,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MethodNotFound => write!(f, "method_not_found"),
            Self::InvalidParams => write!(f, "invalid_params"),
            Self::InternalError => write!(f, "internal_error"),
            Self::FileNotFound => write!(f, "file_not_found"),
        }
    }
}

// ===========================================================================
// Domain types
// ===========================================================================

/// A diagnostic (error/warning).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Info => write!(f, "info"),
            Self::Hint => write!(f, "hint"),
        }
    }
}

/// Type information returned by `type.*` endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeInfo {
    pub name: String,
    pub kind: String,
    pub generics: Vec<String>,
    pub capabilities: Vec<String>,
    pub display: String,
}

/// A single token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: String,
    pub text: String,
    pub offset: usize,
    pub len: usize,
}

/// A simplified AST node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstNode {
    pub kind: String,
    pub text: String,
    pub children: Vec<AstNode>,
    pub span: (usize, usize),
}

// ===========================================================================
// RAP Server
// ===========================================================================

/// The RAP server — routes requests to endpoint handlers.
pub struct RapServer {
    /// Registered endpoint handlers.
    handlers: BTreeMap<String, Box<dyn Fn(&Params) -> RapResult + Send + Sync>>,
}

impl RapServer {
    /// Create a new RAP server with default prototype handlers.
    pub fn new() -> Self {
        let mut server = Self {
            handlers: BTreeMap::new(),
        };
        server.register_defaults();
        server
    }

    /// Register a custom handler for a method.
    pub fn register<F>(&mut self, method: &str, handler: F)
    where
        F: Fn(&Params) -> RapResult + Send + Sync + 'static,
    {
        self.handlers.insert(method.into(), Box::new(handler));
    }

    /// Dispatch a request and return a response.
    pub fn dispatch(&self, request: &Request) -> Response {
        let result = match self.handlers.get(&request.method) {
            Some(handler) => handler(&request.params),
            None => RapResult::Err(
                ErrorCode::MethodNotFound,
                format!("unknown method: {}", request.method),
            ),
        };
        Response {
            id: request.id,
            result,
        }
    }

    /// List all registered method names.
    pub fn methods(&self) -> Vec<&str> {
        self.handlers.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a method is registered.
    pub fn has_method(&self, method: &str) -> bool {
        self.handlers.contains_key(method)
    }

    // -- Default prototype handlers ----------------------------------------

    fn register_defaults(&mut self) {
        // query.* endpoints
        self.register("query.rules", handle_query_rules);
        self.register("query.capabilities", handle_query_capabilities);
        self.register("query.search", handle_query_search);

        // tokens.* endpoints
        self.register("tokens.tokenize", handle_tokens_tokenize);
        self.register("tokens.count", handle_tokens_count);

        // ast.* endpoints
        self.register("ast.parse", handle_ast_parse);
        self.register("ast.children", handle_ast_children);

        // type.* endpoints
        self.register("type.at_point", handle_type_at_point);
        self.register("type.resolve", handle_type_resolve);

        // diagnostic.* endpoints
        self.register("diagnostic.check", handle_diagnostic_check);
        self.register("diagnostic.list", handle_diagnostic_list);
    }
}

impl Default for RapServer {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Prototype handler implementations
// ===========================================================================

fn handle_query_rules(params: &Params) -> RapResult {
    let pattern = params.get_str("pattern").unwrap_or("*");
    // Prototype: return mock rules based on pattern.
    let rules = match pattern {
        "*" => vec![
            "OWN001: Move semantics require ownership transfer".into(),
            "BRW001: Mutable borrows are exclusive".into(),
            "LIF001: References must not outlive referent".into(),
        ],
        "ownership" => vec!["OWN001: Move semantics require ownership transfer".into()],
        "borrow" => vec!["BRW001: Mutable borrows are exclusive".into()],
        "lifetime" => vec!["LIF001: References must not outlive referent".into()],
        _ => vec![],
    };
    RapResult::Ok(ResponseData::List(rules))
}

fn handle_query_capabilities(params: &Params) -> RapResult {
    let crate_name = params.get_str("crate").unwrap_or("unknown");
    let mut caps = BTreeMap::new();
    // Prototype: return mock capabilities.
    caps.insert("crate".into(), crate_name.to_string());
    caps.insert("alloc".into(), "true".into());
    caps.insert("io".into(), "false".into());
    caps.insert("unsafe".into(), "false".into());
    RapResult::Ok(ResponseData::Map(caps))
}

fn handle_query_search(params: &Params) -> RapResult {
    let query = params.get_str("query").unwrap_or("");
    if query.is_empty() {
        return RapResult::Err(ErrorCode::InvalidParams, "query parameter required".into());
    }
    // Prototype: return mock search results.
    RapResult::Ok(ResponseData::List(vec![
        format!("result 1 for '{query}'"),
        format!("result 2 for '{query}'"),
    ]))
}

fn handle_tokens_tokenize(params: &Params) -> RapResult {
    let source = match params.get_str("source") {
        Some(s) => s,
        None => return RapResult::Err(ErrorCode::InvalidParams, "source parameter required".into()),
    };
    // Prototype: simple whitespace-based tokenization.
    let mut offset = 0;
    let tokens: Vec<Token> = source
        .split_whitespace()
        .map(|word| {
            let start = source[offset..].find(word).unwrap_or(0) + offset;
            let tok = Token {
                kind: classify_token(word).into(),
                text: word.into(),
                offset: start,
                len: word.len(),
            };
            offset = start + word.len();
            tok
        })
        .collect();
    RapResult::Ok(ResponseData::Tokens(tokens))
}

fn classify_token(word: &str) -> &str {
    match word {
        "fn" | "let" | "mut" | "if" | "else" | "match" | "struct" | "enum" | "impl" | "pub"
        | "use" | "mod" | "return" | "for" | "while" | "loop" | "break" | "continue"
        | "where" | "trait" | "type" | "const" | "static" | "async" | "await" | "move"
        | "ref" | "self" | "super" | "crate" | "unsafe" | "extern" => "keyword",
        s if s.starts_with('"') => "string",
        s if s.starts_with("//") => "comment",
        s if s.chars().next().is_some_and(|c| c.is_ascii_digit()) => "number",
        _ => "ident",
    }
}

fn handle_tokens_count(params: &Params) -> RapResult {
    let source = match params.get_str("source") {
        Some(s) => s,
        None => return RapResult::Err(ErrorCode::InvalidParams, "source parameter required".into()),
    };
    let count = source.split_whitespace().count();
    RapResult::Ok(ResponseData::Count(count))
}

fn handle_ast_parse(params: &Params) -> RapResult {
    let source = match params.get_str("source") {
        Some(s) => s,
        None => return RapResult::Err(ErrorCode::InvalidParams, "source parameter required".into()),
    };
    // Prototype: produce a simplified AST.
    let node = AstNode {
        kind: "SourceFile".into(),
        text: source.to_string(),
        children: source
            .lines()
            .enumerate()
            .map(|(i, line)| AstNode {
                kind: detect_ast_kind(line).into(),
                text: line.trim().to_string(),
                children: vec![],
                span: (i, i),
            })
            .collect(),
        span: (0, source.lines().count().saturating_sub(1)),
    };
    RapResult::Ok(ResponseData::AstNode(node))
}

fn detect_ast_kind(line: &str) -> &str {
    let trimmed = line.trim();
    if trimmed.starts_with("fn ") {
        "FnDef"
    } else if trimmed.starts_with("struct ") {
        "StructDef"
    } else if trimmed.starts_with("enum ") {
        "EnumDef"
    } else if trimmed.starts_with("let ") {
        "LetStmt"
    } else if trimmed.starts_with("//") {
        "Comment"
    } else if trimmed.starts_with("use ") {
        "UseStmt"
    } else if trimmed.is_empty() {
        "Whitespace"
    } else {
        "ExprStmt"
    }
}

fn handle_ast_children(params: &Params) -> RapResult {
    let kind = params.get_str("kind").unwrap_or("SourceFile");
    // Prototype: return common child node kinds.
    let children = match kind {
        "FnDef" => vec!["Name", "ParamList", "RetType", "BlockExpr"],
        "StructDef" => vec!["Name", "FieldList"],
        "EnumDef" => vec!["Name", "VariantList"],
        _ => vec!["Unknown"],
    };
    RapResult::Ok(ResponseData::List(children.into_iter().map(|s| s.into()).collect()))
}

fn handle_type_at_point(params: &Params) -> RapResult {
    let _file = match params.get_str("file") {
        Some(f) => f,
        None => return RapResult::Err(ErrorCode::InvalidParams, "file parameter required".into()),
    };
    let _line = params.get_int("line").unwrap_or(1);
    let _column = params.get_int("column").unwrap_or(1);
    // Prototype: return a mock type.
    let info = TypeInfo {
        name: "i32".into(),
        kind: "primitive".into(),
        generics: vec![],
        capabilities: vec![],
        display: "i32".into(),
    };
    RapResult::Ok(ResponseData::TypeInfo(info))
}

fn handle_type_resolve(params: &Params) -> RapResult {
    let name = match params.get_str("name") {
        Some(n) => n,
        None => return RapResult::Err(ErrorCode::InvalidParams, "name parameter required".into()),
    };
    // Prototype: return mock type resolution.
    let info = TypeInfo {
        name: name.to_string(),
        kind: if name.starts_with(char::is_uppercase) { "struct" } else { "primitive" }.into(),
        generics: vec![],
        capabilities: vec![],
        display: name.to_string(),
    };
    RapResult::Ok(ResponseData::TypeInfo(info))
}

fn handle_diagnostic_check(params: &Params) -> RapResult {
    let source = match params.get_str("source") {
        Some(s) => s,
        None => return RapResult::Err(ErrorCode::InvalidParams, "source parameter required".into()),
    };
    // Prototype: basic diagnostics.
    let mut diags = Vec::new();
    for (i, line) in source.lines().enumerate() {
        if line.contains("unsafe") {
            diags.push(Diagnostic {
                severity: Severity::Warning,
                message: "unsafe code detected".into(),
                file: "<input>".into(),
                line: i as u32 + 1,
                column: 1,
                code: Some("RAP-W001".into()),
            });
        }
        if line.contains("unwrap()") {
            diags.push(Diagnostic {
                severity: Severity::Warning,
                message: "unwrap() may panic".into(),
                file: "<input>".into(),
                line: i as u32 + 1,
                column: 1,
                code: Some("RAP-W002".into()),
            });
        }
    }
    if diags.is_empty() {
        RapResult::Ok(ResponseData::Count(0))
    } else {
        RapResult::Ok(ResponseData::Diagnostics(diags))
    }
}

fn handle_diagnostic_list(_params: &Params) -> RapResult {
    // Prototype: list available diagnostic categories.
    RapResult::Ok(ResponseData::List(vec![
        "ownership".into(),
        "borrowing".into(),
        "lifetimes".into(),
        "type-safety".into(),
        "effects".into(),
        "capabilities".into(),
    ]))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> RapServer {
        RapServer::new()
    }

    fn req(id: u64, method: &str, params: Params) -> Request {
        Request { id, method: method.into(), params }
    }

    // -- Server basics -----------------------------------------------------

    #[test]
    fn server_has_default_methods() {
        let s = server();
        assert!(s.has_method("query.rules"));
        assert!(s.has_method("query.capabilities"));
        assert!(s.has_method("query.search"));
        assert!(s.has_method("tokens.tokenize"));
        assert!(s.has_method("tokens.count"));
        assert!(s.has_method("ast.parse"));
        assert!(s.has_method("ast.children"));
        assert!(s.has_method("type.at_point"));
        assert!(s.has_method("type.resolve"));
        assert!(s.has_method("diagnostic.check"));
        assert!(s.has_method("diagnostic.list"));
    }

    #[test]
    fn server_methods_list() {
        let s = server();
        let methods = s.methods();
        assert_eq!(methods.len(), 11);
        assert!(methods.contains(&"query.rules"));
    }

    #[test]
    fn unknown_method_returns_error() {
        let s = server();
        let resp = s.dispatch(&req(1, "unknown.method", Params::new()));
        assert!(matches!(resp.result, RapResult::Err(ErrorCode::MethodNotFound, _)));
        assert_eq!(resp.id, 1);
    }

    #[test]
    fn custom_handler_registration() {
        let mut s = server();
        s.register("custom.echo", |params| {
            let msg = params.get_str("msg").unwrap_or("no message");
            RapResult::Ok(ResponseData::Text(msg.into()))
        });
        assert!(s.has_method("custom.echo"));
        let resp = s.dispatch(&req(42, "custom.echo", Params::new().set("msg", ParamValue::Str("hello".into()))));
        assert!(matches!(resp.result, RapResult::Ok(ResponseData::Text(ref t)) if t == "hello"));
    }

    // -- query.* -----------------------------------------------------------

    #[test]
    fn query_rules_all() {
        let s = server();
        let resp = s.dispatch(&req(1, "query.rules", Params::new()));
        if let RapResult::Ok(ResponseData::List(rules)) = &resp.result {
            assert_eq!(rules.len(), 3);
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn query_rules_by_pattern() {
        let s = server();
        let resp = s.dispatch(&req(1, "query.rules", Params::new().set("pattern", ParamValue::Str("ownership".into()))));
        if let RapResult::Ok(ResponseData::List(rules)) = &resp.result {
            assert_eq!(rules.len(), 1);
            assert!(rules[0].contains("OWN001"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn query_capabilities() {
        let s = server();
        let params = Params::new().set("crate", ParamValue::Str("my_crate".into()));
        let resp = s.dispatch(&req(2, "query.capabilities", params));
        if let RapResult::Ok(ResponseData::Map(map)) = &resp.result {
            assert_eq!(map.get("crate").unwrap(), "my_crate");
        } else {
            panic!("expected map");
        }
    }

    #[test]
    fn query_search_missing_query() {
        let s = server();
        let resp = s.dispatch(&req(3, "query.search", Params::new()));
        assert!(matches!(resp.result, RapResult::Err(ErrorCode::InvalidParams, _)));
    }

    #[test]
    fn query_search_with_query() {
        let s = server();
        let params = Params::new().set("query", ParamValue::Str("borrow".into()));
        let resp = s.dispatch(&req(3, "query.search", params));
        if let RapResult::Ok(ResponseData::List(results)) = &resp.result {
            assert_eq!(results.len(), 2);
            assert!(results[0].contains("borrow"));
        } else {
            panic!("expected list");
        }
    }

    // -- tokens.* ----------------------------------------------------------

    #[test]
    fn tokens_tokenize() {
        let s = server();
        let params = Params::new().set("source", ParamValue::Str("fn main() {}".into()));
        let resp = s.dispatch(&req(4, "tokens.tokenize", params));
        if let RapResult::Ok(ResponseData::Tokens(tokens)) = &resp.result {
            assert!(!tokens.is_empty());
            assert_eq!(tokens[0].kind, "keyword");
            assert_eq!(tokens[0].text, "fn");
        } else {
            panic!("expected tokens");
        }
    }

    #[test]
    fn tokens_tokenize_missing_source() {
        let s = server();
        let resp = s.dispatch(&req(4, "tokens.tokenize", Params::new()));
        assert!(matches!(resp.result, RapResult::Err(ErrorCode::InvalidParams, _)));
    }

    #[test]
    fn tokens_count() {
        let s = server();
        let params = Params::new().set("source", ParamValue::Str("let x = 42;".into()));
        let resp = s.dispatch(&req(5, "tokens.count", params));
        assert!(matches!(resp.result, RapResult::Ok(ResponseData::Count(4))));
    }

    #[test]
    fn token_classification() {
        assert_eq!(classify_token("fn"), "keyword");
        assert_eq!(classify_token("let"), "keyword");
        assert_eq!(classify_token("42"), "number");
        assert_eq!(classify_token("my_var"), "ident");
        assert_eq!(classify_token("\"hello\""), "string");
        assert_eq!(classify_token("//comment"), "comment");
    }

    // -- ast.* -------------------------------------------------------------

    #[test]
    fn ast_parse() {
        let s = server();
        let params = Params::new().set("source", ParamValue::Str("fn foo() {}\nlet x = 1;".into()));
        let resp = s.dispatch(&req(6, "ast.parse", params));
        if let RapResult::Ok(ResponseData::AstNode(node)) = &resp.result {
            assert_eq!(node.kind, "SourceFile");
            assert_eq!(node.children.len(), 2);
            assert_eq!(node.children[0].kind, "FnDef");
            assert_eq!(node.children[1].kind, "LetStmt");
        } else {
            panic!("expected ast node");
        }
    }

    #[test]
    fn ast_children() {
        let s = server();
        let params = Params::new().set("kind", ParamValue::Str("FnDef".into()));
        let resp = s.dispatch(&req(7, "ast.children", params));
        if let RapResult::Ok(ResponseData::List(children)) = &resp.result {
            assert!(children.contains(&"Name".to_string()));
            assert!(children.contains(&"BlockExpr".to_string()));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn ast_kind_detection() {
        assert_eq!(detect_ast_kind("fn foo() {}"), "FnDef");
        assert_eq!(detect_ast_kind("struct Foo {}"), "StructDef");
        assert_eq!(detect_ast_kind("enum Bar {}"), "EnumDef");
        assert_eq!(detect_ast_kind("let x = 1;"), "LetStmt");
        assert_eq!(detect_ast_kind("// comment"), "Comment");
        assert_eq!(detect_ast_kind("use std::io;"), "UseStmt");
        assert_eq!(detect_ast_kind(""), "Whitespace");
        assert_eq!(detect_ast_kind("x + 1"), "ExprStmt");
    }

    // -- type.* ------------------------------------------------------------

    #[test]
    fn type_at_point() {
        let s = server();
        let params = Params::new()
            .set("file", ParamValue::Str("main.rs".into()))
            .set("line", ParamValue::Int(1))
            .set("column", ParamValue::Int(5));
        let resp = s.dispatch(&req(8, "type.at_point", params));
        if let RapResult::Ok(ResponseData::TypeInfo(info)) = &resp.result {
            assert_eq!(info.name, "i32");
        } else {
            panic!("expected type info");
        }
    }

    #[test]
    fn type_at_point_missing_file() {
        let s = server();
        let resp = s.dispatch(&req(8, "type.at_point", Params::new()));
        assert!(matches!(resp.result, RapResult::Err(ErrorCode::InvalidParams, _)));
    }

    #[test]
    fn type_resolve_struct() {
        let s = server();
        let params = Params::new().set("name", ParamValue::Str("Vec".into()));
        let resp = s.dispatch(&req(9, "type.resolve", params));
        if let RapResult::Ok(ResponseData::TypeInfo(info)) = &resp.result {
            assert_eq!(info.name, "Vec");
            assert_eq!(info.kind, "struct");
        } else {
            panic!("expected type info");
        }
    }

    #[test]
    fn type_resolve_primitive() {
        let s = server();
        let params = Params::new().set("name", ParamValue::Str("i32".into()));
        let resp = s.dispatch(&req(9, "type.resolve", params));
        if let RapResult::Ok(ResponseData::TypeInfo(info)) = &resp.result {
            assert_eq!(info.kind, "primitive");
        } else {
            panic!("expected type info");
        }
    }

    // -- diagnostic.* ------------------------------------------------------

    #[test]
    fn diagnostic_check_clean() {
        let s = server();
        let params = Params::new().set("source", ParamValue::Str("let x = 1;".into()));
        let resp = s.dispatch(&req(10, "diagnostic.check", params));
        assert!(matches!(resp.result, RapResult::Ok(ResponseData::Count(0))));
    }

    #[test]
    fn diagnostic_check_unsafe() {
        let s = server();
        let params = Params::new().set("source", ParamValue::Str("unsafe { ptr.read() }".into()));
        let resp = s.dispatch(&req(10, "diagnostic.check", params));
        if let RapResult::Ok(ResponseData::Diagnostics(diags)) = &resp.result {
            assert_eq!(diags.len(), 1);
            assert_eq!(diags[0].severity, Severity::Warning);
            assert!(diags[0].message.contains("unsafe"));
            assert_eq!(diags[0].code.as_deref(), Some("RAP-W001"));
        } else {
            panic!("expected diagnostics");
        }
    }

    #[test]
    fn diagnostic_check_unwrap() {
        let s = server();
        let params = Params::new().set("source", ParamValue::Str("x.unwrap()".into()));
        let resp = s.dispatch(&req(10, "diagnostic.check", params));
        if let RapResult::Ok(ResponseData::Diagnostics(diags)) = &resp.result {
            assert_eq!(diags.len(), 1);
            assert!(diags[0].code.as_deref() == Some("RAP-W002"));
        } else {
            panic!("expected diagnostics");
        }
    }

    #[test]
    fn diagnostic_check_multiple() {
        let s = server();
        let params = Params::new().set("source", ParamValue::Str("unsafe { x.unwrap() }".into()));
        let resp = s.dispatch(&req(10, "diagnostic.check", params));
        if let RapResult::Ok(ResponseData::Diagnostics(diags)) = &resp.result {
            assert_eq!(diags.len(), 2);
        } else {
            panic!("expected diagnostics");
        }
    }

    #[test]
    fn diagnostic_list_categories() {
        let s = server();
        let resp = s.dispatch(&req(11, "diagnostic.list", Params::new()));
        if let RapResult::Ok(ResponseData::List(cats)) = &resp.result {
            assert!(cats.contains(&"ownership".to_string()));
            assert!(cats.contains(&"capabilities".to_string()));
        } else {
            panic!("expected list");
        }
    }

    // -- Params ------------------------------------------------------------

    #[test]
    fn params_builder() {
        let p = Params::new()
            .set("name", ParamValue::Str("test".into()))
            .set("count", ParamValue::Int(5))
            .set("flag", ParamValue::Bool(true));
        assert_eq!(p.get_str("name"), Some("test"));
        assert_eq!(p.get_int("count"), Some(5));
        assert_eq!(p.get_bool("flag"), Some(true));
        assert_eq!(p.get_str("missing"), None);
    }

    // -- Response IDs ------------------------------------------------------

    #[test]
    fn response_preserves_request_id() {
        let s = server();
        let resp = s.dispatch(&req(999, "diagnostic.list", Params::new()));
        assert_eq!(resp.id, 999);
    }

    // -- Display -----------------------------------------------------------

    #[test]
    fn error_code_display() {
        assert_eq!(format!("{}", ErrorCode::MethodNotFound), "method_not_found");
        assert_eq!(format!("{}", ErrorCode::InvalidParams), "invalid_params");
        assert_eq!(format!("{}", ErrorCode::InternalError), "internal_error");
        assert_eq!(format!("{}", ErrorCode::FileNotFound), "file_not_found");
    }

    #[test]
    fn severity_display() {
        assert_eq!(format!("{}", Severity::Error), "error");
        assert_eq!(format!("{}", Severity::Warning), "warning");
        assert_eq!(format!("{}", Severity::Info), "info");
        assert_eq!(format!("{}", Severity::Hint), "hint");
    }
}

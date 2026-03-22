//! # Self-Evolving Grammar Extension System
//!
//! Provides `grammar_extension!` macro (simulated), registration API, and
//! extension discovery for the Redox compiler.
//!
//! Grammar extensions allow language syntax to be extended at compile-time
//! through a declarative macro-like system.

use std::collections::HashMap;
use std::fmt;

// ── Token Representation ─────────────────────────────────────────────

/// A token in the grammar extension syntax.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    Ident(String),
    Keyword(String),
    Punct(char),
    Literal(String),
    Group(Delimiter, Vec<Token>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Delimiter {
    Paren,
    Bracket,
    Brace,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ident(s) => write!(f, "{s}"),
            Self::Keyword(s) => write!(f, "{s}"),
            Self::Punct(c) => write!(f, "{c}"),
            Self::Literal(s) => write!(f, "{s}"),
            Self::Group(d, tokens) => {
                let (open, close) = match d {
                    Delimiter::Paren => ('(', ')'),
                    Delimiter::Bracket => ('[', ']'),
                    Delimiter::Brace => ('{', '}'),
                };
                write!(f, "{open}")?;
                for (i, tok) in tokens.iter().enumerate() {
                    if i > 0 { write!(f, " ")?; }
                    write!(f, "{tok}")?;
                }
                write!(f, "{close}")
            }
        }
    }
}

// ── Pattern Language ─────────────────────────────────────────────────

/// A pattern element in a grammar production rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternElem {
    /// Matches a specific keyword.
    Keyword(String),
    /// Matches a specific punctuation.
    Punct(char),
    /// Captures an identifier into a named binding.
    CaptureIdent(String),
    /// Captures an expression into a named binding.
    CaptureExpr(String),
    /// Captures a delimited group into a named binding.
    CaptureBlock(String),
    /// Optional element.
    Optional(Box<PatternElem>),
    /// Repetition (zero or more).
    Repeat(Box<PatternElem>),
    /// Sequence of pattern elements.
    Sequence(Vec<PatternElem>),
}

impl fmt::Display for PatternElem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keyword(k) => write!(f, "{k}"),
            Self::Punct(c) => write!(f, "'{c}'"),
            Self::CaptureIdent(n) => write!(f, "$ident:{n}"),
            Self::CaptureExpr(n) => write!(f, "$expr:{n}"),
            Self::CaptureBlock(n) => write!(f, "$block:{n}"),
            Self::Optional(inner) => write!(f, "[{inner}]"),
            Self::Repeat(inner) => write!(f, "({inner})*"),
            Self::Sequence(elems) => {
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 { write!(f, " ")?; }
                    write!(f, "{e}")?;
                }
                Ok(())
            }
        }
    }
}

// ── Expansion Template ───────────────────────────────────────────────

/// An element in an expansion template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateElem {
    /// Literal text.
    Literal(String),
    /// Substitution from a capture binding.
    Subst(String),
    /// Conditional expansion.
    IfBound(String, Vec<TemplateElem>),
    /// Repeat expansion for each captured repetition.
    RepeatSubst(String, Vec<TemplateElem>),
}

impl fmt::Display for TemplateElem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(s) => write!(f, "{s}"),
            Self::Subst(n) => write!(f, "${{{n}}}"),
            Self::IfBound(n, elems) => {
                write!(f, "#if({n}) {{")?;
                for e in elems { write!(f, " {e}")?; }
                write!(f, " }}")
            }
            Self::RepeatSubst(n, elems) => {
                write!(f, "#repeat({n}) {{")?;
                for e in elems { write!(f, " {e}")?; }
                write!(f, " }}")
            }
        }
    }
}

// ── Grammar Rule ─────────────────────────────────────────────────────

/// A complete grammar extension rule.
#[derive(Debug, Clone)]
pub struct GrammarRule {
    pub name: String,
    pub pattern: PatternElem,
    pub template: Vec<TemplateElem>,
    pub priority: i32,
}

impl fmt::Display for GrammarRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rule {} (priority {}):\n  pattern: {}\n  template:", self.name, self.priority, self.pattern)?;
        for t in &self.template {
            write!(f, " {t}")?;
        }
        Ok(())
    }
}

// ── Grammar Extension ────────────────────────────────────────────────

/// Metadata about a grammar extension.
#[derive(Debug, Clone)]
pub struct ExtensionMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
}

/// A grammar extension package containing rules.
#[derive(Debug, Clone)]
pub struct GrammarExtension {
    pub meta: ExtensionMeta,
    pub rules: Vec<GrammarRule>,
}

impl GrammarExtension {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            meta: ExtensionMeta {
                name: name.into(),
                version: version.into(),
                description: String::new(),
                author: String::new(),
            },
            rules: Vec::new(),
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.meta.description = desc.into();
        self
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.meta.author = author.into();
        self
    }

    pub fn rule(mut self, rule: GrammarRule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl fmt::Display for GrammarExtension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "extension {} v{}", self.meta.name, self.meta.version)?;
        if !self.meta.description.is_empty() {
            writeln!(f, "  {}", self.meta.description)?;
        }
        writeln!(f, "  {} rule(s)", self.rules.len())
    }
}

// ── Capture Bindings ─────────────────────────────────────────────────

/// Captured bindings from pattern matching.
#[derive(Debug, Clone, Default)]
pub struct CaptureBindings {
    idents: HashMap<String, String>,
    exprs: HashMap<String, Vec<Token>>,
    blocks: HashMap<String, Vec<Token>>,
}

impl CaptureBindings {
    pub fn new() -> Self { Self::default() }

    pub fn bind_ident(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.idents.insert(name.into(), value.into());
    }

    pub fn bind_expr(&mut self, name: impl Into<String>, tokens: Vec<Token>) {
        self.exprs.insert(name.into(), tokens);
    }

    pub fn bind_block(&mut self, name: impl Into<String>, tokens: Vec<Token>) {
        self.blocks.insert(name.into(), tokens);
    }

    pub fn get_ident(&self, name: &str) -> Option<&str> {
        self.idents.get(name).map(|s| s.as_str())
    }

    pub fn get_expr(&self, name: &str) -> Option<&[Token]> {
        self.exprs.get(name).map(|v| v.as_slice())
    }

    pub fn get_block(&self, name: &str) -> Option<&[Token]> {
        self.blocks.get(name).map(|v| v.as_slice())
    }

    pub fn is_bound(&self, name: &str) -> bool {
        self.idents.contains_key(name)
            || self.exprs.contains_key(name)
            || self.blocks.contains_key(name)
    }
}

// ── Pattern Matching ─────────────────────────────────────────────────

/// Match a pattern against a token stream.
pub fn match_pattern(pattern: &PatternElem, tokens: &[Token], bindings: &mut CaptureBindings) -> Option<usize> {
    match pattern {
        PatternElem::Keyword(kw) => {
            if let Some(Token::Keyword(k) | Token::Ident(k)) = tokens.first() {
                if k == kw { return Some(1); }
            }
            None
        }
        PatternElem::Punct(p) => {
            if let Some(Token::Punct(c)) = tokens.first() {
                if c == p { return Some(1); }
            }
            None
        }
        PatternElem::CaptureIdent(name) => {
            if let Some(Token::Ident(ident)) = tokens.first() {
                bindings.bind_ident(name, ident.as_str());
                return Some(1);
            }
            None
        }
        PatternElem::CaptureExpr(name) => {
            // Greedy: consume one token as an expression
            if tokens.is_empty() { return None; }
            bindings.bind_expr(name, vec![tokens[0].clone()]);
            Some(1)
        }
        PatternElem::CaptureBlock(name) => {
            if let Some(Token::Group(Delimiter::Brace, inner)) = tokens.first() {
                bindings.bind_block(name, inner.clone());
                return Some(1);
            }
            None
        }
        PatternElem::Optional(inner) => {
            match match_pattern(inner, tokens, bindings) {
                Some(n) => Some(n),
                None => Some(0),
            }
        }
        PatternElem::Repeat(inner) => {
            let mut consumed = 0;
            loop {
                match match_pattern(inner, &tokens[consumed..], bindings) {
                    Some(0) => break,
                    Some(n) => consumed += n,
                    None => break,
                }
            }
            Some(consumed)
        }
        PatternElem::Sequence(elems) => {
            let mut consumed = 0;
            for elem in elems {
                match match_pattern(elem, &tokens[consumed..], bindings) {
                    Some(n) => consumed += n,
                    None => return None,
                }
            }
            Some(consumed)
        }
    }
}

// ── Template Expansion ───────────────────────────────────────────────

/// Expand a template with captured bindings.
pub fn expand_template(template: &[TemplateElem], bindings: &CaptureBindings) -> String {
    let mut out = String::new();
    for elem in template {
        match elem {
            TemplateElem::Literal(s) => out.push_str(s),
            TemplateElem::Subst(name) => {
                if let Some(ident) = bindings.get_ident(name) {
                    out.push_str(ident);
                } else if let Some(tokens) = bindings.get_expr(name) {
                    let s: Vec<String> = tokens.iter().map(|t| format!("{t}")).collect();
                    out.push_str(&s.join(" "));
                } else if let Some(tokens) = bindings.get_block(name) {
                    let s: Vec<String> = tokens.iter().map(|t| format!("{t}")).collect();
                    out.push_str(&s.join(" "));
                }
            }
            TemplateElem::IfBound(name, sub) => {
                if bindings.is_bound(name) {
                    out.push_str(&expand_template(sub, bindings));
                }
            }
            TemplateElem::RepeatSubst(name, sub) => {
                if bindings.is_bound(name) {
                    out.push_str(&expand_template(sub, bindings));
                }
            }
        }
    }
    out
}

// ── Extension Registry ───────────────────────────────────────────────

/// Error from extension registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateExtension(String),
    DuplicateRule(String),
    InvalidExtension(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateExtension(n) => write!(f, "duplicate extension: {n}"),
            Self::DuplicateRule(n) => write!(f, "duplicate rule: {n}"),
            Self::InvalidExtension(msg) => write!(f, "invalid extension: {msg}"),
        }
    }
}

/// Registry of grammar extensions.
#[derive(Debug, Default)]
pub struct ExtensionRegistry {
    extensions: HashMap<String, GrammarExtension>,
    rule_index: HashMap<String, (String, usize)>, // rule_name -> (ext_name, rule_idx)
}

impl ExtensionRegistry {
    pub fn new() -> Self { Self::default() }

    /// Register a grammar extension.
    pub fn register(&mut self, ext: GrammarExtension) -> Result<(), RegistryError> {
        if ext.meta.name.is_empty() {
            return Err(RegistryError::InvalidExtension("empty name".into()));
        }
        if self.extensions.contains_key(&ext.meta.name) {
            return Err(RegistryError::DuplicateExtension(ext.meta.name.clone()));
        }
        // Check rule name conflicts
        for (i, rule) in ext.rules.iter().enumerate() {
            if self.rule_index.contains_key(&rule.name) {
                return Err(RegistryError::DuplicateRule(rule.name.clone()));
            }
            self.rule_index.insert(rule.name.clone(), (ext.meta.name.clone(), i));
        }
        self.extensions.insert(ext.meta.name.clone(), ext);
        Ok(())
    }

    /// Unregister an extension by name.
    pub fn unregister(&mut self, name: &str) -> bool {
        if let Some(ext) = self.extensions.remove(name) {
            for rule in &ext.rules {
                self.rule_index.remove(&rule.name);
            }
            true
        } else {
            false
        }
    }

    pub fn get_extension(&self, name: &str) -> Option<&GrammarExtension> {
        self.extensions.get(name)
    }

    pub fn get_rule(&self, name: &str) -> Option<&GrammarRule> {
        self.rule_index.get(name).and_then(|(ext_name, idx)| {
            self.extensions.get(ext_name).map(|ext| &ext.rules[*idx])
        })
    }

    pub fn extension_count(&self) -> usize {
        self.extensions.len()
    }

    pub fn rule_count(&self) -> usize {
        self.rule_index.len()
    }

    /// Discover extensions matching a pattern.
    pub fn discover(&self, name_prefix: &str) -> Vec<&GrammarExtension> {
        self.extensions.iter()
            .filter(|(name, _)| name.starts_with(name_prefix))
            .map(|(_, ext)| ext)
            .collect()
    }

    /// Get all rules sorted by priority (highest first).
    pub fn rules_by_priority(&self) -> Vec<&GrammarRule> {
        let mut rules: Vec<&GrammarRule> = self.extensions.values()
            .flat_map(|ext| ext.rules.iter())
            .collect();
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        rules
    }
}

// ── Grammar Extension Macro (simulated) ──────────────────────────────

/// Parse a simple grammar extension definition string into a `GrammarExtension`.
///
/// Format:
/// ```text
/// grammar_extension! {
///     name: "my_ext",
///     version: "0.1.0",
///     rule my_rule: keyword $ident:name => "fn ${name}"
/// }
/// ```
pub fn parse_grammar_extension(input: &str) -> Result<GrammarExtension, String> {
    let input = input.trim();
    // Extract fields
    let mut name = String::new();
    let mut version = String::from("0.1.0");
    let mut rules = Vec::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") { continue; }

        if let Some(rest) = line.strip_prefix("name:") {
            name = rest.trim().trim_matches(',').trim_matches('"').to_string();
        } else if let Some(rest) = line.strip_prefix("version:") {
            version = rest.trim().trim_matches(',').trim_matches('"').to_string();
        } else if let Some(rest) = line.strip_prefix("rule ") {
            if let Some(rule) = parse_rule_def(rest) {
                rules.push(rule);
            } else {
                return Err(format!("invalid rule: {rest}"));
            }
        }
    }

    if name.is_empty() {
        return Err("missing name".into());
    }

    let mut ext = GrammarExtension::new(name, version);
    for r in rules {
        ext = ext.rule(r);
    }
    Ok(ext)
}

fn parse_rule_def(input: &str) -> Option<GrammarRule> {
    // Format: rule_name: pattern => template
    let (name_part, rest) = input.split_once(':')?;
    let name = name_part.trim().to_string();
    let (pattern_str, template_str) = rest.split_once("=>")?;

    let pattern = parse_pattern_str(pattern_str.trim());
    let template = parse_template_str(template_str.trim().trim_matches('"'));

    Some(GrammarRule {
        name,
        pattern,
        template,
        priority: 0,
    })
}

fn parse_pattern_str(input: &str) -> PatternElem {
    let mut elems = Vec::new();
    for word in input.split_whitespace() {
        if let Some(rest) = word.strip_prefix("$ident:") {
            elems.push(PatternElem::CaptureIdent(rest.to_string()));
        } else if let Some(rest) = word.strip_prefix("$expr:") {
            elems.push(PatternElem::CaptureExpr(rest.to_string()));
        } else if let Some(rest) = word.strip_prefix("$block:") {
            elems.push(PatternElem::CaptureBlock(rest.to_string()));
        } else if word.len() == 1 && word.chars().next().is_some_and(|c| c.is_ascii_punctuation()) {
            elems.push(PatternElem::Punct(word.chars().next().unwrap()));
        } else {
            elems.push(PatternElem::Keyword(word.to_string()));
        }
    }
    if elems.len() == 1 { elems.into_iter().next().unwrap() }
    else { PatternElem::Sequence(elems) }
}

fn parse_template_str(input: &str) -> Vec<TemplateElem> {
    let mut elems = Vec::new();
    let mut chars = input.chars().peekable();
    let mut buf = String::new();

    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            if !buf.is_empty() {
                elems.push(TemplateElem::Literal(std::mem::take(&mut buf)));
            }
            chars.next(); // consume '{'
            let mut name = String::new();
            for ch in chars.by_ref() {
                if ch == '}' { break; }
                name.push(ch);
            }
            elems.push(TemplateElem::Subst(name));
        } else {
            buf.push(c);
        }
    }
    if !buf.is_empty() {
        elems.push(TemplateElem::Literal(buf));
    }
    elems
}

// ── Apply Extension ──────────────────────────────────────────────────

/// Result of applying extensions to a token stream.
#[derive(Debug)]
pub struct ApplyResult {
    pub rule_name: String,
    pub expansion: String,
    pub bindings: CaptureBindings,
}

/// Try to apply all rules in priority order to a token stream.
pub fn apply_extensions(
    registry: &ExtensionRegistry,
    tokens: &[Token],
) -> Option<ApplyResult> {
    for rule in registry.rules_by_priority() {
        let mut bindings = CaptureBindings::new();
        if match_pattern(&rule.pattern, tokens, &mut bindings).is_some() {
            let expansion = expand_template(&rule.template, &bindings);
            return Some(ApplyResult {
                rule_name: rule.name.clone(),
                expansion,
                bindings,
            });
        }
    }
    None
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_display() {
        assert_eq!(format!("{}", Token::Ident("foo".into())), "foo");
        assert_eq!(format!("{}", Token::Punct('+')), "+");
        let grp = Token::Group(Delimiter::Paren, vec![Token::Ident("x".into())]);
        assert_eq!(format!("{grp}"), "(x)");
    }

    #[test]
    fn test_pattern_display() {
        let p = PatternElem::Keyword("async".into());
        assert_eq!(format!("{p}"), "async");
        let p = PatternElem::CaptureIdent("name".into());
        assert_eq!(format!("{p}"), "$ident:name");
    }

    #[test]
    fn test_template_display() {
        let t = TemplateElem::Subst("name".into());
        assert_eq!(format!("{t}"), "${name}");
        let t = TemplateElem::Literal("hello".into());
        assert_eq!(format!("{t}"), "hello");
    }

    #[test]
    fn test_grammar_rule_display() {
        let r = GrammarRule {
            name: "test".into(),
            pattern: PatternElem::Keyword("async".into()),
            template: vec![TemplateElem::Literal("desugared".into())],
            priority: 5,
        };
        let s = format!("{r}");
        assert!(s.contains("test"));
        assert!(s.contains("priority 5"));
    }

    #[test]
    fn test_extension_builder() {
        let ext = GrammarExtension::new("test_ext", "1.0.0")
            .description("A test")
            .author("me");
        assert_eq!(ext.meta.name, "test_ext");
        assert_eq!(ext.meta.version, "1.0.0");
        assert_eq!(ext.meta.description, "A test");
        assert_eq!(ext.rule_count(), 0);
    }

    #[test]
    fn test_extension_display() {
        let ext = GrammarExtension::new("my_ext", "2.0.0")
            .description("desc");
        let s = format!("{ext}");
        assert!(s.contains("my_ext"));
        assert!(s.contains("v2.0.0"));
    }

    #[test]
    fn test_capture_bindings() {
        let mut b = CaptureBindings::new();
        b.bind_ident("name", "foo");
        b.bind_expr("val", vec![Token::Literal("42".into())]);
        assert_eq!(b.get_ident("name"), Some("foo"));
        assert!(b.get_expr("val").is_some());
        assert!(b.is_bound("name"));
        assert!(!b.is_bound("nope"));
    }

    #[test]
    fn test_match_keyword() {
        let tokens = vec![Token::Keyword("async".into())];
        let mut b = CaptureBindings::new();
        let r = match_pattern(&PatternElem::Keyword("async".into()), &tokens, &mut b);
        assert_eq!(r, Some(1));
    }

    #[test]
    fn test_match_keyword_fail() {
        let tokens = vec![Token::Keyword("sync".into())];
        let mut b = CaptureBindings::new();
        let r = match_pattern(&PatternElem::Keyword("async".into()), &tokens, &mut b);
        assert_eq!(r, None);
    }

    #[test]
    fn test_match_capture_ident() {
        let tokens = vec![Token::Ident("my_var".into())];
        let mut b = CaptureBindings::new();
        let r = match_pattern(&PatternElem::CaptureIdent("name".into()), &tokens, &mut b);
        assert_eq!(r, Some(1));
        assert_eq!(b.get_ident("name"), Some("my_var"));
    }

    #[test]
    fn test_match_sequence() {
        let tokens = vec![
            Token::Keyword("let".into()),
            Token::Ident("x".into()),
            Token::Punct('='),
        ];
        let pattern = PatternElem::Sequence(vec![
            PatternElem::Keyword("let".into()),
            PatternElem::CaptureIdent("name".into()),
            PatternElem::Punct('='),
        ]);
        let mut b = CaptureBindings::new();
        let r = match_pattern(&pattern, &tokens, &mut b);
        assert_eq!(r, Some(3));
        assert_eq!(b.get_ident("name"), Some("x"));
    }

    #[test]
    fn test_match_optional_present() {
        let tokens = vec![Token::Keyword("pub".into())];
        let mut b = CaptureBindings::new();
        let r = match_pattern(
            &PatternElem::Optional(Box::new(PatternElem::Keyword("pub".into()))),
            &tokens, &mut b,
        );
        assert_eq!(r, Some(1));
    }

    #[test]
    fn test_match_optional_absent() {
        let tokens = vec![Token::Keyword("fn".into())];
        let mut b = CaptureBindings::new();
        let r = match_pattern(
            &PatternElem::Optional(Box::new(PatternElem::Keyword("pub".into()))),
            &tokens, &mut b,
        );
        assert_eq!(r, Some(0));
    }

    #[test]
    fn test_match_capture_block() {
        let inner = vec![Token::Ident("x".into()), Token::Punct('+'), Token::Literal("1".into())];
        let tokens = vec![Token::Group(Delimiter::Brace, inner.clone())];
        let mut b = CaptureBindings::new();
        let r = match_pattern(&PatternElem::CaptureBlock("body".into()), &tokens, &mut b);
        assert_eq!(r, Some(1));
        assert_eq!(b.get_block("body").unwrap().len(), 3);
    }

    #[test]
    fn test_expand_template_literal() {
        let template = vec![TemplateElem::Literal("hello world".into())];
        let b = CaptureBindings::new();
        assert_eq!(expand_template(&template, &b), "hello world");
    }

    #[test]
    fn test_expand_template_subst() {
        let template = vec![
            TemplateElem::Literal("fn ".into()),
            TemplateElem::Subst("name".into()),
            TemplateElem::Literal("() {}".into()),
        ];
        let mut b = CaptureBindings::new();
        b.bind_ident("name", "my_func");
        assert_eq!(expand_template(&template, &b), "fn my_func() {}");
    }

    #[test]
    fn test_expand_template_ifbound() {
        let template = vec![
            TemplateElem::IfBound("vis".into(), vec![TemplateElem::Literal("pub ".into())]),
            TemplateElem::Literal("fn".into()),
        ];
        let mut b = CaptureBindings::new();
        b.bind_ident("vis", "pub");
        assert_eq!(expand_template(&template, &b), "pub fn");
    }

    #[test]
    fn test_expand_template_ifbound_absent() {
        let template = vec![
            TemplateElem::IfBound("vis".into(), vec![TemplateElem::Literal("pub ".into())]),
            TemplateElem::Literal("fn".into()),
        ];
        let b = CaptureBindings::new();
        assert_eq!(expand_template(&template, &b), "fn");
    }

    #[test]
    fn test_registry_register() {
        let mut reg = ExtensionRegistry::new();
        let ext = GrammarExtension::new("ext1", "1.0.0")
            .rule(GrammarRule {
                name: "rule1".into(),
                pattern: PatternElem::Keyword("test".into()),
                template: vec![],
                priority: 0,
            });
        assert!(reg.register(ext).is_ok());
        assert_eq!(reg.extension_count(), 1);
        assert_eq!(reg.rule_count(), 1);
    }

    #[test]
    fn test_registry_duplicate_extension() {
        let mut reg = ExtensionRegistry::new();
        reg.register(GrammarExtension::new("ext1", "1.0.0")).unwrap();
        let result = reg.register(GrammarExtension::new("ext1", "1.0.0"));
        assert!(matches!(result, Err(RegistryError::DuplicateExtension(_))));
    }

    #[test]
    fn test_registry_duplicate_rule() {
        let mut reg = ExtensionRegistry::new();
        reg.register(GrammarExtension::new("ext1", "1.0.0").rule(GrammarRule {
            name: "r".into(), pattern: PatternElem::Keyword("a".into()),
            template: vec![], priority: 0,
        })).unwrap();
        let result = reg.register(GrammarExtension::new("ext2", "1.0.0").rule(GrammarRule {
            name: "r".into(), pattern: PatternElem::Keyword("b".into()),
            template: vec![], priority: 0,
        }));
        assert!(matches!(result, Err(RegistryError::DuplicateRule(_))));
    }

    #[test]
    fn test_registry_invalid_empty_name() {
        let mut reg = ExtensionRegistry::new();
        let result = reg.register(GrammarExtension::new("", "1.0.0"));
        assert!(matches!(result, Err(RegistryError::InvalidExtension(_))));
    }

    #[test]
    fn test_registry_unregister() {
        let mut reg = ExtensionRegistry::new();
        reg.register(GrammarExtension::new("ext1", "1.0.0").rule(GrammarRule {
            name: "r".into(), pattern: PatternElem::Keyword("a".into()),
            template: vec![], priority: 0,
        })).unwrap();
        assert!(reg.unregister("ext1"));
        assert_eq!(reg.extension_count(), 0);
        assert_eq!(reg.rule_count(), 0);
        assert!(!reg.unregister("ext1"));
    }

    #[test]
    fn test_registry_discover() {
        let mut reg = ExtensionRegistry::new();
        reg.register(GrammarExtension::new("async_ext", "1.0")).unwrap();
        reg.register(GrammarExtension::new("async_pipeline", "1.0")).unwrap();
        reg.register(GrammarExtension::new("other", "1.0")).unwrap();
        let found = reg.discover("async");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_registry_rules_by_priority() {
        let mut reg = ExtensionRegistry::new();
        reg.register(GrammarExtension::new("ext", "1.0")
            .rule(GrammarRule { name: "low".into(), pattern: PatternElem::Keyword("a".into()), template: vec![], priority: 1 })
            .rule(GrammarRule { name: "high".into(), pattern: PatternElem::Keyword("b".into()), template: vec![], priority: 10 })
        ).unwrap();
        let rules = reg.rules_by_priority();
        assert_eq!(rules[0].name, "high");
        assert_eq!(rules[1].name, "low");
    }

    #[test]
    fn test_parse_grammar_extension() {
        let input = r#"
            name: "my_ext",
            version: "0.2.0",
            rule greet: hello $ident:name => "greeting: ${name}"
        "#;
        let ext = parse_grammar_extension(input).unwrap();
        assert_eq!(ext.meta.name, "my_ext");
        assert_eq!(ext.meta.version, "0.2.0");
        assert_eq!(ext.rule_count(), 1);
    }

    #[test]
    fn test_parse_grammar_extension_no_name() {
        let result = parse_grammar_extension("version: \"1.0\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_extensions() {
        let mut reg = ExtensionRegistry::new();
        let ext = GrammarExtension::new("test", "1.0")
            .rule(GrammarRule {
                name: "async_fn".into(),
                pattern: PatternElem::Sequence(vec![
                    PatternElem::Keyword("async".into()),
                    PatternElem::Keyword("fn".into()),
                    PatternElem::CaptureIdent("name".into()),
                ]),
                template: vec![
                    TemplateElem::Literal("fn ".into()),
                    TemplateElem::Subst("name".into()),
                    TemplateElem::Literal("_async".into()),
                ],
                priority: 0,
            });
        reg.register(ext).unwrap();

        let tokens = vec![
            Token::Keyword("async".into()),
            Token::Keyword("fn".into()),
            Token::Ident("process".into()),
        ];
        let result = apply_extensions(&reg, &tokens).unwrap();
        assert_eq!(result.rule_name, "async_fn");
        assert_eq!(result.expansion, "fn process_async");
    }

    #[test]
    fn test_apply_extensions_no_match() {
        let reg = ExtensionRegistry::new();
        let tokens = vec![Token::Ident("hello".into())];
        assert!(apply_extensions(&reg, &tokens).is_none());
    }

    #[test]
    fn test_registry_error_display() {
        assert!(format!("{}", RegistryError::DuplicateExtension("x".into())).contains("x"));
        assert!(format!("{}", RegistryError::DuplicateRule("r".into())).contains("r"));
        assert!(format!("{}", RegistryError::InvalidExtension("bad".into())).contains("bad"));
    }

    #[test]
    fn test_get_extension_and_rule() {
        let mut reg = ExtensionRegistry::new();
        let r = GrammarRule {
            name: "myrule".into(),
            pattern: PatternElem::Keyword("test".into()),
            template: vec![],
            priority: 3,
        };
        reg.register(GrammarExtension::new("myext", "1.0").rule(r)).unwrap();
        assert!(reg.get_extension("myext").is_some());
        assert!(reg.get_rule("myrule").is_some());
        assert_eq!(reg.get_rule("myrule").unwrap().priority, 3);
    }

    #[test]
    fn test_match_repeat() {
        let tokens = vec![
            Token::Keyword("a".into()),
            Token::Keyword("a".into()),
            Token::Keyword("b".into()),
        ];
        let mut b = CaptureBindings::new();
        let consumed = match_pattern(
            &PatternElem::Repeat(Box::new(PatternElem::Keyword("a".into()))),
            &tokens, &mut b,
        );
        assert_eq!(consumed, Some(2));
    }

    #[test]
    fn test_end_to_end_parse_register_apply() {
        let input = r#"
            name: "pipeline_sugar",
            version: "0.1.0",
            rule pipe: $ident:a | $ident:b => "${a}.pipe(${b})"
        "#;
        let ext = parse_grammar_extension(input).unwrap();
        let mut reg = ExtensionRegistry::new();
        reg.register(ext).unwrap();

        let tokens = vec![
            Token::Ident("input".into()),
            Token::Punct('|'),
            Token::Ident("transform".into()),
        ];
        let result = apply_extensions(&reg, &tokens).unwrap();
        assert_eq!(result.expansion, "input.pipe(transform)");
    }
}

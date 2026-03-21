//! # Agent Discovery Attributes
//!
//! Implements the five agent discovery attributes from the Redox language:
//!
//! | Short | Long form         | Purpose                              |
//! |-------|-------------------|--------------------------------------|
//! | `@as` | `agent_spec`      | Declare an agent-discoverable spec   |
//! | `@ac` | `agent_contract`  | Attach a contract to a function      |
//! | `@ax` | `agent_effect`    | Declare an effect                    |
//! | `@ao` | `agent_capability`| Declare a required/provided cap      |
//! | `@ae` | `agent_entry`     | Mark a function as agent entry point |
//!
//! Modules: parsing → AST → lowering → metadata emission.

use std::collections::BTreeMap;
use std::fmt;

// ===========================================================================
// AST types
// ===========================================================================

/// The five agent attribute kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttrKind {
    /// `@as` / `agent_spec` — discoverable specification.
    AgentSpec,
    /// `@ac` / `agent_contract` — function contract.
    AgentContract,
    /// `@ax` / `agent_effect` — effect declaration.
    AgentEffect,
    /// `@ao` / `agent_capability` — capability declaration.
    AgentCapability,
    /// `@ae` / `agent_entry` — agent entry point.
    AgentEntry,
}

impl AttrKind {
    pub fn short_form(self) -> &'static str {
        match self {
            Self::AgentSpec => "@as",
            Self::AgentContract => "@ac",
            Self::AgentEffect => "@ax",
            Self::AgentCapability => "@ao",
            Self::AgentEntry => "@ae",
        }
    }

    pub fn long_form(self) -> &'static str {
        match self {
            Self::AgentSpec => "agent_spec",
            Self::AgentContract => "agent_contract",
            Self::AgentEffect => "agent_effect",
            Self::AgentCapability => "agent_capability",
            Self::AgentEntry => "agent_entry",
        }
    }

    pub fn all() -> &'static [AttrKind] {
        &[
            Self::AgentSpec,
            Self::AgentContract,
            Self::AgentEffect,
            Self::AgentCapability,
            Self::AgentEntry,
        ]
    }
}

impl fmt::Display for AttrKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short_form())
    }
}

/// A parsed agent attribute with its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentAttr {
    pub kind: AttrKind,
    pub args: AttrArgs,
    /// Optional target item name (set during lowering).
    pub target: Option<String>,
}

/// Arguments carried by an agent attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrArgs {
    /// No arguments: `@ae`.
    Empty,
    /// Positional string: `@as("spec description")`.
    Positional(Vec<String>),
    /// Key-value pairs: `@ac(requires = "x > 0", ensures = "result > 0")`.
    KeyValue(Vec<(String, String)>),
}

impl AttrArgs {
    pub fn get(&self, key: &str) -> Option<&str> {
        match self {
            Self::KeyValue(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()),
            _ => None,
        }
    }

    pub fn positional(&self) -> &[String] {
        match self {
            Self::Positional(args) => args,
            _ => &[],
        }
    }
}

// ===========================================================================
// Parsing
// ===========================================================================

/// Parse errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    UnknownAttribute(String),
    ExpectedOpenParen,
    ExpectedCloseParen,
    ExpectedEquals,
    ExpectedString,
    UnexpectedChar(char),
    UnexpectedEof,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAttribute(s) => write!(f, "unknown agent attribute: {s}"),
            Self::ExpectedOpenParen => write!(f, "expected '('"),
            Self::ExpectedCloseParen => write!(f, "expected ')'"),
            Self::ExpectedEquals => write!(f, "expected '='"),
            Self::ExpectedString => write!(f, "expected string literal"),
            Self::UnexpectedChar(c) => write!(f, "unexpected character: '{c}'"),
            Self::UnexpectedEof => write!(f, "unexpected end of input"),
        }
    }
}

/// Parse an agent attribute from source text (e.g. `@as("spec")`, `@ac(requires = "x > 0")`).
pub fn parse_attr(input: &str) -> Result<AgentAttr, ParseError> {
    let input = input.trim();
    if !input.starts_with('@') {
        return Err(ParseError::UnknownAttribute(input.into()));
    }

    // Extract the attribute name (short or long form).
    let rest = &input[1..];
    let (kind, remainder) = parse_attr_name(rest)?;

    // Parse arguments if present.
    let remainder = remainder.trim();
    let args = if remainder.is_empty() {
        AttrArgs::Empty
    } else if remainder.starts_with('(') {
        parse_args(remainder)?
    } else {
        return Err(ParseError::UnexpectedChar(
            remainder.chars().next().unwrap_or(' '),
        ));
    };

    Ok(AgentAttr { kind, args, target: None })
}

fn parse_attr_name(input: &str) -> Result<(AttrKind, &str), ParseError> {
    // Try short forms first (2 chars).
    if input.len() >= 2 {
        let short = &input[..2];
        let kind = match short {
            "as" => Some(AttrKind::AgentSpec),
            "ac" => Some(AttrKind::AgentContract),
            "ax" => Some(AttrKind::AgentEffect),
            "ao" => Some(AttrKind::AgentCapability),
            "ae" => Some(AttrKind::AgentEntry),
            _ => None,
        };
        if let Some(k) = kind {
            let rest = &input[2..];
            // Ensure the next char isn't alphanumeric (to avoid partial matches).
            if rest.is_empty() || !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                return Ok((k, rest));
            }
        }
    }

    // Try long forms.
    for &kind in AttrKind::all() {
        let long = kind.long_form();
        if input.starts_with(long) {
            let rest = &input[long.len()..];
            if rest.is_empty() || !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                return Ok((kind, rest));
            }
        }
    }

    // Find the complete identifier for error message.
    let end = input
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(input.len());
    Err(ParseError::UnknownAttribute(format!("@{}", &input[..end])))
}

fn parse_args(input: &str) -> Result<AttrArgs, ParseError> {
    let input = input.trim();
    if !input.starts_with('(') {
        return Err(ParseError::ExpectedOpenParen);
    }
    let inner = &input[1..];
    let close_pos = find_matching_close(inner)?;
    let content = inner[..close_pos].trim();

    if content.is_empty() {
        return Ok(AttrArgs::Empty);
    }

    // Check if it's key=value or positional.
    if content.contains('=') && !content.starts_with('"') {
        parse_kv_args(content)
    } else {
        parse_positional_args(content)
    }
}

fn find_matching_close(input: &str) -> Result<usize, ParseError> {
    let mut depth = 0;
    let mut in_string = false;
    let mut escape = false;
    for (i, ch) in input.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape = true,
            '"' => in_string = !in_string,
            '(' if !in_string => depth += 1,
            ')' if !in_string => {
                if depth == 0 {
                    return Ok(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    Err(ParseError::ExpectedCloseParen)
}

fn parse_positional_args(content: &str) -> Result<AttrArgs, ParseError> {
    let mut args = Vec::new();
    for part in split_top_level(content, ',') {
        let part = part.trim();
        if part.starts_with('"') && part.ends_with('"') && part.len() >= 2 {
            args.push(unescape_string(&part[1..part.len() - 1]));
        } else {
            args.push(part.into());
        }
    }
    Ok(AttrArgs::Positional(args))
}

fn parse_kv_args(content: &str) -> Result<AttrArgs, ParseError> {
    let mut pairs = Vec::new();
    for part in split_top_level(content, ',') {
        let part = part.trim();
        let eq_pos = part.find('=').ok_or(ParseError::ExpectedEquals)?;
        let key = part[..eq_pos].trim();
        let val_raw = part[eq_pos + 1..].trim();
        let val = if val_raw.starts_with('"') && val_raw.ends_with('"') && val_raw.len() >= 2 {
            unescape_string(&val_raw[1..val_raw.len() - 1])
        } else {
            val_raw.into()
        };
        pairs.push((key.into(), val));
    }
    Ok(AttrArgs::KeyValue(pairs))
}

fn split_top_level(input: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_string = false;
    let mut escape = false;
    for (i, ch) in input.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape = true,
            '"' => in_string = !in_string,
            c if c == sep && !in_string => {
                parts.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start <= input.len() {
        parts.push(&input[start..]);
    }
    parts
}

fn unescape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some(c) => {
                    out.push('\\');
                    out.push(c);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

// ===========================================================================
// Lowering  (attribute → compiler IR metadata)
// ===========================================================================

/// Lowered form of agent attributes attached to an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredItem {
    pub name: String,
    pub attrs: Vec<LoweredAttr>,
}

/// A lowered agent attribute — ready for metadata emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredAttr {
    pub kind: AttrKind,
    pub metadata: BTreeMap<String, String>,
}

/// Lower a parsed `AgentAttr` into `LoweredAttr`.
pub fn lower_attr(attr: &AgentAttr) -> LoweredAttr {
    let mut metadata = BTreeMap::new();

    match attr.kind {
        AttrKind::AgentSpec => {
            match &attr.args {
                AttrArgs::Positional(args) if !args.is_empty() => {
                    metadata.insert("description".into(), args[0].clone());
                    if args.len() > 1 {
                        metadata.insert("version".into(), args[1].clone());
                    }
                }
                AttrArgs::KeyValue(pairs) => {
                    for (k, v) in pairs {
                        metadata.insert(k.clone(), v.clone());
                    }
                }
                _ => {}
            }
            metadata.entry("kind".to_string()).or_insert_with(|| "spec".into());
        }

        AttrKind::AgentContract => {
            if let Some(req) = attr.args.get("requires") {
                metadata.insert("requires".into(), req.into());
            }
            if let Some(ens) = attr.args.get("ensures") {
                metadata.insert("ensures".into(), ens.into());
            }
            metadata.entry("kind".to_string()).or_insert_with(|| "contract".into());
        }

        AttrKind::AgentEffect => {
            match &attr.args {
                AttrArgs::Positional(args) if !args.is_empty() => {
                    metadata.insert("effect".into(), args.join(", "));
                }
                AttrArgs::KeyValue(pairs) => {
                    for (k, v) in pairs {
                        metadata.insert(k.clone(), v.clone());
                    }
                }
                _ => {}
            }
            metadata.entry("kind".to_string()).or_insert_with(|| "effect".into());
        }

        AttrKind::AgentCapability => {
            match &attr.args {
                AttrArgs::Positional(args) if !args.is_empty() => {
                    metadata.insert("capability".into(), args.join(", "));
                }
                AttrArgs::KeyValue(pairs) => {
                    for (k, v) in pairs {
                        metadata.insert(k.clone(), v.clone());
                    }
                }
                _ => {}
            }
            metadata.entry("kind".to_string()).or_insert_with(|| "capability".into());
        }

        AttrKind::AgentEntry => {
            metadata.insert("kind".into(), "entry".into());
            metadata.insert("discoverable".into(), "true".into());
        }
    }

    if let Some(target) = &attr.target {
        metadata.insert("target".into(), target.clone());
    }

    LoweredAttr {
        kind: attr.kind,
        metadata,
    }
}

/// Lower a set of attributes for a named item.
pub fn lower_item(name: &str, attrs: &[AgentAttr]) -> LoweredItem {
    let lowered = attrs.iter().map(lower_attr).collect();
    LoweredItem {
        name: name.into(),
        attrs: lowered,
    }
}

// ===========================================================================
// Metadata emission  (JSON format)
// ===========================================================================

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl LoweredItem {
    /// Emit this item's agent metadata as a JSON string.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str(&format!("  \"item\": {},\n", json_escape(&self.name)));
        out.push_str("  \"agent_attrs\": [\n");
        for (i, attr) in self.attrs.iter().enumerate() {
            out.push_str("    {\n");
            out.push_str(&format!(
                "      \"kind\": {},\n",
                json_escape(attr.kind.short_form())
            ));
            out.push_str("      \"metadata\": {\n");
            let entries: Vec<_> = attr.metadata.iter().collect();
            for (j, (k, v)) in entries.iter().enumerate() {
                let comma = if j + 1 < entries.len() { "," } else { "" };
                out.push_str(&format!(
                    "        {}: {}{}\n",
                    json_escape(k),
                    json_escape(v),
                    comma
                ));
            }
            out.push_str("      }\n");
            let comma = if i + 1 < self.attrs.len() { "," } else { "" };
            out.push_str(&format!("    }}{comma}\n"));
        }
        out.push_str("  ]\n");
        out.push_str("}");
        out
    }
}

/// Emit metadata for multiple items.
pub fn emit_metadata(items: &[LoweredItem]) -> String {
    let mut out = String::new();
    out.push_str("[\n");
    for (i, item) in items.iter().enumerate() {
        // Indent each item JSON.
        for line in item.to_json().lines() {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
        if i + 1 < items.len() {
            // Remove trailing newline and add comma.
            if out.ends_with('\n') {
                out.pop();
            }
            out.push_str(",\n");
        }
    }
    out.push_str("]");
    out
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- AttrKind ----------------------------------------------------------

    #[test]
    fn attr_kind_forms() {
        assert_eq!(AttrKind::AgentSpec.short_form(), "@as");
        assert_eq!(AttrKind::AgentSpec.long_form(), "agent_spec");
        assert_eq!(AttrKind::AgentContract.short_form(), "@ac");
        assert_eq!(AttrKind::AgentEffect.short_form(), "@ax");
        assert_eq!(AttrKind::AgentCapability.short_form(), "@ao");
        assert_eq!(AttrKind::AgentEntry.short_form(), "@ae");
    }

    #[test]
    fn attr_kind_display() {
        assert_eq!(format!("{}", AttrKind::AgentSpec), "@as");
        assert_eq!(format!("{}", AttrKind::AgentEntry), "@ae");
    }

    #[test]
    fn attr_kind_all() {
        assert_eq!(AttrKind::all().len(), 5);
    }

    // -- Parsing: short forms ----------------------------------------------

    #[test]
    fn parse_agent_entry_bare() {
        let attr = parse_attr("@ae").unwrap();
        assert_eq!(attr.kind, AttrKind::AgentEntry);
        assert_eq!(attr.args, AttrArgs::Empty);
    }

    #[test]
    fn parse_agent_spec_positional() {
        let attr = parse_attr("@as(\"Encryption service\")").unwrap();
        assert_eq!(attr.kind, AttrKind::AgentSpec);
        assert_eq!(attr.args.positional(), &["Encryption service"]);
    }

    #[test]
    fn parse_agent_contract_kv() {
        let attr = parse_attr("@ac(requires = \"x > 0\", ensures = \"result > 0\")").unwrap();
        assert_eq!(attr.kind, AttrKind::AgentContract);
        assert_eq!(attr.args.get("requires"), Some("x > 0"));
        assert_eq!(attr.args.get("ensures"), Some("result > 0"));
    }

    #[test]
    fn parse_agent_effect_positional() {
        let attr = parse_attr("@ax(\"io\", \"alloc\")").unwrap();
        assert_eq!(attr.kind, AttrKind::AgentEffect);
        assert_eq!(attr.args.positional(), &["io", "alloc"]);
    }

    #[test]
    fn parse_agent_capability() {
        let attr = parse_attr("@ao(\"crypto::aead\")").unwrap();
        assert_eq!(attr.kind, AttrKind::AgentCapability);
        assert_eq!(attr.args.positional(), &["crypto::aead"]);
    }

    // -- Parsing: long forms -----------------------------------------------

    #[test]
    fn parse_long_form_entry() {
        let attr = parse_attr("@agent_entry").unwrap();
        assert_eq!(attr.kind, AttrKind::AgentEntry);
    }

    #[test]
    fn parse_long_form_spec() {
        let attr = parse_attr("@agent_spec(\"Data processor\")").unwrap();
        assert_eq!(attr.kind, AttrKind::AgentSpec);
        assert_eq!(attr.args.positional(), &["Data processor"]);
    }

    #[test]
    fn parse_long_form_contract() {
        let attr = parse_attr("@agent_contract(requires = \"n > 0\")").unwrap();
        assert_eq!(attr.kind, AttrKind::AgentContract);
        assert_eq!(attr.args.get("requires"), Some("n > 0"));
    }

    // -- Parsing: edge cases -----------------------------------------------

    #[test]
    fn parse_unknown_attr() {
        let err = parse_attr("@unknown").unwrap_err();
        assert!(matches!(err, ParseError::UnknownAttribute(_)));
    }

    #[test]
    fn parse_no_at_sign() {
        let err = parse_attr("as").unwrap_err();
        assert!(matches!(err, ParseError::UnknownAttribute(_)));
    }

    #[test]
    fn parse_empty_args() {
        let attr = parse_attr("@ae()").unwrap();
        assert_eq!(attr.args, AttrArgs::Empty);
    }

    #[test]
    fn parse_whitespace_tolerance() {
        let attr = parse_attr("  @ae  ").unwrap();
        assert_eq!(attr.kind, AttrKind::AgentEntry);
    }

    #[test]
    fn parse_escaped_string() {
        let attr = parse_attr("@as(\"line\\none\")").unwrap();
        assert_eq!(attr.args.positional(), &["line\none"]);
    }

    #[test]
    fn parse_error_display() {
        assert_eq!(
            format!("{}", ParseError::ExpectedOpenParen),
            "expected '('"
        );
        assert_eq!(
            format!("{}", ParseError::UnexpectedEof),
            "unexpected end of input"
        );
    }

    // -- Lowering ----------------------------------------------------------

    #[test]
    fn lower_entry_attr() {
        let attr = parse_attr("@ae").unwrap();
        let lowered = lower_attr(&attr);
        assert_eq!(lowered.kind, AttrKind::AgentEntry);
        assert_eq!(lowered.metadata.get("kind").unwrap(), "entry");
        assert_eq!(lowered.metadata.get("discoverable").unwrap(), "true");
    }

    #[test]
    fn lower_spec_attr() {
        let attr = parse_attr("@as(\"Crypto service\", \"1.0\")").unwrap();
        let lowered = lower_attr(&attr);
        assert_eq!(lowered.metadata.get("description").unwrap(), "Crypto service");
        assert_eq!(lowered.metadata.get("version").unwrap(), "1.0");
    }

    #[test]
    fn lower_contract_attr() {
        let attr = parse_attr("@ac(requires = \"x > 0\", ensures = \"result >= 0\")").unwrap();
        let lowered = lower_attr(&attr);
        assert_eq!(lowered.metadata.get("requires").unwrap(), "x > 0");
        assert_eq!(lowered.metadata.get("ensures").unwrap(), "result >= 0");
        assert_eq!(lowered.metadata.get("kind").unwrap(), "contract");
    }

    #[test]
    fn lower_effect_attr() {
        let attr = parse_attr("@ax(\"io\", \"alloc\")").unwrap();
        let lowered = lower_attr(&attr);
        assert_eq!(lowered.metadata.get("effect").unwrap(), "io, alloc");
    }

    #[test]
    fn lower_capability_attr() {
        let attr = parse_attr("@ao(\"crypto::aead\")").unwrap();
        let lowered = lower_attr(&attr);
        assert_eq!(lowered.metadata.get("capability").unwrap(), "crypto::aead");
    }

    #[test]
    fn lower_item_multiple_attrs() {
        let attrs = vec![
            parse_attr("@ae").unwrap(),
            parse_attr("@as(\"Entry function\")").unwrap(),
            parse_attr("@ac(requires = \"n > 0\")").unwrap(),
        ];
        let item = lower_item("main", &attrs);
        assert_eq!(item.name, "main");
        assert_eq!(item.attrs.len(), 3);
        assert_eq!(item.attrs[0].kind, AttrKind::AgentEntry);
        assert_eq!(item.attrs[1].kind, AttrKind::AgentSpec);
        assert_eq!(item.attrs[2].kind, AttrKind::AgentContract);
    }

    #[test]
    fn lower_attr_with_target() {
        let mut attr = parse_attr("@ae").unwrap();
        attr.target = Some("process".into());
        let lowered = lower_attr(&attr);
        assert_eq!(lowered.metadata.get("target").unwrap(), "process");
    }

    // -- Metadata emission -------------------------------------------------

    #[test]
    fn emit_single_item_json() {
        let attrs = vec![parse_attr("@ae").unwrap()];
        let item = lower_item("main", &attrs);
        let json = item.to_json();
        assert!(json.contains("\"item\": \"main\""));
        assert!(json.contains("\"kind\": \"@ae\""));
        assert!(json.contains("\"discoverable\": \"true\""));
    }

    #[test]
    fn emit_multiple_items() {
        let item1 = lower_item("foo", &[parse_attr("@ae").unwrap()]);
        let item2 = lower_item("bar", &[parse_attr("@as(\"helper\")").unwrap()]);
        let json = emit_metadata(&[item1, item2]);
        assert!(json.starts_with('['));
        assert!(json.contains("\"foo\""));
        assert!(json.contains("\"bar\""));
    }

    #[test]
    fn emit_empty_metadata() {
        let json = emit_metadata(&[]);
        assert_eq!(json.trim(), "[\n]");
    }

    // -- AttrArgs accessors ------------------------------------------------

    #[test]
    fn attr_args_get_missing_key() {
        assert_eq!(AttrArgs::Empty.get("key"), None);
        assert_eq!(AttrArgs::Positional(vec!["a".into()]).get("key"), None);
    }

    #[test]
    fn attr_args_positional_on_empty() {
        assert!(AttrArgs::Empty.positional().is_empty());
        assert!(AttrArgs::KeyValue(vec![]).positional().is_empty());
    }
}

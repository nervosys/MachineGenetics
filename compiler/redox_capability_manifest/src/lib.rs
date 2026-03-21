//! # Capability Manifest
//!
//! Implements the capability manifest schema from REDOX_PROPOSAL.md §10.2.
//! Each compiled crate produces a capability manifest JSON alongside its
//! rlib/dylib, describing what capabilities it requires and provides.
//!
//! Serialization is hand-written (no serde dependency) to keep the crate
//! lightweight and self-contained.

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::Path;

// ===========================================================================
// Core schema types  (matches §10.2 JSON)
// ===========================================================================

/// Top-level capability manifest for a single crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityManifest {
    /// Crate name.
    pub crate_name: String,
    /// Crate version (semver string).
    pub version: String,
    /// Capabilities this crate requires from the environment.
    pub capabilities_required: Vec<String>,
    /// Capabilities this crate provides to consumers.
    pub capabilities_provided: BTreeMap<String, ProvidedCapability>,
    /// Per-function contracts.
    pub contracts: BTreeMap<String, FunctionContract>,
    /// Compatibility metadata.
    pub compatibility: Compatibility,
}

/// A capability that the crate provides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvidedCapability {
    /// Functions that implement this capability.
    pub functions: Vec<String>,
    /// Safety classification (e.g. "constant-time", "no-unsafe").
    pub safety: String,
    /// Certifications (e.g. "FIPS-140-3").
    pub certifications: Vec<String>,
}

/// A function-level contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionContract {
    /// Precondition expression (Redox syntax).
    pub requires: String,
    /// Postcondition expression (Redox syntax).
    pub ensures: String,
    /// Effects this function performs.
    pub effects: Vec<String>,
}

/// Compatibility metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compatibility {
    pub no_std: bool,
    pub no_alloc: bool,
    /// Target platforms (["all"] or specific list).
    pub platforms: Vec<String>,
}

impl Default for Compatibility {
    fn default() -> Self {
        Self {
            no_std: false,
            no_alloc: false,
            platforms: vec!["all".into()],
        }
    }
}

// ===========================================================================
// Builder
// ===========================================================================

/// Fluent builder for constructing a `CapabilityManifest`.
pub struct ManifestBuilder {
    manifest: CapabilityManifest,
}

impl ManifestBuilder {
    pub fn new(crate_name: &str, version: &str) -> Self {
        Self {
            manifest: CapabilityManifest {
                crate_name: crate_name.into(),
                version: version.into(),
                capabilities_required: Vec::new(),
                capabilities_provided: BTreeMap::new(),
                contracts: BTreeMap::new(),
                compatibility: Compatibility::default(),
            },
        }
    }

    pub fn require(mut self, cap: &str) -> Self {
        self.manifest.capabilities_required.push(cap.into());
        self
    }

    pub fn provide(mut self, name: &str, cap: ProvidedCapability) -> Self {
        self.manifest.capabilities_provided.insert(name.into(), cap);
        self
    }

    pub fn contract(mut self, func: &str, contract: FunctionContract) -> Self {
        self.manifest.contracts.insert(func.into(), contract);
        self
    }

    pub fn no_std(mut self, val: bool) -> Self {
        self.manifest.compatibility.no_std = val;
        self
    }

    pub fn no_alloc(mut self, val: bool) -> Self {
        self.manifest.compatibility.no_alloc = val;
        self
    }

    pub fn platforms(mut self, platforms: Vec<String>) -> Self {
        self.manifest.compatibility.platforms = platforms;
        self
    }

    pub fn build(self) -> CapabilityManifest {
        self.manifest
    }
}

// ===========================================================================
// JSON serialization  (hand-written, no serde)
// ===========================================================================

/// Escape a string for JSON output.
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
            c if c.is_control() => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_string_list(items: &[String]) -> String {
    let inner: Vec<String> = items.iter().map(|s| json_escape(s)).collect();
    format!("[{}]", inner.join(", "))
}

impl CapabilityManifest {
    /// Serialize to pretty-printed JSON string.
    pub fn to_json(&self) -> String {
        let mut out = String::with_capacity(1024);
        out.push_str("{\n");
        out.push_str(&format!("  \"crate\": {},\n", json_escape(&self.crate_name)));
        out.push_str(&format!("  \"version\": {},\n", json_escape(&self.version)));
        out.push_str(&format!(
            "  \"capabilities_required\": {},\n",
            json_string_list(&self.capabilities_required)
        ));

        // capabilities_provided
        out.push_str("  \"capabilities_provided\": {\n");
        let cap_entries: Vec<_> = self.capabilities_provided.iter().collect();
        for (i, (name, cap)) in cap_entries.iter().enumerate() {
            out.push_str(&format!("    {}: {{\n", json_escape(name)));
            out.push_str(&format!(
                "      \"functions\": {},\n",
                json_string_list(&cap.functions)
            ));
            out.push_str(&format!("      \"safety\": {},\n", json_escape(&cap.safety)));
            out.push_str(&format!(
                "      \"certifications\": {}\n",
                json_string_list(&cap.certifications)
            ));
            if i + 1 < cap_entries.len() {
                out.push_str("    },\n");
            } else {
                out.push_str("    }\n");
            }
        }
        out.push_str("  },\n");

        // contracts
        out.push_str("  \"contracts\": {\n");
        let contract_entries: Vec<_> = self.contracts.iter().collect();
        for (i, (name, contract)) in contract_entries.iter().enumerate() {
            out.push_str(&format!("    {}: {{\n", json_escape(name)));
            out.push_str(&format!(
                "      \"requires\": {},\n",
                json_escape(&contract.requires)
            ));
            out.push_str(&format!(
                "      \"ensures\": {},\n",
                json_escape(&contract.ensures)
            ));
            out.push_str(&format!(
                "      \"effects\": {}\n",
                json_string_list(&contract.effects)
            ));
            if i + 1 < contract_entries.len() {
                out.push_str("    },\n");
            } else {
                out.push_str("    }\n");
            }
        }
        out.push_str("  },\n");

        // compatibility
        out.push_str("  \"compatibility\": {\n");
        out.push_str(&format!(
            "    \"no_std\": {},\n",
            if self.compatibility.no_std { "true" } else { "false" }
        ));
        out.push_str(&format!(
            "    \"no_alloc\": {},\n",
            if self.compatibility.no_alloc { "true" } else { "false" }
        ));
        out.push_str(&format!(
            "    \"platforms\": {}\n",
            json_string_list(&self.compatibility.platforms)
        ));
        out.push_str("  }\n");

        out.push_str("}");
        out
    }

    /// Write manifest JSON to a file at the given path.
    pub fn write_to_file(&self, path: &Path) -> io::Result<()> {
        std::fs::write(path, self.to_json())
    }

    /// Derive the manifest filename for a crate (e.g. "my_crate.capability.json").
    pub fn filename(crate_name: &str) -> String {
        format!("{}.capability.json", crate_name)
    }
}

// ===========================================================================
// JSON deserialization  (minimal hand-written parser)
// ===========================================================================

/// Errors from parsing a manifest JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    UnexpectedEof,
    Expected(String),
    InvalidValue(String),
    IoError(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "unexpected end of input"),
            Self::Expected(msg) => write!(f, "expected {msg}"),
            Self::InvalidValue(msg) => write!(f, "invalid value: {msg}"),
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

/// A lightweight JSON value for parsing.
#[derive(Debug, Clone, PartialEq)]
enum JsonValue {
    Null,
    Bool(bool),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    fn as_str(&self) -> Result<&str, ParseError> {
        match self {
            JsonValue::Str(s) => Ok(s),
            _ => Err(ParseError::Expected("string".into())),
        }
    }

    fn as_bool(&self) -> Result<bool, ParseError> {
        match self {
            JsonValue::Bool(b) => Ok(*b),
            _ => Err(ParseError::Expected("boolean".into())),
        }
    }

    fn as_array(&self) -> Result<&[JsonValue], ParseError> {
        match self {
            JsonValue::Array(v) => Ok(v),
            _ => Err(ParseError::Expected("array".into())),
        }
    }

    fn as_object(&self) -> Result<&[(String, JsonValue)], ParseError> {
        match self {
            JsonValue::Object(entries) => Ok(entries),
            _ => Err(ParseError::Expected("object".into())),
        }
    }

    fn get_field<'a>(&'a self, name: &str) -> Result<&'a JsonValue, ParseError> {
        let entries = self.as_object()?;
        for (k, v) in entries {
            if k == name {
                return Ok(v);
            }
        }
        Err(ParseError::Expected(format!("field '{name}'")))
    }

    fn string_array(&self) -> Result<Vec<String>, ParseError> {
        let arr = self.as_array()?;
        arr.iter().map(|v| v.as_str().map(|s| s.to_string())).collect()
    }
}

/// Minimal JSON tokenizer/parser.
struct JsonParser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input: input.as_bytes(), pos: 0 }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.input.len() {
            match self.input[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek(&mut self) -> Result<u8, ParseError> {
        self.skip_ws();
        if self.pos < self.input.len() {
            Ok(self.input[self.pos])
        } else {
            Err(ParseError::UnexpectedEof)
        }
    }

    fn consume(&mut self, expected: u8) -> Result<(), ParseError> {
        self.skip_ws();
        if self.pos < self.input.len() && self.input[self.pos] == expected {
            self.pos += 1;
            Ok(())
        } else {
            Err(ParseError::Expected(format!(
                "'{}'",
                expected as char
            )))
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, ParseError> {
        let ch = self.peek()?;
        match ch {
            b'"' => self.parse_string().map(JsonValue::Str),
            b'{' => self.parse_object(),
            b'[' => self.parse_array(),
            b't' | b'f' => self.parse_bool().map(JsonValue::Bool),
            b'n' => self.parse_null(),
            _ => Err(ParseError::InvalidValue(format!(
                "unexpected character '{}'",
                ch as char
            ))),
        }
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        self.consume(b'"')?;
        let mut s = String::new();
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            self.pos += 1;
            match ch {
                b'"' => return Ok(s),
                b'\\' => {
                    if self.pos >= self.input.len() {
                        return Err(ParseError::UnexpectedEof);
                    }
                    let esc = self.input[self.pos];
                    self.pos += 1;
                    match esc {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'n' => s.push('\n'),
                        b'r' => s.push('\r'),
                        b't' => s.push('\t'),
                        b'u' => {
                            // Parse 4-hex-digit unicode escape
                            if self.pos + 4 > self.input.len() {
                                return Err(ParseError::UnexpectedEof);
                            }
                            let hex = std::str::from_utf8(&self.input[self.pos..self.pos + 4])
                                .map_err(|_| ParseError::InvalidValue("bad unicode escape".into()))?;
                            let cp = u32::from_str_radix(hex, 16)
                                .map_err(|_| ParseError::InvalidValue("bad unicode escape".into()))?;
                            if let Some(c) = char::from_u32(cp) {
                                s.push(c);
                            }
                            self.pos += 4;
                        }
                        _ => {
                            s.push('\\');
                            s.push(esc as char);
                        }
                    }
                }
                _ => s.push(ch as char),
            }
        }
        Err(ParseError::UnexpectedEof)
    }

    fn parse_object(&mut self) -> Result<JsonValue, ParseError> {
        self.consume(b'{')?;
        let mut entries = Vec::new();
        if self.peek()? == b'}' {
            self.pos += 1;
            return Ok(JsonValue::Object(entries));
        }
        loop {
            let key = self.parse_string()?;
            self.consume(b':')?;
            let val = self.parse_value()?;
            entries.push((key, val));
            let next = self.peek()?;
            if next == b',' {
                self.pos += 1;
            } else if next == b'}' {
                self.pos += 1;
                break;
            } else {
                return Err(ParseError::Expected("',' or '}'".into()));
            }
        }
        Ok(JsonValue::Object(entries))
    }

    fn parse_array(&mut self) -> Result<JsonValue, ParseError> {
        self.consume(b'[')?;
        let mut items = Vec::new();
        if self.peek()? == b']' {
            self.pos += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            let next = self.peek()?;
            if next == b',' {
                self.pos += 1;
            } else if next == b']' {
                self.pos += 1;
                break;
            } else {
                return Err(ParseError::Expected("',' or ']'".into()));
            }
        }
        Ok(JsonValue::Array(items))
    }

    fn parse_bool(&mut self) -> Result<bool, ParseError> {
        self.skip_ws();
        if self.input[self.pos..].starts_with(b"true") {
            self.pos += 4;
            Ok(true)
        } else if self.input[self.pos..].starts_with(b"false") {
            self.pos += 5;
            Ok(false)
        } else {
            Err(ParseError::Expected("boolean".into()))
        }
    }

    fn parse_null(&mut self) -> Result<JsonValue, ParseError> {
        self.skip_ws();
        if self.input[self.pos..].starts_with(b"null") {
            self.pos += 4;
            Ok(JsonValue::Null)
        } else {
            Err(ParseError::Expected("null".into()))
        }
    }
}

/// Parse a JSON string into a `JsonValue`.
fn parse_json(input: &str) -> Result<JsonValue, ParseError> {
    let mut parser = JsonParser::new(input);
    let val = parser.parse_value()?;
    parser.skip_ws();
    if parser.pos != parser.input.len() {
        return Err(ParseError::InvalidValue("trailing content".into()));
    }
    Ok(val)
}

fn parse_provided_capability(val: &JsonValue) -> Result<ProvidedCapability, ParseError> {
    Ok(ProvidedCapability {
        functions: val.get_field("functions")?.string_array()?,
        safety: val.get_field("safety")?.as_str()?.to_string(),
        certifications: val.get_field("certifications")?.string_array()?,
    })
}

fn parse_contract(val: &JsonValue) -> Result<FunctionContract, ParseError> {
    Ok(FunctionContract {
        requires: val.get_field("requires")?.as_str()?.to_string(),
        ensures: val.get_field("ensures")?.as_str()?.to_string(),
        effects: val.get_field("effects")?.string_array()?,
    })
}

fn parse_compatibility(val: &JsonValue) -> Result<Compatibility, ParseError> {
    Ok(Compatibility {
        no_std: val.get_field("no_std")?.as_bool()?,
        no_alloc: val.get_field("no_alloc")?.as_bool()?,
        platforms: val.get_field("platforms")?.string_array()?,
    })
}

impl CapabilityManifest {
    /// Parse a capability manifest from a JSON string.
    pub fn from_json(input: &str) -> Result<Self, ParseError> {
        let root = parse_json(input)?;

        let crate_name = root.get_field("crate")?.as_str()?.to_string();
        let version = root.get_field("version")?.as_str()?.to_string();
        let capabilities_required = root.get_field("capabilities_required")?.string_array()?;

        let mut capabilities_provided = BTreeMap::new();
        let prov_obj = root.get_field("capabilities_provided")?.as_object()?;
        for (k, v) in prov_obj {
            capabilities_provided.insert(k.clone(), parse_provided_capability(v)?);
        }

        let mut contracts = BTreeMap::new();
        let contracts_obj = root.get_field("contracts")?.as_object()?;
        for (k, v) in contracts_obj {
            contracts.insert(k.clone(), parse_contract(v)?);
        }

        let compatibility = parse_compatibility(root.get_field("compatibility")?)?;

        Ok(CapabilityManifest {
            crate_name,
            version,
            capabilities_required,
            capabilities_provided,
            contracts,
            compatibility,
        })
    }

    /// Read and parse a manifest from a file.
    pub fn from_file(path: &Path) -> Result<Self, ParseError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ParseError::IoError(e.to_string()))?;
        Self::from_json(&content)
    }
}

// ===========================================================================
// Validation
// ===========================================================================

/// Validation error for a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl CapabilityManifest {
    /// Validate the manifest for structural correctness.
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        if self.crate_name.is_empty() {
            errors.push(ValidationError {
                field: "crate".into(),
                message: "crate name must not be empty".into(),
            });
        }

        if self.version.is_empty() {
            errors.push(ValidationError {
                field: "version".into(),
                message: "version must not be empty".into(),
            });
        }

        // Check semver-like format  (major.minor.patch)
        let parts: Vec<&str> = self.version.split('.').collect();
        if parts.len() != 3 || !parts.iter().all(|p| p.parse::<u32>().is_ok()) {
            errors.push(ValidationError {
                field: "version".into(),
                message: "version must be semver (major.minor.patch)".into(),
            });
        }

        // Provided capabilities must have at least one function
        for (name, cap) in &self.capabilities_provided {
            if cap.functions.is_empty() {
                errors.push(ValidationError {
                    field: format!("capabilities_provided.{name}"),
                    message: "must list at least one function".into(),
                });
            }
        }

        // Contracts must have non-empty requires or ensures
        for (name, contract) in &self.contracts {
            if contract.requires.is_empty() && contract.ensures.is_empty() {
                errors.push(ValidationError {
                    field: format!("contracts.{name}"),
                    message: "contract must specify at least requires or ensures".into(),
                });
            }
        }

        if self.compatibility.platforms.is_empty() {
            errors.push(ValidationError {
                field: "compatibility.platforms".into(),
                message: "platforms must not be empty".into(),
            });
        }

        errors
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> CapabilityManifest {
        ManifestBuilder::new("redox_crypto", "1.0.0")
            .require("alloc::heap")
            .provide(
                "crypto::symmetric::aead",
                ProvidedCapability {
                    functions: vec!["encrypt".into(), "decrypt".into()],
                    safety: "constant-time".into(),
                    certifications: vec!["FIPS-140-3".into()],
                },
            )
            .provide(
                "crypto::hash",
                ProvidedCapability {
                    functions: vec!["sha256".into(), "sha512".into(), "blake3".into()],
                    safety: "no-unsafe".into(),
                    certifications: vec![],
                },
            )
            .contract(
                "encrypt",
                FunctionContract {
                    requires: "key.len() == 32 && nonce.len() == 12".into(),
                    ensures: "result.len() == plaintext.len() + 16".into(),
                    effects: vec!["alloc::heap".into()],
                },
            )
            .no_std(true)
            .no_alloc(false)
            .build()
    }

    // -- Builder -----------------------------------------------------------

    #[test]
    fn builder_creates_manifest() {
        let m = sample_manifest();
        assert_eq!(m.crate_name, "redox_crypto");
        assert_eq!(m.version, "1.0.0");
        assert_eq!(m.capabilities_required, vec!["alloc::heap"]);
        assert_eq!(m.capabilities_provided.len(), 2);
        assert_eq!(m.contracts.len(), 1);
        assert!(m.compatibility.no_std);
        assert!(!m.compatibility.no_alloc);
    }

    #[test]
    fn builder_empty() {
        let m = ManifestBuilder::new("empty", "0.1.0").build();
        assert!(m.capabilities_required.is_empty());
        assert!(m.capabilities_provided.is_empty());
        assert!(m.contracts.is_empty());
        assert_eq!(m.compatibility.platforms, vec!["all"]);
    }

    #[test]
    fn builder_custom_platforms() {
        let m = ManifestBuilder::new("plat", "0.1.0")
            .platforms(vec!["linux".into(), "macos".into()])
            .build();
        assert_eq!(m.compatibility.platforms, vec!["linux", "macos"]);
    }

    // -- JSON roundtrip ----------------------------------------------------

    #[test]
    fn json_roundtrip() {
        let original = sample_manifest();
        let json = original.to_json();
        let parsed = CapabilityManifest::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn json_roundtrip_empty() {
        let original = ManifestBuilder::new("empty", "0.1.0").build();
        let json = original.to_json();
        let parsed = CapabilityManifest::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    // -- Serialization details ---------------------------------------------

    #[test]
    fn json_contains_crate_name() {
        let m = sample_manifest();
        let json = m.to_json();
        assert!(json.contains("\"crate\": \"redox_crypto\""));
    }

    #[test]
    fn json_contains_version() {
        let m = sample_manifest();
        let json = m.to_json();
        assert!(json.contains("\"version\": \"1.0.0\""));
    }

    #[test]
    fn json_contains_capabilities() {
        let m = sample_manifest();
        let json = m.to_json();
        assert!(json.contains("\"capabilities_required\":"));
        assert!(json.contains("\"alloc::heap\""));
    }

    #[test]
    fn json_contains_provided() {
        let m = sample_manifest();
        let json = m.to_json();
        assert!(json.contains("\"crypto::symmetric::aead\""));
        assert!(json.contains("\"encrypt\""));
        assert!(json.contains("\"FIPS-140-3\""));
    }

    #[test]
    fn json_contains_contracts() {
        let m = sample_manifest();
        let json = m.to_json();
        assert!(json.contains("\"requires\":"));
        assert!(json.contains("key.len() == 32"));
    }

    #[test]
    fn json_contains_compatibility() {
        let m = sample_manifest();
        let json = m.to_json();
        assert!(json.contains("\"no_std\": true"));
        assert!(json.contains("\"no_alloc\": false"));
    }

    // -- JSON escape -------------------------------------------------------

    #[test]
    fn json_escape_special_chars() {
        let m = ManifestBuilder::new("test\"crate", "0.1.0").build();
        let json = m.to_json();
        assert!(json.contains("test\\\"crate"));
        let parsed = CapabilityManifest::from_json(&json).unwrap();
        assert_eq!(parsed.crate_name, "test\"crate");
    }

    #[test]
    fn json_escape_backslash() {
        let m = ManifestBuilder::new("test\\path", "0.1.0").build();
        let json = m.to_json();
        assert!(json.contains("test\\\\path"));
        let parsed = CapabilityManifest::from_json(&json).unwrap();
        assert_eq!(parsed.crate_name, "test\\path");
    }

    // -- Parsing -----------------------------------------------------------

    #[test]
    fn parse_proposal_example() {
        let json = r#"{
  "crate": "redox_crypto",
  "version": "1.0.0",
  "capabilities_required": ["alloc::heap"],
  "capabilities_provided": {
    "crypto::symmetric::aead": {
      "functions": ["encrypt", "decrypt"],
      "safety": "constant-time",
      "certifications": ["FIPS-140-3"]
    },
    "crypto::hash": {
      "functions": ["sha256", "sha512", "blake3"],
      "safety": "no-unsafe",
      "certifications": []
    }
  },
  "contracts": {
    "encrypt": {
      "requires": "key.len() == 32 && nonce.len() == 12",
      "ensures": "result.len() == plaintext.len() + 16",
      "effects": ["alloc::heap"]
    }
  },
  "compatibility": {
    "no_std": true,
    "no_alloc": false,
    "platforms": ["all"]
  }
}"#;
        let m = CapabilityManifest::from_json(json).unwrap();
        assert_eq!(m.crate_name, "redox_crypto");
        assert_eq!(m.version, "1.0.0");
        assert_eq!(m.capabilities_required, vec!["alloc::heap"]);
        assert_eq!(m.capabilities_provided.len(), 2);
        assert!(m.capabilities_provided.contains_key("crypto::symmetric::aead"));
        let aead = &m.capabilities_provided["crypto::symmetric::aead"];
        assert_eq!(aead.functions, vec!["encrypt", "decrypt"]);
        assert_eq!(aead.safety, "constant-time");
        assert_eq!(aead.certifications, vec!["FIPS-140-3"]);
        let hash = &m.capabilities_provided["crypto::hash"];
        assert_eq!(hash.functions.len(), 3);
        assert!(hash.certifications.is_empty());
        assert_eq!(m.contracts.len(), 1);
        let enc = &m.contracts["encrypt"];
        assert_eq!(enc.effects, vec!["alloc::heap"]);
        assert!(m.compatibility.no_std);
        assert!(!m.compatibility.no_alloc);
    }

    #[test]
    fn parse_error_missing_field() {
        let json = r#"{"version": "1.0.0"}"#;
        let err = CapabilityManifest::from_json(json).unwrap_err();
        assert!(matches!(err, ParseError::Expected(_)));
    }

    #[test]
    fn parse_error_bad_json() {
        let err = CapabilityManifest::from_json("{invalid}").unwrap_err();
        assert!(matches!(err, ParseError::Expected(_) | ParseError::InvalidValue(_)));
    }

    #[test]
    fn parse_error_trailing_content() {
        let err = parse_json("\"hello\" extra").unwrap_err();
        assert!(matches!(err, ParseError::InvalidValue(_)));
    }

    // -- Validation --------------------------------------------------------

    #[test]
    fn validate_good_manifest() {
        let m = sample_manifest();
        assert!(m.validate().is_empty());
    }

    #[test]
    fn validate_empty_crate_name() {
        let m = ManifestBuilder::new("", "1.0.0").build();
        let errors = m.validate();
        assert!(errors.iter().any(|e| e.field == "crate"));
    }

    #[test]
    fn validate_empty_version() {
        let m = ManifestBuilder::new("test", "").build();
        let errors = m.validate();
        assert!(errors.iter().any(|e| e.field == "version"));
    }

    #[test]
    fn validate_bad_semver() {
        let m = ManifestBuilder::new("test", "1.0").build();
        let errors = m.validate();
        assert!(errors.iter().any(|e| e.message.contains("semver")));
    }

    #[test]
    fn validate_empty_functions() {
        let m = ManifestBuilder::new("test", "1.0.0")
            .provide(
                "cap",
                ProvidedCapability {
                    functions: vec![],
                    safety: "safe".into(),
                    certifications: vec![],
                },
            )
            .build();
        let errors = m.validate();
        assert!(errors.iter().any(|e| e.field.contains("cap")));
    }

    #[test]
    fn validate_empty_contract() {
        let m = ManifestBuilder::new("test", "1.0.0")
            .contract(
                "func",
                FunctionContract {
                    requires: "".into(),
                    ensures: "".into(),
                    effects: vec![],
                },
            )
            .build();
        let errors = m.validate();
        assert!(errors.iter().any(|e| e.field.contains("contracts.func")));
    }

    #[test]
    fn validate_empty_platforms() {
        let m = ManifestBuilder::new("test", "1.0.0")
            .platforms(vec![])
            .build();
        let errors = m.validate();
        assert!(errors.iter().any(|e| e.field.contains("platforms")));
    }

    // -- Filename ----------------------------------------------------------

    #[test]
    fn manifest_filename() {
        assert_eq!(
            CapabilityManifest::filename("my_crate"),
            "my_crate.capability.json"
        );
    }

    // -- File I/O ----------------------------------------------------------

    #[test]
    fn write_and_read_file() {
        let m = sample_manifest();
        let dir = std::env::temp_dir().join("redox_manifest_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(CapabilityManifest::filename(&m.crate_name));
        m.write_to_file(&path).unwrap();
        let loaded = CapabilityManifest::from_file(&path).unwrap();
        assert_eq!(m, loaded);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -- Display for errors ------------------------------------------------

    #[test]
    fn parse_error_display() {
        assert_eq!(format!("{}", ParseError::UnexpectedEof), "unexpected end of input");
        assert!(format!("{}", ParseError::Expected("x".into())).contains("expected x"));
    }

    #[test]
    fn validation_error_display() {
        let e = ValidationError {
            field: "crate".into(),
            message: "empty".into(),
        };
        assert_eq!(format!("{e}"), "crate: empty");
    }
}

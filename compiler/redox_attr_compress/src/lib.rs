// Attribute compression system for the Redox language.
//
// Maps between Rust-style attributes (`#[derive(...)]`, `#[repr(...)]`, `#[test]`, etc.)
// and Redox compact forms (`@d(...)`, `@r(...)`, `@t`, etc.).
//
// All mappings are stored in the standard abbreviation registry and support
// deterministic round-trip conversion (compress ↔ expand).

use std::collections::BTreeMap;

// ── Abbreviation Registry ──────────────────────────────────────────────────

/// The abbreviation registry: stores all known compact ↔ expanded mappings.
pub struct AbbreviationRegistry {
    /// Attribute mappings: compact prefix → handler
    attr_entries: Vec<AttrMapping>,
    /// Derive trait abbreviations: short → full
    derive_abbrevs: BTreeMap<&'static str, &'static str>,
    /// Reverse derive: full → short
    derive_reverse: BTreeMap<&'static str, &'static str>,
    /// Repr abbreviations: short → full
    repr_abbrevs: BTreeMap<&'static str, &'static str>,
    /// Reverse repr: full → short
    repr_reverse: BTreeMap<&'static str, &'static str>,
    /// Lint/allow/deny/warn category abbreviations: short → full
    lint_abbrevs: BTreeMap<&'static str, &'static str>,
    /// Reverse lint: full → short
    lint_reverse: BTreeMap<&'static str, &'static str>,
    /// cfg abbreviations: short key → full key, short value → full value
    cfg_key_abbrevs: BTreeMap<&'static str, &'static str>,
    cfg_key_reverse: BTreeMap<&'static str, &'static str>,
    cfg_val_abbrevs: BTreeMap<&'static str, &'static str>,
    cfg_val_reverse: BTreeMap<&'static str, &'static str>,
}

/// A single attribute mapping entry.
struct AttrMapping {
    /// The compact form prefix (e.g., "@t", "@d", "@r")
    compact: &'static str,
    /// The expanded Rust form prefix (e.g., "#[test]", "#[derive(", "#[repr(")
    expanded: &'static str,
    /// Whether this attribute takes arguments
    has_args: bool,
}

impl AbbreviationRegistry {
    /// Create a new registry pre-populated with all standard abbreviations.
    pub fn new() -> Self {
        let mut reg = Self {
            attr_entries: Vec::new(),
            derive_abbrevs: BTreeMap::new(),
            derive_reverse: BTreeMap::new(),
            repr_abbrevs: BTreeMap::new(),
            repr_reverse: BTreeMap::new(),
            lint_abbrevs: BTreeMap::new(),
            lint_reverse: BTreeMap::new(),
            cfg_key_abbrevs: BTreeMap::new(),
            cfg_key_reverse: BTreeMap::new(),
            cfg_val_abbrevs: BTreeMap::new(),
            cfg_val_reverse: BTreeMap::new(),
        };
        reg.seed_standard();
        reg
    }

    fn seed_standard(&mut self) {
        // ── Simple attribute mappings (no args) ──
        self.attr_entries.push(AttrMapping { compact: "@t", expanded: "#[test]", has_args: false });
        self.attr_entries.push(AttrMapping { compact: "@b", expanded: "#[bench]", has_args: false });
        self.attr_entries.push(AttrMapping { compact: "@ta", expanded: "#[tokio::test]", has_args: false });
        self.attr_entries.push(AttrMapping { compact: "@mu", expanded: "#[must_use]", has_args: false });
        self.attr_entries.push(AttrMapping { compact: "@pi!", expanded: "#[perf::inline(always)]", has_args: false });
        self.attr_entries.push(AttrMapping { compact: "@pnb", expanded: "#[perf::no_bounds_check]", has_args: false });

        // ── Parametric attribute mappings ──
        self.attr_entries.push(AttrMapping { compact: "@d", expanded: "#[derive]", has_args: true });
        self.attr_entries.push(AttrMapping { compact: "@r", expanded: "#[repr]", has_args: true });
        self.attr_entries.push(AttrMapping { compact: "@a", expanded: "#[allow]", has_args: true });
        self.attr_entries.push(AttrMapping { compact: "@x", expanded: "#[deny]", has_args: true });
        self.attr_entries.push(AttrMapping { compact: "@w", expanded: "#[warn]", has_args: true });
        self.attr_entries.push(AttrMapping { compact: "@cfg", expanded: "#[cfg]", has_args: true });
        self.attr_entries.push(AttrMapping { compact: "@se", expanded: "#[serde]", has_args: true });
        self.attr_entries.push(AttrMapping { compact: "@pv", expanded: "#[perf::vectorize]", has_args: true });
        self.attr_entries.push(AttrMapping { compact: "@pt", expanded: "#[perf::target]", has_args: true });
        self.attr_entries.push(AttrMapping { compact: "@pa", expanded: "#[perf::autotune]", has_args: true });
        self.attr_entries.push(AttrMapping { compact: "@i", expanded: "#[inline]", has_args: false });

        // ── Derive trait abbreviations ──
        let derives: &[(&str, &str)] = &[
            ("Cl", "Clone"),
            ("Db", "Debug"),
            ("Disp", "Display"),
            ("Def", "Default"),
            ("PEq", "PartialEq"),
            ("Eq", "Eq"),
            ("POrd", "PartialOrd"),
            ("Ord", "Ord"),
            ("H", "Hash"),
            ("Cp", "Copy"),
            ("Ser", "Serialize"),
            ("De", "Deserialize"),
        ];
        for &(short, full) in derives {
            self.derive_abbrevs.insert(short, full);
            self.derive_reverse.insert(full, short);
        }

        // ── Repr abbreviations ──
        let reprs: &[(&str, &str)] = &[
            ("C", "C"),
            ("t", "transparent"),
            ("u8", "u8"),
            ("u16", "u16"),
            ("u32", "u32"),
            ("u64", "u64"),
            ("i8", "i8"),
            ("i16", "i16"),
            ("i32", "i32"),
            ("i64", "i64"),
            ("pk", "packed"),
            ("al", "align"),
        ];
        for &(short, full) in reprs {
            self.repr_abbrevs.insert(short, full);
            self.repr_reverse.insert(full, short);
        }

        // ── Lint category abbreviations ──
        let lints: &[(&str, &str)] = &[
            ("dc", "dead_code"),
            ("uc", "unsafe_code"),
            ("uu", "unused"),
            ("uv", "unused_variables"),
            ("ui", "unused_imports"),
            ("um", "unused_mut"),
            ("nr", "non_camel_case_types"),
            ("ns", "non_snake_case"),
            ("nu", "non_upper_case_globals"),
            ("dp", "deprecated"),
            ("mr", "missing_docs"),
        ];
        for &(short, full) in lints {
            self.lint_abbrevs.insert(short, full);
            self.lint_reverse.insert(full, short);
        }

        // ── cfg key abbreviations ──
        let cfg_keys: &[(&str, &str)] = &[
            ("os", "target_os"),
            ("arch", "target_arch"),
            ("f", "feature"),
            ("env", "target_env"),
            ("fam", "target_family"),
        ];
        for &(short, full) in cfg_keys {
            self.cfg_key_abbrevs.insert(short, full);
            self.cfg_key_reverse.insert(full, short);
        }

        // ── cfg value abbreviations ──
        let cfg_vals: &[(&str, &str)] = &[
            ("lx", "linux"),
            ("win", "windows"),
            ("mac", "macos"),
            ("fb", "freebsd"),
            ("and", "android"),
            ("ios", "ios"),
            ("wasm", "wasm32"),
            ("x64", "x86_64"),
            ("a64", "aarch64"),
            ("arm", "arm"),
        ];
        for &(short, full) in cfg_vals {
            self.cfg_val_abbrevs.insert(short, full);
            self.cfg_val_reverse.insert(full, short);
        }
    }

    /// Look up a compact attribute prefix. Returns the expanded form and whether it has args.
    pub fn lookup_compact(&self, prefix: &str) -> Option<(&str, bool)> {
        self.attr_entries.iter()
            .find(|e| e.compact == prefix)
            .map(|e| (e.expanded, e.has_args))
    }

    /// Look up an expanded attribute. Returns the compact form and whether it has args.
    pub fn lookup_expanded(&self, expanded: &str) -> Option<(&str, bool)> {
        self.attr_entries.iter()
            .find(|e| e.expanded == expanded)
            .map(|e| (e.compact, e.has_args))
    }

    /// Expand a derive trait abbreviation.
    pub fn expand_derive(&self, short: &str) -> Option<&str> {
        self.derive_abbrevs.get(short).copied()
    }

    /// Compress a derive trait name.
    pub fn compress_derive(&self, full: &str) -> Option<&str> {
        self.derive_reverse.get(full).copied()
    }

    /// Expand a repr abbreviation.
    pub fn expand_repr(&self, short: &str) -> Option<&str> {
        self.repr_abbrevs.get(short).copied()
    }

    /// Compress a repr name.
    pub fn compress_repr(&self, full: &str) -> Option<&str> {
        self.repr_reverse.get(full).copied()
    }

    /// Expand a lint category abbreviation.
    pub fn expand_lint(&self, short: &str) -> Option<&str> {
        self.lint_abbrevs.get(short).copied()
    }

    /// Compress a lint category name.
    pub fn compress_lint(&self, full: &str) -> Option<&str> {
        self.lint_reverse.get(full).copied()
    }

    /// Expand a cfg key abbreviation.
    pub fn expand_cfg_key(&self, short: &str) -> Option<&str> {
        self.cfg_key_abbrevs.get(short).copied()
    }

    /// Compress a cfg key.
    pub fn compress_cfg_key(&self, full: &str) -> Option<&str> {
        self.cfg_key_reverse.get(full).copied()
    }

    /// Expand a cfg value abbreviation.
    pub fn expand_cfg_val(&self, short: &str) -> Option<&str> {
        self.cfg_val_abbrevs.get(short).copied()
    }

    /// Compress a cfg value.
    pub fn compress_cfg_val(&self, full: &str) -> Option<&str> {
        self.cfg_val_reverse.get(full).copied()
    }

    /// Get total number of attribute mappings.
    pub fn attr_count(&self) -> usize {
        self.attr_entries.len()
    }

    /// Get total number of derive abbreviations.
    pub fn derive_count(&self) -> usize {
        self.derive_abbrevs.len()
    }

    /// List all compact attribute prefixes.
    pub fn all_compact_prefixes(&self) -> Vec<&str> {
        self.attr_entries.iter().map(|e| e.compact).collect()
    }

    /// List all derive abbreviations as (short, full) pairs.
    pub fn all_derive_abbrevs(&self) -> Vec<(&str, &str)> {
        self.derive_abbrevs.iter().map(|(&k, &v)| (k, v)).collect()
    }
}

impl Default for AbbreviationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Parsed Attribute ───────────────────────────────────────────────────────

/// A parsed attribute in either compact or expanded form.
#[derive(Debug, Clone, PartialEq)]
pub enum Attribute {
    /// `@d(Cl,Db)` or `#[derive(Clone, Debug)]`
    Derive(Vec<String>),
    /// `@r(C)` or `#[repr(C)]`
    Repr(Vec<String>),
    /// `#[test]` or `@t`
    Test,
    /// `#[bench]` or `@b`
    Bench,
    /// `#[tokio::test]` or `@ta`
    TokioTest,
    /// `#[inline]` or `@i`
    Inline,
    /// `#[must_use]` or `@mu`
    MustUse,
    /// `#[allow(...)]` or `@a(...)`
    Allow(Vec<String>),
    /// `#[deny(...)]` or `@x(...)`
    Deny(Vec<String>),
    /// `#[warn(...)]` or `@w(...)`
    Warn(Vec<String>),
    /// `#[cfg(...)]` or `@cfg(...)`
    Cfg(Vec<CfgPair>),
    /// `#[serde(...)]` or `@se(...)`
    Serde(Vec<KvPair>),
    /// `#[perf::inline(always)]` or `@pi!`
    PerfInlineAlways,
    /// `#[perf::no_bounds_check]` or `@pnb`
    PerfNoBoundsCheck,
    /// `#[perf::vectorize(width = N)]` or `@pv(N)`
    PerfVectorize(Vec<String>),
    /// `#[perf::target(T)]` or `@pt(T)`
    PerfTarget(Vec<String>),
    /// `#[perf::autotune(variants = N)]` or `@pa(N)`
    PerfAutotune(Vec<String>),
}

/// A cfg key=value pair.
#[derive(Debug, Clone, PartialEq)]
pub struct CfgPair {
    pub key: String,
    pub value: String,
}

/// A generic key=value pair.
#[derive(Debug, Clone, PartialEq)]
pub struct KvPair {
    pub key: String,
    pub value: String,
}

// ── Compression (Expanded → Compact) ──────────────────────────────────────

/// Compress a Rust-style attribute string to Redox compact form.
///
/// Examples:
/// - `#[derive(Clone, Debug)]` → `@d(Cl,Db)`
/// - `#[repr(C)]` → `@r(C)`
/// - `#[test]` → `@t`
/// - `#[inline]` → `@i`
/// - `#[allow(dead_code)]` → `@a(dc)`
/// - `#[cfg(target_os = "linux")]` → `@cfg(os=lx)`
pub fn compress(input: &str, registry: &AbbreviationRegistry) -> Result<String, CompressError> {
    let attr = parse_expanded(input)?;
    Ok(emit_compact(&attr, registry))
}

/// Expand a Redox compact attribute to Rust-style form.
///
/// Examples:
/// - `@d(Cl,Db)` → `#[derive(Clone, Debug)]`
/// - `@r(C)` → `#[repr(C)]`
/// - `@t` → `#[test]`
/// - `@i` → `#[inline]`
pub fn expand(input: &str, registry: &AbbreviationRegistry) -> Result<String, CompressError> {
    let attr = parse_compact(input)?;
    Ok(emit_expanded(&attr, registry))
}

/// Round-trip: compress then expand, should yield a normalized form of the original.
pub fn round_trip_compress(input: &str, registry: &AbbreviationRegistry) -> Result<String, CompressError> {
    let compact = compress(input, registry)?;
    expand(&compact, registry)
}

/// Round-trip: expand then compress, should yield a normalized form of the original.
pub fn round_trip_expand(input: &str, registry: &AbbreviationRegistry) -> Result<String, CompressError> {
    let expanded = expand(input, registry)?;
    compress(&expanded, registry)
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompressError {
    UnknownAttribute(String),
    UnknownDerive(String),
    UnknownRepr(String),
    UnknownLint(String),
    MalformedInput(String),
    UnclosedParenthesis,
}

impl std::fmt::Display for CompressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressError::UnknownAttribute(s) => write!(f, "unknown attribute: {s}"),
            CompressError::UnknownDerive(s) => write!(f, "unknown derive trait: {s}"),
            CompressError::UnknownRepr(s) => write!(f, "unknown repr: {s}"),
            CompressError::UnknownLint(s) => write!(f, "unknown lint category: {s}"),
            CompressError::MalformedInput(s) => write!(f, "malformed input: {s}"),
            CompressError::UnclosedParenthesis => write!(f, "unclosed parenthesis"),
        }
    }
}

// ── Parsing Expanded (Rust-style) ─────────────────────────────────────────

fn parse_expanded(input: &str) -> Result<Attribute, CompressError> {
    let input = input.trim();
    if !input.starts_with("#[") || !input.ends_with(']') {
        return Err(CompressError::MalformedInput(input.to_string()));
    }
    let inner = &input[2..input.len() - 1];

    // Simple no-arg attributes
    match inner {
        "test" => return Ok(Attribute::Test),
        "bench" => return Ok(Attribute::Bench),
        "tokio::test" => return Ok(Attribute::TokioTest),
        "inline" => return Ok(Attribute::Inline),
        "must_use" => return Ok(Attribute::MustUse),
        "perf::inline(always)" => return Ok(Attribute::PerfInlineAlways),
        "perf::no_bounds_check" => return Ok(Attribute::PerfNoBoundsCheck),
        _ => {}
    }

    // Parametric attributes: name(args)
    if let Some(paren_pos) = inner.find('(') {
        let name = &inner[..paren_pos];
        if !inner.ends_with(')') {
            return Err(CompressError::UnclosedParenthesis);
        }
        let args_str = &inner[paren_pos + 1..inner.len() - 1];

        match name {
            "derive" => {
                let traits: Vec<String> = split_args(args_str)
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .collect();
                Ok(Attribute::Derive(traits))
            }
            "repr" => {
                let args: Vec<String> = split_args(args_str)
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .collect();
                Ok(Attribute::Repr(args))
            }
            "allow" => {
                let cats: Vec<String> = split_args(args_str)
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .collect();
                Ok(Attribute::Allow(cats))
            }
            "deny" => {
                let cats: Vec<String> = split_args(args_str)
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .collect();
                Ok(Attribute::Deny(cats))
            }
            "warn" => {
                let cats: Vec<String> = split_args(args_str)
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .collect();
                Ok(Attribute::Warn(cats))
            }
            "cfg" => {
                let pairs = parse_cfg_args_expanded(args_str)?;
                Ok(Attribute::Cfg(pairs))
            }
            "serde" => {
                let pairs = parse_kv_args(args_str)?;
                Ok(Attribute::Serde(pairs))
            }
            "perf::vectorize" => {
                let args: Vec<String> = split_args(args_str)
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                // Handle `width = 8` form → extract just "8"
                let args = args.into_iter().map(|a| {
                    if let Some((_k, v)) = a.split_once('=') {
                        v.trim().to_string()
                    } else {
                        a
                    }
                }).collect();
                Ok(Attribute::PerfVectorize(args))
            }
            "perf::target" => {
                let args: Vec<String> = split_args(args_str)
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .collect();
                Ok(Attribute::PerfTarget(args))
            }
            "perf::autotune" => {
                let args: Vec<String> = split_args(args_str)
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                let args = args.into_iter().map(|a| {
                    if let Some((_k, v)) = a.split_once('=') {
                        v.trim().to_string()
                    } else {
                        a
                    }
                }).collect();
                Ok(Attribute::PerfAutotune(args))
            }
            _ => Err(CompressError::UnknownAttribute(name.to_string())),
        }
    } else {
        Err(CompressError::UnknownAttribute(inner.to_string()))
    }
}

fn parse_cfg_args_expanded(args_str: &str) -> Result<Vec<CfgPair>, CompressError> {
    let mut pairs = Vec::new();
    for part in split_args(args_str) {
        let part = part.trim();
        if let Some((key, val)) = part.split_once('=') {
            let key = key.trim().to_string();
            let val = val.trim().trim_matches('"').to_string();
            pairs.push(CfgPair { key, value: val });
        } else if !part.is_empty() {
            // Boolean cfg like `cfg(test)` or `cfg(unix)`
            pairs.push(CfgPair { key: part.to_string(), value: String::new() });
        }
    }
    Ok(pairs)
}

fn parse_kv_args(args_str: &str) -> Result<Vec<KvPair>, CompressError> {
    let mut pairs = Vec::new();
    for part in split_args(args_str) {
        let part = part.trim();
        if let Some((key, val)) = part.split_once('=') {
            let key = key.trim().to_string();
            let val = val.trim().trim_matches('"').to_string();
            pairs.push(KvPair { key, value: val });
        }
    }
    Ok(pairs)
}

// ── Parsing Compact (Redox-style) ─────────────────────────────────────────

fn parse_compact(input: &str) -> Result<Attribute, CompressError> {
    let input = input.trim();
    if !input.starts_with('@') {
        return Err(CompressError::MalformedInput(input.to_string()));
    }

    // Find the prefix and optional args
    let (prefix, args_str) = if let Some(paren_pos) = input.find('(') {
        if !input.ends_with(')') {
            return Err(CompressError::UnclosedParenthesis);
        }
        (&input[..paren_pos], Some(&input[paren_pos + 1..input.len() - 1]))
    } else {
        (input, None)
    };

    match prefix {
        "@t" => Ok(Attribute::Test),
        "@b" => Ok(Attribute::Bench),
        "@ta" => Ok(Attribute::TokioTest),
        "@i" => Ok(Attribute::Inline),
        "@mu" => Ok(Attribute::MustUse),
        "@pi!" => Ok(Attribute::PerfInlineAlways),
        "@pnb" => Ok(Attribute::PerfNoBoundsCheck),
        "@d" => {
            let args_str = args_str.ok_or(CompressError::MalformedInput("@d requires arguments".to_string()))?;
            let traits: Vec<String> = split_args(args_str)
                .into_iter()
                .map(|s| s.trim().to_string())
                .collect();
            Ok(Attribute::Derive(traits))
        }
        "@r" => {
            let args_str = args_str.ok_or(CompressError::MalformedInput("@r requires arguments".to_string()))?;
            let args: Vec<String> = split_args(args_str)
                .into_iter()
                .map(|s| s.trim().to_string())
                .collect();
            Ok(Attribute::Repr(args))
        }
        "@a" => {
            let args_str = args_str.ok_or(CompressError::MalformedInput("@a requires arguments".to_string()))?;
            let cats: Vec<String> = split_args(args_str)
                .into_iter()
                .map(|s| s.trim().to_string())
                .collect();
            Ok(Attribute::Allow(cats))
        }
        "@x" => {
            let args_str = args_str.ok_or(CompressError::MalformedInput("@x requires arguments".to_string()))?;
            let cats: Vec<String> = split_args(args_str)
                .into_iter()
                .map(|s| s.trim().to_string())
                .collect();
            Ok(Attribute::Deny(cats))
        }
        "@w" => {
            let args_str = args_str.ok_or(CompressError::MalformedInput("@w requires arguments".to_string()))?;
            let cats: Vec<String> = split_args(args_str)
                .into_iter()
                .map(|s| s.trim().to_string())
                .collect();
            Ok(Attribute::Warn(cats))
        }
        "@cfg" => {
            let args_str = args_str.ok_or(CompressError::MalformedInput("@cfg requires arguments".to_string()))?;
            let pairs = parse_cfg_args_compact(args_str)?;
            Ok(Attribute::Cfg(pairs))
        }
        "@se" => {
            let args_str = args_str.ok_or(CompressError::MalformedInput("@se requires arguments".to_string()))?;
            let pairs = parse_kv_args(args_str)?;
            Ok(Attribute::Serde(pairs))
        }
        "@pv" => {
            let args_str = args_str.ok_or(CompressError::MalformedInput("@pv requires arguments".to_string()))?;
            let args: Vec<String> = split_args(args_str)
                .into_iter()
                .map(|s| s.trim().to_string())
                .collect();
            Ok(Attribute::PerfVectorize(args))
        }
        "@pt" => {
            let args_str = args_str.ok_or(CompressError::MalformedInput("@pt requires arguments".to_string()))?;
            let args: Vec<String> = split_args(args_str)
                .into_iter()
                .map(|s| s.trim().to_string())
                .collect();
            Ok(Attribute::PerfTarget(args))
        }
        "@pa" => {
            let args_str = args_str.ok_or(CompressError::MalformedInput("@pa requires arguments".to_string()))?;
            let args: Vec<String> = split_args(args_str)
                .into_iter()
                .map(|s| s.trim().to_string())
                .collect();
            Ok(Attribute::PerfAutotune(args))
        }
        _ => Err(CompressError::UnknownAttribute(prefix.to_string())),
    }
}

fn parse_cfg_args_compact(args_str: &str) -> Result<Vec<CfgPair>, CompressError> {
    let mut pairs = Vec::new();
    for part in split_args(args_str) {
        let part = part.trim();
        if let Some((key, val)) = part.split_once('=') {
            pairs.push(CfgPair {
                key: key.trim().to_string(),
                value: val.trim().to_string(),
            });
        } else if !part.is_empty() {
            pairs.push(CfgPair { key: part.to_string(), value: String::new() });
        }
    }
    Ok(pairs)
}

// ── Emission ──────────────────────────────────────────────────────────────

fn emit_compact(attr: &Attribute, registry: &AbbreviationRegistry) -> String {
    match attr {
        Attribute::Test => "@t".to_string(),
        Attribute::Bench => "@b".to_string(),
        Attribute::TokioTest => "@ta".to_string(),
        Attribute::Inline => "@i".to_string(),
        Attribute::MustUse => "@mu".to_string(),
        Attribute::PerfInlineAlways => "@pi!".to_string(),
        Attribute::PerfNoBoundsCheck => "@pnb".to_string(),
        Attribute::Derive(traits) => {
            let abbrevs: Vec<String> = traits.iter().map(|t| {
                registry.compress_derive(t).unwrap_or(t.as_str()).to_string()
            }).collect();
            format!("@d({})", abbrevs.join(","))
        }
        Attribute::Repr(args) => {
            let abbrevs: Vec<String> = args.iter().map(|a| {
                registry.compress_repr(a).unwrap_or(a.as_str()).to_string()
            }).collect();
            format!("@r({})", abbrevs.join(","))
        }
        Attribute::Allow(cats) => {
            let abbrevs: Vec<String> = cats.iter().map(|c| {
                registry.compress_lint(c).unwrap_or(c.as_str()).to_string()
            }).collect();
            format!("@a({})", abbrevs.join(","))
        }
        Attribute::Deny(cats) => {
            let abbrevs: Vec<String> = cats.iter().map(|c| {
                registry.compress_lint(c).unwrap_or(c.as_str()).to_string()
            }).collect();
            format!("@x({})", abbrevs.join(","))
        }
        Attribute::Warn(cats) => {
            let abbrevs: Vec<String> = cats.iter().map(|c| {
                registry.compress_lint(c).unwrap_or(c.as_str()).to_string()
            }).collect();
            format!("@w({})", abbrevs.join(","))
        }
        Attribute::Cfg(pairs) => {
            let parts: Vec<String> = pairs.iter().map(|p| {
                let key = registry.compress_cfg_key(&p.key).unwrap_or(&p.key).to_string();
                if p.value.is_empty() {
                    key
                } else {
                    let val = registry.compress_cfg_val(&p.value).unwrap_or(&p.value).to_string();
                    format!("{key}={val}")
                }
            }).collect();
            format!("@cfg({})", parts.join(","))
        }
        Attribute::Serde(pairs) => {
            let parts: Vec<String> = pairs.iter().map(|p| {
                format!("{}={}", p.key, p.value)
            }).collect();
            format!("@se({})", parts.join(","))
        }
        Attribute::PerfVectorize(args) => format!("@pv({})", args.join(",")),
        Attribute::PerfTarget(args) => format!("@pt({})", args.join(",")),
        Attribute::PerfAutotune(args) => format!("@pa({})", args.join(",")),
    }
}

fn emit_expanded(attr: &Attribute, registry: &AbbreviationRegistry) -> String {
    match attr {
        Attribute::Test => "#[test]".to_string(),
        Attribute::Bench => "#[bench]".to_string(),
        Attribute::TokioTest => "#[tokio::test]".to_string(),
        Attribute::Inline => "#[inline]".to_string(),
        Attribute::MustUse => "#[must_use]".to_string(),
        Attribute::PerfInlineAlways => "#[perf::inline(always)]".to_string(),
        Attribute::PerfNoBoundsCheck => "#[perf::no_bounds_check]".to_string(),
        Attribute::Derive(traits) => {
            let expanded: Vec<String> = traits.iter().map(|t| {
                registry.expand_derive(t).unwrap_or(t.as_str()).to_string()
            }).collect();
            format!("#[derive({})]", expanded.join(", "))
        }
        Attribute::Repr(args) => {
            let expanded: Vec<String> = args.iter().map(|a| {
                registry.expand_repr(a).unwrap_or(a.as_str()).to_string()
            }).collect();
            format!("#[repr({})]", expanded.join(", "))
        }
        Attribute::Allow(cats) => {
            let expanded: Vec<String> = cats.iter().map(|c| {
                registry.expand_lint(c).unwrap_or(c.as_str()).to_string()
            }).collect();
            format!("#[allow({})]", expanded.join(", "))
        }
        Attribute::Deny(cats) => {
            let expanded: Vec<String> = cats.iter().map(|c| {
                registry.expand_lint(c).unwrap_or(c.as_str()).to_string()
            }).collect();
            format!("#[deny({})]", expanded.join(", "))
        }
        Attribute::Warn(cats) => {
            let expanded: Vec<String> = cats.iter().map(|c| {
                registry.expand_lint(c).unwrap_or(c.as_str()).to_string()
            }).collect();
            format!("#[warn({})]", expanded.join(", "))
        }
        Attribute::Cfg(pairs) => {
            let parts: Vec<String> = pairs.iter().map(|p| {
                let key = registry.expand_cfg_key(&p.key).unwrap_or(&p.key).to_string();
                if p.value.is_empty() {
                    key
                } else {
                    let val = registry.expand_cfg_val(&p.value).unwrap_or(&p.value).to_string();
                    format!("{key} = \"{val}\"")
                }
            }).collect();
            format!("#[cfg({})]", parts.join(", "))
        }
        Attribute::Serde(pairs) => {
            let parts: Vec<String> = pairs.iter().map(|p| {
                format!("{} = \"{}\"", p.key, p.value)
            }).collect();
            format!("#[serde({})]", parts.join(", "))
        }
        Attribute::PerfVectorize(args) => {
            if args.len() == 1 {
                format!("#[perf::vectorize(width = {})]", args[0])
            } else {
                format!("#[perf::vectorize({})]", args.join(", "))
            }
        }
        Attribute::PerfTarget(args) => {
            format!("#[perf::target({})]", args.join(", "))
        }
        Attribute::PerfAutotune(args) => {
            if args.len() == 1 {
                format!("#[perf::autotune(variants = {})]", args[0])
            } else {
                format!("#[perf::autotune({})]", args.join(", "))
            }
        }
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────

/// Split comma-separated args respecting nested parentheses and quotes.
fn split_args(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let mut in_quote = false;

    for ch in input.chars() {
        match ch {
            '"' if depth == 0 => {
                in_quote = !in_quote;
                current.push(ch);
            }
            '(' if !in_quote => {
                depth += 1;
                current.push(ch);
            }
            ')' if !in_quote => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 && !in_quote => {
                parts.push(current.clone());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

// ── Batch Operations ──────────────────────────────────────────────────────

/// Compress multiple attributes at once.
pub fn compress_all(inputs: &[&str], registry: &AbbreviationRegistry) -> Vec<Result<String, CompressError>> {
    inputs.iter().map(|input| compress(input, registry)).collect()
}

/// Expand multiple compact attributes at once.
pub fn expand_all(inputs: &[&str], registry: &AbbreviationRegistry) -> Vec<Result<String, CompressError>> {
    inputs.iter().map(|input| expand(input, registry)).collect()
}

/// Compute the token savings ratio for a compress operation.
/// Returns (expanded_tokens, compact_tokens, savings_percent).
pub fn token_savings(expanded: &str, compact: &str) -> (usize, usize, f64) {
    let exp_tokens = count_tokens(expanded);
    let comp_tokens = count_tokens(compact);
    let savings = if exp_tokens > 0 {
        ((exp_tokens - comp_tokens) as f64 / exp_tokens as f64) * 100.0
    } else {
        0.0
    };
    (exp_tokens, comp_tokens, savings)
}

/// Simple token counter: splits on whitespace, punctuation boundaries.
fn count_tokens(input: &str) -> usize {
    let mut count = 0;
    let mut in_word = false;
    for ch in input.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            if !in_word {
                count += 1;
                in_word = true;
            }
        } else {
            in_word = false;
            if !ch.is_whitespace() {
                count += 1; // punctuation is a token
            }
        }
    }
    count
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn reg() -> AbbreviationRegistry {
        AbbreviationRegistry::new()
    }

    // ── Registry tests ──

    #[test]
    fn test_registry_creation() {
        let r = reg();
        assert!(r.attr_count() > 0);
        assert!(r.derive_count() > 0);
    }

    #[test]
    fn test_registry_derive_lookups() {
        let r = reg();
        assert_eq!(r.expand_derive("Cl"), Some("Clone"));
        assert_eq!(r.expand_derive("Db"), Some("Debug"));
        assert_eq!(r.expand_derive("PEq"), Some("PartialEq"));
        assert_eq!(r.compress_derive("Clone"), Some("Cl"));
        assert_eq!(r.compress_derive("Debug"), Some("Db"));
        assert_eq!(r.compress_derive("Hash"), Some("H"));
    }

    #[test]
    fn test_registry_repr_lookups() {
        let r = reg();
        assert_eq!(r.expand_repr("C"), Some("C"));
        assert_eq!(r.expand_repr("t"), Some("transparent"));
        assert_eq!(r.compress_repr("transparent"), Some("t"));
        assert_eq!(r.compress_repr("packed"), Some("pk"));
    }

    #[test]
    fn test_registry_lint_lookups() {
        let r = reg();
        assert_eq!(r.expand_lint("dc"), Some("dead_code"));
        assert_eq!(r.expand_lint("uc"), Some("unsafe_code"));
        assert_eq!(r.compress_lint("dead_code"), Some("dc"));
        assert_eq!(r.compress_lint("unused_imports"), Some("ui"));
    }

    #[test]
    fn test_registry_cfg_lookups() {
        let r = reg();
        assert_eq!(r.expand_cfg_key("os"), Some("target_os"));
        assert_eq!(r.compress_cfg_key("target_os"), Some("os"));
        assert_eq!(r.expand_cfg_val("lx"), Some("linux"));
        assert_eq!(r.compress_cfg_val("windows"), Some("win"));
    }

    #[test]
    fn test_registry_all_compact_prefixes() {
        let r = reg();
        let prefixes = r.all_compact_prefixes();
        assert!(prefixes.contains(&"@t"));
        assert!(prefixes.contains(&"@d"));
        assert!(prefixes.contains(&"@r"));
        assert!(prefixes.contains(&"@i"));
    }

    // ── Compression tests (expanded → compact) ──

    #[test]
    fn test_compress_derive_simple() {
        let r = reg();
        assert_eq!(compress("#[derive(Clone, Debug)]", &r).unwrap(), "@d(Cl,Db)");
    }

    #[test]
    fn test_compress_derive_multiple() {
        let r = reg();
        assert_eq!(
            compress("#[derive(Clone, Debug, PartialEq)]", &r).unwrap(),
            "@d(Cl,Db,PEq)"
        );
    }

    #[test]
    fn test_compress_repr_c() {
        let r = reg();
        assert_eq!(compress("#[repr(C)]", &r).unwrap(), "@r(C)");
    }

    #[test]
    fn test_compress_repr_transparent() {
        let r = reg();
        assert_eq!(compress("#[repr(transparent)]", &r).unwrap(), "@r(t)");
    }

    #[test]
    fn test_compress_test() {
        let r = reg();
        assert_eq!(compress("#[test]", &r).unwrap(), "@t");
    }

    #[test]
    fn test_compress_bench() {
        let r = reg();
        assert_eq!(compress("#[bench]", &r).unwrap(), "@b");
    }

    #[test]
    fn test_compress_inline() {
        let r = reg();
        assert_eq!(compress("#[inline]", &r).unwrap(), "@i");
    }

    #[test]
    fn test_compress_must_use() {
        let r = reg();
        assert_eq!(compress("#[must_use]", &r).unwrap(), "@mu");
    }

    #[test]
    fn test_compress_allow() {
        let r = reg();
        assert_eq!(compress("#[allow(dead_code)]", &r).unwrap(), "@a(dc)");
    }

    #[test]
    fn test_compress_deny() {
        let r = reg();
        assert_eq!(compress("#[deny(unsafe_code)]", &r).unwrap(), "@x(uc)");
    }

    #[test]
    fn test_compress_cfg_target_os() {
        let r = reg();
        assert_eq!(
            compress("#[cfg(target_os = \"linux\")]", &r).unwrap(),
            "@cfg(os=lx)"
        );
    }

    #[test]
    fn test_compress_cfg_feature() {
        let r = reg();
        assert_eq!(
            compress("#[cfg(feature = \"serde\")]", &r).unwrap(),
            "@cfg(f=serde)"
        );
    }

    #[test]
    fn test_compress_serde() {
        let r = reg();
        assert_eq!(
            compress("#[serde(rename_all = \"camelCase\")]", &r).unwrap(),
            "@se(rename_all=camelCase)"
        );
    }

    #[test]
    fn test_compress_perf_vectorize() {
        let r = reg();
        assert_eq!(
            compress("#[perf::vectorize(width = 8)]", &r).unwrap(),
            "@pv(8)"
        );
    }

    #[test]
    fn test_compress_perf_target() {
        let r = reg();
        assert_eq!(compress("#[perf::target(gpu)]", &r).unwrap(), "@pt(gpu)");
    }

    #[test]
    fn test_compress_perf_inline_always() {
        let r = reg();
        assert_eq!(compress("#[perf::inline(always)]", &r).unwrap(), "@pi!");
    }

    // ── Expansion tests (compact → expanded) ──

    #[test]
    fn test_expand_derive() {
        let r = reg();
        assert_eq!(expand("@d(Cl,Db)", &r).unwrap(), "#[derive(Clone, Debug)]");
    }

    #[test]
    fn test_expand_repr_c() {
        let r = reg();
        assert_eq!(expand("@r(C)", &r).unwrap(), "#[repr(C)]");
    }

    #[test]
    fn test_expand_repr_transparent() {
        let r = reg();
        assert_eq!(expand("@r(t)", &r).unwrap(), "#[repr(transparent)]");
    }

    #[test]
    fn test_expand_test() {
        let r = reg();
        assert_eq!(expand("@t", &r).unwrap(), "#[test]");
    }

    #[test]
    fn test_expand_inline() {
        let r = reg();
        assert_eq!(expand("@i", &r).unwrap(), "#[inline]");
    }

    #[test]
    fn test_expand_allow() {
        let r = reg();
        assert_eq!(expand("@a(dc)", &r).unwrap(), "#[allow(dead_code)]");
    }

    #[test]
    fn test_expand_cfg() {
        let r = reg();
        assert_eq!(
            expand("@cfg(os=lx)", &r).unwrap(),
            "#[cfg(target_os = \"linux\")]"
        );
    }

    // ── Round-trip tests ──

    #[test]
    fn test_roundtrip_compress_derive() {
        let r = reg();
        let original = "#[derive(Clone, Debug, PartialEq)]";
        let result = round_trip_compress(original, &r).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_roundtrip_compress_repr() {
        let r = reg();
        let original = "#[repr(transparent)]";
        let result = round_trip_compress(original, &r).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_roundtrip_compress_test() {
        let r = reg();
        let original = "#[test]";
        let result = round_trip_compress(original, &r).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_roundtrip_expand_derive() {
        let r = reg();
        let original = "@d(Cl,Db,PEq)";
        let result = round_trip_expand(original, &r).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_roundtrip_expand_cfg() {
        let r = reg();
        let original = "@cfg(os=lx)";
        let result = round_trip_expand(original, &r).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_roundtrip_expand_allow() {
        let r = reg();
        let original = "@a(dc)";
        let result = round_trip_expand(original, &r).unwrap();
        assert_eq!(result, original);
    }

    // ── Token savings ──

    #[test]
    fn test_token_savings() {
        let r = reg();
        let expanded = "#[derive(Clone, Debug)]";
        let compact = compress(expanded, &r).unwrap();
        let (exp_tok, comp_tok, savings) = token_savings(expanded, &compact);
        assert!(exp_tok > comp_tok);
        assert!(savings > 0.0);
    }

    // ── Batch operations ──

    #[test]
    fn test_compress_all() {
        let r = reg();
        let inputs = vec!["#[test]", "#[inline]", "#[derive(Clone)]"];
        let results = compress_all(&inputs, &r);
        assert_eq!(results[0].as_ref().unwrap(), "@t");
        assert_eq!(results[1].as_ref().unwrap(), "@i");
        assert_eq!(results[2].as_ref().unwrap(), "@d(Cl)");
    }

    #[test]
    fn test_expand_all() {
        let r = reg();
        let inputs = vec!["@t", "@i", "@d(Cl,Db)"];
        let results = expand_all(&inputs, &r);
        assert_eq!(results[0].as_ref().unwrap(), "#[test]");
        assert_eq!(results[1].as_ref().unwrap(), "#[inline]");
        assert_eq!(results[2].as_ref().unwrap(), "#[derive(Clone, Debug)]");
    }

    // ── Error handling ──

    #[test]
    fn test_error_unknown_attribute() {
        let r = reg();
        assert!(compress("#[foobar]", &r).is_err());
    }

    #[test]
    fn test_error_malformed_input() {
        let r = reg();
        assert!(compress("not_an_attribute", &r).is_err());
    }

    #[test]
    fn test_error_unclosed_paren() {
        let r = reg();
        assert!(compress("#[derive(Clone", &r).is_err());
    }

    #[test]
    fn test_error_unknown_compact() {
        let r = reg();
        assert!(expand("@zzz", &r).is_err());
    }

    // ── Edge cases ──

    #[test]
    fn test_derive_unknown_trait_passthrough() {
        let r = reg();
        // Unknown derive traits pass through unabbreviated
        assert_eq!(compress("#[derive(MyCustomTrait)]", &r).unwrap(), "@d(MyCustomTrait)");
        assert_eq!(expand("@d(MyCustomTrait)", &r).unwrap(), "#[derive(MyCustomTrait)]");
    }

    #[test]
    fn test_cfg_boolean_flag() {
        let r = reg();
        assert_eq!(compress("#[cfg(test)]", &r).unwrap(), "@cfg(test)");
        assert_eq!(expand("@cfg(test)", &r).unwrap(), "#[cfg(test)]");
    }

    #[test]
    fn test_perf_autotune() {
        let r = reg();
        assert_eq!(
            compress("#[perf::autotune(variants = 4)]", &r).unwrap(),
            "@pa(4)"
        );
        assert_eq!(
            expand("@pa(4)", &r).unwrap(),
            "#[perf::autotune(variants = 4)]"
        );
    }

    #[test]
    fn test_tokio_test() {
        let r = reg();
        assert_eq!(compress("#[tokio::test]", &r).unwrap(), "@ta");
        assert_eq!(expand("@ta", &r).unwrap(), "#[tokio::test]");
    }

    #[test]
    fn test_warn_attribute() {
        let r = reg();
        assert_eq!(compress("#[warn(unused)]", &r).unwrap(), "@w(uu)");
        assert_eq!(expand("@w(uu)", &r).unwrap(), "#[warn(unused)]");
    }

    #[test]
    fn test_all_derive_abbrevs_listed() {
        let r = reg();
        let abbrevs = r.all_derive_abbrevs();
        assert!(abbrevs.len() >= 12);
        // Check a few
        assert!(abbrevs.iter().any(|&(s, f)| s == "Cl" && f == "Clone"));
        assert!(abbrevs.iter().any(|&(s, f)| s == "Ser" && f == "Serialize"));
    }

    #[test]
    fn test_proposal_derive_example() {
        // From REDOX_PROPOSAL.md: #[derive(Clone, Debug, PartialEq)] → @d(Cl,Db,PEq)
        let r = reg();
        assert_eq!(
            compress("#[derive(Clone, Debug, PartialEq)]", &r).unwrap(),
            "@d(Cl,Db,PEq)"
        );
        assert_eq!(
            expand("@d(Cl,Db,PEq)", &r).unwrap(),
            "#[derive(Clone, Debug, PartialEq)]"
        );
    }
}

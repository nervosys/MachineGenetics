//! Redox.toml project configuration file parsing.
//!
//! Defines the `RedoxConfig` type and a simple parser for the `[safety]` section
//! of a `Redox.toml` file. Per the Redox proposal §9.4, safety checking at
//! compile time is configurable per-project with the following settings:
//!
//! ```toml
//! [safety]
//! mode = "agent"              # "agent" | "human" | "ci"
//! borrow-check = "skip"       # "skip" | "warn" | "error"
//! lifetime-check = "skip"     # "skip" | "warn" | "error"
//! bounds-check = "skip"       # "skip" | "warn" | "error"
//! overflow-check = "skip"     # "skip" | "warn" | "error"
//! ```

use std::fmt;
use std::fs;
use std::path::Path;

/// The safety mode for the project — determines the overall safety posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SafetyMode {
    /// Agent mode: all safety checks skipped by default (agents pre-validate via SKB).
    Agent,
    /// Human mode: all safety checks enforced (traditional compiler-enforced safety).
    #[default]
    Human,
    /// CI mode: all safety checks enforced (same as human mode, for CI pipelines).
    Ci,
}

impl fmt::Display for SafetyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SafetyMode::Agent => write!(f, "agent"),
            SafetyMode::Human => write!(f, "human"),
            SafetyMode::Ci => write!(f, "ci"),
        }
    }
}

impl SafetyMode {
    fn from_str(s: &str) -> Result<Self, ConfigError> {
        match s {
            "agent" => Ok(SafetyMode::Agent),
            "human" => Ok(SafetyMode::Human),
            "ci" => Ok(SafetyMode::Ci),
            _ => Err(ConfigError::InvalidValue {
                key: "mode".to_string(),
                value: s.to_string(),
                expected: r#""agent", "human", or "ci""#.to_string(),
            }),
        }
    }
}

/// A single safety check level — determines how the compiler treats a particular check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CheckLevel {
    /// Skip the check entirely.
    Skip,
    /// Emit a warning but allow compilation to continue.
    Warn,
    /// Emit an error and block compilation.
    #[default]
    Error,
}

impl fmt::Display for CheckLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckLevel::Skip => write!(f, "skip"),
            CheckLevel::Warn => write!(f, "warn"),
            CheckLevel::Error => write!(f, "error"),
        }
    }
}

impl CheckLevel {
    fn from_str(s: &str, key: &str) -> Result<Self, ConfigError> {
        match s {
            "skip" => Ok(CheckLevel::Skip),
            "warn" => Ok(CheckLevel::Warn),
            "error" => Ok(CheckLevel::Error),
            _ => Err(ConfigError::InvalidValue {
                key: key.to_string(),
                value: s.to_string(),
                expected: r#""skip", "warn", or "error""#.to_string(),
            }),
        }
    }
}

/// The `[safety]` section of `Redox.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafetyConfig {
    pub mode: SafetyMode,
    pub borrow_check: CheckLevel,
    pub lifetime_check: CheckLevel,
    pub bounds_check: CheckLevel,
    pub overflow_check: CheckLevel,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        SafetyConfig {
            mode: SafetyMode::Human,
            borrow_check: CheckLevel::Error,
            lifetime_check: CheckLevel::Error,
            bounds_check: CheckLevel::Error,
            overflow_check: CheckLevel::Error,
        }
    }
}

impl SafetyConfig {
    /// Create a config with defaults determined by the safety mode.
    pub fn with_mode(mode: SafetyMode) -> Self {
        match mode {
            SafetyMode::Agent => SafetyConfig {
                mode,
                borrow_check: CheckLevel::Skip,
                lifetime_check: CheckLevel::Skip,
                bounds_check: CheckLevel::Skip,
                overflow_check: CheckLevel::Skip,
            },
            SafetyMode::Human | SafetyMode::Ci => SafetyConfig {
                mode,
                borrow_check: CheckLevel::Error,
                lifetime_check: CheckLevel::Error,
                bounds_check: CheckLevel::Error,
                overflow_check: CheckLevel::Error,
            },
        }
    }
}

/// The top-level Redox.toml configuration.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RedoxConfig {
    pub safety: SafetyConfig,
}

/// Errors that can occur when parsing a `Redox.toml` file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// Invalid value for a configuration key.
    InvalidValue { key: String, value: String, expected: String },
    /// Unknown key encountered in a section.
    UnknownKey { section: String, key: String },
    /// I/O error reading the file.
    IoError(String),
    /// Syntax error in the TOML file.
    SyntaxError { line: usize, message: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::InvalidValue { key, value, expected } => {
                write!(f, "invalid value `{value}` for key `{key}` (expected {expected})")
            }
            ConfigError::UnknownKey { section, key } => {
                write!(f, "unknown key `{key}` in section `[{section}]`")
            }
            ConfigError::IoError(msg) => write!(f, "I/O error: {msg}"),
            ConfigError::SyntaxError { line, message } => {
                write!(f, "syntax error on line {line}: {message}")
            }
        }
    }
}

/// Parse a `Redox.toml` file from the given path.
pub fn parse_redox_config(path: &Path) -> Result<RedoxConfig, ConfigError> {
    let contents =
        fs::read_to_string(path).map_err(|e| ConfigError::IoError(e.to_string()))?;
    parse_redox_config_str(&contents)
}

/// Parse a `Redox.toml` from a string (for testing and programmatic use).
pub fn parse_redox_config_str(input: &str) -> Result<RedoxConfig, ConfigError> {
    let mut config = RedoxConfig::default();
    let mut current_section: Option<&str> = None;

    for (line_idx, raw_line) in input.lines().enumerate() {
        let line_num = line_idx + 1;
        // Strip comments (# ...) and trim whitespace.
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        // Section header: [section_name]
        if line.starts_with('[') {
            if !line.ends_with(']') {
                return Err(ConfigError::SyntaxError {
                    line: line_num,
                    message: "unclosed section header".to_string(),
                });
            }
            let section_name = line[1..line.len() - 1].trim();
            current_section = Some(match section_name {
                "safety" => "safety",
                // Other sections are silently ignored for forward-compatibility.
                _ => "unknown",
            });
            continue;
        }

        // Key-value pair: key = "value" or key = value
        let Some(eq_pos) = line.find('=') else {
            return Err(ConfigError::SyntaxError {
                line: line_num,
                message: format!("expected key = value, found: {line}"),
            });
        };

        let key = line[..eq_pos].trim();
        let raw_value = line[eq_pos + 1..].trim();
        let value = unquote(raw_value);

        match current_section {
            Some("safety") => {
                match key {
                    "mode" => config.safety.mode = SafetyMode::from_str(value)?,
                    "borrow-check" => {
                        config.safety.borrow_check = CheckLevel::from_str(value, key)?
                    }
                    "lifetime-check" => {
                        config.safety.lifetime_check = CheckLevel::from_str(value, key)?
                    }
                    "bounds-check" => {
                        config.safety.bounds_check = CheckLevel::from_str(value, key)?
                    }
                    "overflow-check" => {
                        config.safety.overflow_check = CheckLevel::from_str(value, key)?
                    }
                    _ => {
                        return Err(ConfigError::UnknownKey {
                            section: "safety".to_string(),
                            key: key.to_string(),
                        });
                    }
                }
            }
            // Lines outside [safety] or in unknown sections are silently ignored.
            _ => {}
        }
    }

    Ok(config)
}

/// Strip a `# comment` from a line, respecting quoted strings.
fn strip_comment(line: &str) -> &str {
    let mut in_quote = false;
    for (i, ch) in line.char_indices() {
        if ch == '"' {
            in_quote = !in_quote;
        } else if ch == '#' && !in_quote {
            return &line[..i];
        }
    }
    line
}

/// Remove surrounding double-quotes from a value, if present.
fn unquote(s: &str) -> &str {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_human_mode() {
        let config = RedoxConfig::default();
        assert_eq!(config.safety.mode, SafetyMode::Human);
        assert_eq!(config.safety.borrow_check, CheckLevel::Error);
        assert_eq!(config.safety.lifetime_check, CheckLevel::Error);
        assert_eq!(config.safety.bounds_check, CheckLevel::Error);
        assert_eq!(config.safety.overflow_check, CheckLevel::Error);
    }

    #[test]
    fn parse_agent_mode() {
        let input = r#"
[safety]
mode = "agent"
borrow-check = "skip"
lifetime-check = "skip"
bounds-check = "skip"
overflow-check = "skip"
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Agent);
        assert_eq!(config.safety.borrow_check, CheckLevel::Skip);
        assert_eq!(config.safety.lifetime_check, CheckLevel::Skip);
        assert_eq!(config.safety.bounds_check, CheckLevel::Skip);
        assert_eq!(config.safety.overflow_check, CheckLevel::Skip);
    }

    #[test]
    fn parse_human_mode() {
        let input = r#"
[safety]
mode = "human"
borrow-check = "error"
lifetime-check = "error"
bounds-check = "warn"
overflow-check = "error"
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Human);
        assert_eq!(config.safety.borrow_check, CheckLevel::Error);
        assert_eq!(config.safety.lifetime_check, CheckLevel::Error);
        assert_eq!(config.safety.bounds_check, CheckLevel::Warn);
        assert_eq!(config.safety.overflow_check, CheckLevel::Error);
    }

    #[test]
    fn parse_ci_mode() {
        let input = r#"
[safety]
mode = "ci"
borrow-check = "error"
lifetime-check = "error"
bounds-check = "error"
overflow-check = "error"
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Ci);
        assert_eq!(config.safety.borrow_check, CheckLevel::Error);
    }

    #[test]
    fn parse_mixed_check_levels() {
        let input = r#"
[safety]
mode = "agent"
borrow-check = "warn"
lifetime-check = "error"
bounds-check = "skip"
overflow-check = "warn"
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Agent);
        assert_eq!(config.safety.borrow_check, CheckLevel::Warn);
        assert_eq!(config.safety.lifetime_check, CheckLevel::Error);
        assert_eq!(config.safety.bounds_check, CheckLevel::Skip);
        assert_eq!(config.safety.overflow_check, CheckLevel::Warn);
    }

    #[test]
    fn parse_with_comments() {
        let input = r#"
# This is a Redox.toml file
[safety]
mode = "agent"     # agentic mode
borrow-check = "skip"   # agents pre-validate
lifetime-check = "skip"
bounds-check = "skip"
overflow-check = "skip"
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Agent);
        assert_eq!(config.safety.borrow_check, CheckLevel::Skip);
    }

    #[test]
    fn parse_empty_input() {
        let config = parse_redox_config_str("").unwrap();
        assert_eq!(config, RedoxConfig::default());
    }

    #[test]
    fn parse_unknown_section_ignored() {
        let input = r#"
[package]
name = "my-project"
version = "0.1.0"

[safety]
mode = "agent"
borrow-check = "skip"
lifetime-check = "skip"
bounds-check = "skip"
overflow-check = "skip"
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Agent);
    }

    #[test]
    fn parse_invalid_mode() {
        let input = r#"
[safety]
mode = "turbo"
"#;
        let err = parse_redox_config_str(input).unwrap_err();
        match err {
            ConfigError::InvalidValue { key, value, .. } => {
                assert_eq!(key, "mode");
                assert_eq!(value, "turbo");
            }
            _ => panic!("expected InvalidValue, got {err:?}"),
        }
    }

    #[test]
    fn parse_invalid_check_level() {
        let input = r#"
[safety]
mode = "human"
borrow-check = "ignore"
"#;
        let err = parse_redox_config_str(input).unwrap_err();
        match err {
            ConfigError::InvalidValue { key, value, .. } => {
                assert_eq!(key, "borrow-check");
                assert_eq!(value, "ignore");
            }
            _ => panic!("expected InvalidValue, got {err:?}"),
        }
    }

    #[test]
    fn parse_unknown_key_in_safety() {
        let input = r#"
[safety]
mode = "human"
turbo-check = "error"
"#;
        let err = parse_redox_config_str(input).unwrap_err();
        match err {
            ConfigError::UnknownKey { section, key } => {
                assert_eq!(section, "safety");
                assert_eq!(key, "turbo-check");
            }
            _ => panic!("expected UnknownKey, got {err:?}"),
        }
    }

    #[test]
    fn parse_unclosed_section_header() {
        let input = "[safety\nmode = \"human\"\n";
        let err = parse_redox_config_str(input).unwrap_err();
        match err {
            ConfigError::SyntaxError { line, .. } => assert_eq!(line, 1),
            _ => panic!("expected SyntaxError, got {err:?}"),
        }
    }

    #[test]
    fn parse_missing_equals() {
        let input = "[safety]\nmode \"human\"\n";
        let err = parse_redox_config_str(input).unwrap_err();
        match err {
            ConfigError::SyntaxError { line, .. } => assert_eq!(line, 2),
            _ => panic!("expected SyntaxError, got {err:?}"),
        }
    }

    #[test]
    fn safety_config_with_mode_agent() {
        let config = SafetyConfig::with_mode(SafetyMode::Agent);
        assert_eq!(config.borrow_check, CheckLevel::Skip);
        assert_eq!(config.lifetime_check, CheckLevel::Skip);
        assert_eq!(config.bounds_check, CheckLevel::Skip);
        assert_eq!(config.overflow_check, CheckLevel::Skip);
    }

    #[test]
    fn safety_config_with_mode_human() {
        let config = SafetyConfig::with_mode(SafetyMode::Human);
        assert_eq!(config.borrow_check, CheckLevel::Error);
        assert_eq!(config.lifetime_check, CheckLevel::Error);
        assert_eq!(config.bounds_check, CheckLevel::Error);
        assert_eq!(config.overflow_check, CheckLevel::Error);
    }

    #[test]
    fn safety_config_with_mode_ci() {
        let config = SafetyConfig::with_mode(SafetyMode::Ci);
        assert_eq!(config.borrow_check, CheckLevel::Error);
        assert_eq!(config.lifetime_check, CheckLevel::Error);
    }

    #[test]
    fn display_safety_mode() {
        assert_eq!(SafetyMode::Agent.to_string(), "agent");
        assert_eq!(SafetyMode::Human.to_string(), "human");
        assert_eq!(SafetyMode::Ci.to_string(), "ci");
    }

    #[test]
    fn display_check_level() {
        assert_eq!(CheckLevel::Skip.to_string(), "skip");
        assert_eq!(CheckLevel::Warn.to_string(), "warn");
        assert_eq!(CheckLevel::Error.to_string(), "error");
    }

    #[test]
    fn display_config_error() {
        let err = ConfigError::InvalidValue {
            key: "mode".to_string(),
            value: "bad".to_string(),
            expected: "\"agent\"".to_string(),
        };
        assert!(err.to_string().contains("invalid value"));

        let err = ConfigError::UnknownKey {
            section: "safety".to_string(),
            key: "foo".to_string(),
        };
        assert!(err.to_string().contains("unknown key"));

        let err = ConfigError::IoError("not found".to_string());
        assert!(err.to_string().contains("I/O error"));

        let err = ConfigError::SyntaxError { line: 5, message: "bad".to_string() };
        assert!(err.to_string().contains("line 5"));
    }

    #[test]
    fn parse_unquoted_values() {
        let input = r#"
[safety]
mode = agent
borrow-check = skip
lifetime-check = warn
bounds-check = error
overflow-check = skip
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Agent);
        assert_eq!(config.safety.borrow_check, CheckLevel::Skip);
        assert_eq!(config.safety.lifetime_check, CheckLevel::Warn);
        assert_eq!(config.safety.bounds_check, CheckLevel::Error);
        assert_eq!(config.safety.overflow_check, CheckLevel::Skip);
    }

    #[test]
    fn parse_partial_safety_section() {
        // Only mode specified — other fields keep defaults
        let input = r#"
[safety]
mode = "agent"
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Agent);
        // Defaults remain (Human defaults = Error)
        assert_eq!(config.safety.borrow_check, CheckLevel::Error);
    }

    #[test]
    fn strip_comment_respects_quotes() {
        assert_eq!(strip_comment(r#"mode = "agent" # comment"#), r#"mode = "agent" "#);
        assert_eq!(strip_comment(r#"# full comment"#), "");
        assert_eq!(strip_comment(r#"no comment"#), "no comment");
        assert_eq!(strip_comment(r#"value = "has # inside""#), r#"value = "has # inside""#);
    }

    #[test]
    fn agent_mode_elides_all_safety_checks() {
        let input = r#"
[safety]
mode = "agent"
borrow-check = "skip"
lifetime-check = "skip"
bounds-check = "skip"
overflow-check = "skip"
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Agent);
        // In agent mode, all checks should be skip — this drives safety elision
        // in the lowering pass (unsafe blocks stripped, lifetimes elided).
        assert_eq!(config.safety.borrow_check, CheckLevel::Skip);
        assert_eq!(config.safety.lifetime_check, CheckLevel::Skip);
        assert_eq!(config.safety.bounds_check, CheckLevel::Skip);
        assert_eq!(config.safety.overflow_check, CheckLevel::Skip);
    }

    #[test]
    fn human_mode_enforces_all_safety_checks() {
        let input = r#"
[safety]
mode = "human"
borrow-check = "error"
lifetime-check = "error"
bounds-check = "error"
overflow-check = "error"
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Human);
        // In human mode, all checks enforced — lowering preserves unsafe/lifetimes.
        assert_eq!(config.safety.borrow_check, CheckLevel::Error);
        assert_eq!(config.safety.lifetime_check, CheckLevel::Error);
        assert_eq!(config.safety.bounds_check, CheckLevel::Error);
        assert_eq!(config.safety.overflow_check, CheckLevel::Error);
    }

    #[test]
    fn ci_mode_enforces_all_safety_checks() {
        let input = r#"
[safety]
mode = "ci"
borrow-check = "error"
lifetime-check = "error"
bounds-check = "error"
overflow-check = "error"
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Ci);
        assert_eq!(config.safety.borrow_check, CheckLevel::Error);
    }

    #[test]
    fn full_redox_toml_with_multiple_sections() {
        let input = r#"
[package]
name = "flight-controller"
version = "2.1.0"
edition = "redox-2026"

[performance]
optimization = "aggressive"

[safety]
mode = "agent"
borrow-check = "skip"
lifetime-check = "skip"
bounds-check = "skip"
overflow-check = "skip"

[agents]
allow-synthesis = true
"#;
        let config = parse_redox_config_str(input).unwrap();
        // Only [safety] is parsed; other sections are silently ignored.
        assert_eq!(config.safety.mode, SafetyMode::Agent);
        assert_eq!(config.safety.borrow_check, CheckLevel::Skip);
    }

    // --- Step 20: Safety-free type inference configuration tests ---

    #[test]
    fn agent_mode_enables_mutability_inference() {
        // In agent mode, the compiler should infer `&` vs `&mut` from usage.
        // This is controlled by safety.mode == Agent.
        let config = SafetyConfig::with_mode(SafetyMode::Agent);
        assert_eq!(config.mode, SafetyMode::Agent);
        // Agent mode + skip borrow-check means mutability coercion is unrestricted.
        assert_eq!(config.borrow_check, CheckLevel::Skip);
    }

    #[test]
    fn human_mode_enforces_mutability_checks() {
        let config = SafetyConfig::default();
        assert_eq!(config.mode, SafetyMode::Human);
        // Human mode enforces strict mutability checking.
        assert_eq!(config.borrow_check, CheckLevel::Error);
    }

    #[test]
    fn agent_mode_enables_binding_mode_inference() {
        // In agent mode, explicit `ref`/`ref mut` pattern annotations should be
        // stripped — the compiler infers `move` vs `ref` from context (§5.6.1).
        let config = SafetyConfig::with_mode(SafetyMode::Agent);
        assert_eq!(config.mode, SafetyMode::Agent);
        // Lifetime checks are also skipped — binding modes inferred.
        assert_eq!(config.lifetime_check, CheckLevel::Skip);
    }

    #[test]
    fn agent_mode_enables_dispatch_inference() {
        // In agent mode, `dyn Trait` vs `impl Trait` distinction is unified —
        // the compiler decides dispatch strategy automatically (§5.6.1).
        let config = SafetyConfig::with_mode(SafetyMode::Agent);
        assert_eq!(config.mode, SafetyMode::Agent);
        // Bounds checks skipped — dyn compatibility violations suppressed.
        assert_eq!(config.bounds_check, CheckLevel::Skip);
    }

    #[test]
    fn ci_mode_enforces_all_inference_checks() {
        // CI mode should enforce all checks — no inference relaxation.
        let config = SafetyConfig::with_mode(SafetyMode::Ci);
        assert_eq!(config.mode, SafetyMode::Ci);
        assert_eq!(config.borrow_check, CheckLevel::Error);
        assert_eq!(config.lifetime_check, CheckLevel::Error);
        assert_eq!(config.bounds_check, CheckLevel::Error);
    }

    #[test]
    fn safety_free_function_signature_config() {
        // Test the configuration from REDOX_PROPOSAL.md §5.2.1:
        // "f longest(x: s, y: s) -> s" — no lifetimes, no borrow annotations.
        // This requires agent mode with all safety checks skipped.
        let input = r#"
[safety]
mode = "agent"
borrow-check = "skip"
lifetime-check = "skip"
bounds-check = "skip"
overflow-check = "skip"
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Agent);
        assert_eq!(config.safety.borrow_check, CheckLevel::Skip);
        assert_eq!(config.safety.lifetime_check, CheckLevel::Skip);
        assert_eq!(config.safety.bounds_check, CheckLevel::Skip);
        assert_eq!(config.safety.overflow_check, CheckLevel::Skip);
    }

    #[test]
    fn agent_mode_partial_inference_config() {
        // An agent might want mutability inference but still enforce bounds checks.
        let input = r#"
[safety]
mode = "agent"
borrow-check = "skip"
lifetime-check = "skip"
bounds-check = "error"
overflow-check = "warn"
"#;
        let config = parse_redox_config_str(input).unwrap();
        assert_eq!(config.safety.mode, SafetyMode::Agent);
        assert_eq!(config.safety.borrow_check, CheckLevel::Skip);
        assert_eq!(config.safety.lifetime_check, CheckLevel::Skip);
        assert_eq!(config.safety.bounds_check, CheckLevel::Error);
        assert_eq!(config.safety.overflow_check, CheckLevel::Warn);
    }
}

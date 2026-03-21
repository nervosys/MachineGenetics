//! # Redox Safety Configuration
//!
//! Compile-time safety checking is **opt-in** in Redox, controlled by safety
//! profiles in `Redox.toml`. This crate parses the `[safety]` and
//! `[safety.profiles]` sections, resolves the active profile, and exposes
//! per-pass check levels consumed by the compiler pipeline.
//!
//! Reference: REDOX_PROPOSAL.md §9.4

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

// ===========================================================================
// Check level — per-pass granularity
// ===========================================================================

/// How a single safety pass should behave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckLevel {
    /// Pass is completely skipped — zero overhead.
    Skip,
    /// Pass runs but only emits warnings, never blocks compilation.
    Warn,
    /// Pass runs and violations are hard errors.
    Error,
}

impl CheckLevel {
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "skip" => Some(Self::Skip),
            "warn" => Some(Self::Warn),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

impl fmt::Display for CheckLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Skip => write!(f, "skip"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

// ===========================================================================
// Safety mode — top-level [safety] mode field
// ===========================================================================

/// Top-level safety mode from `[safety] mode = "..."`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafetyMode {
    /// Rust-style compile-time enforcement (for humans / CI).
    Full,
    /// Emit safety warnings but never block compilation.
    Warnings,
    /// No compiler safety passes; agents use SKB directly.
    SkbOnly,
    /// Raw performance mode; zero safety overhead.
    None,
}

impl SafetyMode {
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "full" => Some(Self::Full),
            "warnings" => Some(Self::Warnings),
            "skb-only" | "skb_only" => Some(Self::SkbOnly),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    /// The default check level implied by this mode for unspecified passes.
    fn default_check_level(self) -> CheckLevel {
        match self {
            Self::Full => CheckLevel::Error,
            Self::Warnings => CheckLevel::Warn,
            Self::SkbOnly => CheckLevel::Skip,
            Self::None => CheckLevel::Skip,
        }
    }
}

impl fmt::Display for SafetyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "full"),
            Self::Warnings => write!(f, "warnings"),
            Self::SkbOnly => write!(f, "skb-only"),
            Self::None => write!(f, "none"),
        }
    }
}

// ===========================================================================
// Safety profile — a named collection of per-pass check levels
// ===========================================================================

/// A named safety profile (e.g. `agent-dev`, `human-dev`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafetyProfile {
    pub name: String,
    pub borrow_check: CheckLevel,
    pub lifetime_check: CheckLevel,
    pub bounds_check: CheckLevel,
    pub overflow_check: CheckLevel,
    pub pattern_exhaustiveness: CheckLevel,
}

impl SafetyProfile {
    /// Create a profile with all passes set to the same level.
    pub fn uniform(name: impl Into<String>, level: CheckLevel) -> Self {
        Self {
            name: name.into(),
            borrow_check: level,
            lifetime_check: level,
            bounds_check: level,
            overflow_check: level,
            pattern_exhaustiveness: level,
        }
    }

    /// Merge explicit overrides onto a base level.
    fn from_overrides(
        name: impl Into<String>,
        base: CheckLevel,
        overrides: &HashMap<String, String>,
    ) -> Result<Self, ConfigError> {
        let mut profile = Self::uniform(name, base);
        for (key, val) in overrides {
            let level = CheckLevel::parse(val).ok_or_else(|| ConfigError::InvalidCheckLevel {
                pass: key.clone(),
                value: val.clone(),
            })?;
            match key.as_str() {
                "borrow-check" | "borrow_check" => profile.borrow_check = level,
                "lifetime-check" | "lifetime_check" => profile.lifetime_check = level,
                "bounds-check" | "bounds_check" => profile.bounds_check = level,
                "overflow-check" | "overflow_check" => profile.overflow_check = level,
                "pattern-exhaustiveness" | "pattern_exhaustiveness" => {
                    profile.pattern_exhaustiveness = level;
                }
                _ => {
                    return Err(ConfigError::UnknownPass(key.clone()));
                }
            }
        }
        Ok(profile)
    }

    /// True when every pass is set to [`CheckLevel::Skip`].
    pub fn is_all_skipped(&self) -> bool {
        self.borrow_check == CheckLevel::Skip
            && self.lifetime_check == CheckLevel::Skip
            && self.bounds_check == CheckLevel::Skip
            && self.overflow_check == CheckLevel::Skip
            && self.pattern_exhaustiveness == CheckLevel::Skip
    }

    /// True when every pass is set to [`CheckLevel::Error`].
    pub fn is_fully_enforced(&self) -> bool {
        self.borrow_check == CheckLevel::Error
            && self.lifetime_check == CheckLevel::Error
            && self.bounds_check == CheckLevel::Error
            && self.overflow_check == CheckLevel::Error
            && self.pattern_exhaustiveness == CheckLevel::Error
    }
}

// ===========================================================================
// Built-in presets (§9.4)
// ===========================================================================

/// The four canonical safety presets from REDOX_PROPOSAL.md §9.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Preset {
    AgentDev,
    HumanDev,
    CiPipeline,
    Production,
}

impl Preset {
    /// Canonical string name used in `Redox.toml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::AgentDev => "agent-dev",
            Self::HumanDev => "human-dev",
            Self::CiPipeline => "ci-pipeline",
            Self::Production => "production",
        }
    }

    /// Parse a preset name.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "agent-dev" => Some(Self::AgentDev),
            "human-dev" => Some(Self::HumanDev),
            "ci-pipeline" => Some(Self::CiPipeline),
            "production" => Some(Self::Production),
            _ => None,
        }
    }

    /// Instantiate the preset as a concrete [`SafetyProfile`].
    pub fn to_profile(self) -> SafetyProfile {
        match self {
            Self::AgentDev => SafetyProfile::uniform(self.name(), CheckLevel::Skip),
            Self::HumanDev => SafetyProfile::uniform(self.name(), CheckLevel::Error),
            Self::CiPipeline => SafetyProfile::uniform(self.name(), CheckLevel::Error),
            Self::Production => SafetyProfile::uniform(self.name(), CheckLevel::Error),
        }
    }

    /// All presets in priority order (lowest → highest safety).
    pub fn all() -> &'static [Preset] {
        &[Self::AgentDev, Self::HumanDev, Self::CiPipeline, Self::Production]
    }
}

impl fmt::Display for Preset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ===========================================================================
// Configuration errors
// ===========================================================================

/// Errors that can occur when parsing a `Redox.toml` safety section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// An unknown safety mode string.
    InvalidMode(String),
    /// An unknown check level for a pass.
    InvalidCheckLevel { pass: String, value: String },
    /// An unrecognised pass name.
    UnknownPass(String),
    /// The file could not be read.
    IoError(String),
    /// TOML parsing failed (we do minimal hand-parsing).
    ParseError(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMode(m) => write!(f, "invalid safety mode: `{m}`"),
            Self::InvalidCheckLevel { pass, value } => {
                write!(f, "invalid check level `{value}` for pass `{pass}`")
            }
            Self::UnknownPass(p) => write!(f, "unknown safety pass: `{p}`"),
            Self::IoError(e) => write!(f, "I/O error: {e}"),
            Self::ParseError(e) => write!(f, "parse error: {e}"),
        }
    }
}

// ===========================================================================
// SafetyConfig — the resolved configuration
// ===========================================================================

/// Resolved safety configuration for a compilation session.
///
/// Constructed from the `[safety]` section of `Redox.toml` (or defaults), then
/// a named profile can be selected to override individual passes.
#[derive(Debug, Clone)]
pub struct SafetyConfig {
    /// Top-level safety mode.
    pub mode: SafetyMode,
    /// Named profiles (presets + any custom profiles from TOML).
    pub profiles: HashMap<String, SafetyProfile>,
    /// The currently active profile (if any).
    pub active_profile: Option<String>,
    /// The resolved check levels for this session.
    pub effective: SafetyProfile,
}

impl SafetyConfig {
    // ------------------------------------------------------------------
    // Constructors
    // ------------------------------------------------------------------

    /// Default config: `mode = "full"`, no profile active, all passes
    /// enforce at `error` level (Rust-compatible default).
    pub fn default_config() -> Self {
        let mut profiles = HashMap::new();
        for preset in Preset::all() {
            profiles.insert(preset.name().to_string(), preset.to_profile());
        }
        let effective = SafetyProfile::uniform("default", CheckLevel::Error);
        Self {
            mode: SafetyMode::Full,
            profiles,
            active_profile: None,
            effective,
        }
    }

    /// Convenience: create a config pre-set to the given preset.
    pub fn from_preset(preset: Preset) -> Self {
        let mut config = Self::default_config();
        config.select_profile(preset.name());
        config
    }

    /// Parse from a TOML string representing the `[safety]` and
    /// `[safety.profiles]` sections.
    ///
    /// This is a minimal hand-parser (no TOML crate dependency) that handles
    /// the subset of TOML used by the safety config. Keys are `"` quoted or
    /// bare, values are `"` quoted strings or bare words.
    pub fn parse_toml(input: &str) -> Result<Self, ConfigError> {
        let mut config = Self::default_config();

        let mut in_safety = false;
        let mut in_profiles = false;
        let mut current_profile_name: Option<String> = None;
        let mut current_overrides: HashMap<String, String> = HashMap::new();

        for line in input.lines() {
            let trimmed = line.trim();

            // Skip comments and blank lines.
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Section headers.
            if trimmed.starts_with('[') {
                // Flush any pending profile.
                if let Some(pname) = current_profile_name.take() {
                    let base = config.mode.default_check_level();
                    let profile = SafetyProfile::from_overrides(&pname, base, &current_overrides)?;
                    config.profiles.insert(pname, profile);
                    current_overrides.clear();
                }

                let section = trimmed
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .trim();

                match section {
                    "safety" => {
                        in_safety = true;
                        in_profiles = false;
                    }
                    "safety.profiles" => {
                        in_safety = false;
                        in_profiles = true;
                    }
                    _ => {
                        in_safety = false;
                        in_profiles = false;
                    }
                }
                continue;
            }

            // Key = value pairs.
            if let Some((key, val)) = parse_kv(trimmed) {
                if in_safety && !in_profiles {
                    match key.as_str() {
                        "mode" => {
                            config.mode = SafetyMode::parse(&val)
                                .ok_or_else(|| ConfigError::InvalidMode(val.clone()))?;
                            // Re-derive effective from mode unless a profile overrides.
                            config.effective = SafetyProfile::uniform(
                                "effective",
                                config.mode.default_check_level(),
                            );
                        }
                        _ => {
                            // Individual pass override at top level.
                            let level = CheckLevel::parse(&val).ok_or_else(|| {
                                ConfigError::InvalidCheckLevel {
                                    pass: key.clone(),
                                    value: val.clone(),
                                }
                            })?;
                            apply_pass(&mut config.effective, &key, level)?;
                        }
                    }
                } else if in_profiles {
                    // Inline profile: `agent-dev = { borrow-check = "skip", ... }`
                    if val.starts_with('{') {
                        let base = config.mode.default_check_level();
                        let overrides = parse_inline_table(&val)?;
                        let profile = SafetyProfile::from_overrides(&key, base, &overrides)?;
                        config.profiles.insert(key, profile);
                    } else {
                        // Multi-line profile: accumulate k=v under a named profile.
                        // We treat bare `profile-name.pass = level` syntax.
                        if let Some(dot) = key.find('.') {
                            let pname = &key[..dot];
                            let pass = &key[dot + 1..];
                            if current_profile_name.as_deref() != Some(pname) {
                                // Flush previous.
                                if let Some(prev) = current_profile_name.take() {
                                    let base = config.mode.default_check_level();
                                    let p = SafetyProfile::from_overrides(
                                        &prev,
                                        base,
                                        &current_overrides,
                                    )?;
                                    config.profiles.insert(prev, p);
                                    current_overrides.clear();
                                }
                                current_profile_name = Some(pname.to_string());
                            }
                            current_overrides.insert(pass.to_string(), val);
                        }
                    }
                }
            }
        }

        // Flush trailing profile.
        if let Some(pname) = current_profile_name.take() {
            let base = config.mode.default_check_level();
            let profile = SafetyProfile::from_overrides(&pname, base, &current_overrides)?;
            config.profiles.insert(pname, profile);
        }

        Ok(config)
    }

    /// Parse from a `Redox.toml` file on disk.
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| ConfigError::IoError(e.to_string()))?;
        Self::parse_toml(&content)
    }

    // ------------------------------------------------------------------
    // Profile selection
    // ------------------------------------------------------------------

    /// Select a named profile, overriding the effective check levels.
    /// Returns `true` if the profile was found and applied.
    pub fn select_profile(&mut self, name: &str) -> bool {
        if let Some(profile) = self.profiles.get(name).cloned() {
            self.effective = profile;
            self.active_profile = Some(name.to_string());
            true
        } else {
            false
        }
    }

    /// List all available profile names.
    pub fn profile_names(&self) -> Vec<&str> {
        self.profiles.keys().map(|s| s.as_str()).collect()
    }

    // ------------------------------------------------------------------
    // Query methods
    // ------------------------------------------------------------------

    /// Should the borrow checker run?
    pub fn borrow_check(&self) -> CheckLevel {
        self.effective.borrow_check
    }

    /// Should lifetime checking run?
    pub fn lifetime_check(&self) -> CheckLevel {
        self.effective.lifetime_check
    }

    /// Should bounds checking run?
    pub fn bounds_check(&self) -> CheckLevel {
        self.effective.bounds_check
    }

    /// Should overflow checking run?
    pub fn overflow_check(&self) -> CheckLevel {
        self.effective.overflow_check
    }

    /// Should pattern exhaustiveness checking run?
    pub fn pattern_exhaustiveness(&self) -> CheckLevel {
        self.effective.pattern_exhaustiveness
    }

    /// True if any safety pass is active (not skipped).
    pub fn any_active(&self) -> bool {
        !self.effective.is_all_skipped()
    }

    /// True if all safety passes are enforced (error level).
    pub fn fully_enforced(&self) -> bool {
        self.effective.is_fully_enforced()
    }
}

// ===========================================================================
// Minimal TOML helpers
// ===========================================================================

/// Parse `key = "value"` or `key = value` from a trimmed line.
fn parse_kv(line: &str) -> Option<(String, String)> {
    let eq_pos = line.find('=')?;
    let key = line[..eq_pos].trim().trim_matches('"').to_string();
    let val = line[eq_pos + 1..]
        .trim()
        .trim_matches('"')
        .trim_end_matches(',')
        .to_string();
    if key.is_empty() {
        None
    } else {
        Some((key, val))
    }
}

/// Parse `{ key = "val", key2 = "val2" }` into a map.
fn parse_inline_table(s: &str) -> Result<HashMap<String, String>, ConfigError> {
    let inner = s
        .trim()
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .ok_or_else(|| ConfigError::ParseError("expected inline table `{ ... }`".into()))?;
    let mut map = HashMap::new();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((k, v)) = parse_kv(part) {
            map.insert(k, v);
        }
    }
    Ok(map)
}

/// Apply a check level to a pass by name.
fn apply_pass(profile: &mut SafetyProfile, pass: &str, level: CheckLevel) -> Result<(), ConfigError> {
    match pass {
        "borrow-check" | "borrow_check" => profile.borrow_check = level,
        "lifetime-check" | "lifetime_check" => profile.lifetime_check = level,
        "bounds-check" | "bounds_check" => profile.bounds_check = level,
        "overflow-check" | "overflow_check" => profile.overflow_check = level,
        "pattern-exhaustiveness" | "pattern_exhaustiveness" => {
            profile.pattern_exhaustiveness = level;
        }
        _ => return Err(ConfigError::UnknownPass(pass.to_string())),
    }
    Ok(())
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- CheckLevel ---------------------------------------------------------

    #[test]
    fn check_level_parse() {
        assert_eq!(CheckLevel::parse("skip"), Some(CheckLevel::Skip));
        assert_eq!(CheckLevel::parse("WARN"), Some(CheckLevel::Warn));
        assert_eq!(CheckLevel::parse(" Error "), Some(CheckLevel::Error));
        assert_eq!(CheckLevel::parse("bogus"), None);
    }

    #[test]
    fn check_level_display() {
        assert_eq!(format!("{}", CheckLevel::Skip), "skip");
        assert_eq!(format!("{}", CheckLevel::Warn), "warn");
        assert_eq!(format!("{}", CheckLevel::Error), "error");
    }

    // -- SafetyMode ---------------------------------------------------------

    #[test]
    fn safety_mode_parse() {
        assert_eq!(SafetyMode::parse("full"), Some(SafetyMode::Full));
        assert_eq!(SafetyMode::parse("warnings"), Some(SafetyMode::Warnings));
        assert_eq!(SafetyMode::parse("skb-only"), Some(SafetyMode::SkbOnly));
        assert_eq!(SafetyMode::parse("skb_only"), Some(SafetyMode::SkbOnly));
        assert_eq!(SafetyMode::parse("none"), Some(SafetyMode::None));
        assert_eq!(SafetyMode::parse("invalid"), None);
    }

    #[test]
    fn safety_mode_default_levels() {
        assert_eq!(SafetyMode::Full.default_check_level(), CheckLevel::Error);
        assert_eq!(SafetyMode::Warnings.default_check_level(), CheckLevel::Warn);
        assert_eq!(SafetyMode::SkbOnly.default_check_level(), CheckLevel::Skip);
        assert_eq!(SafetyMode::None.default_check_level(), CheckLevel::Skip);
    }

    // -- Preset -------------------------------------------------------------

    #[test]
    fn preset_names() {
        assert_eq!(Preset::AgentDev.name(), "agent-dev");
        assert_eq!(Preset::HumanDev.name(), "human-dev");
        assert_eq!(Preset::CiPipeline.name(), "ci-pipeline");
        assert_eq!(Preset::Production.name(), "production");
    }

    #[test]
    fn preset_parse() {
        assert_eq!(Preset::parse("agent-dev"), Some(Preset::AgentDev));
        assert_eq!(Preset::parse("agent_dev"), Some(Preset::AgentDev));
        assert_eq!(Preset::parse("HUMAN-DEV"), Some(Preset::HumanDev));
        assert_eq!(Preset::parse("ci-pipeline"), Some(Preset::CiPipeline));
        assert_eq!(Preset::parse("production"), Some(Preset::Production));
        assert_eq!(Preset::parse("unknown"), None);
    }

    #[test]
    fn preset_agent_dev_all_skipped() {
        let profile = Preset::AgentDev.to_profile();
        assert!(profile.is_all_skipped());
        assert!(!profile.is_fully_enforced());
    }

    #[test]
    fn preset_human_dev_fully_enforced() {
        let profile = Preset::HumanDev.to_profile();
        assert!(profile.is_fully_enforced());
        assert!(!profile.is_all_skipped());
    }

    #[test]
    fn preset_ci_pipeline_fully_enforced() {
        let profile = Preset::CiPipeline.to_profile();
        assert!(profile.is_fully_enforced());
    }

    #[test]
    fn preset_production_fully_enforced() {
        let profile = Preset::Production.to_profile();
        assert!(profile.is_fully_enforced());
    }

    #[test]
    fn preset_all_returns_four() {
        assert_eq!(Preset::all().len(), 4);
    }

    // -- SafetyProfile ------------------------------------------------------

    #[test]
    fn uniform_skip() {
        let p = SafetyProfile::uniform("test", CheckLevel::Skip);
        assert!(p.is_all_skipped());
        assert_eq!(p.borrow_check, CheckLevel::Skip);
    }

    #[test]
    fn uniform_error() {
        let p = SafetyProfile::uniform("test", CheckLevel::Error);
        assert!(p.is_fully_enforced());
    }

    #[test]
    fn from_overrides_partial() {
        let overrides: HashMap<String, String> = [
            ("borrow-check".into(), "error".into()),
            ("lifetime-check".into(), "warn".into()),
        ]
        .into_iter()
        .collect();
        let p = SafetyProfile::from_overrides("custom", CheckLevel::Skip, &overrides).unwrap();
        assert_eq!(p.borrow_check, CheckLevel::Error);
        assert_eq!(p.lifetime_check, CheckLevel::Warn);
        assert_eq!(p.bounds_check, CheckLevel::Skip);
        assert_eq!(p.overflow_check, CheckLevel::Skip);
    }

    #[test]
    fn from_overrides_unknown_pass_errors() {
        let overrides: HashMap<String, String> =
            [("bogus-check".into(), "error".into())].into_iter().collect();
        let result = SafetyProfile::from_overrides("bad", CheckLevel::Skip, &overrides);
        assert!(matches!(result, Err(ConfigError::UnknownPass(_))));
    }

    // -- SafetyConfig default -----------------------------------------------

    #[test]
    fn default_config_is_full_enforced() {
        let cfg = SafetyConfig::default_config();
        assert_eq!(cfg.mode, SafetyMode::Full);
        assert!(cfg.fully_enforced());
        assert!(cfg.any_active());
        assert!(cfg.active_profile.is_none());
    }

    #[test]
    fn default_config_has_all_presets() {
        let cfg = SafetyConfig::default_config();
        assert!(cfg.profiles.contains_key("agent-dev"));
        assert!(cfg.profiles.contains_key("human-dev"));
        assert!(cfg.profiles.contains_key("ci-pipeline"));
        assert!(cfg.profiles.contains_key("production"));
    }

    // -- SafetyConfig profile selection -------------------------------------

    #[test]
    fn select_agent_dev_skips_all() {
        let mut cfg = SafetyConfig::default_config();
        assert!(cfg.select_profile("agent-dev"));
        assert!(!cfg.any_active());
        assert_eq!(cfg.borrow_check(), CheckLevel::Skip);
        assert_eq!(cfg.lifetime_check(), CheckLevel::Skip);
        assert_eq!(cfg.bounds_check(), CheckLevel::Skip);
        assert_eq!(cfg.overflow_check(), CheckLevel::Skip);
        assert_eq!(cfg.pattern_exhaustiveness(), CheckLevel::Skip);
        assert_eq!(cfg.active_profile.as_deref(), Some("agent-dev"));
    }

    #[test]
    fn select_human_dev_enforces_all() {
        let mut cfg = SafetyConfig::default_config();
        assert!(cfg.select_profile("human-dev"));
        assert!(cfg.fully_enforced());
        assert_eq!(cfg.active_profile.as_deref(), Some("human-dev"));
    }

    #[test]
    fn select_unknown_profile_returns_false() {
        let mut cfg = SafetyConfig::default_config();
        assert!(!cfg.select_profile("nonexistent"));
        assert!(cfg.active_profile.is_none());
    }

    #[test]
    fn from_preset_convenience() {
        let cfg = SafetyConfig::from_preset(Preset::AgentDev);
        assert!(!cfg.any_active());
        assert_eq!(cfg.active_profile.as_deref(), Some("agent-dev"));
    }

    // -- TOML parsing -------------------------------------------------------

    #[test]
    fn parse_toml_full_mode() {
        let toml = r#"
[safety]
mode = "full"
"#;
        let cfg = SafetyConfig::parse_toml(toml).unwrap();
        assert_eq!(cfg.mode, SafetyMode::Full);
        assert!(cfg.fully_enforced());
    }

    #[test]
    fn parse_toml_skb_only_mode() {
        let toml = r#"
[safety]
mode = "skb-only"
borrow-check = "skip"
lifetime-check = "skip"
"#;
        let cfg = SafetyConfig::parse_toml(toml).unwrap();
        assert_eq!(cfg.mode, SafetyMode::SkbOnly);
        assert_eq!(cfg.borrow_check(), CheckLevel::Skip);
        assert_eq!(cfg.lifetime_check(), CheckLevel::Skip);
    }

    #[test]
    fn parse_toml_with_inline_profiles() {
        let toml = r#"
[safety]
mode = "full"

[safety.profiles]
agent-dev = { borrow-check = "skip", lifetime-check = "skip", bounds-check = "skip", overflow-check = "skip", pattern-exhaustiveness = "skip" }
human-dev = { borrow-check = "error", lifetime-check = "error", bounds-check = "error", overflow-check = "error", pattern-exhaustiveness = "error" }
"#;
        let cfg = SafetyConfig::parse_toml(toml).unwrap();
        let agent = cfg.profiles.get("agent-dev").unwrap();
        assert!(agent.is_all_skipped());
        let human = cfg.profiles.get("human-dev").unwrap();
        assert!(human.is_fully_enforced());
    }

    #[test]
    fn parse_toml_mixed_pass_levels() {
        let toml = r#"
[safety]
mode = "warnings"
borrow-check = "error"
"#;
        let cfg = SafetyConfig::parse_toml(toml).unwrap();
        assert_eq!(cfg.mode, SafetyMode::Warnings);
        // borrow-check overridden to error, rest stays at warn (from mode).
        assert_eq!(cfg.borrow_check(), CheckLevel::Error);
        assert_eq!(cfg.lifetime_check(), CheckLevel::Warn);
        assert_eq!(cfg.bounds_check(), CheckLevel::Warn);
    }

    #[test]
    fn parse_toml_invalid_mode_errors() {
        let toml = r#"
[safety]
mode = "turbo"
"#;
        let result = SafetyConfig::parse_toml(toml);
        assert!(matches!(result, Err(ConfigError::InvalidMode(_))));
    }

    #[test]
    fn parse_toml_invalid_check_level_errors() {
        let toml = r#"
[safety]
borrow-check = "yolo"
"#;
        let result = SafetyConfig::parse_toml(toml);
        assert!(matches!(result, Err(ConfigError::InvalidCheckLevel { .. })));
    }

    #[test]
    fn parse_toml_ignores_other_sections() {
        let toml = r#"
[package]
name = "test"

[safety]
mode = "none"

[dependencies]
foo = "1.0"
"#;
        let cfg = SafetyConfig::parse_toml(toml).unwrap();
        assert_eq!(cfg.mode, SafetyMode::None);
        assert!(!cfg.any_active());
    }

    #[test]
    fn parse_toml_proposal_example() {
        // The exact example from REDOX_PROPOSAL.md §9.4.
        let toml = r#"
[safety]
mode = "skb-only"
borrow-check = "skip"
lifetime-check = "skip"
bounds-check = "skip"
overflow-check = "skip"
pattern-exhaustiveness = "warn"

[safety.profiles]
agent-dev = { borrow-check = "skip", lifetime-check = "skip", bounds-check = "skip" }
human-dev = { borrow-check = "error", lifetime-check = "error", bounds-check = "error" }
ci-pipeline = { borrow-check = "error", lifetime-check = "error", bounds-check = "error" }
production = { borrow-check = "error", lifetime-check = "error", bounds-check = "error" }
"#;
        let cfg = SafetyConfig::parse_toml(toml).unwrap();
        assert_eq!(cfg.mode, SafetyMode::SkbOnly);
        assert_eq!(cfg.pattern_exhaustiveness(), CheckLevel::Warn);
        assert_eq!(cfg.borrow_check(), CheckLevel::Skip);

        // Agent-dev from inline table.
        let agent = cfg.profiles.get("agent-dev").unwrap();
        assert_eq!(agent.borrow_check, CheckLevel::Skip);
        // Unspecified passes default to mode's level (skip for skb-only).
        assert_eq!(agent.overflow_check, CheckLevel::Skip);
    }

    // -- profile_names ------------------------------------------------------

    #[test]
    fn profile_names_includes_presets() {
        let cfg = SafetyConfig::default_config();
        let names = cfg.profile_names();
        assert!(names.contains(&"agent-dev"));
        assert!(names.contains(&"human-dev"));
        assert!(names.contains(&"ci-pipeline"));
        assert!(names.contains(&"production"));
    }

    // -- ConfigError display ------------------------------------------------

    #[test]
    fn config_error_display() {
        let e = ConfigError::InvalidMode("turbo".into());
        assert_eq!(format!("{e}"), "invalid safety mode: `turbo`");

        let e = ConfigError::UnknownPass("bogus".into());
        assert_eq!(format!("{e}"), "unknown safety pass: `bogus`");
    }

    // -- Integration: parse then select profile -----------------------------

    #[test]
    fn parse_then_select_profile() {
        let toml = r#"
[safety]
mode = "full"

[safety.profiles]
agent-dev = { borrow-check = "skip", lifetime-check = "skip", bounds-check = "skip", overflow-check = "skip", pattern-exhaustiveness = "skip" }
"#;
        let mut cfg = SafetyConfig::parse_toml(toml).unwrap();
        assert!(cfg.fully_enforced());

        // Switch to agent-dev — everything should be skipped.
        assert!(cfg.select_profile("agent-dev"));
        assert!(!cfg.any_active());
    }

    // -- Edge cases ---------------------------------------------------------

    #[test]
    fn empty_input_returns_default() {
        let cfg = SafetyConfig::parse_toml("").unwrap();
        // No [safety] section means defaults apply.
        assert_eq!(cfg.mode, SafetyMode::Full);
        assert!(cfg.fully_enforced());
    }

    #[test]
    fn comments_and_blanks_are_ignored() {
        let toml = r#"
# This is a safety configuration.

[safety]
# SKB-only mode for agents.
mode = "skb-only"
"#;
        let cfg = SafetyConfig::parse_toml(toml).unwrap();
        assert_eq!(cfg.mode, SafetyMode::SkbOnly);
    }
}

//! # Redox 2026 Edition
//!
//! Defines the `redox-2026` edition with all new features including
//! token-compact canonical form, agent-native types, and language extensions.

use std::collections::HashMap;
use std::fmt;

// ── Edition Definition ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edition {
    Redox2024,
    Redox2026,
}

impl Edition {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Redox2024 => "redox-2024",
            Self::Redox2026 => "redox-2026",
        }
    }

    pub fn year(&self) -> u16 {
        match self {
            Self::Redox2024 => 2024,
            Self::Redox2026 => 2026,
        }
    }

    pub fn is_current(&self) -> bool {
        *self == Self::Redox2026
    }
}

impl fmt::Display for Edition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ── Feature Flags ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    // Token-compact canonical form
    TokenCompact,
    AbbreviationRegistry,
    FrequencyWeightedAbbrevs,

    // Agent-native types and operations
    AgentNativeTypes,
    SwarmPrimitives,
    PipelineComposition,
    MemoryTiers,

    // Language extensions
    GrammarExtensions,
    SelfEvolvingGrammar,
    PatternSynthesis,

    // Formal methods
    FormalContracts,
    VerificationOracles,
    SynthesisOracles,

    // Concurrency
    StructuredConcurrency,
    StreamingIO,
    SwarmChannels,

    // Type system
    DependentTypes,
    LinearTypes,
    EffectSystem,

    // Tooling
    IntegratedProfiling,
    AutoBenchmark,
    LiveDocumentation,
}

impl Feature {
    pub fn name(&self) -> &str {
        match self {
            Self::TokenCompact => "token_compact",
            Self::AbbreviationRegistry => "abbreviation_registry",
            Self::FrequencyWeightedAbbrevs => "freq_weighted_abbrevs",
            Self::AgentNativeTypes => "agent_native_types",
            Self::SwarmPrimitives => "swarm_primitives",
            Self::PipelineComposition => "pipeline_composition",
            Self::MemoryTiers => "memory_tiers",
            Self::GrammarExtensions => "grammar_extensions",
            Self::SelfEvolvingGrammar => "self_evolving_grammar",
            Self::PatternSynthesis => "pattern_synthesis",
            Self::FormalContracts => "formal_contracts",
            Self::VerificationOracles => "verification_oracles",
            Self::SynthesisOracles => "synthesis_oracles",
            Self::StructuredConcurrency => "structured_concurrency",
            Self::StreamingIO => "streaming_io",
            Self::SwarmChannels => "swarm_channels",
            Self::DependentTypes => "dependent_types",
            Self::LinearTypes => "linear_types",
            Self::EffectSystem => "effect_system",
            Self::IntegratedProfiling => "integrated_profiling",
            Self::AutoBenchmark => "auto_benchmark",
            Self::LiveDocumentation => "live_documentation",
        }
    }

    pub fn stability(&self) -> FeatureStability {
        match self {
            Self::TokenCompact
            | Self::AbbreviationRegistry
            | Self::AgentNativeTypes
            | Self::SwarmPrimitives
            | Self::PipelineComposition
            | Self::FormalContracts
            | Self::StructuredConcurrency
            | Self::StreamingIO => FeatureStability::Stable,

            Self::FrequencyWeightedAbbrevs
            | Self::MemoryTiers
            | Self::GrammarExtensions
            | Self::SwarmChannels
            | Self::VerificationOracles
            | Self::IntegratedProfiling => FeatureStability::Beta,

            Self::SelfEvolvingGrammar
            | Self::PatternSynthesis
            | Self::SynthesisOracles
            | Self::DependentTypes
            | Self::LinearTypes
            | Self::EffectSystem
            | Self::AutoBenchmark
            | Self::LiveDocumentation => FeatureStability::Experimental,
        }
    }
}

impl fmt::Display for Feature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureStability {
    Stable,
    Beta,
    Experimental,
}

impl fmt::Display for FeatureStability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stable => write!(f, "stable"),
            Self::Beta => write!(f, "beta"),
            Self::Experimental => write!(f, "experimental"),
        }
    }
}

// ── Token-Compact Canonical Form ─────────────────────────────────────

/// A token in compact canonical form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactToken {
    pub canonical: String,
    pub expanded: String,
    pub is_abbreviated: bool,
}

impl CompactToken {
    pub fn literal(text: impl Into<String>) -> Self {
        let t = text.into();
        Self { canonical: t.clone(), expanded: t, is_abbreviated: false }
    }

    pub fn abbreviated(abbrev: impl Into<String>, expanded: impl Into<String>) -> Self {
        Self { canonical: abbrev.into(), expanded: expanded.into(), is_abbreviated: true }
    }

    pub fn savings(&self) -> usize {
        self.expanded.len().saturating_sub(self.canonical.len())
    }
}

impl fmt::Display for CompactToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_abbreviated {
            write!(f, "{} (= {})", self.canonical, self.expanded)
        } else {
            write!(f, "{}", self.canonical)
        }
    }
}

/// A sequence of compact tokens representing a code fragment.
#[derive(Debug, Clone)]
pub struct CompactForm {
    pub tokens: Vec<CompactToken>,
}

impl CompactForm {
    pub fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    pub fn push(&mut self, token: CompactToken) {
        self.tokens.push(token);
    }

    pub fn compact_text(&self) -> String {
        self.tokens.iter().map(|t| t.canonical.as_str()).collect::<Vec<_>>().join(" ")
    }

    pub fn expanded_text(&self) -> String {
        self.tokens.iter().map(|t| t.expanded.as_str()).collect::<Vec<_>>().join(" ")
    }

    pub fn total_savings(&self) -> usize {
        self.tokens.iter().map(|t| t.savings()).sum()
    }

    pub fn abbreviation_ratio(&self) -> f64 {
        if self.tokens.is_empty() { return 0.0; }
        let abbrev_count = self.tokens.iter().filter(|t| t.is_abbreviated).count();
        abbrev_count as f64 / self.tokens.len() as f64
    }
}

impl Default for CompactForm {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CompactForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.compact_text())
    }
}

// ── Edition Configuration ────────────────────────────────────────────

/// Configuration for the redox-2026 edition.
#[derive(Debug)]
pub struct EditionConfig {
    pub edition: Edition,
    pub enabled_features: Vec<Feature>,
    pub feature_overrides: HashMap<String, bool>,
    pub agent_mode: AgentMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    Off,
    Canonical,
    Aggressive,
}

impl fmt::Display for AgentMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "off"),
            Self::Canonical => write!(f, "canonical"),
            Self::Aggressive => write!(f, "aggressive"),
        }
    }
}

impl EditionConfig {
    pub fn redox_2026() -> Self {
        Self {
            edition: Edition::Redox2026,
            enabled_features: all_2026_features(),
            feature_overrides: HashMap::new(),
            agent_mode: AgentMode::Canonical,
        }
    }

    pub fn redox_2024() -> Self {
        Self {
            edition: Edition::Redox2024,
            enabled_features: Vec::new(),
            feature_overrides: HashMap::new(),
            agent_mode: AgentMode::Off,
        }
    }

    pub fn is_feature_enabled(&self, feature: Feature) -> bool {
        if let Some(override_val) = self.feature_overrides.get(feature.name()) {
            return *override_val;
        }
        self.enabled_features.contains(&feature)
    }

    pub fn enable_feature(&mut self, feature: Feature) {
        self.feature_overrides.insert(feature.name().to_string(), true);
    }

    pub fn disable_feature(&mut self, feature: Feature) {
        self.feature_overrides.insert(feature.name().to_string(), false);
    }

    pub fn stable_features(&self) -> Vec<Feature> {
        self.enabled_features.iter()
            .filter(|f| f.stability() == FeatureStability::Stable)
            .copied()
            .collect()
    }

    pub fn experimental_features(&self) -> Vec<Feature> {
        self.enabled_features.iter()
            .filter(|f| f.stability() == FeatureStability::Experimental)
            .copied()
            .collect()
    }
}

impl fmt::Display for EditionConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Edition: {}", self.edition)?;
        writeln!(f, "Agent mode: {}", self.agent_mode)?;
        writeln!(f, "Features ({}):", self.enabled_features.len())?;
        for feat in &self.enabled_features {
            writeln!(f, "  {} [{}]", feat, feat.stability())?;
        }
        Ok(())
    }
}

/// All features included in the redox-2026 edition.
pub fn all_2026_features() -> Vec<Feature> {
    vec![
        Feature::TokenCompact,
        Feature::AbbreviationRegistry,
        Feature::FrequencyWeightedAbbrevs,
        Feature::AgentNativeTypes,
        Feature::SwarmPrimitives,
        Feature::PipelineComposition,
        Feature::MemoryTiers,
        Feature::GrammarExtensions,
        Feature::SelfEvolvingGrammar,
        Feature::PatternSynthesis,
        Feature::FormalContracts,
        Feature::VerificationOracles,
        Feature::SynthesisOracles,
        Feature::StructuredConcurrency,
        Feature::StreamingIO,
        Feature::SwarmChannels,
        Feature::DependentTypes,
        Feature::LinearTypes,
        Feature::EffectSystem,
        Feature::IntegratedProfiling,
        Feature::AutoBenchmark,
        Feature::LiveDocumentation,
    ]
}

// ── Edition Migration ────────────────────────────────────────────────

/// A migration step from one edition to another.
#[derive(Debug, Clone)]
pub struct MigrationStep {
    pub description: String,
    pub from_pattern: String,
    pub to_pattern: String,
    pub automated: bool,
}

impl fmt::Display for MigrationStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let auto = if self.automated { "auto" } else { "manual" };
        write!(f, "[{auto}] {}: {} -> {}", self.description, self.from_pattern, self.to_pattern)
    }
}

/// Generate migration steps from 2024 to 2026.
pub fn migration_2024_to_2026() -> Vec<MigrationStep> {
    vec![
        MigrationStep {
            description: "Enable token-compact canonical form".into(),
            from_pattern: "edition = \"redox-2024\"".into(),
            to_pattern: "edition = \"redox-2026\"".into(),
            automated: true,
        },
        MigrationStep {
            description: "Replace verbose keywords with abbreviations".into(),
            from_pattern: "function".into(),
            to_pattern: "fn".into(),
            automated: true,
        },
        MigrationStep {
            description: "Add agent type annotations".into(),
            from_pattern: "struct Agent".into(),
            to_pattern: "#[agent] struct Agent".into(),
            automated: false,
        },
        MigrationStep {
            description: "Convert manual concurrency to structured".into(),
            from_pattern: "thread::spawn".into(),
            to_pattern: "swarm::spawn".into(),
            automated: false,
        },
        MigrationStep {
            description: "Add formal contracts to public APIs".into(),
            from_pattern: "pub fn".into(),
            to_pattern: "#[contract(...)] pub fn".into(),
            automated: false,
        },
    ]
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edition_display() {
        assert_eq!(format!("{}", Edition::Redox2026), "redox-2026");
    }

    #[test]
    fn test_edition_year() {
        assert_eq!(Edition::Redox2026.year(), 2026);
    }

    #[test]
    fn test_edition_is_current() {
        assert!(Edition::Redox2026.is_current());
        assert!(!Edition::Redox2024.is_current());
    }

    #[test]
    fn test_feature_name() {
        assert_eq!(Feature::TokenCompact.name(), "token_compact");
    }

    #[test]
    fn test_feature_stability() {
        assert_eq!(Feature::TokenCompact.stability(), FeatureStability::Stable);
        assert_eq!(Feature::DependentTypes.stability(), FeatureStability::Experimental);
    }

    #[test]
    fn test_feature_stability_display() {
        assert_eq!(format!("{}", FeatureStability::Beta), "beta");
    }

    #[test]
    fn test_compact_token_literal() {
        let t = CompactToken::literal("fn");
        assert!(!t.is_abbreviated);
        assert_eq!(t.savings(), 0);
    }

    #[test]
    fn test_compact_token_abbreviated() {
        let t = CompactToken::abbreviated("fn", "function");
        assert!(t.is_abbreviated);
        assert_eq!(t.savings(), 6);
    }

    #[test]
    fn test_compact_token_display() {
        let t = CompactToken::abbreviated("fn", "function");
        let s = format!("{t}");
        assert!(s.contains("fn"));
        assert!(s.contains("function"));
    }

    #[test]
    fn test_compact_form() {
        let mut form = CompactForm::new();
        form.push(CompactToken::abbreviated("pub", "public"));
        form.push(CompactToken::abbreviated("fn", "function"));
        form.push(CompactToken::literal("main"));
        assert_eq!(form.compact_text(), "pub fn main");
        assert_eq!(form.expanded_text(), "public function main");
    }

    #[test]
    fn test_compact_form_savings() {
        let mut form = CompactForm::new();
        form.push(CompactToken::abbreviated("fn", "function"));
        assert_eq!(form.total_savings(), 6);
    }

    #[test]
    fn test_compact_form_ratio() {
        let mut form = CompactForm::new();
        form.push(CompactToken::abbreviated("fn", "function"));
        form.push(CompactToken::literal("x"));
        assert!((form.abbreviation_ratio() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compact_form_empty_ratio() {
        let form = CompactForm::new();
        assert_eq!(form.abbreviation_ratio(), 0.0);
    }

    #[test]
    fn test_edition_config_2026() {
        let cfg = EditionConfig::redox_2026();
        assert_eq!(cfg.edition, Edition::Redox2026);
        assert!(cfg.is_feature_enabled(Feature::TokenCompact));
    }

    #[test]
    fn test_edition_config_2024() {
        let cfg = EditionConfig::redox_2024();
        assert!(!cfg.is_feature_enabled(Feature::TokenCompact));
    }

    #[test]
    fn test_feature_override() {
        let mut cfg = EditionConfig::redox_2026();
        cfg.disable_feature(Feature::DependentTypes);
        assert!(!cfg.is_feature_enabled(Feature::DependentTypes));
    }

    #[test]
    fn test_enable_override() {
        let mut cfg = EditionConfig::redox_2024();
        cfg.enable_feature(Feature::TokenCompact);
        assert!(cfg.is_feature_enabled(Feature::TokenCompact));
    }

    #[test]
    fn test_stable_features() {
        let cfg = EditionConfig::redox_2026();
        let stable = cfg.stable_features();
        assert!(!stable.is_empty());
        for f in &stable {
            assert_eq!(f.stability(), FeatureStability::Stable);
        }
    }

    #[test]
    fn test_experimental_features() {
        let cfg = EditionConfig::redox_2026();
        let exp = cfg.experimental_features();
        assert!(!exp.is_empty());
    }

    #[test]
    fn test_all_2026_features() {
        let features = all_2026_features();
        assert_eq!(features.len(), 22);
    }

    #[test]
    fn test_agent_mode_display() {
        assert_eq!(format!("{}", AgentMode::Canonical), "canonical");
    }

    #[test]
    fn test_edition_config_display() {
        let cfg = EditionConfig::redox_2026();
        let s = format!("{cfg}");
        assert!(s.contains("redox-2026"));
        assert!(s.contains("Features"));
    }

    #[test]
    fn test_migration_steps() {
        let steps = migration_2024_to_2026();
        assert!(!steps.is_empty());
        assert!(steps.iter().any(|s| s.automated));
        assert!(steps.iter().any(|s| !s.automated));
    }

    #[test]
    fn test_migration_step_display() {
        let step = &migration_2024_to_2026()[0];
        let s = format!("{step}");
        assert!(s.contains("auto"));
    }

    #[test]
    fn test_feature_display() {
        assert_eq!(format!("{}", Feature::SwarmPrimitives), "swarm_primitives");
    }

    #[test]
    fn test_compact_form_display() {
        let mut form = CompactForm::new();
        form.push(CompactToken::literal("hello"));
        assert_eq!(format!("{form}"), "hello");
    }
}

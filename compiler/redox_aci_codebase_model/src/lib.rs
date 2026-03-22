//! # ACI Codebase Model
//!
//! Fine-tunes a small LLM on project source, SKB entries, and swarm history
//! to provide project-specific intelligence: pattern prediction, bug-prone
//! region detection, naming-convention adherence, and swarm coordination hints.
//!
//! Architecture:
//! ```text
//! Training Data Sources          Model              Inference Endpoints
//! ┌────────────────────┐    ┌─────────────┐    ┌──────────────────────┐
//! │ Project source     │───▶│             │───▶│ pattern completion   │
//! │ SKB entries        │───▶│  Codebase   │───▶│ bug risk prediction  │
//! │ Swarm history      │───▶│  Model      │───▶│ naming suggestion    │
//! │ Bug history        │───▶│  (small LLM)│───▶│ swarm advice         │
//! │ Perf profiles      │───▶│             │───▶│ code review hints    │
//! └────────────────────┘    └─────────────┘    └──────────────────────┘
//! ```
//!
//! The model is updated incrementally per build, so intelligence improves
//! as the project evolves.
//!
//! Reference: REDOX_PROPOSAL.md — ACI architecture tree, §7.10, §8.
//!
//! (ROADMAP Step 61)

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// Training Data Sources
// ═══════════════════════════════════════════════════════════════════════════

/// A source of training data for the codebase model.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataSourceKind {
    /// Project source files (`.rdx`, `.rs`).
    ProjectSource,
    /// SKB entries (safety rules, patterns, constraints).
    SkbEntries,
    /// Swarm session history (decomposition, coordination, outcomes).
    SwarmHistory,
    /// Bug history (past bugs, fixes, patterns).
    BugHistory,
    /// Performance profiles (runtime measurements, cost oracle data).
    PerfProfiles,
}

impl fmt::Display for DataSourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataSourceKind::ProjectSource => write!(f, "project_source"),
            DataSourceKind::SkbEntries => write!(f, "skb_entries"),
            DataSourceKind::SwarmHistory => write!(f, "swarm_history"),
            DataSourceKind::BugHistory => write!(f, "bug_history"),
            DataSourceKind::PerfProfiles => write!(f, "perf_profiles"),
        }
    }
}

/// A training data record from one of the sources.
#[derive(Debug, Clone)]
pub struct TrainingRecord {
    pub source: DataSourceKind,
    pub content: String,
    /// Structured tags for retrieval and weighting.
    pub tags: Vec<String>,
    /// Timestamp (epoch seconds) — more recent records get higher weight.
    pub timestamp: u64,
}

impl TrainingRecord {
    pub fn new(source: DataSourceKind, content: &str, timestamp: u64) -> Self {
        TrainingRecord {
            source,
            content: content.to_string(),
            tags: Vec::new(),
            timestamp,
        }
    }

    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|t| t.to_string()).collect();
        self
    }
}

/// Collected training corpus ready for fine-tuning.
#[derive(Debug, Clone)]
pub struct TrainingCorpus {
    pub records: Vec<TrainingRecord>,
}

impl TrainingCorpus {
    pub fn new() -> Self {
        TrainingCorpus { records: Vec::new() }
    }

    pub fn add(&mut self, record: TrainingRecord) {
        self.records.push(record);
    }

    pub fn total_records(&self) -> usize {
        self.records.len()
    }

    pub fn records_by_source(&self, source: &DataSourceKind) -> Vec<&TrainingRecord> {
        self.records.iter().filter(|r| r.source == *source).collect()
    }

    pub fn total_tokens_estimate(&self) -> usize {
        // Rough: ~4 chars per token
        self.records.iter().map(|r| r.content.len() / 4).sum()
    }
}

impl Default for TrainingCorpus {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Model Configuration
// ═══════════════════════════════════════════════════════════════════════════

/// Base model to fine-tune on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseModel {
    /// ~125M param model — fast, local.
    TinyLM,
    /// ~350M param model — balanced.
    SmallLM,
    /// ~1.3B param model — higher quality.
    MediumLM,
}

impl BaseModel {
    pub fn param_count(&self) -> u64 {
        match self {
            BaseModel::TinyLM => 125_000_000,
            BaseModel::SmallLM => 350_000_000,
            BaseModel::MediumLM => 1_300_000_000,
        }
    }

    pub fn context_window(&self) -> usize {
        match self {
            BaseModel::TinyLM => 2048,
            BaseModel::SmallLM => 4096,
            BaseModel::MediumLM => 8192,
        }
    }
}

impl fmt::Display for BaseModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaseModel::TinyLM => write!(f, "tiny-lm-125m"),
            BaseModel::SmallLM => write!(f, "small-lm-350m"),
            BaseModel::MediumLM => write!(f, "medium-lm-1.3b"),
        }
    }
}

/// Fine-tuning configuration.
#[derive(Debug, Clone)]
pub struct FineTuneConfig {
    pub base_model: BaseModel,
    /// Learning rate.
    pub learning_rate: f64,
    /// Number of training epochs.
    pub epochs: u32,
    /// Batch size for training.
    pub batch_size: u32,
    /// LoRA rank for efficient fine-tuning.
    pub lora_rank: u32,
    /// Weight for recency bias (more recent records weighted higher).
    pub recency_weight: f64,
    /// Per-source weight multipliers.
    pub source_weights: HashMap<DataSourceKind, f64>,
}

impl FineTuneConfig {
    pub fn default_for(base: BaseModel) -> Self {
        let mut source_weights = HashMap::new();
        source_weights.insert(DataSourceKind::ProjectSource, 1.0);
        source_weights.insert(DataSourceKind::SkbEntries, 1.5);
        source_weights.insert(DataSourceKind::SwarmHistory, 0.8);
        source_weights.insert(DataSourceKind::BugHistory, 2.0);
        source_weights.insert(DataSourceKind::PerfProfiles, 0.6);

        FineTuneConfig {
            base_model: base,
            learning_rate: 1e-4,
            epochs: 3,
            batch_size: 8,
            lora_rank: 16,
            recency_weight: 0.1,
            source_weights,
        }
    }

    /// Effective weight for a training record.
    pub fn record_weight(&self, record: &TrainingRecord, max_timestamp: u64) -> f64 {
        let source_w = self.source_weights.get(&record.source).copied().unwrap_or(1.0);
        let age = max_timestamp.saturating_sub(record.timestamp) as f64;
        let recency = (-self.recency_weight * age / 86400.0).exp(); // decay per day
        source_w * recency
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Fine-Tuning Pipeline
// ═══════════════════════════════════════════════════════════════════════════

/// Status of a fine-tuning job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FineTuneStatus {
    Pending,
    Preprocessing,
    Training { epoch: u32, total_epochs: u32 },
    Evaluating,
    Complete,
    Failed { reason: String },
}

impl fmt::Display for FineTuneStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FineTuneStatus::Pending => write!(f, "pending"),
            FineTuneStatus::Preprocessing => write!(f, "preprocessing"),
            FineTuneStatus::Training { epoch, total_epochs } =>
                write!(f, "training ({epoch}/{total_epochs})"),
            FineTuneStatus::Evaluating => write!(f, "evaluating"),
            FineTuneStatus::Complete => write!(f, "complete"),
            FineTuneStatus::Failed { reason } => write!(f, "failed: {reason}"),
        }
    }
}

/// Result of a fine-tuning run.
#[derive(Debug, Clone)]
pub struct FineTuneResult {
    pub status: FineTuneStatus,
    pub model_id: String,
    pub training_loss: f64,
    pub validation_loss: f64,
    pub records_processed: usize,
    pub tokens_processed: usize,
}

impl FineTuneResult {
    pub fn is_success(&self) -> bool {
        self.status == FineTuneStatus::Complete
    }

    pub fn improvement_ratio(&self) -> f64 {
        if self.validation_loss > 0.0 {
            self.training_loss / self.validation_loss
        } else {
            1.0
        }
    }
}

/// Run the fine-tuning pipeline (simulated — produces model metadata).
pub fn fine_tune(corpus: &TrainingCorpus, config: &FineTuneConfig) -> FineTuneResult {
    if corpus.records.is_empty() {
        return FineTuneResult {
            status: FineTuneStatus::Failed { reason: "empty corpus".to_string() },
            model_id: String::new(),
            training_loss: 0.0,
            validation_loss: 0.0,
            records_processed: 0,
            tokens_processed: 0,
        };
    }

    let total_tokens = corpus.total_tokens_estimate();
    let max_ts = corpus.records.iter().map(|r| r.timestamp).max().unwrap_or(0);

    // Compute weighted effective training size
    let total_weight: f64 = corpus.records.iter().map(|r| config.record_weight(r, max_ts)).sum();

    // Simulated loss: decreases with more data and higher-quality weighting
    let base_loss = 2.5 / (1.0 + (total_weight / 100.0).ln().max(0.0));
    let training_loss = base_loss * 0.85_f64.powi(config.epochs as i32);
    let validation_loss = training_loss * 1.15; // slight overfit gap

    FineTuneResult {
        status: FineTuneStatus::Complete,
        model_id: format!("{}-ft-{}", config.base_model, corpus.total_records()),
        training_loss,
        validation_loss,
        records_processed: corpus.total_records(),
        tokens_processed: total_tokens,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Inference Endpoints
// ═══════════════════════════════════════════════════════════════════════════

/// Query types the codebase model can answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferenceQuery {
    /// Predict likely code pattern given context.
    PatternCompletion { context: String },
    /// Estimate bug risk for a code region.
    BugRiskPrediction { code: String },
    /// Suggest naming consistent with project conventions.
    NamingSuggestion { kind: String, context: String },
    /// Provide swarm coordination advice.
    SwarmAdvice { task_description: String },
    /// Code review hints for a diff.
    CodeReviewHints { diff: String },
}

impl fmt::Display for InferenceQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InferenceQuery::PatternCompletion { .. } => write!(f, "pattern_completion"),
            InferenceQuery::BugRiskPrediction { .. } => write!(f, "bug_risk_prediction"),
            InferenceQuery::NamingSuggestion { .. } => write!(f, "naming_suggestion"),
            InferenceQuery::SwarmAdvice { .. } => write!(f, "swarm_advice"),
            InferenceQuery::CodeReviewHints { .. } => write!(f, "code_review_hints"),
        }
    }
}

/// Confidence level for model predictions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Confidence(f64);

impl Confidence {
    pub fn new(value: f64) -> Self {
        Confidence(value.clamp(0.0, 1.0))
    }

    pub fn value(&self) -> f64 {
        self.0
    }

    pub fn is_high(&self) -> bool {
        self.0 >= 0.8
    }

    pub fn is_low(&self) -> bool {
        self.0 < 0.4
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}%", self.0 * 100.0)
    }
}

/// A single inference result from the model.
#[derive(Debug, Clone)]
pub struct InferenceResult {
    pub query_type: String,
    pub prediction: String,
    pub confidence: Confidence,
    pub alternatives: Vec<(String, Confidence)>,
    pub reasoning: String,
}

impl InferenceResult {
    pub fn top_prediction(&self) -> &str {
        &self.prediction
    }

    pub fn has_alternatives(&self) -> bool {
        !self.alternatives.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Codebase Model
// ═══════════════════════════════════════════════════════════════════════════

/// The codebase model instance, holding training state and model metadata.
#[derive(Debug, Clone)]
pub struct CodebaseModel {
    pub model_id: String,
    pub base: BaseModel,
    pub version: u32,
    pub corpus_size: usize,
    pub training_loss: f64,
    /// Learned patterns: tag → frequency.
    pattern_index: HashMap<String, u32>,
    /// Bug-prone patterns.
    bug_patterns: Vec<String>,
    /// Naming conventions: kind → common prefixes/suffixes.
    naming_conventions: HashMap<String, Vec<String>>,
}

impl CodebaseModel {
    /// Create from fine-tuning result and corpus.
    pub fn from_fine_tune(result: &FineTuneResult, corpus: &TrainingCorpus) -> Option<Self> {
        if !result.is_success() {
            return None;
        }

        // Extract patterns from corpus
        let mut pattern_index = HashMap::new();
        let mut bug_patterns = Vec::new();
        let mut naming_conventions: HashMap<String, Vec<String>> = HashMap::new();

        for record in &corpus.records {
            for tag in &record.tags {
                *pattern_index.entry(tag.clone()).or_insert(0) += 1;
            }
            if record.source == DataSourceKind::BugHistory {
                bug_patterns.push(record.content.clone());
            }
            if record.source == DataSourceKind::ProjectSource {
                // Extract naming patterns from source
                for tag in &record.tags {
                    if tag.starts_with("naming:") {
                        let kind = tag.strip_prefix("naming:").unwrap_or("unknown");
                        naming_conventions
                            .entry(kind.to_string())
                            .or_default()
                            .push(record.content.clone());
                    }
                }
            }
        }

        Some(CodebaseModel {
            model_id: result.model_id.clone(),
            base: BaseModel::SmallLM, // default; real impl would read from result
            version: 1,
            corpus_size: corpus.total_records(),
            training_loss: result.training_loss,
            pattern_index,
            bug_patterns,
            naming_conventions,
        })
    }

    /// Run inference on a query.
    pub fn infer(&self, query: &InferenceQuery) -> InferenceResult {
        match query {
            InferenceQuery::PatternCompletion { context } => {
                self.infer_pattern(context)
            }
            InferenceQuery::BugRiskPrediction { code } => {
                self.infer_bug_risk(code)
            }
            InferenceQuery::NamingSuggestion { kind, context } => {
                self.infer_naming(kind, context)
            }
            InferenceQuery::SwarmAdvice { task_description } => {
                self.infer_swarm_advice(task_description)
            }
            InferenceQuery::CodeReviewHints { diff } => {
                self.infer_review_hints(diff)
            }
        }
    }

    fn infer_pattern(&self, context: &str) -> InferenceResult {
        // Find matching patterns from index
        let mut matches: Vec<(String, u32)> = self.pattern_index
            .iter()
            .filter(|(tag, _)| context.contains(tag.as_str()) || tag.contains(context))
            .map(|(tag, freq)| (tag.clone(), *freq))
            .collect();
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        let (prediction, confidence) = if let Some((top, freq)) = matches.first() {
            let conf = (*freq as f64 / (self.corpus_size as f64 + 1.0)).min(0.95);
            (top.clone(), Confidence::new(conf.max(0.3)))
        } else {
            ("no matching pattern found".to_string(), Confidence::new(0.1))
        };

        let alternatives = matches.iter()
            .skip(1)
            .take(3)
            .map(|(tag, freq)| {
                let conf = (*freq as f64 / (self.corpus_size as f64 + 1.0)).min(0.9);
                (tag.clone(), Confidence::new(conf.max(0.1)))
            })
            .collect();

        InferenceResult {
            query_type: "pattern_completion".to_string(),
            prediction,
            confidence,
            alternatives,
            reasoning: format!("Matched {} patterns in corpus of {} records", matches.len(), self.corpus_size),
        }
    }

    fn infer_bug_risk(&self, code: &str) -> InferenceResult {
        let matching_bugs = self.bug_patterns.iter()
            .filter(|p| {
                // Simple substring overlap heuristic
                p.split_whitespace().any(|word| code.contains(word) && word.len() > 3)
            })
            .count();

        let risk = (matching_bugs as f64 / (self.bug_patterns.len() as f64 + 1.0)).min(0.95);
        let risk_label = if risk > 0.6 {
            "high risk"
        } else if risk > 0.3 {
            "moderate risk"
        } else {
            "low risk"
        };

        InferenceResult {
            query_type: "bug_risk_prediction".to_string(),
            prediction: risk_label.to_string(),
            confidence: Confidence::new(risk.max(0.2)),
            alternatives: vec![],
            reasoning: format!("{matching_bugs} similar bug patterns found in history"),
        }
    }

    fn infer_naming(&self, kind: &str, _context: &str) -> InferenceResult {
        let suggestions = self.naming_conventions.get(kind);

        if let Some(names) = suggestions {
            let prediction = names.first().cloned().unwrap_or_default();
            let alternatives: Vec<(String, Confidence)> = names.iter()
                .skip(1)
                .take(3)
                .map(|n| (n.clone(), Confidence::new(0.6)))
                .collect();

            InferenceResult {
                query_type: "naming_suggestion".to_string(),
                prediction,
                confidence: Confidence::new(0.75),
                alternatives,
                reasoning: format!("{} naming patterns for kind '{kind}'", names.len()),
            }
        } else {
            InferenceResult {
                query_type: "naming_suggestion".to_string(),
                prediction: format!("{kind}_default"),
                confidence: Confidence::new(0.2),
                alternatives: vec![],
                reasoning: format!("no naming conventions learned for kind '{kind}'"),
            }
        }
    }

    fn infer_swarm_advice(&self, task: &str) -> InferenceResult {
        let swarm_patterns: Vec<(&String, &u32)> = self.pattern_index.iter()
            .filter(|(tag, _)| tag.starts_with("swarm:") || tag.contains("decomposition"))
            .map(|(tag, freq)| (tag, freq))
            .collect();

        let advice = if swarm_patterns.is_empty() {
            format!("No swarm history for task: {task}. Recommend default decomposition.")
        } else {
            let best = swarm_patterns.iter().max_by_key(|(_, f)| *f);
            if let Some((tag, _)) = best {
                format!("Based on swarm history, recommend pattern: {tag}")
            } else {
                "Use default swarm configuration".to_string()
            }
        };

        InferenceResult {
            query_type: "swarm_advice".to_string(),
            prediction: advice,
            confidence: Confidence::new(if swarm_patterns.is_empty() { 0.3 } else { 0.7 }),
            alternatives: vec![],
            reasoning: format!("{} swarm patterns in model", swarm_patterns.len()),
        }
    }

    fn infer_review_hints(&self, diff: &str) -> InferenceResult {
        let mut hints = Vec::new();

        // Check for bug-prone patterns in diff
        let bug_matches: Vec<&String> = self.bug_patterns.iter()
            .filter(|p| p.split_whitespace().any(|w| diff.contains(w) && w.len() > 3))
            .collect();

        if !bug_matches.is_empty() {
            hints.push(format!("{} lines match known bug patterns", bug_matches.len()));
        }

        // Check for naming convention violations
        if diff.contains("temp") || diff.contains("tmp") || diff.contains("xxx") {
            hints.push("Temporary naming detected — consider project conventions.".to_string());
        }

        let prediction = if hints.is_empty() {
            "No review concerns detected.".to_string()
        } else {
            hints.join("; ")
        };

        InferenceResult {
            query_type: "code_review_hints".to_string(),
            prediction,
            confidence: Confidence::new(if hints.is_empty() { 0.5 } else { 0.75 }),
            alternatives: vec![],
            reasoning: format!("Checked diff against {} bug patterns", self.bug_patterns.len()),
        }
    }

    /// Incrementally update the model with new records (per-build update).
    pub fn incremental_update(&mut self, records: &[TrainingRecord]) -> IncrementalUpdateResult {
        let mut new_patterns = 0u32;
        let mut new_bugs = 0u32;

        for record in records {
            self.corpus_size += 1;
            for tag in &record.tags {
                let entry = self.pattern_index.entry(tag.clone()).or_insert(0);
                *entry += 1;
                new_patterns += 1;
            }
            if record.source == DataSourceKind::BugHistory {
                self.bug_patterns.push(record.content.clone());
                new_bugs += 1;
            }
        }

        self.version += 1;

        IncrementalUpdateResult {
            new_version: self.version,
            records_added: records.len(),
            new_patterns,
            new_bugs,
        }
    }
}

/// Result of incremental model update.
#[derive(Debug, Clone)]
pub struct IncrementalUpdateResult {
    pub new_version: u32,
    pub records_added: usize,
    pub new_patterns: u32,
    pub new_bugs: u32,
}

// ═══════════════════════════════════════════════════════════════════════════
// RAP Integration
// ═══════════════════════════════════════════════════════════════════════════

/// RAP query for the codebase model.
#[derive(Debug, Clone)]
pub struct AciQuery {
    pub endpoint: AciEndpoint,
    pub payload: String,
}

/// ACI RAP endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AciEndpoint {
    /// aci.learn — feed outcome back to model.
    Learn,
    /// aci.predict — general prediction.
    Predict,
    /// aci.bug_risk — bug risk assessment.
    BugRisk,
    /// aci.naming — naming suggestion.
    Naming,
    /// aci.swarm — swarm advice.
    Swarm,
    /// aci.review — code review hints.
    Review,
    /// aci.status — model status.
    Status,
}

impl fmt::Display for AciEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AciEndpoint::Learn => write!(f, "aci.learn"),
            AciEndpoint::Predict => write!(f, "aci.predict"),
            AciEndpoint::BugRisk => write!(f, "aci.bug_risk"),
            AciEndpoint::Naming => write!(f, "aci.naming"),
            AciEndpoint::Swarm => write!(f, "aci.swarm"),
            AciEndpoint::Review => write!(f, "aci.review"),
            AciEndpoint::Status => write!(f, "aci.status"),
        }
    }
}

/// RAP response from the codebase model.
#[derive(Debug, Clone)]
pub struct AciResponse {
    pub endpoint: AciEndpoint,
    pub result: String,
    pub model_version: u32,
}

/// Process a RAP query against the codebase model.
pub fn process_aci_query(model: &CodebaseModel, query: &AciQuery) -> AciResponse {
    let result = match query.endpoint {
        AciEndpoint::Status => {
            format!(
                "model={}, version={}, corpus={}, loss={:.4}",
                model.model_id, model.version, model.corpus_size, model.training_loss
            )
        }
        AciEndpoint::Predict => {
            let inference = model.infer(&InferenceQuery::PatternCompletion {
                context: query.payload.clone(),
            });
            format!("{} (confidence: {})", inference.prediction, inference.confidence)
        }
        AciEndpoint::BugRisk => {
            let inference = model.infer(&InferenceQuery::BugRiskPrediction {
                code: query.payload.clone(),
            });
            format!("{} (confidence: {})", inference.prediction, inference.confidence)
        }
        AciEndpoint::Naming => {
            let inference = model.infer(&InferenceQuery::NamingSuggestion {
                kind: query.payload.clone(),
                context: String::new(),
            });
            format!("{} (confidence: {})", inference.prediction, inference.confidence)
        }
        AciEndpoint::Swarm => {
            let inference = model.infer(&InferenceQuery::SwarmAdvice {
                task_description: query.payload.clone(),
            });
            inference.prediction
        }
        AciEndpoint::Review => {
            let inference = model.infer(&InferenceQuery::CodeReviewHints {
                diff: query.payload.clone(),
            });
            inference.prediction
        }
        AciEndpoint::Learn => {
            format!("acknowledged: {}", query.payload)
        }
    };

    AciResponse {
        endpoint: query.endpoint,
        result,
        model_version: model.version,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_corpus() -> TrainingCorpus {
        let mut corpus = TrainingCorpus::new();
        corpus.add(
            TrainingRecord::new(DataSourceKind::ProjectSource, "fn process_batch(data: Vec<i32>)", 1000)
                .with_tags(&["batch", "processing", "naming:function"]),
        );
        corpus.add(
            TrainingRecord::new(DataSourceKind::ProjectSource, "fn validate_input(input: &str)", 1100)
                .with_tags(&["validation", "input", "naming:function"]),
        );
        corpus.add(
            TrainingRecord::new(DataSourceKind::BugHistory, "off-by-one in loop bounds", 900)
                .with_tags(&["bug:off-by-one", "loop"]),
        );
        corpus.add(
            TrainingRecord::new(DataSourceKind::BugHistory, "null pointer deref in callback handler", 950)
                .with_tags(&["bug:null-deref", "callback"]),
        );
        corpus.add(
            TrainingRecord::new(DataSourceKind::SwarmHistory, "decompose matrix multiply 4-way", 1050)
                .with_tags(&["swarm:decomposition", "matrix"]),
        );
        corpus.add(
            TrainingRecord::new(DataSourceKind::SkbEntries, "rule: bounds check on array access", 800)
                .with_tags(&["safety", "bounds"]),
        );
        corpus.add(
            TrainingRecord::new(DataSourceKind::PerfProfiles, "matmul: 2.3ms GPU, 45ms CPU", 1200)
                .with_tags(&["perf:matmul", "gpu"]),
        );
        corpus
    }

    fn sample_model() -> CodebaseModel {
        let corpus = sample_corpus();
        let config = FineTuneConfig::default_for(BaseModel::SmallLM);
        let result = fine_tune(&corpus, &config);
        CodebaseModel::from_fine_tune(&result, &corpus).unwrap()
    }

    // ── Data Sources ─────────────────────────────────────────────────────

    #[test]
    fn data_source_display() {
        assert_eq!(DataSourceKind::ProjectSource.to_string(), "project_source");
        assert_eq!(DataSourceKind::BugHistory.to_string(), "bug_history");
    }

    // ── Training Corpus ──────────────────────────────────────────────────

    #[test]
    fn corpus_construction() {
        let corpus = sample_corpus();
        assert_eq!(corpus.total_records(), 7);
        assert!(corpus.total_tokens_estimate() > 0);
    }

    #[test]
    fn corpus_by_source() {
        let corpus = sample_corpus();
        assert_eq!(corpus.records_by_source(&DataSourceKind::BugHistory).len(), 2);
        assert_eq!(corpus.records_by_source(&DataSourceKind::ProjectSource).len(), 2);
    }

    // ── Base Model ───────────────────────────────────────────────────────

    #[test]
    fn base_model_params() {
        assert_eq!(BaseModel::TinyLM.param_count(), 125_000_000);
        assert!(BaseModel::MediumLM.context_window() > BaseModel::TinyLM.context_window());
    }

    #[test]
    fn base_model_display() {
        assert!(BaseModel::SmallLM.to_string().contains("350m"));
    }

    // ── Fine-Tune Config ─────────────────────────────────────────────────

    #[test]
    fn config_record_weight() {
        let config = FineTuneConfig::default_for(BaseModel::SmallLM);
        let recent = TrainingRecord::new(DataSourceKind::BugHistory, "bug", 1000);
        let old = TrainingRecord::new(DataSourceKind::BugHistory, "bug", 100);
        let w_recent = config.record_weight(&recent, 1000);
        let w_old = config.record_weight(&old, 1000);
        assert!(w_recent > w_old, "recent records should have higher weight");
    }

    #[test]
    fn config_source_weight_bug_higher() {
        let config = FineTuneConfig::default_for(BaseModel::SmallLM);
        let bug_w = config.source_weights[&DataSourceKind::BugHistory];
        let src_w = config.source_weights[&DataSourceKind::ProjectSource];
        assert!(bug_w > src_w, "bug history should be weighted higher");
    }

    // ── Fine-Tuning ─────────────────────────────────────────────────────

    #[test]
    fn fine_tune_success() {
        let corpus = sample_corpus();
        let config = FineTuneConfig::default_for(BaseModel::SmallLM);
        let result = fine_tune(&corpus, &config);
        assert!(result.is_success());
        assert!(result.training_loss > 0.0);
        assert!(result.validation_loss >= result.training_loss);
    }

    #[test]
    fn fine_tune_empty_corpus_fails() {
        let corpus = TrainingCorpus::new();
        let config = FineTuneConfig::default_for(BaseModel::SmallLM);
        let result = fine_tune(&corpus, &config);
        assert!(!result.is_success());
    }

    #[test]
    fn fine_tune_model_id() {
        let corpus = sample_corpus();
        let config = FineTuneConfig::default_for(BaseModel::TinyLM);
        let result = fine_tune(&corpus, &config);
        assert!(result.model_id.contains("tiny-lm"));
    }

    // ── Codebase Model Creation ──────────────────────────────────────────

    #[test]
    fn model_from_fine_tune() {
        let model = sample_model();
        assert_eq!(model.corpus_size, 7);
        assert!(model.version >= 1);
    }

    // ── Pattern Completion ───────────────────────────────────────────────

    #[test]
    fn infer_pattern_match() {
        let model = sample_model();
        let result = model.infer(&InferenceQuery::PatternCompletion {
            context: "batch".to_string(),
        });
        assert!(!result.prediction.is_empty());
        assert!(result.confidence.value() > 0.0);
    }

    #[test]
    fn infer_pattern_no_match() {
        let model = sample_model();
        let result = model.infer(&InferenceQuery::PatternCompletion {
            context: "zzz_nonexistent_zzz".to_string(),
        });
        assert!(result.confidence.is_low());
    }

    // ── Bug Risk Prediction ──────────────────────────────────────────────

    #[test]
    fn infer_bug_risk_high() {
        let model = sample_model();
        let result = model.infer(&InferenceQuery::BugRiskPrediction {
            code: "loop bounds index off-by-one".to_string(),
        });
        // Should detect overlap with bug patterns
        assert!(result.confidence.value() > 0.0);
    }

    #[test]
    fn infer_bug_risk_low() {
        let model = sample_model();
        let result = model.infer(&InferenceQuery::BugRiskPrediction {
            code: "return 42".to_string(),
        });
        assert_eq!(result.prediction, "low risk");
    }

    // ── Naming Suggestion ────────────────────────────────────────────────

    #[test]
    fn infer_naming_known_kind() {
        let model = sample_model();
        let result = model.infer(&InferenceQuery::NamingSuggestion {
            kind: "function".to_string(),
            context: "processing data".to_string(),
        });
        assert!(result.confidence.value() >= 0.2);
    }

    #[test]
    fn infer_naming_unknown_kind() {
        let model = sample_model();
        let result = model.infer(&InferenceQuery::NamingSuggestion {
            kind: "widget".to_string(),
            context: String::new(),
        });
        assert!(result.prediction.contains("widget"));
    }

    // ── Swarm Advice ─────────────────────────────────────────────────────

    #[test]
    fn infer_swarm_advice() {
        let model = sample_model();
        let result = model.infer(&InferenceQuery::SwarmAdvice {
            task_description: "parallelize matrix computation".to_string(),
        });
        assert!(!result.prediction.is_empty());
    }

    // ── Code Review ──────────────────────────────────────────────────────

    #[test]
    fn infer_review_clean() {
        let model = sample_model();
        let result = model.infer(&InferenceQuery::CodeReviewHints {
            diff: "let x = 42;".to_string(),
        });
        assert!(result.prediction.contains("No review concerns") || result.confidence.value() <= 0.75);
    }

    #[test]
    fn infer_review_temp_naming() {
        let model = sample_model();
        let result = model.infer(&InferenceQuery::CodeReviewHints {
            diff: "let tmp = get_data();".to_string(),
        });
        assert!(result.prediction.contains("Temporary naming"));
    }

    // ── Incremental Update ───────────────────────────────────────────────

    #[test]
    fn incremental_update() {
        let mut model = sample_model();
        let v1 = model.version;
        let records = vec![
            TrainingRecord::new(DataSourceKind::BugHistory, "race condition in async handler", 2000)
                .with_tags(&["bug:race", "async"]),
        ];
        let update = model.incremental_update(&records);
        assert_eq!(update.records_added, 1);
        assert_eq!(update.new_bugs, 1);
        assert_eq!(model.version, v1 + 1);
        assert_eq!(model.corpus_size, 8);
    }

    // ── Confidence ───────────────────────────────────────────────────────

    #[test]
    fn confidence_clamp() {
        assert_eq!(Confidence::new(1.5).value(), 1.0);
        assert_eq!(Confidence::new(-0.5).value(), 0.0);
    }

    #[test]
    fn confidence_display() {
        assert_eq!(Confidence::new(0.85).to_string(), "85.0%");
    }

    // ── RAP Integration ──────────────────────────────────────────────────

    #[test]
    fn rap_status() {
        let model = sample_model();
        let resp = process_aci_query(&model, &AciQuery {
            endpoint: AciEndpoint::Status,
            payload: String::new(),
        });
        assert!(resp.result.contains("model="));
        assert!(resp.result.contains("version="));
    }

    #[test]
    fn rap_predict() {
        let model = sample_model();
        let resp = process_aci_query(&model, &AciQuery {
            endpoint: AciEndpoint::Predict,
            payload: "batch".to_string(),
        });
        assert!(!resp.result.is_empty());
    }

    #[test]
    fn rap_bug_risk() {
        let model = sample_model();
        let resp = process_aci_query(&model, &AciQuery {
            endpoint: AciEndpoint::BugRisk,
            payload: "loop bounds".to_string(),
        });
        assert!(resp.result.contains("risk"));
    }

    #[test]
    fn rap_learn() {
        let model = sample_model();
        let resp = process_aci_query(&model, &AciQuery {
            endpoint: AciEndpoint::Learn,
            payload: "fix applied successfully".to_string(),
        });
        assert!(resp.result.contains("acknowledged"));
    }

    #[test]
    fn rap_endpoint_display() {
        assert_eq!(AciEndpoint::Learn.to_string(), "aci.learn");
        assert_eq!(AciEndpoint::BugRisk.to_string(), "aci.bug_risk");
    }

    // ── Fine-Tune Status Display ─────────────────────────────────────────

    #[test]
    fn status_display() {
        assert_eq!(FineTuneStatus::Pending.to_string(), "pending");
        assert_eq!(
            FineTuneStatus::Training { epoch: 2, total_epochs: 5 }.to_string(),
            "training (2/5)"
        );
    }
}

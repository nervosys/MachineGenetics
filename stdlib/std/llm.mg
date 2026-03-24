//! # std::llm — Language Model Integration
//!
//! Native types for invoking large language models.
//! All LLM operations declare the `llm` effect.

// ---------------------------------------------------------------------------
// LLM type
// ---------------------------------------------------------------------------

/// A language model handle.
pub struct LLM {
    model_id: String,
    config: LLMConfig,
}

/// Configuration for LLM inference.
pub struct LLMConfig {
    pub max_tokens: usize,
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: usize,
    pub stop_sequences: Vec<String>,
}

impl LLMConfig {
    pub fn default() -> LLMConfig {
        LLMConfig {
            max_tokens: 1024,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 50,
            stop_sequences: vec![],
        }
    }
}

impl LLM {
    /// Load a model from a URI.
    /// Supported schemes: `local://`, `hf://`, `api://`.
    pub fn load(uri: &str) -> LLM / io;

    /// Load with custom config.
    pub fn load_with(uri: &str, config: LLMConfig) -> LLM / io;

    /// Generate text from a prompt.
    pub fn generate(&self, prompt: Prompt, max_tokens: usize) -> Response / llm;

    /// Generate with full config override.
    pub fn generate_with(&self, prompt: Prompt, config: &LLMConfig) -> Response / llm;

    /// Compute embeddings for text.
    pub fn embed(&self, text: &str) -> Vec<f32> / llm;

    /// Analyze text and return structured analysis.
    pub fn analyze(&self, text: &str, context: &str) -> Analysis / llm;

    /// Chat-style multi-turn generation.
    pub fn chat(&self, messages: &[ChatMessage]) -> Response / llm;
}

// ---------------------------------------------------------------------------
// Prompt
// ---------------------------------------------------------------------------

/// A prompt for LLM generation.
pub struct Prompt {
    pub text: String,
    pub system: Option<String>,
}

impl Prompt {
    pub fn new(text: &str) -> Prompt {
        Prompt { text: text.into(), system: None }
    }

    pub fn with_system(text: &str, system: &str) -> Prompt {
        Prompt { text: text.into(), system: Some(system.into()) }
    }
}

// ---------------------------------------------------------------------------
// Response
// ---------------------------------------------------------------------------

/// A response from an LLM.
pub struct Response {
    text_content: String,
    tokens_used: usize,
    finish_reason: FinishReason,
}

impl Response {
    pub fn text(&self) -> &str { &self.text_content }
    pub fn tokens(&self) -> usize { self.tokens_used }
    pub fn finish_reason(&self) -> FinishReason { self.finish_reason }
    pub fn score(&self) -> f64;
}

pub enum FinishReason {
    Stop,
    MaxTokens,
    ContentFilter,
}

// ---------------------------------------------------------------------------
// Chat
// ---------------------------------------------------------------------------

pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

pub enum Role {
    System,
    User,
    Assistant,
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

/// Structured analysis output from an LLM.
pub struct Analysis {
    pub summary: String,
    pub findings: Vec<Finding>,
    pub confidence: f64,
}

pub struct Finding {
    pub category: String,
    pub description: String,
    pub severity: Severity,
    pub location: Option<String>,
}

pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

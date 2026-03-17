use serde::{Deserialize, Serialize};

/// Metadata for a published module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleMetadata {
    /// Short description of the module
    pub description: String,
    /// SPDX license identifier
    pub license: String,
    /// List of authors
    pub authors: Vec<String>,
    /// Repository URL
    pub repository: Option<String>,
    /// Searchable keywords
    pub keywords: Vec<String>,
    /// Module categories
    pub categories: Vec<String>,
    /// Redox edition (e.g., "2025")
    pub edition: String,
    /// Minimum Rust version if transpilable to Rust
    pub rust_compatibility: Option<String>,
}

impl Default for ModuleMetadata {
    fn default() -> Self {
        ModuleMetadata {
            description: String::new(),
            license: String::new(),
            authors: Vec::new(),
            repository: None,
            keywords: Vec::new(),
            categories: Vec::new(),
            edition: "2025".to_string(),
            rust_compatibility: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_metadata() {
        let meta = ModuleMetadata::default();
        assert_eq!(meta.edition, "2025");
        assert!(meta.authors.is_empty());
        assert!(meta.repository.is_none());
    }

    #[test]
    fn test_metadata_serialization() {
        let meta = ModuleMetadata {
            description: "A test module".to_string(),
            license: "MIT".to_string(),
            authors: vec!["Alice".to_string()],
            repository: Some("https://github.com/example/test".to_string()),
            keywords: vec!["http".to_string(), "web".to_string()],
            categories: vec!["networking".to_string()],
            edition: "2025".to_string(),
            rust_compatibility: Some("1.75".to_string()),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: ModuleMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.description, "A test module");
        assert_eq!(deserialized.authors.len(), 1);
    }
}

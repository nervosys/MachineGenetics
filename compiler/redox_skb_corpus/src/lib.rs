// redox_skb_corpus: Structured Knowledge Base rule corpus for agent training.
//
//  Models rules as structured records with category, severity, tags,
//  examples, and relationships. Provides query APIs, export formats,
//  and a pre-built corpus of Redox-specific rules.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Rule severity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Hint => "hint",
        }
    }
}

// ---------------------------------------------------------------------------
// Rule category
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Ownership,
    TypeSystem,
    Concurrency,
    Safety,
    Performance,
    Style,
    CodeSmell,
    Security,
    Correctness,
    Idiom,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ownership => "ownership",
            Self::TypeSystem => "type-system",
            Self::Concurrency => "concurrency",
            Self::Safety => "safety",
            Self::Performance => "performance",
            Self::Style => "style",
            Self::CodeSmell => "code-smell",
            Self::Security => "security",
            Self::Correctness => "correctness",
            Self::Idiom => "idiom",
        }
    }
}

// ---------------------------------------------------------------------------
// Code example
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeExample {
    pub label: String,
    pub code: String,
    pub is_positive: bool, // true = good example, false = bad example
}

// ---------------------------------------------------------------------------
// Rule
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkbRule {
    pub id: RuleId,
    pub title: String,
    pub description: String,
    pub category: Category,
    pub severity: Severity,
    pub tags: Vec<String>,
    pub examples: Vec<CodeExample>,
    pub related: Vec<RuleId>,
    pub fixable: bool,
}

impl SkbRule {
    pub fn positive_examples(&self) -> Vec<&CodeExample> {
        self.examples.iter().filter(|e| e.is_positive).collect()
    }

    pub fn negative_examples(&self) -> Vec<&CodeExample> {
        self.examples.iter().filter(|e| !e.is_positive).collect()
    }
}

// ---------------------------------------------------------------------------
// Corpus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SkbCorpus {
    rules: Vec<SkbRule>,
    index: HashMap<String, usize>,
}

impl SkbCorpus {
    pub fn new(rules: Vec<SkbRule>) -> Self {
        let mut index = HashMap::new();
        for (i, r) in rules.iter().enumerate() {
            index.insert(r.id.0.clone(), i);
        }
        Self { rules, index }
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&SkbRule> {
        self.index.get(id).map(|&i| &self.rules[i])
    }

    pub fn all(&self) -> &[SkbRule] {
        &self.rules
    }

    pub fn by_category(&self, cat: Category) -> Vec<&SkbRule> {
        self.rules.iter().filter(|r| r.category == cat).collect()
    }

    pub fn by_severity(&self, sev: Severity) -> Vec<&SkbRule> {
        self.rules.iter().filter(|r| r.severity == sev).collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&SkbRule> {
        self.rules.iter().filter(|r| r.tags.iter().any(|t| t == tag)).collect()
    }

    pub fn fixable(&self) -> Vec<&SkbRule> {
        self.rules.iter().filter(|r| r.fixable).collect()
    }

    pub fn search(&self, keyword: &str) -> Vec<&SkbRule> {
        let kw = keyword.to_lowercase();
        self.rules.iter().filter(|r| {
            r.title.to_lowercase().contains(&kw) || r.description.to_lowercase().contains(&kw)
        }).collect()
    }

    pub fn categories(&self) -> Vec<Category> {
        let mut cats: Vec<Category> = self.rules.iter().map(|r| r.category).collect();
        cats.sort_by_key(|c| c.label());
        cats.dedup();
        cats
    }

    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.rules.iter().flat_map(|r| r.tags.clone()).collect();
        tags.sort();
        tags.dedup();
        tags
    }

    pub fn stats(&self) -> CorpusStats {
        CorpusStats {
            total_rules: self.rules.len(),
            errors: self.by_severity(Severity::Error).len(),
            warnings: self.by_severity(Severity::Warning).len(),
            fixable: self.fixable().len(),
            categories: self.categories().len(),
            total_examples: self.rules.iter().map(|r| r.examples.len()).sum(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusStats {
    pub total_rules: usize,
    pub errors: usize,
    pub warnings: usize,
    pub fixable: usize,
    pub categories: usize,
    pub total_examples: usize,
}

// ---------------------------------------------------------------------------
// Export format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Csv,
    Markdown,
}

pub fn export_corpus(corpus: &SkbCorpus, format: ExportFormat) -> String {
    match format {
        ExportFormat::Json => export_json(corpus),
        ExportFormat::Csv => export_csv(corpus),
        ExportFormat::Markdown => export_markdown(corpus),
    }
}

fn export_json(corpus: &SkbCorpus) -> String {
    let mut out = String::from("[\n");
    for (i, r) in corpus.all().iter().enumerate() {
        if i > 0 { out.push_str(",\n"); }
        out.push_str(&format!(
            "  {{\"id\":\"{}\",\"title\":\"{}\",\"category\":\"{}\",\"severity\":\"{}\"}}",
            r.id.0, r.title, r.category.label(), r.severity.label(),
        ));
    }
    out.push_str("\n]");
    out
}

fn export_csv(corpus: &SkbCorpus) -> String {
    let mut out = String::from("id,title,category,severity,fixable\n");
    for r in corpus.all() {
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            r.id.0, r.title, r.category.label(), r.severity.label(), r.fixable,
        ));
    }
    out
}

fn export_markdown(corpus: &SkbCorpus) -> String {
    let mut out = String::from("# SKB Rule Corpus\n\n| ID | Title | Category | Severity |\n|---|---|---|---|\n");
    for r in corpus.all() {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            r.id.0, r.title, r.category.label(), r.severity.label(),
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Pre-built corpus
// ---------------------------------------------------------------------------

fn example(label: &str, code: &str, positive: bool) -> CodeExample {
    CodeExample { label: label.to_string(), code: code.to_string(), is_positive: positive }
}

pub fn build_redox_corpus() -> SkbCorpus {
    let rules = vec![
        SkbRule {
            id: RuleId("SKB-OWN-001".into()),
            title: "Use-after-move".into(),
            description: "Accessing a value after it has been moved is a compile error.".into(),
            category: Category::Ownership,
            severity: Severity::Error,
            tags: vec!["ownership".into(), "move".into()],
            examples: vec![
                example("bad: use after move", "let x = vec![1]; let y = x; println!(\"{:?}\", x);", false),
                example("good: clone", "let x = vec![1]; let y = x.clone(); println!(\"{:?}\", x);", true),
            ],
            related: vec![RuleId("SKB-OWN-002".into())],
            fixable: true,
        },
        SkbRule {
            id: RuleId("SKB-OWN-002".into()),
            title: "Dangling reference".into(),
            description: "A reference must not outlive the data it points to.".into(),
            category: Category::Ownership,
            severity: Severity::Error,
            tags: vec!["ownership".into(), "lifetime".into()],
            examples: vec![
                example("bad: dangling ref", "fn bad() -> &i32 { let x = 42; &x }", false),
            ],
            related: vec![RuleId("SKB-OWN-001".into())],
            fixable: false,
        },
        SkbRule {
            id: RuleId("SKB-TYP-001".into()),
            title: "Type mismatch".into(),
            description: "Function return type must match the declared signature.".into(),
            category: Category::TypeSystem,
            severity: Severity::Error,
            tags: vec!["type".into(), "mismatch".into()],
            examples: vec![
                example("bad: wrong return", "fn f() -> i32 { \"hello\" }", false),
                example("good: correct", "fn f() -> i32 { 42 }", true),
            ],
            related: vec![],
            fixable: true,
        },
        SkbRule {
            id: RuleId("SKB-CONC-001".into()),
            title: "Data race potential".into(),
            description: "Shared mutable state without synchronization may cause data races.".into(),
            category: Category::Concurrency,
            severity: Severity::Error,
            tags: vec!["concurrency".into(), "data-race".into()],
            examples: vec![],
            related: vec![],
            fixable: false,
        },
        SkbRule {
            id: RuleId("SKB-PERF-001".into()),
            title: "Unnecessary allocation".into(),
            description: "Prefer stack allocation over heap allocation when the size is known.".into(),
            category: Category::Performance,
            severity: Severity::Warning,
            tags: vec!["performance".into(), "allocation".into()],
            examples: vec![
                example("bad: heap", "let v: Box<[u8; 4]> = Box::new([0;4]);", false),
                example("good: stack", "let v: [u8; 4] = [0; 4];", true),
            ],
            related: vec![],
            fixable: true,
        },
        SkbRule {
            id: RuleId("SKB-SEC-001".into()),
            title: "Unchecked input".into(),
            description: "External input must be validated before use.".into(),
            category: Category::Security,
            severity: Severity::Error,
            tags: vec!["security".into(), "input-validation".into()],
            examples: vec![],
            related: vec![],
            fixable: false,
        },
        SkbRule {
            id: RuleId("SKB-STY-001".into()),
            title: "Non-idiomatic naming".into(),
            description: "Types should use CamelCase; functions and variables should use snake_case.".into(),
            category: Category::Style,
            severity: Severity::Info,
            tags: vec!["style".into(), "naming".into()],
            examples: vec![
                example("bad: wrong case", "fn MyFunc() {}", false),
                example("good: snake_case", "fn my_func() {}", true),
            ],
            related: vec![],
            fixable: true,
        },
        SkbRule {
            id: RuleId("SKB-IDM-001".into()),
            title: "Prefer if-let over match".into(),
            description: "When matching a single variant, prefer if-let for clarity.".into(),
            category: Category::Idiom,
            severity: Severity::Hint,
            tags: vec!["idiom".into(), "pattern".into()],
            examples: vec![
                example("good: if-let", "if let Some(x) = opt { use(x); }", true),
            ],
            related: vec![],
            fixable: true,
        },
    ];
    SkbCorpus::new(rules)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> SkbCorpus {
        build_redox_corpus()
    }

    // -- Severity --
    #[test]
    fn test_severity_labels() {
        assert_eq!(Severity::Error.label(), "error");
        assert_eq!(Severity::Hint.label(), "hint");
    }

    // -- Category --
    #[test]
    fn test_category_labels() {
        assert_eq!(Category::Ownership.label(), "ownership");
        assert_eq!(Category::Security.label(), "security");
    }

    // -- Corpus basics --
    #[test]
    fn test_corpus_len() {
        assert_eq!(corpus().len(), 8);
    }

    #[test]
    fn test_corpus_not_empty() {
        assert!(!corpus().is_empty());
    }

    // -- get --
    #[test]
    fn test_get_existing() {
        let c = corpus();
        let r = c.get("SKB-OWN-001").unwrap();
        assert_eq!(r.title, "Use-after-move");
    }

    #[test]
    fn test_get_missing() {
        assert!(corpus().get("NOPE").is_none());
    }

    // -- by_category --
    #[test]
    fn test_by_category_ownership() {
        assert_eq!(corpus().by_category(Category::Ownership).len(), 2);
    }

    // -- by_severity --
    #[test]
    fn test_by_severity_error() {
        let c = corpus(); let errs = c.by_severity(Severity::Error);
        assert!(errs.len() >= 4);
    }

    // -- by_tag --
    #[test]
    fn test_by_tag() {
        assert_eq!(corpus().by_tag("ownership").len(), 2);
    }

    // -- fixable --
    #[test]
    fn test_fixable() {
        let c = corpus(); let f = c.fixable();
        assert!(f.len() >= 4);
    }

    // -- search --
    #[test]
    fn test_search() {
        let c = corpus(); let results = c.search("allocation");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id.0, "SKB-PERF-001");
    }

    #[test]
    fn test_search_case_insensitive() {
        let c = corpus(); assert!(!c.search("MOVE").is_empty());
    }

    // -- categories / all_tags --
    #[test]
    fn test_categories() {
        let c = corpus(); assert!(c.categories().len() >= 6);
    }

    #[test]
    fn test_all_tags() {
        let c = corpus(); assert!(c.all_tags().len() >= 5);
    }

    // -- stats --
    #[test]
    fn test_stats() {
        let s = corpus().stats();
        assert_eq!(s.total_rules, 8);
        assert!(s.total_examples >= 7);
    }

    // -- examples --
    #[test]
    fn test_positive_examples() {
        let c = corpus();
        let r = c.get("SKB-OWN-001").unwrap();
        assert_eq!(r.positive_examples().len(), 1);
        assert_eq!(r.negative_examples().len(), 1);
    }

    // -- export json --
    #[test]
    fn test_export_json() {
        let json = export_corpus(&corpus(), ExportFormat::Json);
        assert!(json.starts_with('['));
        assert!(json.contains("SKB-OWN-001"));
    }

    // -- export csv --
    #[test]
    fn test_export_csv() {
        let csv = export_corpus(&corpus(), ExportFormat::Csv);
        assert!(csv.starts_with("id,title"));
        assert!(csv.contains("SKB-PERF-001"));
    }

    // -- export markdown --
    #[test]
    fn test_export_markdown() {
        let md = export_corpus(&corpus(), ExportFormat::Markdown);
        assert!(md.contains("# SKB Rule Corpus"));
        assert!(md.contains("| SKB-OWN-001"));
    }

    // -- RuleId hash --
    #[test]
    fn test_rule_id_hash() {
        let mut set = std::collections::HashSet::new();
        set.insert(RuleId("A".into()));
        set.insert(RuleId("B".into()));
        assert_eq!(set.len(), 2);
    }

    // -- empty corpus --
    #[test]
    fn test_empty_corpus() {
        let c = SkbCorpus::new(vec![]);
        assert!(c.is_empty());
        assert_eq!(c.stats().total_rules, 0);
    }

    // -- related rules --
    #[test]
    fn test_related_rules() {
        let c = corpus();
        let r = c.get("SKB-OWN-001").unwrap();
        assert_eq!(r.related.len(), 1);
        assert_eq!(r.related[0].0, "SKB-OWN-002");
    }
}

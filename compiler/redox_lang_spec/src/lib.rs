// redox_lang_spec: Machine-readable Redox language specification.
//
//  Models the specification as a structured document tree with chapters,
//  sections, normative rules, and cross-references. Provides query APIs
//  for tooling (linters, conformance suites, documentation generators).

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Specification versioning
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpecVersion {
    pub major: u32,
    pub minor: u32,
    pub edition: String,
}

impl SpecVersion {
    pub fn new(major: u32, minor: u32, edition: &str) -> Self {
        Self { major, minor, edition: edition.to_string() }
    }

    pub fn label(&self) -> String {
        format!("Redox {} v{}.{}", self.edition, self.major, self.minor)
    }
}

// ---------------------------------------------------------------------------
// Normative status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Normativity {
    Normative,
    Informative,
    Deprecated,
    Experimental,
}

impl Normativity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normative => "normative",
            Self::Informative => "informative",
            Self::Deprecated => "deprecated",
            Self::Experimental => "experimental",
        }
    }
}

// ---------------------------------------------------------------------------
// Rule / requirement
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub id: RuleId,
    pub title: String,
    pub description: String,
    pub normativity: Normativity,
    pub related: Vec<RuleId>,
}

// ---------------------------------------------------------------------------
// Section
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SectionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub id: SectionId,
    pub title: String,
    pub normativity: Normativity,
    pub rules: Vec<Rule>,
    pub subsections: Vec<SectionId>,
    pub prose: String,
}

// ---------------------------------------------------------------------------
// Chapter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChapterId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub id: ChapterId,
    pub number: u32,
    pub title: String,
    pub sections: Vec<Section>,
}

impl Chapter {
    pub fn rule_count(&self) -> usize {
        self.sections.iter().map(|s| s.rules.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// Specification document
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Specification {
    pub version: SpecVersion,
    pub chapters: Vec<Chapter>,
    index: HashMap<String, (usize, usize, usize)>, // rule_id -> (ch, sec, rule)
}

impl Specification {
    pub fn new(version: SpecVersion, chapters: Vec<Chapter>) -> Self {
        let mut index = HashMap::new();
        for (ci, ch) in chapters.iter().enumerate() {
            for (si, sec) in ch.sections.iter().enumerate() {
                for (ri, rule) in sec.rules.iter().enumerate() {
                    index.insert(rule.id.0.clone(), (ci, si, ri));
                }
            }
        }
        Self { version, chapters, index }
    }

    pub fn total_rules(&self) -> usize {
        self.chapters.iter().map(|c| c.rule_count()).sum()
    }

    pub fn total_chapters(&self) -> usize {
        self.chapters.len()
    }

    pub fn total_sections(&self) -> usize {
        self.chapters.iter().map(|c| c.sections.len()).sum()
    }

    pub fn lookup_rule(&self, id: &str) -> Option<&Rule> {
        let &(ci, si, ri) = self.index.get(id)?;
        Some(&self.chapters[ci].sections[si].rules[ri])
    }

    pub fn normative_rules(&self) -> Vec<&Rule> {
        self.all_rules().into_iter().filter(|r| r.normativity == Normativity::Normative).collect()
    }

    pub fn deprecated_rules(&self) -> Vec<&Rule> {
        self.all_rules().into_iter().filter(|r| r.normativity == Normativity::Deprecated).collect()
    }

    pub fn experimental_rules(&self) -> Vec<&Rule> {
        self.all_rules().into_iter().filter(|r| r.normativity == Normativity::Experimental).collect()
    }

    pub fn all_rules(&self) -> Vec<&Rule> {
        let mut out = Vec::new();
        for ch in &self.chapters {
            for sec in &ch.sections {
                for r in &sec.rules {
                    out.push(r);
                }
            }
        }
        out
    }

    pub fn chapter_by_number(&self, n: u32) -> Option<&Chapter> {
        self.chapters.iter().find(|c| c.number == n)
    }

    pub fn search_rules(&self, keyword: &str) -> Vec<&Rule> {
        let kw = keyword.to_lowercase();
        self.all_rules().into_iter().filter(|r| {
            r.title.to_lowercase().contains(&kw) || r.description.to_lowercase().contains(&kw)
        }).collect()
    }
}

// ---------------------------------------------------------------------------
// Builder helpers
// ---------------------------------------------------------------------------

pub fn make_rule(id: &str, title: &str, desc: &str, norm: Normativity) -> Rule {
    Rule {
        id: RuleId(id.to_string()),
        title: title.to_string(),
        description: desc.to_string(),
        normativity: norm,
        related: Vec::new(),
    }
}

pub fn make_section(id: &str, title: &str, norm: Normativity, rules: Vec<Rule>) -> Section {
    Section {
        id: SectionId(id.to_string()),
        title: title.to_string(),
        normativity: norm,
        rules,
        subsections: Vec::new(),
        prose: String::new(),
    }
}

pub fn make_chapter(id: &str, number: u32, title: &str, sections: Vec<Section>) -> Chapter {
    Chapter {
        id: ChapterId(id.to_string()),
        number,
        title: title.to_string(),
        sections,
    }
}

// ---------------------------------------------------------------------------
// Build the Redox 2026 specification (sample)
// ---------------------------------------------------------------------------

pub fn build_redox_2026_spec() -> Specification {
    let version = SpecVersion::new(1, 0, "2026");

    // Chapter 1: Lexical Structure
    let ch1 = make_chapter("ch1", 1, "Lexical Structure", vec![
        make_section("ch1.1", "Tokens", Normativity::Normative, vec![
            make_rule("R-LEX-001", "Token categories",
                "The lexer shall produce keyword, identifier, literal, operator, and punctuation tokens.",
                Normativity::Normative),
            make_rule("R-LEX-002", "Unicode identifiers",
                "Identifiers shall support Unicode XID_Start / XID_Continue.",
                Normativity::Normative),
        ]),
        make_section("ch1.2", "Comments", Normativity::Informative, vec![
            make_rule("R-LEX-010", "Line comments",
                "Line comments begin with // and extend to end of line.",
                Normativity::Normative),
        ]),
    ]);

    // Chapter 2: Type System
    let ch2 = make_chapter("ch2", 2, "Type System", vec![
        make_section("ch2.1", "Primitive types", Normativity::Normative, vec![
            make_rule("R-TYP-001", "Integer types",
                "The language provides i8..i128, u8..u128, isize, usize.",
                Normativity::Normative),
            make_rule("R-TYP-002", "Floating-point types",
                "f32 and f64 conform to IEEE 754-2019.",
                Normativity::Normative),
        ]),
        make_section("ch2.2", "Ownership", Normativity::Normative, vec![
            make_rule("R-OWN-001", "Move semantics",
                "Values are moved by default unless the type implements Copy.",
                Normativity::Normative),
            make_rule("R-OWN-002", "Borrow checking",
                "The borrow checker enforces exclusive XOR shared reference discipline.",
                Normativity::Normative),
        ]),
        make_section("ch2.3", "Capability types (experimental)", Normativity::Experimental, vec![
            make_rule("R-CAP-001", "Network capability",
                "A function requiring network access must declare Net capability.",
                Normativity::Experimental),
        ]),
    ]);

    // Chapter 3: Expressions
    let ch3 = make_chapter("ch3", 3, "Expressions", vec![
        make_section("ch3.1", "Arithmetic", Normativity::Normative, vec![
            make_rule("R-EXPR-001", "Overflow behavior",
                "Integer overflow in debug mode panics; in release mode wraps.",
                Normativity::Normative),
        ]),
        make_section("ch3.2", "Pattern matching", Normativity::Normative, vec![
            make_rule("R-EXPR-010", "Exhaustiveness",
                "Match expressions must be exhaustive.",
                Normativity::Normative),
        ]),
    ]);

    // Chapter 4: Concurrency
    let ch4 = make_chapter("ch4", 4, "Concurrency", vec![
        make_section("ch4.1", "Send and Sync", Normativity::Normative, vec![
            make_rule("R-CONC-001", "Send trait",
                "Types implementing Send may be transferred across thread boundaries.",
                Normativity::Normative),
        ]),
        make_section("ch4.2", "Deprecated threading model", Normativity::Deprecated, vec![
            make_rule("R-CONC-D01", "Green threads",
                "Green threads are deprecated in favor of async/await.",
                Normativity::Deprecated),
        ]),
    ]);

    Specification::new(version, vec![ch1, ch2, ch3, ch4])
}

// ---------------------------------------------------------------------------
// Table of contents generator
// ---------------------------------------------------------------------------

pub fn table_of_contents(spec: &Specification) -> String {
    let mut toc = String::new();
    toc.push_str(&format!("{}\n\n", spec.version.label()));
    for ch in &spec.chapters {
        toc.push_str(&format!("  {}. {}\n", ch.number, ch.title));
        for sec in &ch.sections {
            toc.push_str(&format!("    - {} [{}]\n", sec.title, sec.normativity.label()));
        }
    }
    toc
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> Specification {
        build_redox_2026_spec()
    }

    // -- SpecVersion --
    #[test]
    fn test_spec_version_label() {
        let v = SpecVersion::new(1, 0, "2026");
        assert_eq!(v.label(), "Redox 2026 v1.0");
    }

    // -- Normativity --
    #[test]
    fn test_normativity_labels() {
        assert_eq!(Normativity::Normative.label(), "normative");
        assert_eq!(Normativity::Deprecated.label(), "deprecated");
    }

    // -- Build spec --
    #[test]
    fn test_total_chapters() {
        assert_eq!(spec().total_chapters(), 4);
    }

    #[test]
    fn test_total_sections() {
        // ch1:2, ch2:3, ch3:2, ch4:2 = 9
        assert_eq!(spec().total_sections(), 9);
    }

    #[test]
    fn test_total_rules() {
        // ch1: 3, ch2: 5, ch3: 2, ch4: 2 = 12
        assert_eq!(spec().total_rules(), 12);
    }

    // -- lookup --
    #[test]
    fn test_lookup_existing_rule() {
        let s = spec();
        let r = s.lookup_rule("R-OWN-001").unwrap();
        assert_eq!(r.title, "Move semantics");
    }

    #[test]
    fn test_lookup_missing_rule() {
        assert!(spec().lookup_rule("R-NONE-999").is_none());
    }

    // -- filters --
    #[test]
    fn test_normative_rules() {
        let s = spec();
        let norms = s.normative_rules();
        assert!(norms.len() >= 9); // most are normative
        assert!(norms.iter().all(|r| r.normativity == Normativity::Normative));
    }

    #[test]
    fn test_deprecated_rules() {
        let s = spec();
        let dep = s.deprecated_rules();
        assert_eq!(dep.len(), 1);
        assert_eq!(dep[0].id.0, "R-CONC-D01");
    }

    #[test]
    fn test_experimental_rules() {
        let s = spec();
        let exp = s.experimental_rules();
        assert_eq!(exp.len(), 1);
        assert_eq!(exp[0].id.0, "R-CAP-001");
    }

    // -- chapter_by_number --
    #[test]
    fn test_chapter_by_number() {
        let s = spec();
        let ch = s.chapter_by_number(2).unwrap();
        assert_eq!(ch.title, "Type System");
    }

    #[test]
    fn test_chapter_by_number_missing() {
        assert!(spec().chapter_by_number(99).is_none());
    }

    // -- search --
    #[test]
    fn test_search_rules() {
        let s = spec();
        let results = s.search_rules("borrow");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id.0, "R-OWN-002");
    }

    #[test]
    fn test_search_case_insensitive() {
        let s = spec();
        assert!(!s.search_rules("UNICODE").is_empty());
    }

    #[test]
    fn test_search_no_results() {
        assert!(spec().search_rules("zzznotfound").is_empty());
    }

    // -- chapter rule_count --
    #[test]
    fn test_chapter_rule_count() {
        let s = spec();
        let ch1 = s.chapter_by_number(1).unwrap();
        assert_eq!(ch1.rule_count(), 3);
    }

    // -- all_rules --
    #[test]
    fn test_all_rules_count() {
        assert_eq!(spec().all_rules().len(), 12);
    }

    // -- table_of_contents --
    #[test]
    fn test_toc_contains_chapters() {
        let toc = table_of_contents(&spec());
        assert!(toc.contains("Lexical Structure"));
        assert!(toc.contains("Type System"));
        assert!(toc.contains("Expressions"));
        assert!(toc.contains("Concurrency"));
    }

    #[test]
    fn test_toc_contains_normativity() {
        let toc = table_of_contents(&spec());
        assert!(toc.contains("[normative]"));
        assert!(toc.contains("[experimental]"));
        assert!(toc.contains("[deprecated]"));
    }

    // -- builder helpers --
    #[test]
    fn test_make_rule() {
        let r = make_rule("X-1", "Title", "Desc", Normativity::Informative);
        assert_eq!(r.id.0, "X-1");
        assert_eq!(r.normativity, Normativity::Informative);
    }

    #[test]
    fn test_make_section() {
        let s = make_section("s1", "Sec", Normativity::Normative, vec![]);
        assert!(s.rules.is_empty());
    }

    #[test]
    fn test_make_chapter() {
        let c = make_chapter("c1", 1, "Ch", vec![]);
        assert_eq!(c.rule_count(), 0);
    }

    // -- SpecVersion equality --
    #[test]
    fn test_spec_version_eq() {
        let a = SpecVersion::new(1, 0, "2026");
        let b = SpecVersion::new(1, 0, "2026");
        assert_eq!(a, b);
    }

    // -- RuleId / SectionId / ChapterId hashing --
    #[test]
    fn test_ids_hashable() {
        let mut set = std::collections::HashSet::new();
        set.insert(RuleId("R-1".into()));
        set.insert(RuleId("R-2".into()));
        assert_eq!(set.len(), 2);
    }
}

//! Standard Abbreviation Registry v1
//!
//! Deterministic, versioned compact forms for all standard library types, traits,
//! derives, keywords, and attributes. This is the single source of truth for
//! compact ↔ expanded mappings used by the lexer, parser, `rust2redox`, and
//! `redoxfmt` tools.
//!
//! Reference: REDOX_PROPOSAL.md §5.5.6

// ---------------------------------------------------------------------------
// Registry version
// ---------------------------------------------------------------------------

/// Current registry version.
pub const REGISTRY_VERSION: &str = "1.0";

// ---------------------------------------------------------------------------
// Entry type
// ---------------------------------------------------------------------------

/// A single abbreviation entry: maps a compact form to an expanded form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbbrevEntry {
    /// The compact (Redox) form, e.g. `"Cl"`.
    pub compact: &'static str,
    /// The expanded (Rust) form, e.g. `"Clone"`.
    pub expanded: &'static str,
}

/// Category of abbreviation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbbrevCategory {
    Type,
    Trait,
    Keyword,
    Attribute,
    Macro,
}

// ---------------------------------------------------------------------------
// Type abbreviations (§5.5.6)
// ---------------------------------------------------------------------------

/// Standard type abbreviations.
///
/// For generic types, the compact form shows the *wrapper* syntax only.
/// The inner type parameter(s) are handled by the parser separately.
///
/// | Rust               | Redox          | Notes                    |
/// |--------------------|----------------|--------------------------|
/// | `Vec<T>`           | `[T]~`         | tilde marks owned vec    |
/// | `Option<T>`        | `?T`           | prefix `?`               |
/// | `Result<T, E>`     | `R[T,E]`       |                          |
/// | `Box<T>`           | `^T`           | prefix `^`               |
/// | `Arc<T>`           | `@T`           | prefix `@`               |
/// | `Rc<T>`            | `$T`           | prefix `$`               |
/// | `HashMap<K, V>`    | `{K,V}`        | curly braces             |
/// | `String`           | `s""`          |                          |
/// | `&str`             | `&s`           |                          |
/// | `Cow<T>`           | `&~T`          | prefix `&~`              |
/// | `Cell<T>`          | `%T`           | prefix `%`               |
/// | `RefCell<T>`       | `%!T`          | prefix `%!`              |
/// | `Mutex<T>`         | `#T`           | prefix `#`               |
/// | `RwLock<T>`        | `#~T`          | prefix `#~`              |
/// | `Pin<T>`           | *(eliminated)* | compiler handles pinning |
/// | `PhantomData<T>`   | *(eliminated)* | compiler infers variance |
pub const TYPE_ABBREVS: &[AbbrevEntry] = &[
    AbbrevEntry { compact: "[T]~",   expanded: "Vec<T>" },
    AbbrevEntry { compact: "?T",     expanded: "Option<T>" },
    AbbrevEntry { compact: "R[T,E]", expanded: "Result<T,E>" },
    AbbrevEntry { compact: "^T",     expanded: "Box<T>" },
    AbbrevEntry { compact: "@T",     expanded: "Arc<T>" },
    AbbrevEntry { compact: "$T",     expanded: "Rc<T>" },
    AbbrevEntry { compact: "{K,V}",  expanded: "HashMap<K,V>" },
    AbbrevEntry { compact: "s\"\"",  expanded: "String" },
    AbbrevEntry { compact: "&s",     expanded: "&str" },
    AbbrevEntry { compact: "&~T",    expanded: "Cow<T>" },
    AbbrevEntry { compact: "%T",     expanded: "Cell<T>" },
    AbbrevEntry { compact: "%!T",    expanded: "RefCell<T>" },
    AbbrevEntry { compact: "#T",     expanded: "Mutex<T>" },
    AbbrevEntry { compact: "#~T",    expanded: "RwLock<T>" },
];

// ---------------------------------------------------------------------------
// Trait abbreviations (§5.5.6)
// ---------------------------------------------------------------------------

/// Standard trait abbreviations, used in derive macros and trait bounds.
///
/// | Rust           | Redox   |
/// |----------------|---------|
/// | `Clone`        | `Cl`    |
/// | `Debug`        | `Db`    |
/// | `Display`      | `Disp`  |
/// | `Default`      | `Def`   |
/// | `PartialEq`    | `PEq`   |
/// | `Eq`           | `Eq`    |
/// | `PartialOrd`   | `POrd`  |
/// | `Ord`          | `Ord`   |
/// | `Hash`         | `H`     |
/// | `Copy`         | `Cp`    |
/// | `Serialize`    | `Ser`   |
/// | `Deserialize`  | `De`    |
/// | `Iterator`     | `Iter`  |
/// | `From<T>`      | `Fr[T]` |
/// | `Into<T>`      | `In[T]` |
/// | `TryFrom<T>`   | `TFr[T]`|
/// | `AsRef<T>`     | `AR[T]` |
/// | `Deref`        | `Dr`    |
/// | `DerefMut`     | `DrM`   |
pub const TRAIT_ABBREVS: &[AbbrevEntry] = &[
    AbbrevEntry { compact: "Cl",     expanded: "Clone" },
    AbbrevEntry { compact: "Db",     expanded: "Debug" },
    AbbrevEntry { compact: "Disp",   expanded: "Display" },
    AbbrevEntry { compact: "Def",    expanded: "Default" },
    AbbrevEntry { compact: "PEq",    expanded: "PartialEq" },
    AbbrevEntry { compact: "Eq",     expanded: "Eq" },
    AbbrevEntry { compact: "POrd",   expanded: "PartialOrd" },
    AbbrevEntry { compact: "Ord",    expanded: "Ord" },
    AbbrevEntry { compact: "H",      expanded: "Hash" },
    AbbrevEntry { compact: "Cp",     expanded: "Copy" },
    AbbrevEntry { compact: "Ser",    expanded: "Serialize" },
    AbbrevEntry { compact: "De",     expanded: "Deserialize" },
    AbbrevEntry { compact: "Iter",   expanded: "Iterator" },
    AbbrevEntry { compact: "Fr[T]",  expanded: "From<T>" },
    AbbrevEntry { compact: "In[T]",  expanded: "Into<T>" },
    AbbrevEntry { compact: "TFr[T]", expanded: "TryFrom<T>" },
    AbbrevEntry { compact: "AR[T]",  expanded: "AsRef<T>" },
    AbbrevEntry { compact: "Dr",     expanded: "Deref" },
    AbbrevEntry { compact: "DrM",    expanded: "DerefMut" },
];

// ---------------------------------------------------------------------------
// Keyword abbreviations (§5.5.1)
// ---------------------------------------------------------------------------

/// Multi-word keyword abbreviations (must be matched before single-word).
pub const KEYWORD_MULTI_ABBREVS: &[AbbrevEntry] = &[
    AbbrevEntry { compact: "+af",  expanded: "pub async fn" },
    AbbrevEntry { compact: "~f",   expanded: "pub(crate) fn" },
    AbbrevEntry { compact: "+f",   expanded: "pub fn" },
    AbbrevEntry { compact: "+S",   expanded: "pub struct" },
    AbbrevEntry { compact: "+E",   expanded: "pub enum" },
    AbbrevEntry { compact: "+T",   expanded: "pub trait" },
    AbbrevEntry { compact: "+M",   expanded: "pub mod" },
    AbbrevEntry { compact: "af",   expanded: "async fn" },
    AbbrevEntry { compact: "uf",   expanded: "unsafe fn" },
    AbbrevEntry { compact: "m",    expanded: "let mut" },
];

/// Single-word keyword abbreviations.
pub const KEYWORD_SINGLE_ABBREVS: &[AbbrevEntry] = &[
    AbbrevEntry { compact: "f",  expanded: "fn" },
    AbbrevEntry { compact: "S",  expanded: "struct" },
    AbbrevEntry { compact: "E",  expanded: "enum" },
    AbbrevEntry { compact: "I",  expanded: "impl" },
    AbbrevEntry { compact: "T",  expanded: "trait" },
    AbbrevEntry { compact: "Y",  expanded: "type" },
    AbbrevEntry { compact: "C",  expanded: "const" },
    AbbrevEntry { compact: "Z",  expanded: "static" },
    AbbrevEntry { compact: "v",  expanded: "let" },
    AbbrevEntry { compact: "^",  expanded: "return" },
    AbbrevEntry { compact: "M",  expanded: "mod" },
];

// ---------------------------------------------------------------------------
// Attribute abbreviations (§5.5.2)
// ---------------------------------------------------------------------------

/// Attribute abbreviations.
///
/// | Rust                    | Redox               |
/// |-------------------------|---------------------|
/// | `#[derive(...)]`        | `@d(...)`           |
/// | `#[cfg(...)]`           | `@cfg(...)`         |
/// | `#[allow(...)]`         | `@a(...)`           |
/// | `#[deny(...)]`          | `@x(...)`           |
/// | `#[repr(...)]`          | `@r(...)`           |
/// | `#[inline(always)]`     | `@i!`               |
/// | `#[test]`               | `@t`                |
/// | `#[bench]`              | `@b`                |
/// | `#[must_use]`           | `@mu`               |
pub const ATTR_ABBREVS: &[AbbrevEntry] = &[
    AbbrevEntry { compact: "@d",   expanded: "#[derive]" },
    AbbrevEntry { compact: "@cfg", expanded: "#[cfg]" },
    AbbrevEntry { compact: "@a",   expanded: "#[allow]" },
    AbbrevEntry { compact: "@x",   expanded: "#[deny]" },
    AbbrevEntry { compact: "@r",   expanded: "#[repr]" },
    AbbrevEntry { compact: "@i!",  expanded: "#[inline(always)]" },
    AbbrevEntry { compact: "@t",   expanded: "#[test]" },
    AbbrevEntry { compact: "@b",   expanded: "#[bench]" },
    AbbrevEntry { compact: "@mu",  expanded: "#[must_use]" },
];

// ---------------------------------------------------------------------------
// Macro abbreviations
// ---------------------------------------------------------------------------

/// Macro abbreviations.
pub const MACRO_ABBREVS: &[AbbrevEntry] = &[
    AbbrevEntry { compact: "??",  expanded: "todo!()" },
    AbbrevEntry { compact: "???", expanded: "unimplemented!()" },
];

// ---------------------------------------------------------------------------
// Lint abbreviations (used inside @a(...) / @x(...))
// ---------------------------------------------------------------------------

/// Lint name abbreviations for `allow`/`deny`/`warn` attributes.
pub const LINT_ABBREVS: &[AbbrevEntry] = &[
    AbbrevEntry { compact: "un", expanded: "unused" },
    AbbrevEntry { compact: "dc", expanded: "dead_code" },
    AbbrevEntry { compact: "uc", expanded: "unsafe_code" },
    AbbrevEntry { compact: "ui", expanded: "unused_imports" },
    AbbrevEntry { compact: "uv", expanded: "unused_variables" },
];

// ---------------------------------------------------------------------------
// Safety-eliminated forms (§5.6.1)
// ---------------------------------------------------------------------------

/// Types/traits that are *eliminated* (not abbreviated) in Redox because the
/// compiler handles them automatically.
pub const ELIMINATED_FORMS: &[&str] = &[
    "Pin",         // compiler handles pinning
    "PhantomData", // compiler infers variance
    "Send",        // implicit, validated via SKB
    "Sync",        // implicit, validated via SKB
    "Copy",        // auto-derived where applicable
];

// ---------------------------------------------------------------------------
// Lookup functions
// ---------------------------------------------------------------------------

/// Look up the expanded (Rust) form for a compact (Redox) form in the given table.
pub fn lookup_expanded(table: &[AbbrevEntry], compact: &str) -> Option<&'static str> {
    table.iter().find(|e| e.compact == compact).map(|e| e.expanded)
}

/// Look up the compact (Redox) form for an expanded (Rust) form in the given table.
pub fn lookup_compact(table: &[AbbrevEntry], expanded: &str) -> Option<&'static str> {
    table.iter().find(|e| e.expanded == expanded).map(|e| e.compact)
}

/// Look up a trait abbreviation by expanded name (e.g. `"Clone"` → `"Cl"`).
pub fn compact_trait(expanded: &str) -> Option<&'static str> {
    lookup_compact(TRAIT_ABBREVS, expanded)
}

/// Look up a trait expansion by compact name (e.g. `"Cl"` → `"Clone"`).
pub fn expand_trait(compact: &str) -> Option<&'static str> {
    lookup_expanded(TRAIT_ABBREVS, compact)
}

/// Check if a type/trait name is eliminated in Redox.
pub fn is_eliminated(name: &str) -> bool {
    ELIMINATED_FORMS.contains(&name)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Type abbreviations -------------------------------------------------

    #[test]
    fn type_table_has_all_core_types() {
        let names: Vec<&str> = TYPE_ABBREVS.iter().map(|e| e.expanded).collect();
        assert!(names.contains(&"Vec<T>"));
        assert!(names.contains(&"Option<T>"));
        assert!(names.contains(&"Result<T,E>"));
        assert!(names.contains(&"Box<T>"));
        assert!(names.contains(&"Arc<T>"));
        assert!(names.contains(&"Rc<T>"));
        assert!(names.contains(&"HashMap<K,V>"));
        assert!(names.contains(&"String"));
        assert!(names.contains(&"&str"));
        assert!(names.contains(&"Cow<T>"));
        assert!(names.contains(&"Cell<T>"));
        assert!(names.contains(&"RefCell<T>"));
        assert!(names.contains(&"Mutex<T>"));
        assert!(names.contains(&"RwLock<T>"));
    }

    #[test]
    fn type_lookup_compact() {
        assert_eq!(lookup_compact(TYPE_ABBREVS, "Vec<T>"), Some("[T]~"));
        assert_eq!(lookup_compact(TYPE_ABBREVS, "Option<T>"), Some("?T"));
        assert_eq!(lookup_compact(TYPE_ABBREVS, "HashMap<K,V>"), Some("{K,V}"));
        assert_eq!(lookup_compact(TYPE_ABBREVS, "String"), Some("s\"\""));
        assert_eq!(lookup_compact(TYPE_ABBREVS, "&str"), Some("&s"));
        assert_eq!(lookup_compact(TYPE_ABBREVS, "Cow<T>"), Some("&~T"));
        assert_eq!(lookup_compact(TYPE_ABBREVS, "Cell<T>"), Some("%T"));
        assert_eq!(lookup_compact(TYPE_ABBREVS, "RefCell<T>"), Some("%!T"));
        assert_eq!(lookup_compact(TYPE_ABBREVS, "Mutex<T>"), Some("#T"));
        assert_eq!(lookup_compact(TYPE_ABBREVS, "RwLock<T>"), Some("#~T"));
    }

    #[test]
    fn type_lookup_expanded() {
        assert_eq!(lookup_expanded(TYPE_ABBREVS, "[T]~"), Some("Vec<T>"));
        assert_eq!(lookup_expanded(TYPE_ABBREVS, "?T"), Some("Option<T>"));
        assert_eq!(lookup_expanded(TYPE_ABBREVS, "^T"), Some("Box<T>"));
        assert_eq!(lookup_expanded(TYPE_ABBREVS, "@T"), Some("Arc<T>"));
        assert_eq!(lookup_expanded(TYPE_ABBREVS, "$T"), Some("Rc<T>"));
    }

    #[test]
    fn type_lookup_not_found() {
        assert_eq!(lookup_compact(TYPE_ABBREVS, "BTreeMap<K,V>"), None);
        assert_eq!(lookup_expanded(TYPE_ABBREVS, "???"), None);
    }

    // -- Trait abbreviations ------------------------------------------------

    #[test]
    fn trait_table_has_all_core_traits() {
        let names: Vec<&str> = TRAIT_ABBREVS.iter().map(|e| e.expanded).collect();
        assert!(names.contains(&"Clone"));
        assert!(names.contains(&"Debug"));
        assert!(names.contains(&"Display"));
        assert!(names.contains(&"Default"));
        assert!(names.contains(&"PartialEq"));
        assert!(names.contains(&"Eq"));
        assert!(names.contains(&"PartialOrd"));
        assert!(names.contains(&"Ord"));
        assert!(names.contains(&"Hash"));
        assert!(names.contains(&"Copy"));
        assert!(names.contains(&"Serialize"));
        assert!(names.contains(&"Deserialize"));
        assert!(names.contains(&"Iterator"));
        assert!(names.contains(&"From<T>"));
        assert!(names.contains(&"Into<T>"));
        assert!(names.contains(&"TryFrom<T>"));
        assert!(names.contains(&"AsRef<T>"));
        assert!(names.contains(&"Deref"));
        assert!(names.contains(&"DerefMut"));
    }

    #[test]
    fn trait_compact_and_expand() {
        assert_eq!(compact_trait("Clone"), Some("Cl"));
        assert_eq!(compact_trait("Debug"), Some("Db"));
        assert_eq!(compact_trait("Default"), Some("Def"));
        assert_eq!(compact_trait("PartialEq"), Some("PEq"));
        assert_eq!(compact_trait("Hash"), Some("H"));
        assert_eq!(compact_trait("Serialize"), Some("Ser"));
        assert_eq!(compact_trait("Deserialize"), Some("De"));
        assert_eq!(compact_trait("Deref"), Some("Dr"));
        assert_eq!(compact_trait("DerefMut"), Some("DrM"));

        assert_eq!(expand_trait("Cl"), Some("Clone"));
        assert_eq!(expand_trait("Db"), Some("Debug"));
        assert_eq!(expand_trait("Def"), Some("Default"));
        assert_eq!(expand_trait("Iter"), Some("Iterator"));
        assert_eq!(expand_trait("Dr"), Some("Deref"));
    }

    #[test]
    fn trait_lookup_unknown() {
        assert_eq!(compact_trait("MyCustomTrait"), None);
        assert_eq!(expand_trait("Xyz"), None);
    }

    // -- Keyword abbreviations ----------------------------------------------

    #[test]
    fn keyword_multi_word() {
        assert_eq!(lookup_expanded(KEYWORD_MULTI_ABBREVS, "+af"), Some("pub async fn"));
        assert_eq!(lookup_expanded(KEYWORD_MULTI_ABBREVS, "~f"), Some("pub(crate) fn"));
        assert_eq!(lookup_expanded(KEYWORD_MULTI_ABBREVS, "+f"), Some("pub fn"));
        assert_eq!(lookup_expanded(KEYWORD_MULTI_ABBREVS, "m"), Some("let mut"));
    }

    #[test]
    fn keyword_single_word() {
        assert_eq!(lookup_expanded(KEYWORD_SINGLE_ABBREVS, "f"), Some("fn"));
        assert_eq!(lookup_expanded(KEYWORD_SINGLE_ABBREVS, "S"), Some("struct"));
        assert_eq!(lookup_expanded(KEYWORD_SINGLE_ABBREVS, "E"), Some("enum"));
        assert_eq!(lookup_expanded(KEYWORD_SINGLE_ABBREVS, "I"), Some("impl"));
        assert_eq!(lookup_expanded(KEYWORD_SINGLE_ABBREVS, "T"), Some("trait"));
        assert_eq!(lookup_expanded(KEYWORD_SINGLE_ABBREVS, "Y"), Some("type"));
        assert_eq!(lookup_expanded(KEYWORD_SINGLE_ABBREVS, "C"), Some("const"));
        assert_eq!(lookup_expanded(KEYWORD_SINGLE_ABBREVS, "Z"), Some("static"));
        assert_eq!(lookup_expanded(KEYWORD_SINGLE_ABBREVS, "v"), Some("let"));
        assert_eq!(lookup_expanded(KEYWORD_SINGLE_ABBREVS, "^"), Some("return"));
    }

    #[test]
    fn keyword_compact_lookup() {
        assert_eq!(lookup_compact(KEYWORD_MULTI_ABBREVS, "pub fn"), Some("+f"));
        assert_eq!(lookup_compact(KEYWORD_SINGLE_ABBREVS, "fn"), Some("f"));
        assert_eq!(lookup_compact(KEYWORD_SINGLE_ABBREVS, "struct"), Some("S"));
    }

    // -- Attribute abbreviations --------------------------------------------

    #[test]
    fn attr_abbreviations() {
        assert_eq!(lookup_expanded(ATTR_ABBREVS, "@d"), Some("#[derive]"));
        assert_eq!(lookup_expanded(ATTR_ABBREVS, "@t"), Some("#[test]"));
        assert_eq!(lookup_expanded(ATTR_ABBREVS, "@b"), Some("#[bench]"));
        assert_eq!(lookup_expanded(ATTR_ABBREVS, "@mu"), Some("#[must_use]"));
        assert_eq!(lookup_expanded(ATTR_ABBREVS, "@i!"), Some("#[inline(always)]"));
    }

    #[test]
    fn attr_compact_lookup() {
        assert_eq!(lookup_compact(ATTR_ABBREVS, "#[test]"), Some("@t"));
        assert_eq!(lookup_compact(ATTR_ABBREVS, "#[derive]"), Some("@d"));
    }

    // -- Macro abbreviations ------------------------------------------------

    #[test]
    fn macro_abbreviations() {
        assert_eq!(lookup_expanded(MACRO_ABBREVS, "??"), Some("todo!()"));
        assert_eq!(lookup_expanded(MACRO_ABBREVS, "???"), Some("unimplemented!()"));
        assert_eq!(lookup_compact(MACRO_ABBREVS, "todo!()"), Some("??"));
    }

    // -- Lint abbreviations -------------------------------------------------

    #[test]
    fn lint_abbreviations() {
        assert_eq!(lookup_expanded(LINT_ABBREVS, "un"), Some("unused"));
        assert_eq!(lookup_expanded(LINT_ABBREVS, "dc"), Some("dead_code"));
        assert_eq!(lookup_compact(LINT_ABBREVS, "unused_imports"), Some("ui"));
    }

    // -- Elimination --------------------------------------------------------

    #[test]
    fn eliminated_forms() {
        assert!(is_eliminated("Pin"));
        assert!(is_eliminated("PhantomData"));
        assert!(is_eliminated("Send"));
        assert!(is_eliminated("Sync"));
        assert!(is_eliminated("Copy"));
        assert!(!is_eliminated("Clone"));
        assert!(!is_eliminated("Vec"));
    }

    // -- Registry version ---------------------------------------------------

    #[test]
    fn registry_version() {
        assert_eq!(REGISTRY_VERSION, "1.0");
    }

    // -- No duplicate compact forms within a table --------------------------

    #[test]
    fn no_duplicate_compact_in_types() {
        let mut seen = std::collections::HashSet::new();
        for e in TYPE_ABBREVS {
            assert!(seen.insert(e.compact), "duplicate compact: {}", e.compact);
        }
    }

    #[test]
    fn no_duplicate_compact_in_traits() {
        let mut seen = std::collections::HashSet::new();
        for e in TRAIT_ABBREVS {
            assert!(seen.insert(e.compact), "duplicate compact: {}", e.compact);
        }
    }

    #[test]
    fn no_duplicate_compact_in_keywords() {
        let mut seen = std::collections::HashSet::new();
        for e in KEYWORD_MULTI_ABBREVS.iter().chain(KEYWORD_SINGLE_ABBREVS) {
            assert!(seen.insert(e.compact), "duplicate compact: {}", e.compact);
        }
    }

    #[test]
    fn no_duplicate_compact_in_attrs() {
        let mut seen = std::collections::HashSet::new();
        for e in ATTR_ABBREVS {
            assert!(seen.insert(e.compact), "duplicate compact: {}", e.compact);
        }
    }
}

//! Keyword compression pass: Rust keywords → Redox compact forms (§5.5.1).

/// Compress Rust keywords to their Redox compact equivalents.
///
/// Ordering matters: multi-word forms must be matched before single-word forms.
pub fn compress_keywords(input: &str) -> String {
    let mut result = input.to_string();

    // Multi-word first (longer patterns before shorter to avoid partial matches)
    let replacements: &[(&str, &str)] = &[
        // 3-word visibility+qualifier+keyword
        ("pub async fn", "+af"),
        // 2-word visibility+keyword
        ("pub(crate) fn", "~f"),
        ("pub fn", "+f"),
        ("pub struct", "+S"),
        ("pub enum", "+E"),
        ("pub trait", "+T"),
        ("pub mod", "+M"),
        // 2-word qualifier+keyword
        ("async fn", "af"),
        ("unsafe fn", "uf"),
        ("let mut", "m"),
    ];

    for &(from, to) in replacements {
        result = replace_keyword(&result, from, to);
    }

    // Single-word keywords (must come after multi-word to avoid conflicts)
    let single_replacements: &[(&str, &str)] = &[
        ("fn", "f"),
        ("struct", "S"),
        ("enum", "E"),
        ("impl", "I"),
        ("trait", "T"),
        ("type", "Y"),
        ("const", "C"),
        ("static", "Z"),
        ("let", "v"),
        ("return", "^"),
        ("mod", "M"),
    ];

    for &(from, to) in single_replacements {
        result = replace_keyword(&result, from, to);
    }

    // Macro abbreviations
    result = result.replace("todo!()", "??");
    result = result.replace("unimplemented!()", "???");

    result
}

/// Replace a keyword `from` with `to`, ensuring word-boundary matching.
///
/// A keyword match requires that the character before `from` (if any) is not
/// alphanumeric/underscore, and the character after is not alphanumeric/underscore.
fn replace_keyword(input: &str, from: &str, to: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(pos) = remaining.find(from) {
        let before_ok = if pos == 0 {
            true
        } else {
            let ch = remaining.as_bytes()[pos - 1];
            !ch.is_ascii_alphanumeric() && ch != b'_'
        };

        let after_pos = pos + from.len();
        let after_ok = if after_pos >= remaining.len() {
            true
        } else {
            let ch = remaining.as_bytes()[after_pos];
            !ch.is_ascii_alphanumeric() && ch != b'_'
        };

        if before_ok && after_ok {
            result.push_str(&remaining[..pos]);
            result.push_str(to);
            remaining = &remaining[after_pos..];
        } else {
            // Not a word boundary match — skip past this occurrence
            result.push_str(&remaining[..pos + from.len()]);
            remaining = &remaining[after_pos..];
        }
    }

    result.push_str(remaining);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_replace_inside_identifiers() {
        // "fn" inside "function" should not be replaced
        assert_eq!(compress_keywords("function"), "function");
    }

    #[test]
    fn replaces_at_boundaries() {
        assert_eq!(compress_keywords("fn foo"), "f foo");
        assert_eq!(compress_keywords("(fn)"), "(f)");
    }

    #[test]
    fn multiple_keywords_in_line() {
        assert_eq!(
            compress_keywords("pub fn foo() { let mut x = 1; return x; }"),
            "+f foo() { m x = 1; ^ x; }"
        );
    }

    #[test]
    fn preserves_non_keyword_text() {
        assert_eq!(compress_keywords("hello world"), "hello world");
    }
}

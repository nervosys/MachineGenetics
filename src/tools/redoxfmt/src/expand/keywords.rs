//! Expand keyword abbreviations back to full Rust keywords.

/// Expand Redox keyword abbreviations to their Rust equivalents.
pub fn expand_keywords(input: &str) -> String {
    let mut result = input.to_string();

    // Multi-word expansions first (longest match)
    result = replace_keyword_start(&result, "+af ", "pub async fn ");
    result = replace_keyword_start(&result, "+uf ", "pub unsafe fn ");
    result = replace_keyword_start(&result, "+f ", "pub fn ");
    result = replace_keyword_start(&result, "~f ", "pub(crate) fn ");
    result = replace_keyword_start(&result, "af ", "async fn ");
    result = replace_keyword_start(&result, "uf ", "unsafe fn ");
    result = replace_keyword_start(&result, "+S ", "pub struct ");
    result = replace_keyword_start(&result, "+E ", "pub enum ");
    result = replace_keyword_start(&result, "+T ", "pub trait ");
    result = replace_keyword_start(&result, "+M ", "pub mod ");
    result = replace_keyword_start(&result, "+C ", "pub const ");

    // Single keyword expansions
    result = replace_keyword_start(&result, "f ", "fn ");
    result = replace_keyword_start(&result, "S ", "struct ");
    result = replace_keyword_start(&result, "E ", "enum ");
    result = replace_keyword_start(&result, "I ", "impl ");
    result = replace_keyword_start(&result, "T ", "trait ");
    result = replace_keyword_start(&result, "M ", "mod ");
    result = replace_keyword_start(&result, "C ", "const ");
    result = replace_keyword_start(&result, "Z ", "static ");
    result = replace_keyword_start(&result, "Y ", "type ");

    // let/let mut
    result = replace_keyword_start(&result, "m ", "let mut ");
    result = replace_keyword_start(&result, "v ", "let ");

    // return
    result = replace_keyword_only(&result, "^ ", "return ");

    // Macro abbreviations
    result = replace_word(&result, "???", "unimplemented!()");
    result = replace_word(&result, "??", "todo!()");

    result
}

/// Replace a keyword at the beginning of a line (or after certain boundaries).
fn replace_keyword_start(input: &str, from: &str, to: &str) -> String {
    let mut result = String::new();

    for line in input.split('\n') {
        if !result.is_empty() {
            result.push('\n');
        }

        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];

        if let Some(rest) = trimmed.strip_prefix(from) {
            result.push_str(indent);
            result.push_str(to);
            result.push_str(rest);
        } else {
            result.push_str(line);
        }
    }

    result
}

/// Replace a keyword only when surrounded by appropriate boundaries.
fn replace_keyword_only(input: &str, from: &str, to: &str) -> String {
    let mut result = String::new();

    for line in input.split('\n') {
        if !result.is_empty() {
            result.push('\n');
        }

        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];

        if let Some(rest) = trimmed.strip_prefix(from) {
            result.push_str(indent);
            result.push_str(to);
            result.push_str(rest);
        } else {
            result.push_str(line);
        }
    }

    result
}

/// Replace standalone word across text.
fn replace_word(input: &str, from: &str, to: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(pos) = remaining.find(from) {
        let after_pos = pos + from.len();

        let before_ok = if pos == 0 {
            true
        } else {
            let ch = remaining.as_bytes()[pos - 1];
            !ch.is_ascii_alphanumeric() && ch != b'_'
        };

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
            result.push_str(&remaining[..after_pos]);
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
    fn expand_pub_fn() {
        assert_eq!(expand_keywords("+f foo() {}"), "pub fn foo() {}");
    }

    #[test]
    fn expand_fn() {
        assert_eq!(expand_keywords("f foo() {}"), "fn foo() {}");
    }

    #[test]
    fn expand_let_mut() {
        assert_eq!(expand_keywords("m x = 5;"), "let mut x = 5;");
    }

    #[test]
    fn expand_let() {
        assert_eq!(expand_keywords("v x = 5;"), "let x = 5;");
    }

    #[test]
    fn expand_struct() {
        assert_eq!(expand_keywords("S Foo {}"), "struct Foo {}");
    }

    #[test]
    fn expand_pub_struct() {
        assert_eq!(expand_keywords("+S Foo {}"), "pub struct Foo {}");
    }

    #[test]
    fn expand_return() {
        assert_eq!(expand_keywords("^ x;"), "return x;");
    }

    #[test]
    fn expand_todo() {
        assert_eq!(expand_keywords("??"), "todo!()");
    }

    #[test]
    fn expand_unimplemented() {
        assert_eq!(expand_keywords("???"), "unimplemented!()");
    }
}

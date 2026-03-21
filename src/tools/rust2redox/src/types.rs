//! Type abbreviation pass: Rust standard types → Redox compact forms (§5.5.6).

/// Abbreviate standard library types to their Redox compact forms.
pub fn abbreviate_types(input: &str) -> String {
    let mut result = input.to_string();

    // Order matters: longer/more-specific patterns first

    // HashMap<K, V> → {K:V}  (two type args)
    result = replace_generic_2("HashMap", &result, |k, v| format!("{{{k},{v}}}"));

    // Result<T, E> → R[T,E]
    result = replace_generic_2("Result", &result, |t, e| format!("R[{t},{e}]"));

    // Vec<T> → [T]~
    result = replace_generic_1("Vec", &result, |t| format!("[{t}]~"));

    // Option<T> → ?T
    result = replace_generic_1("Option", &result, |t| format!("?{t}"));

    // Box<T> → ^T
    result = replace_generic_1("Box", &result, |t| format!("^{t}"));

    // Arc<T> → @T
    result = replace_generic_1("Arc", &result, |t| format!("@{t}"));

    // Rc<T> → $T
    result = replace_generic_1("Rc", &result, |t| format!("${t}"));

    // &str → &s
    result = replace_type_word(&result, "&str", "&s");

    // String → s""
    result = replace_standalone_type(&result, "String", "s\"\"");

    result
}

/// Replace `TypeName<T>` patterns with a one-arg compact form.
fn replace_generic_1(name: &str, input: &str, fmt: impl Fn(&str) -> String) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    let pattern = format!("{}<", name);
    while let Some(pos) = remaining.find(&pattern) {
        // Check word boundary before the type name
        if pos > 0 {
            let ch = remaining.as_bytes()[pos - 1];
            if ch.is_ascii_alphanumeric() || ch == b'_' {
                result.push_str(&remaining[..pos + pattern.len()]);
                remaining = &remaining[pos + pattern.len()..];
                continue;
            }
        }

        result.push_str(&remaining[..pos]);
        let after_angle = pos + pattern.len();

        if let Some((inner, consumed)) = extract_balanced_angle(&remaining[after_angle..]) {
            result.push_str(&fmt(inner.trim()));
            remaining = &remaining[after_angle + consumed..];
        } else {
            result.push_str(&remaining[pos..pos + pattern.len()]);
            remaining = &remaining[after_angle..];
        }
    }

    result.push_str(remaining);
    result
}

/// Replace `TypeName<K, V>` patterns with a two-arg compact form.
fn replace_generic_2(
    name: &str,
    input: &str,
    fmt: impl Fn(&str, &str) -> String,
) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    let pattern = format!("{}<", name);
    while let Some(pos) = remaining.find(&pattern) {
        if pos > 0 {
            let ch = remaining.as_bytes()[pos - 1];
            if ch.is_ascii_alphanumeric() || ch == b'_' {
                result.push_str(&remaining[..pos + pattern.len()]);
                remaining = &remaining[pos + pattern.len()..];
                continue;
            }
        }

        result.push_str(&remaining[..pos]);
        let after_angle = pos + pattern.len();

        if let Some((inner, consumed)) = extract_balanced_angle(&remaining[after_angle..]) {
            // Split on the first top-level comma
            if let Some((left, right)) = split_at_top_comma(inner) {
                result.push_str(&fmt(left.trim(), right.trim()));
            } else {
                // No comma — pass through
                result.push_str(&remaining[pos..after_angle + consumed]);
            }
            remaining = &remaining[after_angle + consumed..];
        } else {
            result.push_str(&remaining[pos..pos + pattern.len()]);
            remaining = &remaining[after_angle..];
        }
    }

    result.push_str(remaining);
    result
}

/// Extract content inside `<...>` respecting nesting. Returns (inner, bytes_consumed including `>`).
fn extract_balanced_angle(input: &str) -> Option<(& str, usize)> {
    let mut depth = 1i32;
    for (i, ch) in input.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&input[..i], i + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// Split at the first comma at depth 0.
fn split_at_top_comma(input: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (i, ch) in input.char_indices() {
        match ch {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                return Some((&input[..i], &input[i + 1..]));
            }
            _ => {}
        }
    }
    None
}

/// Replace a standalone type word (must be at word boundaries).
fn replace_standalone_type(input: &str, from: &str, to: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(pos) = remaining.find(from) {
        let before_ok = if pos == 0 {
            true
        } else {
            let ch = remaining.as_bytes()[pos - 1];
            !ch.is_ascii_alphanumeric() && ch != b'_' && ch != b'&'
        };

        let after_pos = pos + from.len();
        let after_ok = if after_pos >= remaining.len() {
            true
        } else {
            let ch = remaining.as_bytes()[after_pos];
            !ch.is_ascii_alphanumeric() && ch != b'_' && ch != b'<'
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

/// Replace a type reference (like `&str`) with exact matching.
fn replace_type_word(input: &str, from: &str, to: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(pos) = remaining.find(from) {
        let after_pos = pos + from.len();
        let after_ok = if after_pos >= remaining.len() {
            true
        } else {
            let ch = remaining.as_bytes()[after_pos];
            !ch.is_ascii_alphanumeric() && ch != b'_'
        };

        if after_ok {
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
    fn vec_abbreviation() {
        assert_eq!(abbreviate_types("Vec<u8>"), "[u8]~");
    }

    #[test]
    fn option_abbreviation() {
        assert_eq!(abbreviate_types("Option<i32>"), "?i32");
    }

    #[test]
    fn result_abbreviation() {
        assert_eq!(abbreviate_types("Result<T, E>"), "R[T,E]");
    }

    #[test]
    fn box_abbreviation() {
        assert_eq!(abbreviate_types("Box<Foo>"), "^Foo");
    }

    #[test]
    fn arc_abbreviation() {
        assert_eq!(abbreviate_types("Arc<Foo>"), "@Foo");
    }

    #[test]
    fn hashmap_abbreviation() {
        assert_eq!(abbreviate_types("HashMap<K, V>"), "{K,V}");
    }

    #[test]
    fn str_ref_abbreviation() {
        assert_eq!(abbreviate_types("&str"), "&s");
    }

    #[test]
    fn nested_type() {
        assert_eq!(abbreviate_types("Vec<Option<i32>>"), "[?i32]~");
    }

    #[test]
    fn no_match_passthrough() {
        assert_eq!(abbreviate_types("FooBar<T>"), "FooBar<T>");
    }
}

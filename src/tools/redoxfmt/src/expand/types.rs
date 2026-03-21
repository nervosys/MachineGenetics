//! Expand type abbreviations back to full Rust forms.

/// Expand all Redox type abbreviations to their Rust equivalents.
pub fn expand_types(input: &str) -> String {
    let mut result = input.to_string();

    // &! → &mut (must come before other & processing)
    result = expand_mut_ref(&result);

    // &s → &str (word boundary)
    result = replace_word(&result, "&s", "&str");

    // s"" → String (standalone)
    result = replace_standalone(&result, "s\"\"", "String");

    // [T]~ → Vec<T>
    result = expand_vec(&result);

    // ?T → Option<T>
    result = expand_prefix_type(&result, '?', "Option");

    // ^T → Box<T>
    result = expand_prefix_type(&result, '^', "Box");

    // @T → Arc<T>  (must not match @d, @t, @cfg etc. — those are attrs)
    result = expand_arc(&result);

    // $T → Rc<T>
    result = expand_prefix_type(&result, '$', "Rc");

    // R[T,E] → Result<T, E>
    result = expand_result(&result);

    // {K,V} → HashMap<K, V>
    result = expand_hashmap(&result);

    // ?? → todo!()
    result = replace_word(&result, "???", "unimplemented!()");
    result = replace_word(&result, "??", "todo!()");

    result
}

/// Expand `&!T` → `&mut T`.
fn expand_mut_ref(input: &str) -> String {
    input.replace("&!", "&mut ")
}

/// Expand `[T]~` → `Vec<T>`.
fn expand_vec(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(pos) = remaining.find('[') {
        result.push_str(&remaining[..pos]);
        let after = &remaining[pos + 1..];

        if let Some((inner, consumed)) = extract_until_tilde_bracket(after) {
            result.push_str(&format!("Vec<{inner}>"));
            remaining = &after[consumed..];
        } else {
            result.push('[');
            remaining = after;
        }
    }

    result.push_str(remaining);
    result
}

/// Find `INNER]~` and return (inner, bytes consumed including `]~`).
fn extract_until_tilde_bracket(input: &str) -> Option<(&str, usize)> {
    let mut depth = 1i32;
    for (i, ch) in input.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    // Check for trailing ~
                    if input.get(i + 1..i + 2) == Some("~") {
                        return Some((&input[..i], i + 2));
                    }
                    return None; // ] without ~ — not a Vec
                }
            }
            _ => {}
        }
    }
    None
}

/// Expand `?T`, `^T`, `$T` → `Option<T>`, `Box<T>`, `Rc<T>` etc.
/// T is read until a word-boundary character is hit (space, comma, paren, >, etc.)
fn expand_prefix_type(input: &str, prefix: char, type_name: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == prefix {
            // Check this isn't part of an identifier
            let before_ok = if i == 0 {
                true
            } else {
                let prev = chars[i - 1];
                !prev.is_ascii_alphanumeric() && prev != '_'
            };

            // After prefix, must be a type-start char
            let next_is_type = i + 1 < chars.len()
                && (chars[i + 1].is_ascii_alphabetic()
                    || chars[i + 1] == '('
                    || chars[i + 1] == '['
                    || chars[i + 1] == '?'
                    || chars[i + 1] == '^'
                    || chars[i + 1] == '@'
                    || chars[i + 1] == '$');

            if before_ok && next_is_type {
                // Read the type argument
                let start = i + 1;
                let inner = read_type_arg(&chars, start);
                let end = start + inner.chars().count();
                result.push_str(&format!("{type_name}<{inner}>"));
                i = end;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Expand `@T` → `Arc<T>`, avoiding attribute prefixes (`@d(`, `@t`, `@cfg` etc).
fn expand_arc(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '@' && i + 1 < chars.len() {
            // Don't expand if this is an attribute: @d(, @t, @cfg, @a(, @x(, @r(, @i!, @b, @mu
            let next = chars[i + 1];
            let is_attr = matches!(next, 'd' | 't' | 'c' | 'a' | 'x' | 'r' | 'i' | 'b' | 'm');

            let before_ok = if i == 0 {
                true
            } else {
                let prev = chars[i - 1];
                !prev.is_ascii_alphanumeric() && prev != '_'
            };

            if before_ok && !is_attr && next.is_ascii_uppercase() {
                let start = i + 1;
                let inner = read_type_arg(&chars, start);
                let end = start + inner.chars().count();
                result.push_str(&format!("Arc<{inner}>"));
                i = end;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Expand `R[T,E]` → `Result<T, E>`.
fn expand_result(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(pos) = remaining.find("R[") {
        // Check word boundary before R
        let before_ok = if pos == 0 {
            true
        } else {
            let ch = remaining.as_bytes()[pos - 1];
            !ch.is_ascii_alphanumeric() && ch != b'_'
        };

        if !before_ok {
            result.push_str(&remaining[..pos + 2]);
            remaining = &remaining[pos + 2..];
            continue;
        }

        result.push_str(&remaining[..pos]);
        let after = &remaining[pos + 2..];

        if let Some(bracket_end) = find_matching_bracket(after, '[', ']') {
            let inner = &after[..bracket_end];
            if let Some(comma) = find_top_comma(inner) {
                let left = inner[..comma].trim();
                let right = inner[comma + 1..].trim();
                result.push_str(&format!("Result<{left}, {right}>"));
            } else {
                result.push_str(&format!("Result<{inner}>"));
            }
            remaining = &after[bracket_end + 1..];
        } else {
            result.push_str("R[");
            remaining = after;
        }
    }

    result.push_str(remaining);
    result
}

/// Expand `{K,V}` → `HashMap<K, V>`.
fn expand_hashmap(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(pos) = remaining.find('{') {
        result.push_str(&remaining[..pos]);
        let after = &remaining[pos + 1..];

        if let Some(brace_end) = find_matching_bracket(after, '{', '}') {
            let inner = &after[..brace_end];
            // Must have a comma and NO colon (colon indicates struct body, not HashMap)
            if !inner.contains(':') {
                if let Some(comma) = find_top_comma(inner) {
                    let left = inner[..comma].trim();
                    let right = inner[comma + 1..].trim();
                    result.push_str(&format!("HashMap<{left}, {right}>"));
                    remaining = &after[brace_end + 1..];
                } else {
                    result.push('{');
                    remaining = after;
                }
            } else {
                result.push('{');
                remaining = after;
            }
        } else {
            result.push('{');
            remaining = after;
        }
    }

    result.push_str(remaining);
    result
}

/// Read a type argument starting at `start`, handling nesting.
fn read_type_arg(chars: &[char], start: usize) -> String {
    let mut result = String::new();
    let mut depth = 0i32;
    let mut i = start;

    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '<' | '(' | '[' => {
                depth += 1;
                result.push(ch);
            }
            '>' | ')' | ']' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                result.push(ch);
            }
            ',' | ' ' | ';' | '{' | '}' if depth == 0 => break,
            _ => result.push(ch),
        }
        i += 1;
    }

    result
}

fn find_matching_bracket(input: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 1i32;
    for (i, ch) in input.char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

fn find_top_comma(input: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in input.char_indices() {
        match ch {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Replace a standalone word, with word-boundary checking.
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

/// Replace standalone value (not preceded by &).
fn replace_standalone(input: &str, from: &str, to: &str) -> String {
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
    fn expand_vec() {
        assert_eq!(expand_types("[u8]~"), "Vec<u8>");
    }

    #[test]
    fn expand_option() {
        assert_eq!(expand_types("?i32"), "Option<i32>");
    }

    #[test]
    fn expand_result() {
        assert_eq!(expand_types("R[T,E]"), "Result<T, E>");
    }

    #[test]
    fn expand_box() {
        assert_eq!(expand_types("^Foo"), "Box<Foo>");
    }

    #[test]
    fn expand_arc_upper() {
        assert_eq!(expand_types("@Foo"), "Arc<Foo>");
    }

    #[test]
    fn expand_hashmap() {
        assert_eq!(expand_types("{K,V}"), "HashMap<K, V>");
    }

    #[test]
    fn expand_mut() {
        assert_eq!(expand_types("&!T"), "&mut T");
    }

    #[test]
    fn expand_str() {
        assert_eq!(expand_types("&s"), "&str");
    }

    #[test]
    fn expand_string() {
        assert_eq!(expand_types("s\"\""), "String");
    }

    #[test]
    fn expand_nested() {
        assert_eq!(expand_types("[?i32]~"), "Vec<Option<i32>>");
    }
}

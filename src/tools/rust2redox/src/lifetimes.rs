//! Lifetime/borrow elision pass: strip lifetime parameters and annotations.
//!
//! In Redox, lifetimes are compiler-inferred (§5.6.1). This pass:
//! - Removes lifetime parameters from generics: `<'a>`, `<'a, 'b>`, `<'a, T>`→`<T>`
//! - Removes lifetime annotations from references: `&'a T` → `&T`
//! - Converts `&mut T` → `&!T` (Redox mutability shorthand)

/// Elide all lifetime annotations and compress `&mut` to `&!`.
pub fn elide_lifetimes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Match `&'lifetime` references (e.g., `&'a T`, `&'a mut T`)
        if chars[i] == '&' && i + 1 < len && chars[i + 1] == '\'' {
            result.push('&');
            // Skip the lifetime: `'identifier`
            i += 2; // skip `&'`
            // Skip the lifetime name (alphanumeric + _)
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            // Skip whitespace after the lifetime
            while i < len && chars[i] == ' ' {
                i += 1;
            }
            // Check for `mut` after the lifetime
            if i + 3 <= len && &input[i..i + 3] == "mut" {
                let after_mut = i + 3;
                if after_mut >= len
                    || !chars[after_mut].is_alphanumeric() && chars[after_mut] != '_'
                {
                    result.push('!');
                    i = after_mut;
                    // Skip whitespace after `mut`
                    while i < len && chars[i] == ' ' {
                        i += 1;
                    }
                    continue;
                }
            }
            continue;
        }

        // Match `&mut` (without lifetime) → `&!`
        if chars[i] == '&'
            && i + 4 <= len
            && &input[i..i + 4] == "&mut"
            && (i + 4 >= len || !chars[i + 4].is_alphanumeric() && chars[i + 4] != '_')
        {
            result.push_str("&!");
            i += 4;
            // Skip whitespace after `mut`
            while i < len && chars[i] == ' ' {
                i += 1;
            }
            continue;
        }

        // Match generic lifetime-only parameters: `<'a>`, `<'a, 'b>`
        // or mixed: `<'a, T>` → `<T>`
        if chars[i] == '<' && i + 1 < len && has_lifetime_in_generics(&chars[i + 1..]) {
            let (compressed, consumed) = compress_generic_lifetimes(&input[i..]);
            result.push_str(&compressed);
            i += consumed;
            continue;
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Check if the generic parameter list starting at `chars` contains any lifetime (`'`).
fn has_lifetime_in_generics(chars: &[char]) -> bool {
    let mut depth = 1i32;
    for &ch in chars {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return false;
                }
            }
            '\'' => return true,
            _ => {}
        }
    }
    false
}

/// Compress generic parameters by removing lifetime parameters.
/// Returns (compressed string, number of bytes consumed from input).
fn compress_generic_lifetimes(input: &str) -> (String, usize) {
    // Find the matching '>'
    let chars: Vec<char> = input.chars().collect();
    let mut depth = 0i32;
    let mut end = 0;
    for (idx, &ch) in chars.iter().enumerate() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    end = idx;
                    break;
                }
            }
            _ => {}
        }
    }

    if end == 0 {
        // No matching '>', return as-is
        return (chars[0].to_string(), 1);
    }

    let inner: String = chars[1..end].iter().collect();
    let consumed = input.char_indices().nth(end + 1).map(|(i, _)| i).unwrap_or(input.len());

    // Parse the generic parameters
    let params: Vec<&str> = split_generic_params(&inner);
    let non_lifetime: Vec<&str> =
        params.into_iter().map(|p| p.trim()).filter(|p| !p.starts_with('\'')).collect();

    if non_lifetime.is_empty() {
        // All parameters were lifetimes — remove the entire `<...>`
        (String::new(), consumed)
    } else {
        let compressed = format!("<{}>", non_lifetime.join(", "));
        (compressed, consumed)
    }
}

/// Split generic parameters by comma, respecting nested `<>`.
fn split_generic_params(input: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;

    for (i, ch) in input.char_indices() {
        match ch {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                result.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    result.push(&input[start..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elide_single_lifetime() {
        assert_eq!(elide_lifetimes("&'a str"), "&str");
    }

    #[test]
    fn elide_lifetime_mut() {
        assert_eq!(elide_lifetimes("&'a mut T"), "&!T");
    }

    #[test]
    fn elide_mut_without_lifetime() {
        assert_eq!(elide_lifetimes("&mut T"), "&!T");
    }

    #[test]
    fn elide_generic_lifetime_only() {
        assert_eq!(elide_lifetimes("foo<'a>(x: &'a str)"), "foo(x: &str)");
    }

    #[test]
    fn elide_generic_mixed() {
        assert_eq!(elide_lifetimes("foo<'a, T>(x: &'a T)"), "foo<T>(x: &T)");
    }

    #[test]
    fn elide_multiple_lifetimes() {
        assert_eq!(elide_lifetimes("foo<'a, 'b>(x: &'a str, y: &'b str)"), "foo(x: &str, y: &str)");
    }

    #[test]
    fn no_lifetime_passes_through() {
        assert_eq!(elide_lifetimes("foo<T>(x: &T)"), "foo<T>(x: &T)");
    }

    #[test]
    fn plain_ref_unchanged() {
        assert_eq!(elide_lifetimes("&str"), "&str");
    }
}

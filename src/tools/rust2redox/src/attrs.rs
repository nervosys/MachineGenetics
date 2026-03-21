//! Attribute compression pass: Rust attributes → Redox compact forms (§5.5.2).

/// Compress Rust attributes to their Redox compact equivalents.
pub fn compress_attrs(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(pos) = remaining.find("#[") {
        result.push_str(&remaining[..pos]);

        // Find matching ']'
        let attr_start = pos + 2;
        if let Some(end_offset) = find_matching_bracket(&remaining[attr_start..]) {
            let attr_body = &remaining[attr_start..attr_start + end_offset];
            let compressed = compress_single_attr(attr_body.trim());
            result.push_str(&compressed);
            remaining = &remaining[attr_start + end_offset + 1..]; // skip past ']'
        } else {
            // No matching ']', pass through as-is
            result.push_str("#[");
            remaining = &remaining[attr_start..];
        }
    }

    result.push_str(remaining);
    result
}

/// Find the position of the matching `]`, handling nested brackets.
fn find_matching_bracket(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' if depth == 0 => return Some(i),
            ']' | ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Compress a single attribute body (content between `#[` and `]`).
fn compress_single_attr(body: &str) -> String {
    // Derive attributes
    if let Some(inner) = strip_prefix_and_parens(body, "derive") {
        let abbrevs = compress_derive_args(inner);
        return format!("@d({})", abbrevs);
    }

    // cfg attributes
    if let Some(inner) = strip_prefix_and_parens(body, "cfg") {
        let compressed = compress_cfg_inner(inner);
        return format!("@cfg({})", compressed);
    }

    // allow(...)
    if let Some(inner) = strip_prefix_and_parens(body, "allow") {
        let compressed = compress_lint(inner);
        return format!("@a({})", compressed);
    }

    // deny(...)
    if let Some(inner) = strip_prefix_and_parens(body, "deny") {
        let compressed = compress_lint(inner);
        return format!("@x({})", compressed);
    }

    // repr(...)
    if let Some(inner) = strip_prefix_and_parens(body, "repr") {
        let compressed = compress_repr(inner);
        return format!("@r({})", compressed);
    }

    // inline(always)
    if body == "inline(always)" {
        return "@i!".to_string();
    }

    // Simple attributes
    match body {
        "test" => "@t".to_string(),
        "bench" => "@b".to_string(),
        "must_use" => "@mu".to_string(),
        _ => format!("@{}", body), // passthrough with @ prefix
    }
}

/// Strip a prefix and its surrounding parens, e.g. "derive(Clone, Debug)" → "Clone, Debug"
fn strip_prefix_and_parens<'a>(body: &'a str, prefix: &str) -> Option<&'a str> {
    let stripped = body.strip_prefix(prefix)?.trim_start();
    let inner = stripped.strip_prefix('(')?.strip_suffix(')')?;
    Some(inner.trim())
}

/// Compress derive arguments using the standard trait abbreviation registry.
fn compress_derive_args(inner: &str) -> String {
    inner.split(',').map(|s| abbreviate_trait(s.trim())).collect::<Vec<_>>().join(",")
}

/// Abbreviate a trait name per §5.5.6.
fn abbreviate_trait(name: &str) -> &str {
    match name {
        "Clone" => "Cl",
        "Debug" => "Db",
        "Display" => "Disp",
        "Default" => "Def",
        "PartialEq" => "PEq",
        "Eq" => "Eq",
        "PartialOrd" => "POrd",
        "Ord" => "Ord",
        "Hash" => "H",
        "Copy" => "Cp",
        "Serialize" => "Ser",
        "Deserialize" => "De",
        "Iterator" => "Iter",
        _ => name,
    }
}

/// Compress cfg(...) inner content.
fn compress_cfg_inner(inner: &str) -> &str {
    match inner.trim() {
        "test" => "t",
        other => other,
    }
}

/// Compress lint names for allow/deny/warn.
fn compress_lint(inner: &str) -> &str {
    match inner.trim() {
        "unused" => "un",
        "dead_code" => "dc",
        "unsafe_code" => "uc",
        "unused_imports" => "ui",
        "unused_variables" => "uv",
        other => other,
    }
}

/// Compress repr(...) inner content.
fn compress_repr(inner: &str) -> &str {
    match inner.trim() {
        "transparent" => "t",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_basic() {
        assert_eq!(compress_attrs("#[derive(Clone)]"), "@d(Cl)");
    }

    #[test]
    fn derive_multiple() {
        assert_eq!(compress_attrs("#[derive(Clone, Debug, PartialEq)]"), "@d(Cl,Db,PEq)");
    }

    #[test]
    fn cfg_test() {
        assert_eq!(compress_attrs("#[cfg(test)]"), "@cfg(t)");
    }

    #[test]
    fn nested_attr_passthrough() {
        // cfg with non-standard content passes through
        assert_eq!(compress_attrs("#[cfg(feature = \"serde\")]"), "@cfg(feature = \"serde\")");
    }

    #[test]
    fn two_attrs_on_separate_lines() {
        assert_eq!(compress_attrs("#[test]\n#[derive(Debug)]"), "@t\n@d(Db)");
    }
}

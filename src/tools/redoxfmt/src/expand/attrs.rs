//! Expand attribute abbreviations back to full `#[...]` form.

/// Expand Redox attribute abbreviations to their Rust equivalents.
pub fn expand_attrs(input: &str) -> String {
    let mut result = String::new();

    for line in input.split('\n') {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&expand_attrs_line(line));
    }

    result
}

fn expand_attrs_line(line: &str) -> String {
    let trimmed = line.trim();

    // @d(...) → #[derive(...)]
    if let Some(rest) = trimmed.strip_prefix("@d(") {
        if let Some(inner_end) = rest.find(')') {
            let inner = &rest[..inner_end];
            let expanded = expand_derive_traits(inner);
            return format!("#[derive({expanded})]");
        }
    }

    // @cfg(...) → #[cfg(...)]
    if let Some(rest) = trimmed.strip_prefix("@cfg(") {
        if let Some(inner_end) = find_matching_paren(rest) {
            let inner = &rest[..inner_end];
            let expanded = expand_cfg_inner(inner);
            return format!("#[cfg({expanded})]");
        }
    }

    // @a(...) → #[allow(...)]
    if let Some(rest) = trimmed.strip_prefix("@a(") {
        if let Some(inner_end) = rest.find(')') {
            let inner = &rest[..inner_end];
            let expanded = expand_lint(inner);
            return format!("#[allow({expanded})]");
        }
    }

    // @x(...) → #[deny(...)]
    if let Some(rest) = trimmed.strip_prefix("@x(") {
        if let Some(inner_end) = rest.find(')') {
            let inner = &rest[..inner_end];
            let expanded = expand_lint(inner);
            return format!("#[deny({expanded})]");
        }
    }

    // @r(...) → #[repr(...)]
    if let Some(rest) = trimmed.strip_prefix("@r(") {
        if let Some(inner_end) = rest.find(')') {
            let inner = &rest[..inner_end];
            let expanded = expand_repr(inner);
            return format!("#[repr({expanded})]");
        }
    }

    // @i! → #[inline(always)]
    if trimmed == "@i!" {
        return "#[inline(always)]".to_string();
    }

    // @mu → #[must_use]
    if trimmed == "@mu" {
        return "#[must_use]".to_string();
    }

    // @t → #[test]
    if trimmed == "@t" {
        return "#[test]".to_string();
    }

    // @b → #[bench]
    if trimmed == "@b" {
        return "#[bench]".to_string();
    }

    line.to_string()
}

fn expand_derive_traits(inner: &str) -> String {
    inner
        .split(',')
        .map(|t| expand_trait_abbrev(t.trim()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn expand_trait_abbrev(abbrev: &str) -> &str {
    match abbrev {
        "Cl" => "Clone",
        "Db" => "Debug",
        "Disp" => "Display",
        "Def" => "Default",
        "PEq" => "PartialEq",
        "Eq" => "Eq",
        "POrd" => "PartialOrd",
        "Ord" => "Ord",
        "H" => "Hash",
        "Cp" => "Copy",
        "Ser" => "Serialize",
        "De" => "Deserialize",
        other => other,
    }
}

fn expand_cfg_inner(inner: &str) -> String {
    match inner.trim() {
        "t" => "test".to_string(),
        other => other.to_string(),
    }
}

fn expand_lint(inner: &str) -> String {
    match inner.trim() {
        "un" => "unused".to_string(),
        "dc" => "dead_code".to_string(),
        "uc" => "unsafe_code".to_string(),
        other => other.to_string(),
    }
}

fn expand_repr(inner: &str) -> String {
    match inner.trim() {
        "t" => "transparent".to_string(),
        "C" => "C".to_string(),
        other => other.to_string(),
    }
}

fn find_matching_paren(input: &str) -> Option<usize> {
    let mut depth = 1i32;
    for (i, ch) in input.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_expand() {
        assert_eq!(expand_attrs("@d(Cl,Db)"), "#[derive(Clone, Debug)]");
    }

    #[test]
    fn test_attr_expand() {
        assert_eq!(expand_attrs("@t"), "#[test]");
    }

    #[test]
    fn cfg_test_expand() {
        assert_eq!(expand_attrs("@cfg(t)"), "#[cfg(test)]");
    }

    #[test]
    fn inline_always_expand() {
        assert_eq!(expand_attrs("@i!"), "#[inline(always)]");
    }

    #[test]
    fn allow_unused_expand() {
        assert_eq!(expand_attrs("@a(un)"), "#[allow(unused)]");
    }

    #[test]
    fn repr_c_expand() {
        assert_eq!(expand_attrs("@r(C)"), "#[repr(C)]");
    }

    #[test]
    fn must_use_expand() {
        assert_eq!(expand_attrs("@mu"), "#[must_use]");
    }

    #[test]
    fn non_attr_passthrough() {
        assert_eq!(expand_attrs("+f foo() {}"), "+f foo() {}");
    }
}

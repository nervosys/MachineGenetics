/// Source-level translation from Rust to Redox canonical syntax.
///
/// Implements the translation rules from REDOX_ECOSYSTEM.md §2.1.2.
/// This is a text-level transformer — it processes Rust source line-by-line
/// and applies pattern-based rewriting rules.

/// Translate a complete Rust source file to Redox.
pub fn translate(source: &str) -> String {
    let mut output = String::with_capacity(source.len());

    for line in source.lines() {
        let translated = translate_line(line);
        output.push_str(&translated);
        output.push('\n');
    }

    // Trim trailing newline.
    if output.ends_with('\n') {
        output.pop();
    }

    output
}

/// Translate a single line of Rust to Redox.
fn translate_line(line: &str) -> String {
    let indent = leading_whitespace(line);
    let trimmed = line.trim();

    // Skip empty lines and preserve comments.
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with("//") {
        return line.to_string();
    }

    let mut result = trimmed.to_string();

    // ── Item-level transforms ────────────────────────────────────────

    // pub fn → +f
    result = result.replace("pub fn ", "+f ");
    // pub(crate) fn → f (crate-visible = private in Redox)
    result = result.replace("pub(crate) fn ", "f ");
    // fn → f (must come after pub fn)
    if !result.contains("+f ") {
        result = replace_keyword_at_boundary(&result, "fn ", "f ");
    }

    // pub struct → +S
    result = result.replace("pub struct ", "+S ");
    // struct → S
    if !result.contains("+S ") {
        result = replace_keyword_at_boundary(&result, "struct ", "S ");
    }

    // pub enum → +E
    result = result.replace("pub enum ", "+E ");
    // enum → E
    if !result.contains("+E ") {
        result = replace_keyword_at_boundary(&result, "enum ", "E ");
    }

    // pub trait → +T
    result = result.replace("pub trait ", "+T ");
    // trait → T
    if !result.contains("+T ") {
        result = replace_keyword_at_boundary(&result, "trait ", "T ");
    }

    // impl → I
    result = replace_keyword_at_boundary(&result, "impl ", "I ");
    // impl<T> → I [T] (handled below in generics)

    // mod → M
    result = result.replace("pub mod ", "+M ");
    if !result.contains("+M ") {
        result = replace_keyword_at_boundary(&result, "mod ", "M ");
    }

    // use → u + path separator :: → .
    if result.starts_with("use ") || result.starts_with("+u ") {
        result = translate_use(&result);
    }

    // const → c
    result = result.replace("pub const ", "+c ");
    if !result.contains("+c ") {
        result = replace_keyword_at_boundary(&result, "const ", "c ");
    }

    // ── Variable binding transforms ──────────────────────────────────

    // let mut → m
    result = result.replace("let mut ", "m ");
    // let → v
    result = replace_keyword_at_boundary(&result, "let ", "v ");

    // ── Attribute transforms ─────────────────────────────────────────

    // #[derive(...)] → @d(...)
    if result.starts_with("#[derive(") {
        result = translate_derive(&result);
    }
    // #[inline] → @i
    result = result.replace("#[inline]", "@i");
    result = result.replace("#[inline(always)]", "@i!");
    // #[cfg(...)] → @cfg(...)
    if result.contains("#[cfg(") {
        result = result.replace("#[cfg(", "@cfg(");
        result = result.replace(")]", ")");
    }
    // #[test] → @test
    result = result.replace("#[test]", "@test");
    // #[allow(...)] → @allow(...)
    if result.contains("#[allow(") {
        result = result.replace("#[allow(", "@allow(");
        result = result.replace(")]", ")");
    }

    // ── Type transforms ──────────────────────────────────────────────

    result = translate_types(&result);

    // ── Macro transforms ─────────────────────────────────────────────

    // println!("...") → p"..."
    result = translate_println(&result);
    // format!("...") → f"..."
    result = translate_format(&result);
    // vec![...] → [...] ~
    result = translate_vec_macro(&result);

    // ── Path separator :: → . ────────────────────────────────────────
    result = result.replace("::", ".");

    // ── Generics <T> → [T] ──────────────────────────────────────────
    result = translate_generics(&result);

    // ── Lifetime annotations → removed ───────────────────────────────
    result = remove_lifetimes(&result);

    // ── async fn → af ────────────────────────────────────────────────
    result = result.replace("async f ", "af ");

    // ── Return keyword → ret ─────────────────────────────────────────
    result = replace_keyword_at_boundary(&result, "return ", "ret ");

    // ── if/else → ?/: ────────────────────────────────────────────────
    result = translate_if_else(&result);

    // ── match → ? { } ────────────────────────────────────────────────
    result = translate_match(&result);

    // Reconstruct with original indentation.
    format!("{indent}{result}")
}

// ── Type translation ─────────────────────────────────────────────────

fn translate_types(s: &str) -> String {
    let mut result = s.to_string();

    // Vec<T> → [T]~ — must handle nested types
    result = translate_generic_type(&result, "Vec<", "[", "]~");

    // Option<T> → ?T
    result = translate_generic_type(&result, "Option<", "?", "");

    // Box<T> → ^T
    result = translate_generic_type(&result, "Box<", "^", "");

    // Rc<T> → $T
    result = translate_generic_type(&result, "Rc<", "$", "");

    // Arc<T> → @T
    result = translate_generic_type(&result, "Arc<", "@", "");

    // HashMap<K, V> → {K: V}
    result = translate_hashmap(&result);

    // Result<T, E> → R[T, E]
    result = translate_result_type(&result);

    // &str → s, String → s (contextual — be conservative)
    // Only replace standalone type annotations.
    result = result.replace(": &str", ": s");
    result = result.replace(": String", ": s");
    result = result.replace("-> &str", "-> s");
    result = result.replace("-> String", "-> s");

    result
}

fn translate_generic_type(s: &str, pattern: &str, prefix: &str, suffix: &str) -> String {
    let mut result = s.to_string();

    while let Some(start) = result.find(pattern) {
        let after = start + pattern.len();
        if let Some(end) = find_matching_angle_bracket(&result, after - 1) {
            let inner = &result[after..end].to_string();
            let replacement = format!("{prefix}{inner}{suffix}");
            result = format!("{}{replacement}{}", &result[..start], &result[end + 1..]);
        } else {
            break;
        }
    }

    result
}

fn translate_hashmap(s: &str) -> String {
    let mut result = s.to_string();

    while let Some(start) = result.find("HashMap<") {
        let after = start + "HashMap<".len();
        if let Some(end) = find_matching_angle_bracket(&result, after - 1) {
            let inner = &result[after..end];
            // Split on the first comma: K, V
            if let Some(comma) = inner.find(", ") {
                let key = &inner[..comma];
                let val = &inner[comma + 2..];
                let replacement = format!("{{{key}: {val}}}");
                result = format!("{}{replacement}{}", &result[..start], &result[end + 1..]);
            } else {
                break;
            }
        } else {
            break;
        }
    }

    result
}

fn translate_result_type(s: &str) -> String {
    let mut result = s.to_string();

    while let Some(start) = result.find("Result<") {
        let after = start + "Result<".len();
        if let Some(end) = find_matching_angle_bracket(&result, after - 1) {
            let inner = &result[after..end];
            if let Some(comma) = inner.find(", ") {
                let ok = &inner[..comma];
                let err = &inner[comma + 2..];
                let replacement = format!("R[{ok}, {err}]");
                result = format!("{}{replacement}{}", &result[..start], &result[end + 1..]);
            } else {
                break;
            }
        } else {
            break;
        }
    }

    result
}

/// Find the matching `>` for a `<` at position `pos`.
fn find_matching_angle_bracket(s: &str, pos: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if pos >= bytes.len() || bytes[pos] != b'<' {
        return None;
    }
    let mut depth = 0;
    for i in pos..bytes.len() {
        match bytes[i] {
            b'<' => depth += 1,
            b'>' => {
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

// ── Use declaration ──────────────────────────────────────────────────

fn translate_use(s: &str) -> String {
    let mut result = s.to_string();
    // use path::to::Item; → u path.to.Item;
    result = result.replace("use ", "u ");
    result = result.replace("::", ".");
    // Remove `{` and `}` from use groups (simplified).
    result
}

// ── Derive attribute ─────────────────────────────────────────────────

fn translate_derive(s: &str) -> String {
    // #[derive(Debug, Clone)] → @d(Debug, Clone)
    if let Some(start) = s.find("#[derive(") {
        let after = start + "#[derive(".len();
        if let Some(end) = s[after..].find(")]") {
            let inner = &s[after..after + end];
            return format!("@d({inner})");
        }
    }
    s.to_string()
}

// ── println!/format! ─────────────────────────────────────────────────

fn translate_println(s: &str) -> String {
    let mut result = s.to_string();

    for pattern in &["println!(", "eprintln!("] {
        while let Some(start) = result.find(pattern) {
            let after = start + pattern.len();
            if let Some(end) = find_matching_paren(&result, after - 1) {
                let inner = &result[after..end].trim();
                // Strip outer quotes if it's a simple string.
                let content = inner.trim_start_matches('"').trim_end_matches('"');
                let replacement = format!("p\"{content}\"");
                result = format!("{}{replacement}{}", &result[..start], &result[end + 1..]);
            } else {
                break;
            }
        }
    }

    result
}

fn translate_format(s: &str) -> String {
    let mut result = s.to_string();

    while let Some(start) = result.find("format!(") {
        let after = start + "format!(".len();
        if let Some(end) = find_matching_paren(&result, after - 1) {
            let inner = &result[after..end].trim();
            let content = inner.trim_start_matches('"').trim_end_matches('"');
            let replacement = format!("f\"{content}\"");
            result = format!("{}{replacement}{}", &result[..start], &result[end + 1..]);
        } else {
            break;
        }
    }

    result
}

fn translate_vec_macro(s: &str) -> String {
    let mut result = s.to_string();

    while let Some(start) = result.find("vec![") {
        let after = start + "vec![".len();
        if let Some(end) = result[after..].find(']') {
            let inner = &result[after..after + end];
            let replacement = format!("[{inner}]~");
            result = format!("{}{replacement}{}", &result[..start], &result[after + end + 1..]);
        } else {
            break;
        }
    }

    result
}

fn find_matching_paren(s: &str, pos: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if pos >= bytes.len() || bytes[pos] != b'(' {
        return None;
    }
    let mut depth = 0;
    for i in pos..bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
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

// ── Generics <T> → [T] ──────────────────────────────────────────────

fn translate_generics(s: &str) -> String {
    let mut result = s.to_string();
    let mut changed = true;

    while changed {
        changed = false;
        // Find patterns like `Name<T>` that aren't type constructors (already handled).
        // Only translate generics on identifiers (not operators like < >).
        if let Some(pos) = find_generic_bracket(&result) {
            if let Some(end) = find_matching_angle_bracket(&result, pos) {
                let inner = &result[pos + 1..end];
                // Replace <...> with [...].
                let new = format!("[{inner}]");
                result = format!("{}{new}{}", &result[..pos], &result[end + 1..]);
                changed = true;
            }
        }
    }

    result
}

/// Find a `<` that's part of generics (preceded by an identifier character).
fn find_generic_bracket(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    for i in 1..bytes.len() {
        if bytes[i] == b'<' {
            let prev = bytes[i - 1];
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                return Some(i);
            }
        }
    }
    None
}

// ── Lifetime removal ─────────────────────────────────────────────────

fn remove_lifetimes(s: &str) -> String {
    // Remove 'a, 'b, 'static, etc. from type annotations.
    // Pattern: &'name — remove the 'name part.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\'' {
            // Check if this is a lifetime: 'a, 'static, etc.
            let mut name = String::new();
            while let Some(&next) = chars.peek() {
                if next.is_ascii_alphanumeric() || next == '_' {
                    name.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            if !name.is_empty() && name != "\"" {
                // Skip the lifetime annotation.
                // Also skip a trailing space or comma.
                if let Some(&next) = chars.peek() {
                    if next == ' ' || next == ',' {
                        chars.next();
                        // If we ate a comma, also eat trailing space.
                        if next == ',' {
                            if let Some(&sp) = chars.peek() {
                                if sp == ' ' {
                                    chars.next();
                                }
                            }
                        }
                    }
                }
            } else {
                // It's a char literal — keep it.
                out.push('\'');
                out.push_str(&name);
            }
        } else {
            out.push(ch);
        }
    }

    out
}

// ── if/else → ?/: ────────────────────────────────────────────────────

fn translate_if_else(s: &str) -> String {
    let mut result = s.to_string();
    // Simple: if cond { → ? cond {
    result = replace_keyword_at_boundary(&result, "if ", "? ");
    // } else { → } : {
    result = result.replace("} else {", "} : {");
    // } else if → } : ? (else if chains)
    result = result.replace("} else ? ", "} : ? ");
    result
}

// ── match → ? { ... } ────────────────────────────────────────────────

fn translate_match(s: &str) -> String {
    let mut result = s.to_string();
    // match expr { → ? expr {
    // (Only at statement start, not inside expressions.)
    result = replace_keyword_at_boundary(&result, "match ", "? ");
    // => stays as => (Redox uses same pattern arm syntax)
    result
}

// ── Helpers ──────────────────────────────────────────────────────────

fn leading_whitespace(s: &str) -> &str {
    let trimmed = s.trim_start();
    &s[..s.len() - trimmed.len()]
}

/// Replace a keyword at a word boundary (not inside another word).
fn replace_keyword_at_boundary(s: &str, from: &str, to: &str) -> String {
    if let Some(pos) = s.find(from) {
        // Check that the position is either at start or preceded by non-alphanumeric.
        if pos == 0 || !s.as_bytes()[pos - 1].is_ascii_alphanumeric() {
            return format!("{}{to}{}", &s[..pos], &s[pos + from.len()..]);
        }
    }
    s.to_string()
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pub_fn() {
        assert_eq!(
            translate_line("pub fn add(a: i32, b: i32) -> i32 {"),
            "+f add(a: i32, b: i32) -> i32 {"
        );
    }

    #[test]
    fn test_fn() {
        assert_eq!(translate_line("fn helper() {"), "f helper() {");
    }

    #[test]
    fn test_let() {
        assert_eq!(translate_line("    let x = 42;"), "    v x = 42;");
    }

    #[test]
    fn test_let_mut() {
        assert_eq!(translate_line("    let mut count = 0;"), "    m count = 0;");
    }

    #[test]
    fn test_struct() {
        assert_eq!(translate_line("pub struct Point {"), "+S Point {");
    }

    #[test]
    fn test_enum() {
        assert_eq!(translate_line("enum Color {"), "E Color {");
    }

    #[test]
    fn test_trait() {
        assert_eq!(translate_line("pub trait Display {"), "+T Display {");
    }

    #[test]
    fn test_impl() {
        assert_eq!(translate_line("impl Point {"), "I Point {");
    }

    #[test]
    fn test_use_path() {
        assert_eq!(translate_line("use std::io::Read;"), "u std.io.Read;");
    }

    #[test]
    fn test_vec_type() {
        assert_eq!(translate_line("    items: Vec<i32>,"), "    items: [i32]~,");
    }

    #[test]
    fn test_option_type() {
        assert_eq!(translate_line("fn foo(x: Option<i32>) {"), "f foo(x: ?i32) {");
    }

    #[test]
    fn test_result_type() {
        assert_eq!(
            translate_line("fn foo() -> Result<i32, Error> {"),
            "f foo() -> R[i32, Error] {"
        );
    }

    #[test]
    fn test_box_type() {
        assert_eq!(translate_line("    data: Box<Node>,"), "    data: ^Node,");
    }

    #[test]
    fn test_rc_type() {
        assert_eq!(translate_line("    shared: Rc<Data>,"), "    shared: $Data,");
    }

    #[test]
    fn test_arc_type() {
        assert_eq!(translate_line("    atomic: Arc<Data>,"), "    atomic: @Data,");
    }

    #[test]
    fn test_hashmap_type() {
        assert_eq!(translate_line("    map: HashMap<String, i32>,"), "    map: {String: i32},");
    }

    #[test]
    fn test_derive() {
        assert_eq!(translate_line("#[derive(Debug, Clone)]"), "@d(Debug, Clone)");
    }

    #[test]
    fn test_inline() {
        assert_eq!(translate_line("#[inline]"), "@i");
    }

    #[test]
    fn test_println() {
        assert_eq!(translate_line("    println!(\"hello\");"), "    p\"hello\";");
    }

    #[test]
    fn test_if_else() {
        assert_eq!(translate_line("    if x > 0 {"), "    ? x > 0 {");
    }

    #[test]
    fn test_else_block() {
        assert_eq!(translate_line("    } else {"), "    } : {");
    }

    #[test]
    fn test_lifetime_removal() {
        let result = remove_lifetimes("fn foo<'a>(x: &'a str) -> &'a str {");
        assert!(!result.contains("'a"), "lifetime not removed: {result}");
    }

    #[test]
    fn test_match() {
        assert_eq!(translate_line("    match value {"), "    ? value {");
    }

    #[test]
    fn test_return() {
        assert_eq!(translate_line("    return 42;"), "    ret 42;");
    }

    #[test]
    fn test_path_separator() {
        assert_eq!(translate_line("    std::io::Write"), "    std.io.Write");
    }

    #[test]
    fn test_full_translation() {
        let rust = "\
pub fn add(a: i32, b: i32) -> i32 {
    let result = a + b;
    result
}";
        let rdx = translate(rust);
        assert!(rdx.contains("+f add"));
        assert!(rdx.contains("v result"));
    }
}

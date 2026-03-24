/// MechGen Legacy Syntax Support — Rust → MechGen canonical syntax translator.
///
/// When `--syntax=legacy` is active, this module translates standard Rust
/// source into MechGen canonical syntax before lexing/parsing.
///
/// This is the same algorithm as `tools/rust2mg` but embedded in the
/// prototype compiler so that `--syntax=legacy` works in a single binary.
///
/// Implements the translation rules from REDOX_PROPOSAL §5.3.4:
///   - `pub fn` → `+f`, `fn` → `f`, `async fn` → `af`, `unsafe fn` → `uf`
///   - `struct`/`enum`/`trait`/`impl`/`mod` → `S`/`E`/`T`/`I`/`M`
///   - `let`/`let mut` → `v`/`m`, `const` → `C`, `type` → `Y`
///   - `Vec<T>` → `[T]~`, `Option<T>` → `?T`, `Box<T>` → `^T`, etc.
///   - `<T>` → `[T]`, `::` → `.`
///   - Lifetime removal, match → `?=`, if/else → `?`/`:`
///   - `return` → `ret`

/// Translate a complete Rust source file to MechGen canonical syntax.
pub fn translate(source: &str) -> String {
    let mut output = String::with_capacity(source.len());

    for line in source.lines() {
        let translated = translate_line(line);
        output.push_str(&translated);
        output.push('\n');
    }

    if output.ends_with('\n') {
        output.pop();
    }

    output
}

// ── Line-level translation ───────────────────────────────────────────

fn translate_line(line: &str) -> String {
    let indent = leading_whitespace(line);
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with("//") {
        return line.to_string();
    }

    let mut r = trimmed.to_string();

    // ── Function keywords ────────────────────────────────────────────
    // Order matters: more specific first.
    r = r.replace("pub async unsafe fn ", "+auf ");
    r = r.replace("pub async fn ", "+af ");
    r = r.replace("pub unsafe fn ", "+uf ");
    r = r.replace("pub fn ", "+f ");
    r = r.replace("async unsafe fn ", "auf ");
    r = r.replace("async fn ", "af ");
    r = r.replace("unsafe fn ", "uf ");
    if !r.contains("+f ") && !r.contains("+af ") && !r.contains("+uf ") && !r.contains("+auf ") {
        r = boundary_replace(&r, "fn ", "f ");
    }

    // ── Item keywords ────────────────────────────────────────────────
    r = r.replace("pub struct ", "+S ");
    if !r.contains("+S ") {
        r = boundary_replace(&r, "struct ", "S ");
    }
    r = r.replace("pub enum ", "+E ");
    if !r.contains("+E ") {
        r = boundary_replace(&r, "enum ", "E ");
    }
    r = r.replace("pub trait ", "+T ");
    if !r.contains("+T ") {
        r = boundary_replace(&r, "trait ", "T ");
    }
    r = boundary_replace(&r, "impl ", "I ");
    r = r.replace("pub mod ", "+M ");
    if !r.contains("+M ") {
        r = boundary_replace(&r, "mod ", "M ");
    }

    // ── use → u ──────────────────────────────────────────────────────
    if r.starts_with("use ") || r.starts_with("pub use ") {
        r = r.replace("pub use ", "+u ");
        r = r.replace("use ", "u ");
    }

    // ── Const / static / type alias ──────────────────────────────────
    r = r.replace("pub const ", "+C ");
    if !r.contains("+C ") {
        r = boundary_replace(&r, "const ", "C ");
    }
    r = r.replace("pub static mut ", "+Z m ");
    r = r.replace("pub static ", "+Z ");
    r = r.replace("static mut ", "Z m ");
    if !r.contains("+Z ") && !r.contains("Z m ") {
        r = boundary_replace(&r, "static ", "Z ");
    }
    r = r.replace("pub type ", "+Y ");
    if !r.contains("+Y ") {
        r = boundary_replace(&r, "type ", "Y ");
    }

    // ── Variable bindings ────────────────────────────────────────────
    r = r.replace("let mut ", "m ");
    r = boundary_replace(&r, "let ", "v ");

    // ── Attributes ───────────────────────────────────────────────────
    if r.starts_with("#[derive(") {
        r = translate_derive(&r);
    }
    r = r.replace("#[inline(always)]", "@i!");
    r = r.replace("#[inline]", "@i");
    r = r.replace("#[test]", "@test");
    if r.contains("#[cfg(") {
        r = r.replace("#[cfg(", "@cfg(");
        r = r.replace(")]", ")");
    }
    if r.contains("#[allow(") {
        r = r.replace("#[allow(", "@allow(");
        r = r.replace(")]", ")");
    }

    // ── Type transforms ──────────────────────────────────────────────
    r = translate_types(&r);

    // ── Macros ───────────────────────────────────────────────────────
    r = translate_println(&r);
    r = translate_format(&r);
    r = translate_vec_macro(&r);

    // ── Path separator :: → . ────────────────────────────────────────
    r = r.replace("::", ".");

    // ── Generics <T> → [T] ──────────────────────────────────────────
    r = translate_generics(&r);

    // ── Lifetime removal ─────────────────────────────────────────────
    r = remove_lifetimes(&r);

    // ── return → ret ─────────────────────────────────────────────────
    r = boundary_replace(&r, "return ", "ret ");

    // ── if/else → ?/: ────────────────────────────────────────────────
    r = boundary_replace(&r, "if ", "? ");
    r = r.replace("} else {", "} : {");
    r = r.replace("} else ? ", "} : ? ");

    // ── match → ?= ──────────────────────────────────────────────────
    r = boundary_replace(&r, "match ", "?= ");

    // ── Loop constructs ──────────────────────────────────────────────
    r = boundary_replace(&r, "while ", "@w ");
    // Standalone `loop {` → `@@ {`
    r = boundary_replace(&r, "loop {", "@@ {");
    r = boundary_replace(&r, "loop{", "@@{");
    // for .. in → @ .. in
    r = boundary_replace(&r, "for ", "@ ");

    // ── break / continue ─────────────────────────────────────────────
    r = boundary_replace(&r, "break;", "!;");
    r = boundary_replace(&r, "break ", "! ");
    r = boundary_replace(&r, "continue;", ">>;");

    format!("{indent}{r}")
}

// ── Type translation ─────────────────────────────────────────────────

fn translate_types(s: &str) -> String {
    let mut r = s.to_string();
    r = translate_generic_type(&r, "Vec<", "[", "]~");
    r = translate_generic_type(&r, "Option<", "?", "");
    r = translate_generic_type(&r, "Box<", "^", "");
    r = translate_generic_type(&r, "Rc<", "$", "");
    r = translate_generic_type(&r, "Arc<", "@", "");
    r = translate_hashmap(&r);
    r = translate_result(&r);
    // &str / String → s (type position only)
    r = r.replace(": &str", ": s");
    r = r.replace(": String", ": s");
    r = r.replace("-> &str", "-> s");
    r = r.replace("-> String", "-> s");
    r
}

fn translate_generic_type(s: &str, pattern: &str, prefix: &str, suffix: &str) -> String {
    let mut r = s.to_string();
    while let Some(start) = r.find(pattern) {
        let after = start + pattern.len();
        if let Some(end) = find_matching_angle(&r, after - 1) {
            let inner = r[after..end].to_string();
            let replacement = format!("{prefix}{inner}{suffix}");
            r = format!("{}{replacement}{}", &r[..start], &r[end + 1..]);
        } else {
            break;
        }
    }
    r
}

fn translate_hashmap(s: &str) -> String {
    let mut r = s.to_string();
    while let Some(start) = r.find("HashMap<") {
        let after = start + "HashMap<".len();
        if let Some(end) = find_matching_angle(&r, after - 1) {
            let inner = &r[after..end];
            if let Some(comma) = inner.find(", ") {
                let key = &inner[..comma];
                let val = &inner[comma + 2..];
                let replacement = format!("{{{key}: {val}}}");
                r = format!("{}{replacement}{}", &r[..start], &r[end + 1..]);
            } else {
                break;
            }
        } else {
            break;
        }
    }
    r
}

fn translate_result(s: &str) -> String {
    let mut r = s.to_string();
    while let Some(start) = r.find("Result<") {
        let after = start + "Result<".len();
        if let Some(end) = find_matching_angle(&r, after - 1) {
            let inner = &r[after..end];
            if let Some(comma) = inner.find(", ") {
                let ok = &inner[..comma];
                let err = &inner[comma + 2..];
                let replacement = format!("R[{ok}, {err}]");
                r = format!("{}{replacement}{}", &r[..start], &r[end + 1..]);
            } else {
                break;
            }
        } else {
            break;
        }
    }
    r
}

fn find_matching_angle(s: &str, pos: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if pos >= bytes.len() || bytes[pos] != b'<' {
        return None;
    }
    let mut depth = 0u32;
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

// ── Derive ───────────────────────────────────────────────────────────

fn translate_derive(s: &str) -> String {
    if let Some(start) = s.find("#[derive(") {
        let after = start + "#[derive(".len();
        if let Some(end) = s[after..].find(")]") {
            let inner = &s[after..after + end];
            return format!("@d({inner})");
        }
    }
    s.to_string()
}

// ── Print / format / vec ─────────────────────────────────────────────

fn translate_println(s: &str) -> String {
    let mut r = s.to_string();
    for pat in &["println!(", "eprintln!("] {
        while let Some(start) = r.find(pat) {
            let after = start + pat.len();
            if let Some(end) = find_matching_paren(&r, after - 1) {
                let inner = r[after..end].trim();
                let content = inner.trim_start_matches('"').trim_end_matches('"');
                let replacement = format!("p\"{content}\"");
                r = format!("{}{replacement}{}", &r[..start], &r[end + 1..]);
            } else {
                break;
            }
        }
    }
    r
}

fn translate_format(s: &str) -> String {
    let mut r = s.to_string();
    while let Some(start) = r.find("format!(") {
        let after = start + "format!(".len();
        if let Some(end) = find_matching_paren(&r, after - 1) {
            let inner = r[after..end].trim();
            let content = inner.trim_start_matches('"').trim_end_matches('"');
            let replacement = format!("f\"{content}\"");
            r = format!("{}{replacement}{}", &r[..start], &r[end + 1..]);
        } else {
            break;
        }
    }
    r
}

fn translate_vec_macro(s: &str) -> String {
    let mut r = s.to_string();
    while let Some(start) = r.find("vec![") {
        let after = start + "vec![".len();
        if let Some(end) = r[after..].find(']') {
            let inner = &r[after..after + end];
            let replacement = format!("[{inner}]~");
            r = format!("{}{replacement}{}", &r[..start], &r[after + end + 1..]);
        } else {
            break;
        }
    }
    r
}

fn find_matching_paren(s: &str, pos: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if pos >= bytes.len() || bytes[pos] != b'(' {
        return None;
    }
    let mut depth = 0u32;
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
    let mut r = s.to_string();
    let mut changed = true;
    while changed {
        changed = false;
        if let Some(pos) = find_generic_bracket(&r) {
            if let Some(end) = find_matching_angle(&r, pos) {
                let inner = &r[pos + 1..end];
                let new = format!("[{inner}]");
                r = format!("{}{new}{}", &r[..pos], &r[end + 1..]);
                changed = true;
            }
        }
    }
    r
}

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
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\'' {
            let mut name = String::new();
            while let Some(&next) = chars.peek() {
                if next.is_ascii_alphanumeric() || next == '_' {
                    name.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            if !name.is_empty() && name != "\"" {
                // Skip lifetime. Also eat trailing space/comma.
                if let Some(&next) = chars.peek() {
                    if next == ' ' || next == ',' {
                        let eaten = chars.next().unwrap();
                        if eaten == ',' {
                            if let Some(&sp) = chars.peek() {
                                if sp == ' ' {
                                    chars.next();
                                }
                            }
                        }
                    }
                }
            } else {
                out.push('\'');
                out.push_str(&name);
            }
        } else {
            out.push(ch);
        }
    }

    out
}

// ── Helpers ──────────────────────────────────────────────────────────

fn leading_whitespace(s: &str) -> &str {
    let trimmed = s.trim_start();
    &s[..s.len() - trimmed.len()]
}

fn boundary_replace(s: &str, from: &str, to: &str) -> String {
    if let Some(pos) = s.find(from) {
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
    fn translate_pub_fn() {
        assert_eq!(
            translate_line("pub fn add(a: i32, b: i32) -> i32 {"),
            "+f add(a: i32, b: i32) -> i32 {"
        );
    }

    #[test]
    fn translate_fn() {
        assert_eq!(translate_line("fn helper() {"), "f helper() {");
    }

    #[test]
    fn translate_async_fn() {
        assert_eq!(translate_line("async fn fetch() {"), "af fetch() {");
    }

    #[test]
    fn translate_unsafe_fn() {
        assert_eq!(translate_line("unsafe fn danger() {"), "uf danger() {");
    }

    #[test]
    fn translate_let() {
        assert_eq!(translate_line("    let x = 42;"), "    v x = 42;");
    }

    #[test]
    fn translate_let_mut() {
        assert_eq!(translate_line("    let mut count = 0;"), "    m count = 0;");
    }

    #[test]
    fn translate_struct() {
        assert_eq!(translate_line("pub struct Point {"), "+S Point {");
    }

    #[test]
    fn translate_enum() {
        assert_eq!(translate_line("enum Color {"), "E Color {");
    }

    #[test]
    fn translate_trait_kw() {
        assert_eq!(translate_line("pub trait Display {"), "+T Display {");
    }

    #[test]
    fn translate_impl_kw() {
        assert_eq!(translate_line("impl Point {"), "I Point {");
    }

    #[test]
    fn translate_use_kw() {
        assert_eq!(translate_line("use std::io::Read;"), "u std.io.Read;");
    }

    #[test]
    fn translate_vec_type() {
        assert_eq!(translate_line("    items: Vec<i32>,"), "    items: [i32]~,");
    }

    #[test]
    fn translate_option_type() {
        assert_eq!(translate_line("fn foo(x: Option<i32>) {"), "f foo(x: ?i32) {");
    }

    #[test]
    fn translate_result_type() {
        assert_eq!(
            translate_line("fn foo() -> Result<i32, Error> {"),
            "f foo() -> R[i32, Error] {"
        );
    }

    #[test]
    fn translate_box_type() {
        assert_eq!(translate_line("    data: Box<Node>,"), "    data: ^Node,");
    }

    #[test]
    fn translate_arc_type() {
        assert_eq!(translate_line("    shared: Arc<Data>,"), "    shared: @Data,");
    }

    #[test]
    fn translate_hashmap_type() {
        assert_eq!(translate_line("    map: HashMap<String, i32>,"), "    map: {String: i32},");
    }

    #[test]
    fn translate_derive_attr() {
        assert_eq!(translate_line("#[derive(Debug, Clone)]"), "@d(Debug, Clone)");
    }

    #[test]
    fn translate_if_else() {
        assert_eq!(translate_line("    if x > 0 {"), "    ? x > 0 {");
    }

    #[test]
    fn translate_else_block() {
        assert_eq!(translate_line("    } else {"), "    } : {");
    }

    #[test]
    fn translate_match_kw() {
        assert_eq!(translate_line("    match value {"), "    ?= value {");
    }

    #[test]
    fn translate_while_kw() {
        assert_eq!(translate_line("    while running {"), "    @w running {");
    }

    #[test]
    fn translate_loop_kw() {
        assert_eq!(translate_line("    loop {"), "    @@ {");
    }

    #[test]
    fn translate_for_kw() {
        assert_eq!(translate_line("    for x in items {"), "    @ x in items {");
    }

    #[test]
    fn translate_return_kw() {
        assert_eq!(translate_line("    return 42;"), "    ret 42;");
    }

    #[test]
    fn translate_break_kw() {
        assert_eq!(translate_line("    break;"), "    !;");
    }

    #[test]
    fn translate_continue_kw() {
        assert_eq!(translate_line("    continue;"), "    >>;");
    }

    #[test]
    fn translate_lifetime_removal() {
        let r = remove_lifetimes("fn foo<'a>(x: &'a str) -> &'a str {");
        assert!(!r.contains("'a"), "lifetime not removed: {r}");
    }

    #[test]
    fn translate_full_function() {
        let rust = "\
pub fn add(a: i32, b: i32) -> i32 {
    let result = a + b;
    result
}";
        let rdx = translate(rust);
        assert!(rdx.contains("+f add"), "got: {rdx}");
        assert!(rdx.contains("v result"), "got: {rdx}");
    }
}

/// Source-level translation from Redox canonical syntax to Rust.
///
/// Implements the reverse translation rules from REDOX_ECOSYSTEM.md §2.2.
/// This is a text-level transformer — it processes Redox source line-by-line
/// and applies pattern-based rewriting rules to produce valid Rust.

/// Translate a complete Redox source file to Rust.
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

/// Translate a single line of Redox to Rust.
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

    // ── Effect annotations → comment ─────────────────────────────────
    // Must come before other transforms since / conflicts with division.
    result = translate_effects(&result);

    // ── Attribute transforms ─────────────────────────────────────────
    // @d(Debug, Clone) → #[derive(Debug, Clone)]
    if result.starts_with("@d(") {
        result = translate_derive(&result);
        return format!("{indent}{result}");
    }
    // @test → #[test]
    result = result.replace("@test", "#[test]");
    // @bench → #[bench]
    result = result.replace("@bench", "#[bench]");
    // @i! → #[inline(always)]
    result = result.replace("@i!", "#[inline(always)]");
    // @i → #[inline]
    if result.contains("@i") && !result.contains("#[inline") {
        result = result.replace("@i", "#[inline]");
    }
    // @cfg(...) → #[cfg(...)]
    if result.contains("@cfg(") {
        result = translate_cfg(&result);
    }
    // @allow(...) → #[allow(...)]
    if result.contains("@allow(") {
        result = translate_allow(&result);
    }

    // ── Item-level transforms ────────────────────────────────────────

    // +af → pub async fn (must come before +f)
    result = replace_keyword_at_start(&result, "+af ", "pub async fn ");
    // af → async fn
    result = replace_keyword_at_start(&result, "af ", "async fn ");

    // c f → const fn
    result = replace_keyword_at_start(&result, "c f ", "const fn ");

    // +f → pub fn (must come before f)
    result = replace_keyword_at_start(&result, "+f ", "pub fn ");
    // f → fn (contextual — only at line start for function defs)
    result = replace_keyword_at_start(&result, "f ", "fn ");

    // +S → pub struct
    result = replace_keyword_at_start(&result, "+S ", "pub struct ");
    // S → struct
    result = replace_keyword_at_start(&result, "S ", "struct ");

    // +E → pub enum
    result = replace_keyword_at_start(&result, "+E ", "pub enum ");
    // E → enum
    result = replace_keyword_at_start(&result, "E ", "enum ");

    // +T → pub trait
    result = replace_keyword_at_start(&result, "+T ", "pub trait ");
    // T → trait (only at line start — T alone could be a generic)
    if result.starts_with("T ") && is_trait_def(&result) {
        result = format!("trait {}", &result[2..]);
    }

    // I Trait ~ Type → impl Trait for Type
    // I ~ Type → impl Type
    result = translate_impl(&result);

    // +M → pub mod
    result = replace_keyword_at_start(&result, "+M ", "pub mod ");
    // M → mod
    result = replace_keyword_at_start(&result, "M ", "mod ");

    // +v → pub const
    result = replace_keyword_at_start(&result, "+v ", "pub const ");
    // +c → pub const (alternative)
    result = replace_keyword_at_start(&result, "+c ", "pub const ");
    // c → const (at line start)
    if result.starts_with("c ") && !result.starts_with("const ") {
        result = format!("const {}", &result[2..]);
    }

    // +u → pub use (must come before u)
    if result.starts_with("+u ") {
        result = translate_use(&result, true);
    } else if result.starts_with("u ") {
        result = translate_use(&result, false);
    }

    // ── Variable binding transforms ──────────────────────────────────

    // m → let mut
    result = replace_keyword_at_start(&result, "m ", "let mut ");
    // v → let
    result = replace_keyword_at_start(&result, "v ", "let ");

    // ── Return keyword ───────────────────────────────────────────────
    result = replace_keyword_at_boundary(&result, "ret ", "return ");

    // ── Control flow transforms ──────────────────────────────────────
    result = translate_control_flow(&result);

    // ── Type transforms ──────────────────────────────────────────────
    result = translate_types(&result);

    // ── Macro transforms ─────────────────────────────────────────────
    result = translate_print_macros(&result);

    // ── Struct literal @{ → { ────────────────────────────────────────
    result = result.replace("@{", "{");

    // ── Where clause ~> → where ──────────────────────────────────────
    result = result.replace(" ~> ", " where ");

    // ── Path separator . → :: ────────────────────────────────────────
    // Must be selective — don't convert method calls, only module paths.
    result = translate_paths(&result);

    // ── Boolean literals ─────────────────────────────────────────────
    result = translate_booleans(&result);

    // ── Generic brackets [T] → <T> ───────────────────────────────────
    result = translate_generics_to_angle(&result);

    // Reconstruct with original indentation.
    format!("{indent}{result}")
}

// ── Effect annotation removal ────────────────────────────────────────

fn translate_effects(s: &str) -> String {
    // Remove effect annotations like `/ io` or `/ io, net`
    // These appear after return type and before `{`
    let mut result = s.to_string();

    // Pattern: `/ effect_name` before `{` at end of function sig
    if let Some(slash_pos) = find_effect_annotation(&result) {
        // Find the end of effects (next `{` or end of line)
        let rest = &result[slash_pos..];
        let end = rest.find('{').unwrap_or(rest.len());
        let effects = rest[2..end].trim(); // skip "/ "
        let comment = format!("/* effect: {effects} */ ");
        result = format!(
            "{}{}{}",
            &result[..slash_pos],
            comment,
            &result[slash_pos + end..]
        );
    }

    result
}

/// Find a `/ ` that looks like an effect annotation (not a division operator).
fn find_effect_annotation(s: &str) -> Option<usize> {
    // Effect annotations appear after `)` or return type, before `{`.
    // Pattern: `) / io {` or `-> Type / io {` or `-> Type / io, net {`
    let bytes = s.as_bytes();
    for i in 1..bytes.len().saturating_sub(1) {
        if bytes[i] == b'/' && bytes[i + 1] == b' ' {
            // Check that the previous non-space char is `)` or a type-like char
            let before = s[..i].trim_end();
            if before.ends_with(')')
                || before.ends_with(']')
                || before.ends_with('}')
                || before
                    .bytes()
                    .last()
                    .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                // Check that what follows looks like an effect name, not a number
                let after = s[i + 2..].trim_start();
                if after
                    .bytes()
                    .next()
                    .is_some_and(|b| b.is_ascii_alphabetic())
                {
                    return Some(i);
                }
            }
        }
    }
    None
}

// ── Derive ───────────────────────────────────────────────────────────

fn translate_derive(s: &str) -> String {
    // @d(Debug, Clone) → #[derive(Debug, Clone)]
    if let Some(start) = s.find("@d(") {
        let after = start + "@d(".len();
        if let Some(end) = s[after..].find(')') {
            let inner = &s[after..after + end];
            return format!("#[derive({inner})]");
        }
    }
    s.to_string()
}

// ── cfg ──────────────────────────────────────────────────────────────

fn translate_cfg(s: &str) -> String {
    s.replace("@cfg(", "#[cfg(").replace(")", ")]")
}

// ── allow ────────────────────────────────────────────────────────────

fn translate_allow(s: &str) -> String {
    s.replace("@allow(", "#[allow(").replace(")", ")]")
}

// ── Use declarations ─────────────────────────────────────────────────

fn translate_use(s: &str, public: bool) -> String {
    let prefix = if public { "pub use " } else { "use " };
    let skip = if public { "+u ".len() } else { "u ".len() };
    let path = &s[skip..];

    // Replace . with :: in the path
    let rust_path = path.replace('.', "::");
    format!("{prefix}{rust_path}")
}

// ── Impl blocks ──────────────────────────────────────────────────────

fn translate_impl(s: &str) -> String {
    if !s.starts_with("I ") {
        return s.to_string();
    }

    let rest = &s[2..]; // skip "I "

    // I ~ Type { → impl Type {
    if rest.starts_with("~ ") {
        let after_tilde = rest[2..].trim();
        return format!("impl {after_tilde}");
    }

    // I Trait ~ Type { → impl Trait for Type {
    if let Some(tilde) = rest.find(" ~ ") {
        let before_tilde = rest[..tilde].trim();
        let after_tilde = rest[tilde + 3..].trim();
        return format!("impl {before_tilde} for {after_tilde}");
    }

    // I Type { → impl Type { (no trait, no tilde — legacy compat)
    format!("impl {rest}")
}

// ── Control flow ─────────────────────────────────────────────────────

fn translate_control_flow(s: &str) -> String {
    let mut result = s.to_string();

    // @ item ~ iter { → for item in iter {
    if result.starts_with("@ ") && result.contains(" ~ ") {
        result = translate_for_loop(&result);
        return result;
    }

    // @ { → loop {
    if result == "@ {" || result.starts_with("@ {") {
        result = result.replacen("@ {", "loop {", 1);
        return result;
    }

    // } : ? → } else if
    result = result.replace("} : ? ", "} else if ");
    // } : { → } else {
    result = result.replace("} : {", "} else {");

    // ? expr { (match or if)
    // If it looks like a match (pattern arms with =>), keep as match.
    // Otherwise treat as if.
    if result.starts_with("? ") {
        let rest = &result[2..];
        if rest.ends_with('{') {
            // Could be if or match — check for comparison operators
            let cond = rest.trim_end_matches('{').trim();
            if contains_comparison(cond) || cond == "!" || is_boolean_expr(cond) {
                result = format!("if {rest}");
            } else {
                result = format!("match {rest}");
            }
        }
    }

    result
}

fn translate_for_loop(s: &str) -> String {
    // @ item ~ iter { → for item in iter {
    let rest = &s[2..]; // skip "@ "
    if let Some(tilde) = rest.find(" ~ ") {
        let pattern = rest[..tilde].trim();
        let iter_and_brace = rest[tilde + 3..].trim();
        return format!("for {pattern} in {iter_and_brace}");
    }
    s.to_string()
}

fn contains_comparison(s: &str) -> bool {
    s.contains(" == ")
        || s.contains(" != ")
        || s.contains(" > ")
        || s.contains(" < ")
        || s.contains(" >= ")
        || s.contains(" <= ")
        || s.contains(" && ")
        || s.contains(" || ")
}

fn is_boolean_expr(s: &str) -> bool {
    s.starts_with('!')
        || s == "1b"
        || s == "0b"
        || s == "true"
        || s == "false"
}

// ── Type transforms ──────────────────────────────────────────────────

fn translate_types(s: &str) -> String {
    let mut result = s.to_string();

    // [T]~ → Vec<T>
    result = translate_vec_type(&result);

    // ?T → Option<T> (in type position)
    result = translate_option_type(&result);

    // R[T, E] → Result<T, E>
    result = translate_result_type(&result);

    // ^T → Box<T>
    result = translate_ptr_type(&result, '^', "Box");

    // $T → Rc<T>
    result = translate_ptr_type(&result, '$', "Rc");

    // @T → Arc<T>  (careful: @ is also used for attributes and struct literals)
    result = translate_arc_type(&result);

    // {K: V} as type → HashMap<K, V>
    result = translate_map_type(&result);

    // {K} as type → HashSet<K>
    result = translate_set_type(&result);

    // &!T → &mut T
    result = result.replace("&!", "&mut ");

    // s type → String (in type position only)
    result = translate_string_type(&result);

    result
}

fn translate_vec_type(s: &str) -> String {
    let mut result = s.to_string();

    // Pattern: [T]~ → Vec<T>
    while let Some(start) = result.find('[') {
        if let Some(end) = find_matching_bracket(&result, start) {
            if result.get(end + 1..end + 2) == Some("~") {
                let inner = &result[start + 1..end];
                let replacement = format!("Vec<{inner}>");
                result = format!(
                    "{}{}{}",
                    &result[..start],
                    replacement,
                    &result[end + 2..]
                );
                continue;
            }
        }
        break;
    }

    result
}

fn translate_option_type(s: &str) -> String {
    let mut result = s.to_string();

    // ?T in type position: after `:`, `->`, `(`, `,`
    // Be careful not to transform ? used as postfix operator
    let type_prefixes = [": ?", "-> ?", "(?", ", ?"];

    for prefix in &type_prefixes {
        while let Some(pos) = result.find(prefix) {
            let after = pos + prefix.len();
            if let Some(end) = find_type_end(&result, after) {
                let inner = &result[after..end];
                let replacement = format!(
                    "{}Option<{inner}>",
                    &prefix[..prefix.len() - 1]
                );
                result = format!(
                    "{}{}{}",
                    &result[..pos],
                    replacement,
                    &result[end..]
                );
            } else {
                break;
            }
        }
    }

    result
}

fn translate_result_type(s: &str) -> String {
    let mut result = s.to_string();

    // R[T, E] → Result<T, E>
    while let Some(start) = result.find("R[") {
        // Make sure it's a type (preceded by type-context chars)
        if start > 0 {
            let prev = result.as_bytes()[start - 1];
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                break;
            }
        }
        let bracket_start = start + 1;
        if let Some(end) = find_matching_bracket(&result, bracket_start) {
            let inner = &result[bracket_start + 1..end];
            let replacement = format!("Result<{inner}>");
            result = format!(
                "{}{}{}",
                &result[..start],
                replacement,
                &result[end + 1..]
            );
        } else {
            break;
        }
    }

    result
}

fn translate_ptr_type(s: &str, sigil: char, name: &str) -> String {
    let mut result = s.to_string();

    // ^T → Box<T>, $T → Rc<T>
    // Must be in type position
    let type_prefixes: Vec<String> = vec![
        format!(": {sigil}"),
        format!("-> {sigil}"),
        format!("({sigil}"),
        format!(", {sigil}"),
    ];

    for prefix in &type_prefixes {
        while let Some(pos) = result.find(prefix.as_str()) {
            let after = pos + prefix.len();
            if let Some(end) = find_type_end(&result, after) {
                let inner = &result[after..end];
                let ctx = &prefix[..prefix.len() - 1];
                let replacement = format!("{ctx}{name}<{inner}>");
                result = format!(
                    "{}{}{}",
                    &result[..pos],
                    replacement,
                    &result[end..]
                );
            } else {
                break;
            }
        }
    }

    result
}

fn translate_arc_type(s: &str) -> String {
    let mut result = s.to_string();

    // @T in type context → Arc<T>
    // Must distinguish from @d, @test, @cfg, @{, @i, @allow, @bench
    let type_prefixes = [": @", "-> @", "(@", ", @"];

    for prefix in &type_prefixes {
        while let Some(pos) = result.find(prefix) {
            let after = pos + prefix.len();
            // Skip if followed by known attribute chars
            if let Some(next) = result.get(after..after + 1) {
                if next == "d"
                    || next == "{"
                    || next == "i"
                    || next == "t"
                    || next == "c"
                    || next == "b"
                    || next == "a"
                {
                    // Could be an attribute — check more carefully
                    let rest = &result[after..];
                    if rest.starts_with("d(")
                        || rest.starts_with("test")
                        || rest.starts_with("cfg(")
                        || rest.starts_with("i ")
                        || rest.starts_with("i!")
                        || rest.starts_with("allow(")
                        || rest.starts_with("bench")
                    {
                        break;
                    }
                }
            }
            if let Some(end) = find_type_end(&result, after) {
                let inner = &result[after..end];
                let ctx = &prefix[..prefix.len() - 1];
                let replacement = format!("{ctx}Arc<{inner}>");
                result = format!(
                    "{}{}{}",
                    &result[..pos],
                    replacement,
                    &result[end..]
                );
            } else {
                break;
            }
        }
    }

    result
}

fn translate_map_type(s: &str) -> String {
    let mut result = s.to_string();

    // {K: V} in type context → HashMap<K, V>
    let type_prefixes = [": {", "-> {", "({", ", {"];

    for prefix in &type_prefixes {
        while let Some(pos) = result.find(prefix) {
            let brace_pos = pos + prefix.len() - 1;
            if let Some(end) = find_matching_brace(&result, brace_pos) {
                let inner = &result[brace_pos + 1..end].trim();
                if let Some(colon) = inner.find(": ") {
                    let key = inner[..colon].trim();
                    let val = inner[colon + 2..].trim();
                    let ctx = &prefix[..prefix.len() - 1];
                    let replacement = format!("{ctx}HashMap<{key}, {val}>");
                    result = format!(
                        "{}{}{}",
                        &result[..pos],
                        replacement,
                        &result[end + 1..]
                    );
                    continue;
                }
            }
            break;
        }
    }

    result
}

fn translate_set_type(s: &str) -> String {
    let mut result = s.to_string();

    // {K} in type context → HashSet<K>
    let type_prefixes = [": {", "-> {", "({", ", {"];

    for prefix in &type_prefixes {
        while let Some(pos) = result.find(prefix) {
            let brace_pos = pos + prefix.len() - 1;
            if let Some(end) = find_matching_brace(&result, brace_pos) {
                let inner = result[brace_pos + 1..end].trim();
                // Only if it doesn't contain `:` (that's a map)
                if !inner.contains(": ") {
                    let ctx = &prefix[..prefix.len() - 1];
                    let replacement = format!("{ctx}HashSet<{inner}>");
                    result = format!(
                        "{}{}{}",
                        &result[..pos],
                        replacement,
                        &result[end + 1..]
                    );
                    continue;
                }
            }
            break;
        }
    }

    result
}

fn translate_string_type(s: &str) -> String {
    let mut result = s.to_string();
    // `: s` → `: String`, `-> s` → `-> String`, `(&s)` → `(&str)`
    // Only in type position — be conservative

    // &s followed by a non-alphanumeric → &str
    result = replace_type_s(&result, ": &s", ": &str");
    result = replace_type_s(&result, "-> &s", "-> &str");

    // s as standalone type (followed by non-alpha)
    result = replace_type_s(&result, ": s", ": String");
    result = replace_type_s(&result, "-> s", "-> String");

    result
}

/// Replace `from` with `to` only when the character after `from` is
/// not alphanumeric (avoiding partial-word matches like `str` → `Stringtr`).
fn replace_type_s(s: &str, from: &str, to: &str) -> String {
    let mut result = s.to_string();
    let mut search_from = 0;

    while let Some(pos) = result[search_from..].find(from) {
        let abs = search_from + pos;
        let after = abs + from.len();
        let followed_ok = after >= result.len()
            || !result.as_bytes()[after].is_ascii_alphanumeric();
        if followed_ok {
            result = format!("{}{to}{}", &result[..abs], &result[after..]);
            search_from = abs + to.len();
        } else {
            search_from = after;
        }
    }

    result
}

// ── Print macros ─────────────────────────────────────────────────────

fn translate_print_macros(s: &str) -> String {
    let mut result = s.to_string();

    // p"..." → println!("...")
    result = translate_print_macro(&result, "p\"", "println!(\"", "\")");

    // ep"..." → eprintln!("...")
    result = translate_print_macro(&result, "ep\"", "eprintln!(\"", "\")");

    // f"..." → format!("...")
    result = translate_format_macro(&result);

    result
}

fn translate_print_macro(s: &str, prefix: &str, rust_start: &str, rust_end: &str) -> String {
    let mut result = s.to_string();

    while let Some(start) = result.find(prefix) {
        let after = start + prefix.len();
        // Find the closing quote
        if let Some(end) = find_closing_quote(&result, after) {
            let content = &result[after..end];
            let replacement = format!("{rust_start}{content}{rust_end}");
            result = format!(
                "{}{}{}",
                &result[..start],
                replacement,
                &result[end + 1..]
            );
        } else {
            break;
        }
    }

    result
}

fn translate_format_macro(s: &str) -> String {
    let mut result = s.to_string();

    // f"..." → format!("...")
    // Must not conflict with `fn` (already handled) or function calls
    // Look for f" specifically (not preceded by alphanumeric)
    while let Some(pos) = find_format_string(&result) {
        let after = pos + 2; // skip f"
        if let Some(end) = find_closing_quote(&result, after) {
            let content = &result[after..end];
            let replacement = format!("format!(\"{content}\")");
            result = format!(
                "{}{}{}",
                &result[..pos],
                replacement,
                &result[end + 1..]
            );
        } else {
            break;
        }
    }

    result
}

fn find_format_string(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    for i in 0..bytes.len().saturating_sub(1) {
        if bytes[i] == b'f' && bytes[i + 1] == b'"' {
            // Must not be preceded by alphanumeric (would be part of identifier)
            if i == 0 || !bytes[i - 1].is_ascii_alphanumeric() {
                return Some(i);
            }
        }
    }
    None
}

fn find_closing_quote(s: &str, start: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            // Check not escaped
            if i == 0 || bytes[i - 1] != b'\\' {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

// ── Path translation ─────────────────────────────────────────────────

fn translate_paths(s: &str) -> String {
    // Selectively convert `.` to `::` for module paths only.
    // Method calls (preceded by `)` or identifier) stay as `.`
    // Module paths (like `std.io.Read`) convert to `std::io::Read`
    //
    // Heuristic: if the token before the `.` starts with uppercase or is
    // a known module prefix, convert to `::`.
    // For simplicity in this text-level transpiler, we leave `.` as is
    // since Rust also uses `.` for method calls. The `use` statements
    // are already handled separately.
    s.to_string()
}

// ── Boolean literals ─────────────────────────────────────────────────

fn translate_booleans(s: &str) -> String {
    let mut result = s.to_string();
    // 1b → true, 0b → false
    // Must not conflict with binary literals like 0b1010
    // Our pattern: standalone 1b or 0b followed by non-alphanumeric

    result = replace_boolean(&result, "1b", "true");
    result = replace_boolean(&result, "0b", "false");

    result
}

fn replace_boolean(s: &str, from: &str, to: &str) -> String {
    let mut result = s.to_string();
    let bytes = from.as_bytes();

    let mut search_from = 0;
    while let Some(pos) = result[search_from..].find(from) {
        let abs_pos = search_from + pos;
        let after = abs_pos + from.len();

        // Check not part of a larger token
        let preceded_ok = abs_pos == 0
            || !result.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
        let followed_ok = after >= result.len()
            || !result.as_bytes()[after].is_ascii_alphanumeric();

        // For 0b, make sure it's not a binary literal (0b1010)
        if bytes == b"0b" && after < result.len() && result.as_bytes()[after].is_ascii_digit() {
            search_from = after;
            continue;
        }

        if preceded_ok && followed_ok {
            result = format!("{}{to}{}", &result[..abs_pos], &result[after..]);
            search_from = abs_pos + to.len();
        } else {
            search_from = after;
        }
    }

    result
}

// ── Generics [T] → <T> ──────────────────────────────────────────────

fn translate_generics_to_angle(s: &str) -> String {
    let mut result = s.to_string();
    let mut changed = true;

    while changed {
        changed = false;
        if let Some(pos) = find_generic_square_bracket(&result) {
            if let Some(end) = find_matching_bracket(&result, pos) {
                let inner = &result[pos + 1..end];
                let replacement = format!("<{inner}>");
                result = format!(
                    "{}{}{}",
                    &result[..pos],
                    replacement,
                    &result[end + 1..]
                );
                changed = true;
            }
        }
    }

    result
}

/// Find a `[` that's part of generics (preceded by an identifier).
fn find_generic_square_bracket(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    for i in 1..bytes.len() {
        if bytes[i] == b'[' {
            let prev = bytes[i - 1];
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                // Make sure it's not a Vec literal like [1, 2]~ (already handled)
                // or array indexing
                return Some(i);
            }
        }
    }
    None
}

// ── Bracket/brace helpers ────────────────────────────────────────────

fn find_matching_bracket(s: &str, pos: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if pos >= bytes.len() || bytes[pos] != b'[' {
        return None;
    }
    let mut depth = 0;
    for i in pos..bytes.len() {
        match bytes[i] {
            b'[' => depth += 1,
            b']' => {
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

fn find_matching_brace(s: &str, pos: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if pos >= bytes.len() || bytes[pos] != b'{' {
        return None;
    }
    let mut depth = 0;
    for i in pos..bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
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

/// Find the end of a type name starting at `start`.
/// Types end at `,`, `)`, `{`, `>`, `]`, whitespace, or end of string.
fn find_type_end(s: &str, start: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = start;
    let mut depth_angle = 0i32;
    let mut depth_bracket = 0i32;
    let mut depth_paren = 0i32;

    while i < bytes.len() {
        match bytes[i] {
            b'<' | b'[' => {
                if bytes[i] == b'<' {
                    depth_angle += 1;
                } else {
                    depth_bracket += 1;
                }
            }
            b'>' => {
                if depth_angle > 0 {
                    depth_angle -= 1;
                } else {
                    return Some(i);
                }
            }
            b']' => {
                if depth_bracket > 0 {
                    depth_bracket -= 1;
                } else {
                    return Some(i);
                }
            }
            b'(' => depth_paren += 1,
            b')' => {
                if depth_paren > 0 {
                    depth_paren -= 1;
                } else {
                    return Some(i);
                }
            }
            b',' | b'{' | b';'
                if depth_angle == 0 && depth_bracket == 0 && depth_paren == 0 =>
            {
                return Some(i);
            }
            b' ' if depth_angle == 0 && depth_bracket == 0 && depth_paren == 0 => {
                return Some(i);
            }
            _ => {}
        }
        i += 1;
    }

    if i > start {
        Some(i)
    } else {
        None
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn leading_whitespace(s: &str) -> &str {
    let trimmed = s.trim_start();
    &s[..s.len() - trimmed.len()]
}

fn replace_keyword_at_start(s: &str, from: &str, to: &str) -> String {
    if s.starts_with(from) {
        format!("{to}{}", &s[from.len()..])
    } else {
        s.to_string()
    }
}

fn replace_keyword_at_boundary(s: &str, from: &str, to: &str) -> String {
    if let Some(pos) = s.find(from) {
        if pos == 0 || !s.as_bytes()[pos - 1].is_ascii_alphanumeric() {
            return format!("{}{to}{}", &s[..pos], &s[pos + from.len()..]);
        }
    }
    s.to_string()
}

fn is_trait_def(s: &str) -> bool {
    // Heuristic: T followed by an identifier and then `{` or `:`
    let rest = &s[2..];
    let first_word_end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_');
    if let Some(end) = first_word_end {
        let after = rest[end..].trim_start();
        after.starts_with('{') || after.starts_with(':') || after.starts_with('<')
    } else {
        false
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pub_fn() {
        assert_eq!(
            translate_line("+f add(a: i32, b: i32) -> i32 {"),
            "pub fn add(a: i32, b: i32) -> i32 {"
        );
    }

    #[test]
    fn test_fn() {
        assert_eq!(translate_line("f helper() {"), "fn helper() {");
    }

    #[test]
    fn test_let() {
        assert_eq!(translate_line("    v x = 42;"), "    let x = 42;");
    }

    #[test]
    fn test_let_mut() {
        assert_eq!(translate_line("    m count = 0;"), "    let mut count = 0;");
    }

    #[test]
    fn test_struct() {
        assert_eq!(translate_line("+S Point {"), "pub struct Point {");
    }

    #[test]
    fn test_enum() {
        assert_eq!(translate_line("E Color {"), "enum Color {");
    }

    #[test]
    fn test_trait() {
        assert_eq!(translate_line("+T Display {"), "pub trait Display {");
    }

    #[test]
    fn test_impl_inherent() {
        assert_eq!(translate_line("I ~ Point {"), "impl Point {");
    }

    #[test]
    fn test_impl_trait() {
        assert_eq!(
            translate_line("I Display ~ Point {"),
            "impl Display for Point {"
        );
    }

    #[test]
    fn test_use_path() {
        assert_eq!(translate_line("u std.io.Read;"), "use std::io::Read;");
    }

    #[test]
    fn test_pub_use() {
        assert_eq!(
            translate_line("+u std.fmt.Display;"),
            "pub use std::fmt::Display;"
        );
    }

    #[test]
    fn test_vec_type() {
        assert_eq!(
            translate_line("    items: [i32]~,"),
            "    items: Vec<i32>,"
        );
    }

    #[test]
    fn test_option_type() {
        assert_eq!(
            translate_line("fn foo(x: ?i32) {"),
            "fn foo(x: Option<i32>) {"
        );
    }

    #[test]
    fn test_result_type() {
        assert_eq!(
            translate_line("fn foo() -> R[i32, Error] {"),
            "fn foo() -> Result<i32, Error> {"
        );
    }

    #[test]
    fn test_box_type() {
        assert_eq!(translate_line("    data: ^Node,"), "    data: Box<Node>,");
    }

    #[test]
    fn test_rc_type() {
        assert_eq!(translate_line("    shared: $Data,"), "    shared: Rc<Data>,");
    }

    #[test]
    fn test_arc_type() {
        assert_eq!(
            translate_line("    atomic: @Data,"),
            "    atomic: Arc<Data>,"
        );
    }

    #[test]
    fn test_map_type() {
        assert_eq!(
            translate_line("    map: {String: i32},"),
            "    map: HashMap<String, i32>,"
        );
    }

    #[test]
    fn test_derive() {
        assert_eq!(
            translate_line("@d(Debug, Clone)"),
            "#[derive(Debug, Clone)]"
        );
    }

    #[test]
    fn test_inline() {
        assert_eq!(translate_line("@i"), "#[inline]");
    }

    #[test]
    fn test_test_attr() {
        assert_eq!(translate_line("@test"), "#[test]");
    }

    #[test]
    fn test_println() {
        assert_eq!(
            translate_line("    p\"hello\";"),
            "    println!(\"hello\");"
        );
    }

    #[test]
    fn test_if() {
        assert_eq!(translate_line("? x > 0 {"), "if x > 0 {");
    }

    #[test]
    fn test_else() {
        assert_eq!(translate_line("} : {"), "} else {");
    }

    #[test]
    fn test_for_loop() {
        assert_eq!(
            translate_line("@ item ~ items {"),
            "for item in items {"
        );
    }

    #[test]
    fn test_loop() {
        assert_eq!(translate_line("@ {"), "loop {");
    }

    #[test]
    fn test_match() {
        assert_eq!(translate_line("? value {"), "match value {");
    }

    #[test]
    fn test_return() {
        assert_eq!(translate_line("    ret 42;"), "    return 42;");
    }

    #[test]
    fn test_boolean_true() {
        assert_eq!(translate_line("    v x = 1b;"), "    let x = true;");
    }

    #[test]
    fn test_boolean_false() {
        assert_eq!(translate_line("    v x = 0b;"), "    let x = false;");
    }

    #[test]
    fn test_async_fn() {
        assert_eq!(
            translate_line("+af fetch(url: &str) {"),
            "pub async fn fetch(url: &str) {"
        );
    }

    #[test]
    fn test_const_fn() {
        assert_eq!(
            translate_line("c f max_size() -> usize { 1024 }"),
            "const fn max_size() -> usize { 1024 }"
        );
    }

    #[test]
    fn test_pub_mod() {
        assert_eq!(translate_line("+M math {"), "pub mod math {");
    }

    #[test]
    fn test_mut_ref() {
        assert_eq!(
            translate_line("fn foo(x: &!i32) {"),
            "fn foo(x: &mut i32) {"
        );
    }

    #[test]
    fn test_where_clause() {
        assert_eq!(
            translate_line("fn foo() ~> T: Display {"),
            "fn foo() where T: Display {"
        );
    }

    #[test]
    fn test_struct_literal() {
        assert_eq!(
            translate_line("Point @{ x: 1, y: 2 }"),
            "Point { x: 1, y: 2 }"
        );
    }

    #[test]
    fn test_full_translation() {
        let rdx = "\
+f add(a: i32, b: i32) -> i32 {
    v result = a + b;
    result
}";
        let rs = translate(rdx);
        assert!(rs.contains("pub fn add"));
        assert!(rs.contains("let result"));
    }

    #[test]
    fn test_effect_annotation() {
        let result = translate_effects("+f read() -> s / io {");
        assert!(result.contains("/* effect: io */"));
        assert!(!result.contains("/ io"));
    }

    #[test]
    fn test_pub_const() {
        assert_eq!(
            translate_line("+v MAX: usize = 100;"),
            "pub const MAX: usize = 100;"
        );
    }
}

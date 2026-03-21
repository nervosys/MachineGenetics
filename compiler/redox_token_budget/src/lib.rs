// Token budget reporting for the Redox compiler.
//
// Implements `redox build --token-report` functionality:
// - Per-file and per-function token counts
// - Compact vs expanded savings metrics
// - Module-level aggregation
// - Budget tracking and threshold alerts

use std::collections::BTreeMap;

// ── Token Counting ─────────────────────────────────────────────────────────

/// Count tokens in a source string using a language-aware tokenizer.
///
/// Tokens are: identifiers, keywords, literals, operators, delimiters,
/// and punctuation. Whitespace and comments are not counted.
pub fn count_tokens(source: &str) -> usize {
    let mut count = 0;
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // Skip whitespace
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        // Skip line comments
        if ch == '/' && i + 1 < len && chars[i + 1] == '/' {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Skip block comments
        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2;
            }
            continue;
        }

        // String literals count as 1 token
        if ch == '"' {
            count += 1;
            i += 1;
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1; // skip escaped char
                }
                i += 1;
            }
            if i < len {
                i += 1;
            }
            continue;
        }

        // Char literals count as 1 token
        if ch == '\'' && i + 1 < len && chars[i + 1] != '\'' {
            // Could be a lifetime or a char literal
            // Lifetime: 'a, 'static — identifier following quote
            // Char literal: 'x', '\n'
            count += 1;
            i += 1;
            if i < len && chars[i] == '\\' {
                i += 1; // skip escape
            }
            while i < len && chars[i] != '\'' && chars[i].is_alphanumeric() {
                i += 1;
            }
            if i < len && chars[i] == '\'' {
                i += 1;
            }
            continue;
        }

        // Identifiers and keywords
        if ch.is_alphabetic() || ch == '_' || ch == '@' {
            count += 1;
            i += 1;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '!') {
                // Include trailing `!` for macros like `println!`
                if chars[i] == '!' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Numeric literals
        if ch.is_ascii_digit() {
            count += 1;
            i += 1;
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                i += 1;
            }
            continue;
        }

        // Multi-char operators: ->, =>, ::, .., ==, !=, <=, >=, &&, ||, <<, >>, +=, -=, etc.
        if i + 1 < len {
            let two = &source[i..i + 2];
            match two {
                "->" | "=>" | "::" | ".." | "==" | "!=" | "<=" | ">="
                | "&&" | "||" | "<<" | ">>" | "+=" | "-=" | "*=" | "/="
                | "&=" | "|=" | "^=" | "%=" => {
                    count += 1;
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }

        // Single-char punctuation/operators
        count += 1;
        i += 1;
    }

    count
}

// ── Source Item Parsing ────────────────────────────────────────────────────

/// A source item (function, struct, impl block, etc.) with token count.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemTokens {
    pub kind: ItemKind,
    pub name: String,
    pub line: usize,
    pub token_count: usize,
    pub compact_token_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    Function,
    Struct,
    Enum,
    Impl,
    Trait,
    Mod,
    Const,
    Static,
    TypeAlias,
    Use,
    Other,
}

impl std::fmt::Display for ItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemKind::Function => write!(f, "fn"),
            ItemKind::Struct => write!(f, "struct"),
            ItemKind::Enum => write!(f, "enum"),
            ItemKind::Impl => write!(f, "impl"),
            ItemKind::Trait => write!(f, "trait"),
            ItemKind::Mod => write!(f, "mod"),
            ItemKind::Const => write!(f, "const"),
            ItemKind::Static => write!(f, "static"),
            ItemKind::TypeAlias => write!(f, "type"),
            ItemKind::Use => write!(f, "use"),
            ItemKind::Other => write!(f, "other"),
        }
    }
}

/// Extract items from source and compute per-item token counts.
///
/// This is a lightweight heuristic parser — it identifies top-level items
/// by scanning for keyword patterns at the start of lines and tracks
/// brace depth to find item boundaries.
pub fn extract_items(source: &str) -> Vec<ItemTokens> {
    let mut items = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
            i += 1;
            continue;
        }

        // Skip attributes — they'll be counted with the item following them
        if trimmed.starts_with("#[") || trimmed.starts_with("@") {
            i += 1;
            continue;
        }

        let (kind, name) = if let Some(parsed) = parse_item_header(trimmed) {
            parsed
        } else {
            i += 1;
            continue;
        };

        // Collect the item's source: from current line to matching close brace
        let start_line = i;
        let item_source = if trimmed.contains('{') {
            collect_braced_item(&lines, &mut i)
        } else if trimmed.ends_with(';') {
            let s = lines[i].to_string();
            i += 1;
            s
        } else {
            // Multi-line item without braces (e.g., function signature spanning lines)
            let mut s = String::new();
            while i < lines.len() {
                s.push_str(lines[i]);
                s.push('\n');
                if lines[i].contains('{') {
                    let rest = collect_braced_rest(&lines, &mut i);
                    s.push_str(&rest);
                    break;
                }
                if lines[i].trim().ends_with(';') {
                    i += 1;
                    break;
                }
                i += 1;
            }
            s
        };

        let token_count = count_tokens(&item_source);
        items.push(ItemTokens {
            kind,
            name,
            line: start_line + 1, // 1-based
            token_count,
            compact_token_count: None,
        });
    }

    items
}

fn parse_item_header(line: &str) -> Option<(ItemKind, String)> {
    // Strip visibility qualifiers
    let line = strip_vis(line);
    let line = line.trim();

    // Match item keywords
    if line.starts_with("fn ") || line.starts_with("async fn ") {
        let name = extract_name_after(line, "fn ");
        Some((ItemKind::Function, name))
    } else if line.starts_with("struct ") {
        let name = extract_name_after(line, "struct ");
        Some((ItemKind::Struct, name))
    } else if line.starts_with("enum ") {
        let name = extract_name_after(line, "enum ");
        Some((ItemKind::Enum, name))
    } else if line.starts_with("impl") {
        let rest = line.strip_prefix("impl").unwrap().trim();
        let name = rest.split(|c: char| c == '{' || c == '<' || c.is_whitespace())
            .next()
            .unwrap_or("?")
            .to_string();
        Some((ItemKind::Impl, name))
    } else if line.starts_with("trait ") {
        let name = extract_name_after(line, "trait ");
        Some((ItemKind::Trait, name))
    } else if line.starts_with("mod ") {
        let name = extract_name_after(line, "mod ");
        Some((ItemKind::Mod, name))
    } else if line.starts_with("const ") {
        let name = extract_name_after(line, "const ");
        Some((ItemKind::Const, name))
    } else if line.starts_with("static ") {
        let name = extract_name_after(line, "static ");
        Some((ItemKind::Static, name))
    } else if line.starts_with("type ") {
        let name = extract_name_after(line, "type ");
        Some((ItemKind::TypeAlias, name))
    } else if line.starts_with("use ") {
        Some((ItemKind::Use, "use".to_string()))
    } else {
        None
    }
}

fn strip_vis(line: &str) -> &str {
    if let Some(rest) = line.strip_prefix("pub(crate) ") {
        rest
    } else if let Some(rest) = line.strip_prefix("pub(super) ") {
        rest
    } else if let Some(rest) = line.strip_prefix("pub ") {
        rest
    } else {
        line
    }
}

fn extract_name_after(line: &str, keyword: &str) -> String {
    let after = if let Some(pos) = line.find(keyword) {
        &line[pos + keyword.len()..]
    } else {
        return "?".to_string();
    };
    let after = after.trim();
    after.split(|c: char| c == '(' || c == '<' || c == '{' || c == ':' || c == ';' || c.is_whitespace())
        .next()
        .unwrap_or("?")
        .to_string()
}

fn collect_braced_item(lines: &[&str], i: &mut usize) -> String {
    let mut depth = 0;
    let mut source = String::new();
    loop {
        if *i >= lines.len() {
            break;
        }
        let line = lines[*i];
        source.push_str(line);
        source.push('\n');
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
            }
        }
        *i += 1;
        if depth == 0 {
            break;
        }
    }
    source
}

fn collect_braced_rest(lines: &[&str], i: &mut usize) -> String {
    // We're on the line with '{' — count from this line's braces
    let mut depth = 0;
    let start_line = lines[*i];
    for ch in start_line.chars() {
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
        }
    }
    *i += 1;

    let mut source = String::new();
    while *i < lines.len() && depth > 0 {
        let line = lines[*i];
        source.push_str(line);
        source.push('\n');
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
            }
        }
        *i += 1;
    }
    source
}

// ── File Report ────────────────────────────────────────────────────────────

/// Token report for a single source file.
#[derive(Debug, Clone)]
pub struct FileReport {
    pub path: String,
    pub total_tokens: usize,
    pub compact_tokens: Option<usize>,
    pub items: Vec<ItemTokens>,
}

impl FileReport {
    /// Create a file report from source code.
    pub fn from_source(path: &str, source: &str) -> Self {
        let total_tokens = count_tokens(source);
        let items = extract_items(source);
        Self {
            path: path.to_string(),
            total_tokens,
            compact_tokens: None,
            items,
        }
    }

    /// Set the compact token count (from a compact-formatted version of same source).
    pub fn with_compact_tokens(mut self, compact_tokens: usize) -> Self {
        self.compact_tokens = Some(compact_tokens);
        self
    }

    /// Compute savings percentage vs compact form.
    pub fn savings_percent(&self) -> Option<f64> {
        self.compact_tokens.map(|ct| {
            if self.total_tokens > 0 {
                ((self.total_tokens - ct) as f64 / self.total_tokens as f64) * 100.0
            } else {
                0.0
            }
        })
    }

    /// Get the number of items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Get the function with the highest token count.
    pub fn most_expensive_function(&self) -> Option<&ItemTokens> {
        self.items.iter()
            .filter(|it| it.kind == ItemKind::Function)
            .max_by_key(|it| it.token_count)
    }

    /// Format the report as human-readable text.
    pub fn format_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("File: {}\n", self.path));
        out.push_str(&format!("  Total tokens: {}\n", self.total_tokens));
        if let Some(ct) = self.compact_tokens {
            out.push_str(&format!("  Compact tokens: {}\n", ct));
            if let Some(pct) = self.savings_percent() {
                out.push_str(&format!("  Savings: {:.1}%\n", pct));
            }
        }
        out.push_str(&format!("  Items: {}\n", self.items.len()));
        for item in &self.items {
            out.push_str(&format!(
                "    L{:>4} {:>6} {:>5} tok  {}\n",
                item.line, item.kind, item.token_count, item.name
            ));
        }
        out
    }
}

// ── Module Report ──────────────────────────────────────────────────────────

/// Aggregated token report for a module (directory of files).
#[derive(Debug, Clone)]
pub struct ModuleReport {
    pub name: String,
    pub files: Vec<FileReport>,
}

impl ModuleReport {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            files: Vec::new(),
        }
    }

    pub fn add_file(&mut self, report: FileReport) {
        self.files.push(report);
    }

    pub fn total_tokens(&self) -> usize {
        self.files.iter().map(|f| f.total_tokens).sum()
    }

    pub fn total_compact_tokens(&self) -> Option<usize> {
        let mut total = 0;
        for f in &self.files {
            total += f.compact_tokens?;
        }
        Some(total)
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn total_items(&self) -> usize {
        self.files.iter().map(|f| f.items.len()).sum()
    }

    pub fn savings_percent(&self) -> Option<f64> {
        let total = self.total_tokens();
        let compact = self.total_compact_tokens()?;
        if total > 0 {
            Some(((total - compact) as f64 / total as f64) * 100.0)
        } else {
            Some(0.0)
        }
    }
}

// ── Build Report ───────────────────────────────────────────────────────────

/// Full build-level token report.
#[derive(Debug, Clone)]
pub struct BuildReport {
    pub modules: BTreeMap<String, ModuleReport>,
}

impl BuildReport {
    pub fn new() -> Self {
        Self {
            modules: BTreeMap::new(),
        }
    }

    pub fn add_file(&mut self, module: &str, report: FileReport) {
        self.modules.entry(module.to_string())
            .or_insert_with(|| ModuleReport::new(module))
            .add_file(report);
    }

    pub fn total_tokens(&self) -> usize {
        self.modules.values().map(|m| m.total_tokens()).sum()
    }

    pub fn total_files(&self) -> usize {
        self.modules.values().map(|m| m.file_count()).sum()
    }

    pub fn total_items(&self) -> usize {
        self.modules.values().map(|m| m.total_items()).sum()
    }

    /// Format the full build report as text (the `--token-report` output).
    pub fn format_text(&self) -> String {
        let mut out = String::new();
        out.push_str("═══ Token Budget Report ═══\n\n");

        for (name, module) in &self.modules {
            out.push_str(&format!("Module: {}\n", name));
            out.push_str(&format!("  Files: {}  Tokens: {}", module.file_count(), module.total_tokens()));
            if let Some(pct) = module.savings_percent() {
                out.push_str(&format!("  Compact savings: {:.1}%", pct));
            }
            out.push('\n');

            for file in &module.files {
                out.push_str(&format!("  {}: {} tokens", file.path, file.total_tokens));
                if let Some(pct) = file.savings_percent() {
                    out.push_str(&format!(" ({:.1}% savings)", pct));
                }
                out.push('\n');

                for item in &file.items {
                    out.push_str(&format!(
                        "    L{:>4} {:>6} {:>5} tok  {}\n",
                        item.line, item.kind, item.token_count, item.name
                    ));
                }
            }
            out.push('\n');
        }

        out.push_str(&format!("Total: {} tokens across {} files ({} items)\n",
            self.total_tokens(), self.total_files(), self.total_items()));
        out
    }

    /// Format as JSON for machine consumption.
    pub fn format_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str(&format!("  \"total_tokens\": {},\n", self.total_tokens()));
        out.push_str(&format!("  \"total_files\": {},\n", self.total_files()));
        out.push_str(&format!("  \"total_items\": {},\n", self.total_items()));
        out.push_str("  \"modules\": {\n");

        let modules: Vec<(&String, &ModuleReport)> = self.modules.iter().collect();
        for (mi, (name, module)) in modules.iter().enumerate() {
            out.push_str(&format!("    \"{}\": {{\n", escape_json(name)));
            out.push_str(&format!("      \"total_tokens\": {},\n", module.total_tokens()));
            out.push_str(&format!("      \"file_count\": {},\n", module.file_count()));
            out.push_str("      \"files\": [\n");

            for (fi, file) in module.files.iter().enumerate() {
                out.push_str("        {\n");
                out.push_str(&format!("          \"path\": \"{}\",\n", escape_json(&file.path)));
                out.push_str(&format!("          \"total_tokens\": {},\n", file.total_tokens));
                if let Some(ct) = file.compact_tokens {
                    out.push_str(&format!("          \"compact_tokens\": {},\n", ct));
                }
                out.push_str("          \"items\": [\n");

                for (ii, item) in file.items.iter().enumerate() {
                    out.push_str("            {\n");
                    out.push_str(&format!("              \"kind\": \"{}\",\n", item.kind));
                    out.push_str(&format!("              \"name\": \"{}\",\n", escape_json(&item.name)));
                    out.push_str(&format!("              \"line\": {},\n", item.line));
                    out.push_str(&format!("              \"token_count\": {}\n", item.token_count));
                    out.push_str("            }");
                    if ii + 1 < file.items.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }

                out.push_str("          ]\n");
                out.push_str("        }");
                if fi + 1 < module.files.len() {
                    out.push(',');
                }
                out.push('\n');
            }

            out.push_str("      ]\n");
            out.push_str("    }");
            if mi + 1 < modules.len() {
                out.push(',');
            }
            out.push('\n');
        }

        out.push_str("  }\n");
        out.push_str("}\n");
        out
    }
}

impl Default for BuildReport {
    fn default() -> Self {
        Self::new()
    }
}

// ── Budget Threshold ───────────────────────────────────────────────────────

/// A token budget threshold that can trigger warnings.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// Maximum tokens per function
    pub max_function_tokens: Option<usize>,
    /// Maximum tokens per file
    pub max_file_tokens: Option<usize>,
    /// Maximum tokens per module
    pub max_module_tokens: Option<usize>,
}

/// A budget violation.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetViolation {
    pub kind: ViolationKind,
    pub location: String,
    pub actual: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViolationKind {
    FunctionTooLarge,
    FileTooLarge,
    ModuleTooLarge,
}

impl std::fmt::Display for ViolationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViolationKind::FunctionTooLarge => write!(f, "function exceeds token budget"),
            ViolationKind::FileTooLarge => write!(f, "file exceeds token budget"),
            ViolationKind::ModuleTooLarge => write!(f, "module exceeds token budget"),
        }
    }
}

impl TokenBudget {
    pub fn new() -> Self {
        Self {
            max_function_tokens: None,
            max_file_tokens: None,
            max_module_tokens: None,
        }
    }

    pub fn with_function_limit(mut self, limit: usize) -> Self {
        self.max_function_tokens = Some(limit);
        self
    }

    pub fn with_file_limit(mut self, limit: usize) -> Self {
        self.max_file_tokens = Some(limit);
        self
    }

    pub fn with_module_limit(mut self, limit: usize) -> Self {
        self.max_module_tokens = Some(limit);
        self
    }

    /// Check a build report against this budget.
    pub fn check(&self, report: &BuildReport) -> Vec<BudgetViolation> {
        let mut violations = Vec::new();

        for (mod_name, module) in &report.modules {
            if let Some(limit) = self.max_module_tokens {
                let total = module.total_tokens();
                if total > limit {
                    violations.push(BudgetViolation {
                        kind: ViolationKind::ModuleTooLarge,
                        location: mod_name.clone(),
                        actual: total,
                        limit,
                    });
                }
            }

            for file in &module.files {
                if let Some(limit) = self.max_file_tokens {
                    if file.total_tokens > limit {
                        violations.push(BudgetViolation {
                            kind: ViolationKind::FileTooLarge,
                            location: file.path.clone(),
                            actual: file.total_tokens,
                            limit,
                        });
                    }
                }

                if let Some(fn_limit) = self.max_function_tokens {
                    for item in &file.items {
                        if item.kind == ItemKind::Function && item.token_count > fn_limit {
                            violations.push(BudgetViolation {
                                kind: ViolationKind::FunctionTooLarge,
                                location: format!("{}::{}", file.path, item.name),
                                actual: item.token_count,
                                limit: fn_limit,
                            });
                        }
                    }
                }
            }
        }

        violations
    }
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self::new()
    }
}

// ── Utilities ──────────────────────────────────────────────────────────────

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Token counting ──

    #[test]
    fn test_count_empty() {
        assert_eq!(count_tokens(""), 0);
    }

    #[test]
    fn test_count_simple_fn() {
        let source = "fn main() {}";
        let count = count_tokens(source);
        // fn, main, (, ), {, } = 6
        assert_eq!(count, 6);
    }

    #[test]
    fn test_count_with_let() {
        let source = "let x = 42;";
        let count = count_tokens(source);
        // let, x, =, 42, ; = 5
        assert_eq!(count, 5);
    }

    #[test]
    fn test_count_skips_line_comments() {
        let source = "let x = 1; // this is a comment\nlet y = 2;";
        let count = count_tokens(source);
        // let, x, =, 1, ;, let, y, =, 2, ; = 10
        assert_eq!(count, 10);
    }

    #[test]
    fn test_count_skips_block_comments() {
        let source = "let x = /* comment */ 1;";
        let count = count_tokens(source);
        // let, x, =, 1, ; = 5
        assert_eq!(count, 5);
    }

    #[test]
    fn test_count_string_literal() {
        let source = "let s = \"hello world\";";
        let count = count_tokens(source);
        // let, s, =, "hello world", ; = 5
        assert_eq!(count, 5);
    }

    #[test]
    fn test_count_operators() {
        let source = "a + b * c -> d";
        let count = count_tokens(source);
        // a, +, b, *, c, ->, d = 7
        assert_eq!(count, 7);
    }

    #[test]
    fn test_count_double_colon() {
        let source = "std::io::Read";
        let count = count_tokens(source);
        // std, ::, io, ::, Read = 5
        assert_eq!(count, 5);
    }

    #[test]
    fn test_count_macro() {
        let source = "println!(\"test\")";
        let count = count_tokens(source);
        // println!, (, "test", ) = 4
        assert_eq!(count, 4);
    }

    #[test]
    fn test_count_multiline() {
        let source = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}";
        let count = count_tokens(source);
        // pub, fn, add, (, a, :, i32, ,, b, :, i32, ), ->, i32, {, a, +, b, } = 19
        assert_eq!(count, 19);
    }

    // ── Item extraction ──

    #[test]
    fn test_extract_simple_fn() {
        let source = "fn hello() {\n    println!(\"hi\");\n}\n";
        let items = extract_items(source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::Function);
        assert_eq!(items[0].name, "hello");
        assert!(items[0].token_count > 0);
    }

    #[test]
    fn test_extract_pub_fn() {
        let source = "pub fn greet(name: &str) -> String {\n    format!(\"Hi {}\", name)\n}\n";
        let items = extract_items(source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::Function);
        assert_eq!(items[0].name, "greet");
    }

    #[test]
    fn test_extract_struct() {
        let source = "pub struct Foo {\n    bar: i32,\n}\n";
        let items = extract_items(source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::Struct);
        assert_eq!(items[0].name, "Foo");
    }

    #[test]
    fn test_extract_enum() {
        let source = "enum Color {\n    Red,\n    Green,\n    Blue,\n}\n";
        let items = extract_items(source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::Enum);
        assert_eq!(items[0].name, "Color");
    }

    #[test]
    fn test_extract_multiple_items() {
        let source = "fn a() {}\n\nfn b() {}\n\nstruct C {}\n";
        let items = extract_items(source);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].name, "a");
        assert_eq!(items[1].name, "b");
        assert_eq!(items[2].name, "C");
    }

    #[test]
    fn test_extract_const() {
        let source = "const MAX: usize = 100;\n";
        let items = extract_items(source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::Const);
        assert_eq!(items[0].name, "MAX");
    }

    #[test]
    fn test_extract_impl() {
        let source = "impl Foo {\n    fn bar(&self) {}\n}\n";
        let items = extract_items(source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::Impl);
        assert_eq!(items[0].name, "Foo");
    }

    // ── File report ──

    #[test]
    fn test_file_report_basic() {
        let source = "fn main() {\n    let x = 1;\n}\n";
        let report = FileReport::from_source("main.rs", source);
        assert_eq!(report.path, "main.rs");
        assert!(report.total_tokens > 0);
        assert_eq!(report.item_count(), 1);
    }

    #[test]
    fn test_file_report_savings() {
        let source = "fn main() {\n    let x = 1;\n}\n";
        let report = FileReport::from_source("main.rs", source)
            .with_compact_tokens(5);
        assert!(report.savings_percent().unwrap() > 0.0);
    }

    #[test]
    fn test_file_report_most_expensive() {
        let source = "\
fn short() {}

fn long_function() {
    let a = 1;
    let b = 2;
    let c = a + b;
    let d = c * 2;
    let e = d - a;
}
";
        let report = FileReport::from_source("test.rs", source);
        let expensive = report.most_expensive_function().unwrap();
        assert_eq!(expensive.name, "long_function");
    }

    #[test]
    fn test_file_report_format_text() {
        let source = "fn hello() {}\n";
        let report = FileReport::from_source("hello.rs", source);
        let text = report.format_text();
        assert!(text.contains("hello.rs"));
        assert!(text.contains("hello"));
    }

    // ── Module report ──

    #[test]
    fn test_module_report() {
        let mut module = ModuleReport::new("my_mod");
        module.add_file(FileReport::from_source("a.rs", "fn a() {}"));
        module.add_file(FileReport::from_source("b.rs", "fn b() {}"));
        assert_eq!(module.file_count(), 2);
        assert!(module.total_tokens() > 0);
    }

    // ── Build report ──

    #[test]
    fn test_build_report() {
        let mut report = BuildReport::new();
        report.add_file("core", FileReport::from_source("core/lib.rs", "fn init() {}"));
        report.add_file("core", FileReport::from_source("core/util.rs", "fn helper() {}"));
        report.add_file("api", FileReport::from_source("api/lib.rs", "pub fn serve() {}"));

        assert_eq!(report.total_files(), 3);
        assert!(report.total_tokens() > 0);
        assert_eq!(report.modules.len(), 2);
    }

    #[test]
    fn test_build_report_format_text() {
        let mut report = BuildReport::new();
        report.add_file("core", FileReport::from_source("core/lib.rs", "fn init() {}"));
        let text = report.format_text();
        assert!(text.contains("Token Budget Report"));
        assert!(text.contains("core"));
        assert!(text.contains("init"));
    }

    #[test]
    fn test_build_report_format_json() {
        let mut report = BuildReport::new();
        report.add_file("core", FileReport::from_source("core/lib.rs", "fn init() {}"));
        let json = report.format_json();
        assert!(json.contains("\"total_tokens\""));
        assert!(json.contains("\"core\""));
        assert!(json.contains("\"init\""));
    }

    // ── Budget checking ──

    #[test]
    fn test_budget_no_violations() {
        let budget = TokenBudget::new()
            .with_function_limit(1000)
            .with_file_limit(10000);
        let mut report = BuildReport::new();
        report.add_file("m", FileReport::from_source("a.rs", "fn a() {}"));
        let violations = budget.check(&report);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_budget_function_violation() {
        let budget = TokenBudget::new().with_function_limit(3);
        let mut report = BuildReport::new();
        report.add_file("m", FileReport::from_source("a.rs", "fn big_fn() {\n    let a = 1;\n    let b = 2;\n}"));
        let violations = budget.check(&report);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].kind, ViolationKind::FunctionTooLarge);
    }

    #[test]
    fn test_budget_file_violation() {
        let budget = TokenBudget::new().with_file_limit(3);
        let mut report = BuildReport::new();
        report.add_file("m", FileReport::from_source("a.rs", "fn a() { let x = 1; }"));
        let violations = budget.check(&report);
        assert!(violations.iter().any(|v| v.kind == ViolationKind::FileTooLarge));
    }

    #[test]
    fn test_budget_module_violation() {
        let budget = TokenBudget::new().with_module_limit(5);
        let mut report = BuildReport::new();
        report.add_file("m", FileReport::from_source("a.rs", "fn a() { let x = 1; let y = 2; let z = 3; }"));
        let violations = budget.check(&report);
        assert!(violations.iter().any(|v| v.kind == ViolationKind::ModuleTooLarge));
    }

    // ── Edge cases ──

    #[test]
    fn test_count_empty_fn() {
        let source = "fn empty() {}";
        let count = count_tokens(source);
        assert_eq!(count, 6); // fn, empty, (, ), {, }
        // fn, empty, (, ), {, } = 6
        assert!(count >= 5);
    }

    #[test]
    fn test_extract_with_attributes() {
        let source = "#[test]\nfn test_something() {\n    assert!(true);\n}\n";
        let items = extract_items(source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "test_something");
    }

    #[test]
    fn test_extract_with_redox_attrs() {
        let source = "@t\nfn test_something() {\n    assert!(true);\n}\n";
        let items = extract_items(source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "test_something");
    }

    #[test]
    fn test_extract_type_alias() {
        let source = "type Result<T> = std::result::Result<T, Error>;\n";
        let items = extract_items(source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ItemKind::TypeAlias);
    }

    #[test]
    fn test_savings_zero_tokens() {
        let report = FileReport {
            path: "empty.rs".to_string(),
            total_tokens: 0,
            compact_tokens: Some(0),
            items: Vec::new(),
        };
        assert_eq!(report.savings_percent(), Some(0.0));
    }

    #[test]
    fn test_module_no_compact_tokens() {
        let mut module = ModuleReport::new("m");
        module.add_file(FileReport::from_source("a.rs", "fn a() {}"));
        assert!(module.total_compact_tokens().is_none());
        assert!(module.savings_percent().is_none());
    }

    #[test]
    fn test_build_report_default() {
        let report = BuildReport::default();
        assert_eq!(report.total_tokens(), 0);
        assert_eq!(report.total_files(), 0);
    }
}

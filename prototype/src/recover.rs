//! Three-stage source recovery — the bench-verified pipeline.
//!
//! Originally inlined in `prototype/src/bin/reliability_bench.rs` over
//! Phases 31–44. Lifted to a shared module so the RAP server can offer
//! the same `build/recover` surface the bench exercises against the
//! 100-task corpus and the perturbed-8 menu.
//!
//! Stage order:
//! 1. **Multi-pass pattern heal** — iterate every candidate from
//!    [`crate::heal::heal_one`] in confidence order, stop on first
//!    re-parse success.
//! 2. **Structural brace balance** — walk source, skip string/char/
//!    comment regions, close unbalanced `(`/`[`/`{` at EOF in reverse
//!    order.
//! 3. **Partial-statement completion** — when source ends mid-
//!    expression (`=`, `+`, `,`, `->`, etc.), splice the minimal
//!    placeholder (`()`, `0`, `_`) before re-balancing braces.

use crate::heal;
use crate::hir;
use crate::lexer;
use crate::parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStage {
    /// Source already parsed; no recovery needed.
    AlreadyValid,
    /// Stage 1 — at least one pattern-heal candidate parsed.
    PatternHeal,
    /// Stage 2a — brace-balance at EOF made the source parse.
    StructuralBalance,
    /// Stage 2b — partial-statement completion + brace-balance worked.
    StructuralCompletion,
    /// Stage 2c — deleting the token at (or just before) the parse
    /// error position recovered the parse. Targets word-swap / extra-
    /// token failures that the other structural fallbacks can't fix.
    TrimBadToken,
    /// All stages exhausted; no candidate parsed.
    Failed,
}

impl RecoveryStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AlreadyValid => "already-valid",
            Self::PatternHeal => "pattern-heal",
            Self::StructuralBalance => "structural-balance",
            Self::StructuralCompletion => "structural-completion",
            Self::TrimBadToken => "trim-bad-token",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug)]
pub struct RecoveryResult {
    /// Which stage produced the final source (or `Failed` if none did).
    pub stage: RecoveryStage,
    /// The final source. On `Failed` this is the original input unchanged.
    pub source: String,
    /// How many pattern-heal candidates were tried in stage 1.
    pub candidates_tried: usize,
    /// Did the final source actually parse?
    pub parsed_ok: bool,
}

/// Run the three-stage recovery pipeline against `source`. Returns the
/// best version found (original if nothing worked) and which stage
/// produced it.
///
/// This is the function the RAP `build/recover` method calls, and is
/// equivalent to the recovery sequence in the reliability bench.
pub fn recover(source: &str) -> RecoveryResult {
    if parses(source) {
        return RecoveryResult {
            stage: RecoveryStage::AlreadyValid,
            source: source.to_string(),
            candidates_tried: 0,
            parsed_ok: true,
        };
    }

    // Stage 1: multi-pass pattern heal.
    let (pattern_source, candidates_tried) = try_pattern_heal(source);
    if let Some(healed) = pattern_source {
        return RecoveryResult {
            stage: RecoveryStage::PatternHeal,
            source: healed,
            candidates_tried,
            parsed_ok: true,
        };
    }

    // Stage 2a: brace-balance at EOF.
    if let Some(balanced) = structural_heal(source) {
        if parses(&balanced) {
            return RecoveryResult {
                stage: RecoveryStage::StructuralBalance,
                source: balanced,
                candidates_tried,
                parsed_ok: true,
            };
        }
    }

    // Stage 2b: partial-statement completion + brace-balance.
    if let Some(completed) = structural_completion(source) {
        if parses(&completed) {
            return RecoveryResult {
                stage: RecoveryStage::StructuralCompletion,
                source: completed,
                candidates_tried,
                parsed_ok: true,
            };
        }
    }

    // Stage 2c: trim-bad-token. When the parser pointed at a specific
    // (line, col), try deleting the token there and a few neighbors;
    // re-parse on each variant. Targets word-swap / extra-token shapes
    // that brace-balance and completion can't fix.
    if let Some(trimmed) = trim_bad_token(source) {
        if parses(&trimmed) {
            return RecoveryResult {
                stage: RecoveryStage::TrimBadToken,
                source: trimmed,
                candidates_tried,
                parsed_ok: true,
            };
        }
    }

    RecoveryResult {
        stage: RecoveryStage::Failed,
        source: source.to_string(),
        candidates_tried,
        parsed_ok: false,
    }
}

fn parses(source: &str) -> bool {
    let tokens = lexer::lex(source);
    if tokens.iter().any(|t| t.kind == lexer::TokenKind::Error) {
        return false;
    }
    parser::parse(&tokens).is_ok()
}

/// Stage 1 — gather every candidate from `heal::heal_one` for the parse
/// error, sort by confidence descending, return the first one whose
/// applied text re-parses cleanly. Returns `(Some(healed), tried)` on
/// success or `(None, tried)` on exhaustion.
fn try_pattern_heal(source: &str) -> (Option<String>, usize) {
    let tokens = lexer::lex(source);
    let parse_err = match parser::parse(&tokens) {
        Ok(_) => return (None, 0), // shouldn't happen — caller pre-checked
        Err(e) => e,
    };
    let diag = hir::Diagnostic {
        severity: hir::Severity::Error,
        message: parse_err.message.clone(),
        span: Some(hir::Span {
            line: parse_err.line as u32,
            col: parse_err.col as u32,
        }),
        id: None,
        category: Some(hir::DiagnosticCategory::SyntaxError),
    };
    let healed = heal::heal_one(&diag);
    let mut candidates = healed.fixes;
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let tried = candidates.len();
    for cand in &candidates {
        if let Some(applied) = apply_text_edits(source, &cand.edits) {
            if parses(&applied) {
                return (Some(applied), tried);
            }
        }
    }
    (None, tried)
}

/// Stage 2a — walk `source`, track unbalanced `(`/`[`/`{` on a stack
/// while skipping string, char, and line-comment regions, then append
/// matching closers at EOF in reverse order. Returns `None` if already
/// balanced.
pub fn structural_heal(source: &str) -> Option<String> {
    let mut stack: Vec<u8> = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'"' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if i < bytes.len() {
                i += 1;
            }
            continue;
        }
        if b == b'\'' && i + 2 < bytes.len() {
            let save = i;
            i += 1;
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
            } else {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'\'' {
                i += 1;
                continue;
            }
            i = save + 1;
            continue;
        }
        match b {
            b'(' | b'[' | b'{' => stack.push(b),
            b')' => {
                if stack.last() == Some(&b'(') {
                    stack.pop();
                }
            }
            b']' => {
                if stack.last() == Some(&b'[') {
                    stack.pop();
                }
            }
            b'}' => {
                if stack.last() == Some(&b'{') {
                    stack.pop();
                }
            }
            _ => {}
        }
        i += 1;
    }
    if stack.is_empty() {
        return None;
    }
    let mut suffix = String::with_capacity(stack.len());
    for &open in stack.iter().rev() {
        suffix.push(match open {
            b'(' => ')',
            b'[' => ']',
            b'{' => '}',
            _ => return None,
        });
    }
    let mut out = String::with_capacity(source.len() + suffix.len());
    out.push_str(source);
    out.push_str(&suffix);
    Some(out)
}

/// Stage 2b — when `source` ends with a token that needs a follow-up
/// (`=`, `+`, `->`, `,`, …), splice the smallest valid placeholder then
/// re-balance braces.
///
/// Phase 56 made this **type-aware** for the assignment case: when the
/// trailing `=` belongs to a typed binding (`v x: T =` or `m x: T =`),
/// the placeholder is chosen to match `T` instead of always using `()`.
/// E.g. `v n: i32 =` splices ` 0` (not ` ()`), so the resulting source
/// type-checks instead of failing with a type-mismatch the next phase
/// would have to recover from.
pub fn structural_completion(source: &str) -> Option<String> {
    let trimmed = source.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    let last = trimmed.bytes().last()?;
    let splice: String = match last {
        b'=' => {
            // Type-aware: look at the line containing the trailing `=`
            // for a typed binding declaration. Falls back to `()`.
            type_aware_assign_splice(trimmed).unwrap_or(" ()".to_string())
        }
        b'+' | b'-' | b'*' | b'/' | b'%' => " 0".to_string(),
        b',' => " _".to_string(),
        b'(' | b'[' => String::new(),
        _ => {
            if trimmed.ends_with("->") {
                " _".to_string()
            } else {
                return None;
            }
        }
    };
    let intermediate = format!("{trimmed}{splice}");
    structural_heal(&intermediate).or(Some(intermediate))
}

/// Inspect the last non-blank line of `source` for a `v|m IDENT: TYPE =`
/// pattern; if found, return the type-appropriate placeholder. Type
/// detection is byte-pattern-based (no full parse needed) — covers the
/// common scalar / string / option cases the truncation perturbation
/// produces. Returns `None` for unknown types so the caller falls back
/// to the generic `()` placeholder.
fn type_aware_assign_splice(trimmed: &str) -> Option<String> {
    // The trailing `=` is at the end of `trimmed`. Walk back from
    // there to find the nearest `v ` or `m ` binding keyword,
    // separated from the surrounding source by `{`, `;`, `\n`, or
    // start-of-string. Between that keyword and the `=`, look for
    // a `: TYPE` annotation.
    let bytes = trimmed.as_bytes();
    let eq = trimmed.rfind('=')?;
    // Find latest binding-keyword start before the `=`.
    let mut kw_start: Option<usize> = None;
    let mut i = 0usize;
    while i + 2 <= eq {
        let two = &bytes[i..i + 2];
        if two == b"v " || two == b"m " {
            // Boundary: preceded by `{`, `;`, `\n`, ` `, or start.
            let before = if i == 0 { None } else { Some(bytes[i - 1]) };
            let is_boundary = matches!(
                before,
                None | Some(b' ' | b'{' | b';' | b'\n' | b'\t')
            );
            if is_boundary {
                kw_start = Some(i);
            }
        }
        i += 1;
    }
    let kw_start = kw_start?;
    let segment = &trimmed[kw_start..eq];
    // Need `name : Type =` — find the colon.
    let colon = segment.find(':')?;
    let type_part = segment[colon + 1..].trim();
    // Strip a trailing whitespace if any (already trimmed).
    let placeholder = match type_part {
        "i8" | "i16" | "i32" | "i64" | "isize"
        | "u8" | "u16" | "u32" | "u64" | "usize" => " 0",
        "f32" | "f64" => " 0.0",
        "bool" => " 1b",
        "char" => " '_'",
        "s" | "S" => " \"\"",
        t if t.starts_with('?') => " None",            // ?T → Option<T>
        t if t.starts_with('[') && t.ends_with("]~") => " []",  // Vec<T>
        t if t.starts_with('[') && t.ends_with(']') => " []",   // slice
        _ => return None,
    };
    Some(placeholder.to_string())
}

/// Apply heal text edits right-to-left so earlier offsets stay valid.
pub fn apply_text_edits(source: &str, edits: &[heal::TextEdit]) -> Option<String> {
    if edits.is_empty() {
        return None;
    }
    let mut line_starts = vec![0usize];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(i + 1);
        }
    }
    line_starts.push(source.len());

    let line_col_to_offset = |line: u32, col: u32| -> Option<usize> {
        let l = line.saturating_sub(1) as usize;
        let c = col.saturating_sub(1) as usize;
        let start = *line_starts.get(l)?;
        Some((start + c).min(source.len()))
    };

    let mut ranges: Vec<(usize, usize, String)> = Vec::with_capacity(edits.len());
    for e in edits {
        let start = line_col_to_offset(e.start_line, e.start_col)?;
        let end = line_col_to_offset(e.end_line, e.end_col)?;
        if start > end {
            return None;
        }
        ranges.push((start, end, e.new_text.clone()));
    }
    ranges.sort_by(|a, b| b.0.cmp(&a.0));
    let mut result = source.to_string();
    for (start, end, new_text) in ranges {
        result.replace_range(start..end, &new_text);
    }
    Some(result)
}

/// Stage 2c — try deleting the token at the parse error position, and
/// the preceding token. Returns the **first** variant that parses, or
/// `None` if neither does. Targets word-swap perturbations where one
/// of the swapped tokens is now in a position the parser can't accept.
///
/// Heuristic: parses(`source`), grab the error (line, col), find the
/// word boundaries straddling that offset, build a variant with that
/// word elided, re-parse. If that fails, also try eliding the word
/// just before it.
pub fn trim_bad_token(source: &str) -> Option<String> {
    let tokens = lexer::lex(source);
    let parse_err = parser::parse(&tokens).err()?;

    // Resolve (line, col) to a byte offset in `source`.
    let target = line_col_to_byte(source, parse_err.line, parse_err.col)?;

    // Find the word that contains or immediately follows `target`.
    let bytes = source.as_bytes();
    let (w_start, w_end) = word_bounds(bytes, target)?;

    // Variant 1: delete the word at the error position (plus a
    // following whitespace byte so we don't leave a double space).
    let variant1 = elide_range(source, w_start, w_end);
    if !variant1.is_empty() && parses(&variant1) {
        return Some(variant1);
    }

    // Variant 2: delete the word *before* the error position.
    if w_start > 0 {
        if let Some(prev_end) = bytes[..w_start]
            .iter()
            .rposition(|b| !b.is_ascii_whitespace())
            .map(|i| i + 1)
        {
            if let Some((p_start, _)) = word_bounds(bytes, prev_end.saturating_sub(1)) {
                let variant2 = elide_range(source, p_start, prev_end);
                if !variant2.is_empty() && parses(&variant2) {
                    return Some(variant2);
                }
            }
        }
    }

    None
}

fn line_col_to_byte(source: &str, line: usize, col: usize) -> Option<usize> {
    let mut current_line = 1usize;
    let mut current_col = 1usize;
    for (i, ch) in source.char_indices() {
        if current_line == line && current_col == col {
            return Some(i);
        }
        if ch == '\n' {
            current_line += 1;
            current_col = 1;
        } else {
            current_col += 1;
        }
    }
    // (line, col) may point just past end-of-source.
    if current_line == line && current_col == col {
        Some(source.len())
    } else {
        None
    }
}

/// Find the byte range `[start, end)` of the contiguous word that
/// contains `offset`, where "word" is any maximal run of ASCII
/// alphanumeric or `_` characters. If `offset` is on whitespace,
/// search forward for the next word start. Returns `None` if no word
/// exists at-or-after `offset`.
fn word_bounds(bytes: &[u8], offset: usize) -> Option<(usize, usize)> {
    if offset > bytes.len() {
        return None;
    }
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';

    // If offset is in whitespace, advance to next word.
    let mut start = offset;
    while start < bytes.len() && !is_word(bytes[start]) {
        start += 1;
    }
    if start >= bytes.len() {
        return None;
    }
    // Walk backward to word start (in case offset was mid-word).
    while start > 0 && is_word(bytes[start - 1]) {
        start -= 1;
    }
    // Walk forward to word end.
    let mut end = start;
    while end < bytes.len() && is_word(bytes[end]) {
        end += 1;
    }
    Some((start, end))
}

/// Cut `[start, end)` plus one trailing whitespace byte (to avoid
/// leaving a double-space gap). Returns the resulting string.
fn elide_range(source: &str, start: usize, end: usize) -> String {
    let bytes = source.as_bytes();
    let mut cut_end = end;
    if cut_end < bytes.len() && bytes[cut_end] == b' ' {
        cut_end += 1;
    }
    let mut out = String::with_capacity(source.len());
    out.push_str(&source[..start]);
    out.push_str(&source[cut_end..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn already_valid_short_circuits() {
        let r = recover("+f main() {}");
        assert_eq!(r.stage, RecoveryStage::AlreadyValid);
        assert_eq!(r.candidates_tried, 0);
        assert!(r.parsed_ok);
    }

    #[test]
    fn structural_balance_closes_braces() {
        let src = "+f main() { v x = 1;";
        let r = recover(src);
        assert!(r.parsed_ok, "should recover by closing brace");
        assert!(matches!(
            r.stage,
            RecoveryStage::StructuralBalance | RecoveryStage::PatternHeal
        ));
        assert!(r.source.ends_with('}'));
    }

    #[test]
    fn structural_completion_splices_after_equals() {
        // Trailing `=` should get an `()` splice + brace balance.
        let out = structural_completion("+f main() { v x =").expect("splices");
        assert!(out.contains("= ()"), "got: {out}");
        assert!(out.ends_with('}'), "should balance brace, got: {out}");
    }

    #[test]
    fn structural_completion_splices_after_arrow() {
        // Trailing `->` (return-type slot) should get a `_` placeholder.
        let out = structural_completion("+f f() ->").expect("splices");
        assert!(out.ends_with("-> _"), "got: {out}");
    }

    #[test]
    fn structural_completion_splices_after_comma() {
        let out = structural_completion("call(a,").expect("splices");
        assert!(out.contains(", _"), "got: {out}");
        assert!(out.ends_with(')'), "should close paren, got: {out}");
    }

    #[test]
    fn failed_path_returns_original() {
        // Pure garbage — no stage can save it.
        let src = "@@@!!!###";
        let r = recover(src);
        assert!(!r.parsed_ok);
        assert_eq!(r.stage, RecoveryStage::Failed);
        assert_eq!(r.source, src);
    }

    #[test]
    fn structural_heal_skips_strings() {
        // Brace inside a string literal must not be counted.
        let src = r#"+f main() { v s = "{";"#;
        let r = recover(src);
        // Either pattern-heal or structural-balance can fix this.
        assert!(r.parsed_ok, "got stage {:?}: {}", r.stage, r.source);
    }

    #[test]
    fn structural_heal_returns_none_when_balanced() {
        assert!(structural_heal("+f main() { v x = 1; }").is_none());
    }

    #[test]
    fn structural_completion_returns_none_on_trailing_normal_token() {
        // No trailing operator — no completion to apply.
        assert!(structural_completion("+f main() { }").is_none());
    }

    #[test]
    fn structural_completion_type_aware_int() {
        let out = structural_completion("+f main() { v n: i32 =").expect("splices");
        assert!(out.contains("= 0"), "got: {out}");
        assert!(!out.contains("= ()"), "should not fall back to (); got: {out}");
    }

    #[test]
    fn structural_completion_type_aware_float() {
        let out = structural_completion("+f main() { v x: f64 =").expect("splices");
        assert!(out.contains("= 0.0"), "got: {out}");
    }

    #[test]
    fn structural_completion_type_aware_bool() {
        let out = structural_completion("+f main() { m b: bool =").expect("splices");
        assert!(out.contains("= 1b"), "got: {out}");
    }

    #[test]
    fn structural_completion_type_aware_string() {
        let out = structural_completion("+f main() { v s: s =").expect("splices");
        assert!(out.contains("= \"\""), "got: {out}");
    }

    #[test]
    fn structural_completion_type_aware_option() {
        let out = structural_completion("+f main() { v o: ?i32 =").expect("splices");
        assert!(out.contains("= None"), "got: {out}");
    }

    #[test]
    fn structural_completion_falls_back_when_no_type_annotation() {
        // No `: T` → fall back to `()`.
        let out = structural_completion("+f main() { v x =").expect("splices");
        assert!(out.contains("= ()"), "got: {out}");
    }

    #[test]
    fn structural_completion_falls_back_on_unknown_type() {
        let out = structural_completion("+f main() { v c: SomeStruct =").expect("splices");
        assert!(out.contains("= ()"), "got: {out}");
    }

    #[test]
    fn word_bounds_finds_word_at_offset() {
        let s = b"hello world foo";
        assert_eq!(word_bounds(s, 0), Some((0, 5)));   // "hello"
        assert_eq!(word_bounds(s, 3), Some((0, 5)));   // mid-word
        assert_eq!(word_bounds(s, 5), Some((6, 11)));  // whitespace -> next word
        assert_eq!(word_bounds(s, 6), Some((6, 11)));  // "world"
        assert_eq!(word_bounds(s, 12), Some((12, 15))); // "foo"
    }

    #[test]
    fn word_bounds_returns_none_past_end() {
        assert_eq!(word_bounds(b"abc   ", 5), None);
    }

    #[test]
    fn line_col_byte_offset_basic() {
        let s = "abc\ndef\nghi";
        assert_eq!(line_col_to_byte(s, 1, 1), Some(0));
        assert_eq!(line_col_to_byte(s, 2, 1), Some(4));
        assert_eq!(line_col_to_byte(s, 3, 2), Some(9));
    }

    #[test]
    fn elide_range_removes_word_and_one_space() {
        assert_eq!(elide_range("foo bar baz", 4, 7), "foo baz");
        // No trailing space — just plain cut.
        assert_eq!(elide_range("foobar", 0, 3), "bar");
    }

    #[test]
    fn trim_bad_token_recovers_swapped_words() {
        // Simulates word-swap: `pub fn` -> `fn pub`. Trimming `pub`
        // should leave parseable source.
        let bad = "fn pub main() {}";
        let r = recover(bad);
        // Either trim or one of the earlier stages may rescue this;
        // assert *some* recovery, and that the bad word disappeared.
        assert!(
            r.parsed_ok,
            "expected recovery for {bad:?}, got stage {:?}: {}",
            r.stage, r.source
        );
        assert!(
            !r.source.contains("fn pub"),
            "recovered source still has bad swap: {}",
            r.source
        );
    }
}

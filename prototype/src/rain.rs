//! Digital rain — a Matrix-inspired, dense-UTF-8 representation of MAGE.
//!
//! Each MAGE lexer token is mapped to a single dense codepoint, drawn first
//! from **half-width katakana** (`ｦｧｨ…` — the iconic Matrix "digital rain"
//! glyphs), then full-width katakana, then CJK unified ideographs. One glyph per
//! token, so the **character** stream is maximally compressed (≈ one symbol per
//! source token). The map travels as a `legend`, so the encoding is reversible.
//!
//! **Honest caveat (measured, see `agentic-eval/examples/rain_tokens`):** this
//! compresses *characters/visual footprint*, not *LLM token streams*. BPE
//! tokenizers split rare multi-byte glyphs into multiple tokens, so a katakana/
//! CJK stream usually costs **more** BPE tokens than the ASCII source — the same
//! token-floor lesson (an LLM emits tokens, not bytes/glyphs). The win is
//! aesthetic + byte/character density, not token efficiency.

use crate::lexer::{self, TokenKind};
use std::collections::HashMap;

/// The glyph alphabet — Matrix rain first.
fn glyph_alphabet() -> Vec<char> {
    let mut v = Vec::new();
    // Half-width katakana ｦ–ﾝ (U+FF66–FF9D) — the canonical digital-rain glyphs.
    v.extend((0xFF66u32..=0xFF9D).filter_map(char::from_u32));
    // Full-width katakana ァ–ヺ (U+30A1–U+30FA).
    v.extend((0x30A1u32..=0x30FA).filter_map(char::from_u32));
    // CJK unified ideographs 一–龥 (U+4E00–U+9FA5) — a deep long-tail.
    v.extend((0x4E00u32..=0x9FA5).filter_map(char::from_u32));
    v
}

/// A digital-rain encoding of some MAGE source.
pub struct Rain {
    /// The glyph stream — one codepoint per source token.
    pub stream: String,
    /// Reversal map: `glyph → token text`, in first-seen (assignment) order.
    pub legend: Vec<(char, String)>,
}

impl Rain {
    /// Distinct tokens (= legend size).
    pub fn distinct(&self) -> usize {
        self.legend.len()
    }
    /// Total tokens (= glyph count).
    pub fn tokens(&self) -> usize {
        self.stream.chars().count()
    }
    /// Legend serialized as `glyph\ttext\n` lines (what must ship for reversal).
    pub fn legend_text(&self) -> String {
        self.legend.iter().map(|(g, t)| format!("{g}\t{t}")).collect::<Vec<_>>().join("\n")
    }
}

/// Encode MAGE source into digital rain (one glyph per lexer token).
pub fn encode(src: &str) -> Rain {
    let alphabet = glyph_alphabet();
    let mut map: HashMap<String, char> = HashMap::new();
    let mut legend: Vec<(char, String)> = Vec::new();
    let mut stream = String::new();
    for t in lexer::lex(src) {
        if t.kind == TokenKind::Eof || t.kind == TokenKind::Error {
            continue;
        }
        let key = t.text.clone();
        let g = *map.entry(key.clone()).or_insert_with(|| {
            let idx = legend.len();
            // Wrap if a (pathologically large) file exhausts the alphabet.
            let g = alphabet[idx % alphabet.len()];
            legend.push((g, key.clone()));
            g
        });
        stream.push(g);
    }
    Rain { stream, legend }
}

/// Decode digital rain back to a re-lexable MAGE source (token texts joined
/// by spaces). Round-trips through the lexer to the same token stream as the
/// original (whitespace/layout are not preserved — the *tokens* are).
pub fn decode(rain: &Rain) -> String {
    let map: HashMap<char, &str> = rain.legend.iter().map(|(g, t)| (*g, t.as_str())).collect();
    rain.stream
        .chars()
        .filter_map(|c| map.get(&c).copied())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token_texts(src: &str) -> Vec<String> {
        lexer::lex(src)
            .into_iter()
            .filter(|t| t.kind != TokenKind::Eof && t.kind != TokenKind::Error)
            .map(|t| t.text)
            .collect()
    }

    #[test]
    fn encode_is_one_glyph_per_token() {
        let src = "net M { layer fc: Linear(8, 4); forward { fc } }";
        let r = encode(src);
        assert_eq!(r.tokens(), token_texts(src).len(), "one glyph per token");
        assert!(r.distinct() <= r.tokens());
        // Stream is pure dense glyphs (no ASCII).
        assert!(r.stream.chars().all(|c| c as u32 >= 0x3000), "all glyphs are dense UTF-8");
    }

    #[test]
    fn round_trips_through_the_lexer() {
        let src = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let decoded = decode(&encode(src));
        // The token stream is preserved (layout is not).
        assert_eq!(token_texts(&decoded), token_texts(src), "tokens reversible");
    }

    #[test]
    fn repeated_tokens_share_a_glyph() {
        let r = encode("a a a b a");
        assert_eq!(r.tokens(), 5);
        assert_eq!(r.distinct(), 2, "`a` and `b` → 2 glyphs");
    }
}

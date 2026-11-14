//! # `bpe` - a byte-level BPE tokenizer, with no dependencies
//!
//! Everything the arena and `token-bench` have said about "tokens" so far was a
//! proxy: a lexer count (`lexer::lex`), an AST walk (`token_budget`) or a
//! lexical rule (`token-bench`). None of them is what a language model is
//! charged. This module is the real thing, written from scratch so the crate
//! gains no dependency: a ranked byte-pair vocabulary, the rank-driven merge
//! encoder, a hand-written pre-tokenizer following `cl100k_base`'s split
//! pattern, and a deterministic trainer.
//!
//! ## Vocabulary file
//!
//! The tiktoken text format, one entry per line: `<base64 of the token's bytes>
//! <rank>`. A vocabulary read from a real `cl100k_base.tiktoken` file works
//! here (minus its special tokens, which are not part of the file). Validation
//! on load: no empty token, no duplicate token, no duplicate rank, and all 256
//! single bytes present, which is what makes every input encodable.
//!
//! ## Encoder
//!
//! A pre-token that is itself in the vocabulary is one token. Otherwise the
//! standard rank-based merge runs: start from single bytes and repeatedly merge
//! the adjacent pair whose concatenation has the lowest rank (leftmost on
//! ties) until none is in the vocabulary. tiktoken scans the whole list for the
//! minimum each round, which is quadratic in the pre-token length; this
//! implementation keeps candidate pairs in a heap over a linked list with lazy
//! invalidation, O(n log n), and yields the same tokens.
//!
//! ## Pre-tokenizer: where it differs from `cl100k_base`
//!
//! Hand-written (no regex crate), following the seven alternatives of the
//! cl100k pattern in order. The Unicode classes are approximated with `char`
//! methods from `std`:
//!
//! - `\p{L}` is `char::is_alphabetic` (the Alphabetic property). That is
//!   letters *plus* `Nl` (letter numbers such as Roman numerals) and
//!   `Other_Alphabetic` combining marks (e.g. Devanagari vowel signs), which
//!   `\p{L}` excludes; cl100k would treat those as punctuation.
//! - `\p{N}` is `char::is_numeric` (Nd, Nl, No), which is exactly `\p{N}`. A
//!   character that is both (a Roman numeral) is a letter here because the
//!   letter alternative is tried first, as in the regex.
//! - `\s` is `char::is_whitespace` (White_Space), the same set the regex
//!   engines use.
//! - `(?i:[sdmt]|ll|ve|re)` folds ASCII case only; full Unicode case folding
//!   would also let U+017F (long s) match `s`.
//!
//! For ASCII text, and for all the MAGE and Rust sources this is used on, the
//! split is identical to cl100k's. Because this is a *different vocabulary*
//! when trained here, counts from [`Vocab::mage`] are MAGE-BPE tokens, not
//! cl100k tokens; only a cl100k file loaded with [`Vocab::load`] approximates
//! those, and only up to the class differences above.
//!
//! ## Trainer
//!
//! Classic BPE over pre-token counts, starting from the 256 bytes and adding
//! the most frequent adjacent pair each round. Pair counts are updated
//! incrementally for the words a merge touches and the best pair comes from a
//! lazily-invalidated heap. Ties break on the pair's bytes, lexicographically
//! smallest first, so the same corpus always gives the same vocabulary.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};
use std::fmt;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Everything that can go wrong loading a vocabulary or running the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BpeError {
    /// A character outside the base64 alphabet, or bad padding/length.
    Base64(String),
    /// A line that is not `<base64> <rank>`.
    Line { line: usize, msg: String },
    /// The vocabulary as a whole is invalid (duplicates, missing bytes...).
    Invalid(String),
    /// `decode` met a rank that is not in the vocabulary.
    UnknownRank(u32),
}

impl fmt::Display for BpeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BpeError::Base64(m) => write!(f, "base64: {m}"),
            BpeError::Line { line, msg } => write!(f, "line {line}: {msg}"),
            BpeError::Invalid(m) => write!(f, "invalid vocabulary: {m}"),
            BpeError::UnknownRank(r) => write!(f, "unknown token rank {r}"),
        }
    }
}

impl std::error::Error for BpeError {}

// ---------------------------------------------------------------------------
// Base64 (standard alphabet, padded)
// ---------------------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with `=` padding.
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Strict base64 decode: padded input only, alphabet characters only, and the
/// unused low bits of the last group must be zero (so decode is the exact
/// inverse of [`base64_encode`]).
pub fn base64_decode(s: &str) -> Result<Vec<u8>, BpeError> {
    let b = s.as_bytes();
    if b.len() % 4 != 0 {
        return Err(BpeError::Base64(format!("length {} is not a multiple of 4", b.len())));
    }
    let val = |c: u8| -> Result<u32, BpeError> {
        match c {
            b'A'..=b'Z' => Ok(u32::from(c - b'A')),
            b'a'..=b'z' => Ok(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Ok(u32::from(c - b'0') + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(BpeError::Base64(format!("invalid character {:?}", c as char))),
        }
    };
    let mut out = Vec::with_capacity(b.len() / 4 * 3);
    let groups = b.len() / 4;
    for (gi, g) in b.chunks(4).enumerate() {
        let last = gi + 1 == groups;
        let pad = if last { g.iter().rev().take_while(|&&c| c == b'=').count() } else { 0 };
        if pad > 2 {
            return Err(BpeError::Base64("too much padding".into()));
        }
        let mut n = 0u32;
        for (k, &c) in g.iter().enumerate() {
            let v = if k >= 4 - pad { 0 } else { val(c)? };
            n = (n << 6) | v;
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        let keep = 3 - pad;
        if bytes[keep..].iter().any(|&x| x != 0) {
            return Err(BpeError::Base64("non-zero trailing bits".into()));
        }
        out.extend_from_slice(&bytes[..keep]);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

/// A ranked byte-pair vocabulary: token bytes to rank, and back.
///
/// Lower rank means merged earlier. The 256 single bytes are always present.
#[derive(Debug, Clone)]
pub struct Vocab {
    ranks: HashMap<Vec<u8>, u32>,
    tokens: HashMap<u32, Vec<u8>>,
}

impl Vocab {
    /// Build and validate a vocabulary from `(bytes, rank)` entries.
    pub fn from_entries(entries: Vec<(Vec<u8>, u32)>) -> Result<Vocab, BpeError> {
        let mut ranks = HashMap::with_capacity(entries.len());
        let mut tokens = HashMap::with_capacity(entries.len());
        for (bytes, rank) in entries {
            if bytes.is_empty() {
                return Err(BpeError::Invalid(format!("empty token at rank {rank}")));
            }
            if tokens.insert(rank, bytes.clone()).is_some() {
                return Err(BpeError::Invalid(format!("duplicate rank {rank}")));
            }
            if ranks.insert(bytes.clone(), rank).is_some() {
                return Err(BpeError::Invalid(format!(
                    "duplicate token {} (rank {rank})",
                    base64_encode(&bytes)
                )));
            }
        }
        for b in 0..=255u8 {
            if !ranks.contains_key(&[b][..]) {
                return Err(BpeError::Invalid(format!("single byte 0x{b:02x} is missing")));
            }
        }
        Ok(Vocab { ranks, tokens })
    }

    /// Parse the tiktoken text format. Blank lines are skipped; `\r\n` is fine.
    pub fn parse(text: &str) -> Result<Vocab, BpeError> {
        let mut entries = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let err = |msg: &str| BpeError::Line { line: i + 1, msg: msg.to_string() };
            let mut parts = line.split(' ');
            let (Some(tok), Some(rank), None) = (parts.next(), parts.next(), parts.next()) else {
                return Err(err("expected `<base64> <rank>`"));
            };
            let bytes = base64_decode(tok).map_err(|e| err(&e.to_string()))?;
            let rank: u32 = rank.parse().map_err(|_| err("rank is not a u32"))?;
            entries.push((bytes, rank));
        }
        Vocab::from_entries(entries)
    }

    /// Read a vocabulary file.
    pub fn load(path: &str) -> Result<Vocab, BpeError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| BpeError::Invalid(format!("cannot read {path}: {e}")))?;
        Vocab::parse(&text)
    }

    /// Serialise in the tiktoken text format, sorted by rank, `\n` endings.
    pub fn to_text(&self) -> String {
        let mut v: Vec<(&u32, &Vec<u8>)> = self.tokens.iter().collect();
        v.sort_by_key(|(r, _)| **r);
        let mut s = String::new();
        for (r, b) in v {
            s.push_str(&base64_encode(b));
            s.push(' ');
            s.push_str(&r.to_string());
            s.push('\n');
        }
        s
    }

    /// Write the vocabulary to a file.
    pub fn save(&self, path: &str) -> Result<(), BpeError> {
        std::fs::write(path, self.to_text())
            .map_err(|e| BpeError::Invalid(format!("cannot write {path}: {e}")))
    }

    /// Number of tokens.
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Never true: the 256 bytes are always present.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Rank of a byte sequence, if it is a token.
    pub fn rank(&self, bytes: &[u8]) -> Option<u32> {
        self.ranks.get(bytes).copied()
    }

    /// Bytes of a rank, if it exists.
    pub fn token(&self, rank: u32) -> Option<&[u8]> {
        self.tokens.get(&rank).map(Vec::as_slice)
    }

    /// The vocabulary trained on the repo's MAGE sources and shipped in
    /// `prototype/data/` (see `data/MAGE_BPE.md` for how it is regenerated).
    pub fn mage() -> &'static Vocab {
        static V: OnceLock<Vocab> = OnceLock::new();
        V.get_or_init(|| Vocab::parse(MAGE_VOCAB_TEXT).expect("shipped MAGE vocab is valid"))
    }

    /// Encode text to ranks.
    pub fn encode(&self, text: &str) -> Vec<u32> {
        let mut out = Vec::with_capacity(text.len() / 3 + 1);
        for piece in pretokenize(text) {
            self.encode_piece(piece.as_bytes(), &mut out);
        }
        out
    }

    /// Number of tokens `encode` would produce.
    pub fn count(&self, text: &str) -> usize {
        self.encode(text).len()
    }

    /// Bytes for a sequence of ranks. Unknown ranks are dropped; use
    /// [`Vocab::try_decode`] to be told about them.
    pub fn decode(&self, ids: &[u32]) -> Vec<u8> {
        let mut out = Vec::new();
        for id in ids {
            if let Some(b) = self.tokens.get(id) {
                out.extend_from_slice(b);
            }
        }
        out
    }

    /// Like [`Vocab::decode`] but an unknown rank is an error.
    pub fn try_decode(&self, ids: &[u32]) -> Result<Vec<u8>, BpeError> {
        let mut out = Vec::new();
        for id in ids {
            out.extend_from_slice(self.tokens.get(id).ok_or(BpeError::UnknownRank(*id))?);
        }
        Ok(out)
    }

    fn encode_piece(&self, piece: &[u8], out: &mut Vec<u32>) {
        if let Some(r) = self.rank(piece) {
            out.push(r);
            return;
        }
        let n = piece.len();
        // Doubly linked list of parts; a part is identified by its start and
        // ends where the next one starts (`next`, sentinel `n`).
        let mut next: Vec<usize> = (1..=n).collect();
        let mut prev: Vec<usize> = (0..n).map(|i| i.wrapping_sub(1)).collect();
        let mut alive = vec![true; n];
        // (rank, left start, right start, right end): min-heap, leftmost first
        // among equal ranks. An entry is stale once its parts have changed.
        let mut heap: BinaryHeap<Reverse<(u32, usize, usize, usize)>> = BinaryHeap::new();
        for i in 0..n - 1 {
            if let Some(r) = self.rank(&piece[i..i + 2]) {
                heap.push(Reverse((r, i, i + 1, i + 2)));
            }
        }
        while let Some(Reverse((_, i, j, jend))) = heap.pop() {
            if !(alive[i] && alive[j] && next[i] == j && next[j] == jend) {
                continue;
            }
            next[i] = jend;
            alive[j] = false;
            if jend < n {
                prev[jend] = i;
                let end = next[jend];
                if let Some(r) = self.rank(&piece[i..end]) {
                    heap.push(Reverse((r, i, jend, end)));
                }
            }
            let p = prev[i];
            if p != usize::MAX {
                if let Some(r) = self.rank(&piece[p..jend]) {
                    heap.push(Reverse((r, p, i, jend)));
                }
            }
        }
        let mut i = 0;
        while i < n {
            out.push(self.ranks[&piece[i..next[i]]]);
            i = next[i];
        }
    }
}

/// The shipped vocabulary text (trained by [`train`] on the repo's `.mg` files).
const MAGE_VOCAB_TEXT: &str = include_str!("../data/mage_bpe_4096.tiktoken");

// ---------------------------------------------------------------------------
// Pre-tokenizer
// ---------------------------------------------------------------------------

fn is_l(c: char) -> bool {
    c.is_alphabetic()
}
fn is_n(c: char) -> bool {
    c.is_numeric()
}
fn is_s(c: char) -> bool {
    c.is_whitespace()
}
fn is_nl(c: char) -> bool {
    c == '\r' || c == '\n'
}

/// Split text into pre-tokens following `cl100k_base`'s pattern. The pieces
/// are contiguous and concatenate back to the input exactly.
pub fn pretokenize(text: &str) -> Vec<&str> {
    let chars: Vec<char> = text.chars().collect();
    let mut offs: Vec<usize> = Vec::with_capacity(chars.len() + 1);
    let mut o = 0;
    for c in &chars {
        offs.push(o);
        o += c.len_utf8();
    }
    offs.push(o);
    let n = chars.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let end = split_end(&chars, i);
        debug_assert!(end > i);
        out.push(&text[offs[i]..offs[end]]);
        i = end;
    }
    out
}

/// End (exclusive, in chars) of the pre-token starting at `i`.
fn split_end(c: &[char], i: usize) -> usize {
    let n = c.len();
    let c0 = c[i];

    // 1. '(?i:[sdmt]|ll|ve|re)
    if c0 == '\'' && i + 1 < n {
        let d = c[i + 1].to_ascii_lowercase();
        if matches!(d, 's' | 'd' | 'm' | 't') {
            return i + 2;
        }
        if i + 2 < n {
            let e = c[i + 2].to_ascii_lowercase();
            if matches!((d, e), ('l', 'l') | ('v', 'e') | ('r', 'e')) {
                return i + 3;
            }
        }
    }

    // 2. [^\r\n\p{L}\p{N}]?\p{L}+
    {
        let start = if is_l(c0) {
            Some(i)
        } else if !is_nl(c0) && !is_n(c0) && i + 1 < n && is_l(c[i + 1]) {
            Some(i + 1)
        } else {
            None
        };
        if let Some(mut k) = start {
            while k < n && is_l(c[k]) {
                k += 1;
            }
            return k;
        }
    }

    // 3. \p{N}{1,3}
    if is_n(c0) {
        let mut k = i;
        while k < n && k < i + 3 && is_n(c[k]) {
            k += 1;
        }
        return k;
    }

    // 4.  ?[^\s\p{L}\p{N}]+[\r\n]*
    {
        let j = if c0 == ' ' { i + 1 } else { i };
        let mut k = j;
        while k < n && !is_s(c[k]) && !is_l(c[k]) && !is_n(c[k]) {
            k += 1;
        }
        if k > j {
            while k < n && is_nl(c[k]) {
                k += 1;
            }
            return k;
        }
    }

    // Only whitespace is left (letters, numbers and everything else matched
    // above): alternatives 5-7 all start inside a whitespace run.
    if is_s(c0) {
        let mut e = i;
        while e < n && is_s(c[e]) {
            e += 1;
        }
        // 5. \s*[\r\n]+  -> up to and including the run's last newline
        if let Some(last) = (i..e).rev().find(|&k| is_nl(c[k])) {
            return last + 1;
        }
        // 6. \s+(?!\S)  -> all of it at end of text, else leave one for the word
        if e == n {
            return e;
        }
        if e - i > 1 {
            return e - 1;
        }
        // 7. \s+
        return e;
    }
    i + 1
}

// ---------------------------------------------------------------------------
// Trainer
// ---------------------------------------------------------------------------

/// Heap entry: highest count first, then the lexicographically smallest pair
/// bytes (`Reverse` turns the max-heap's "greatest" into "smallest").
type PairKey = (i64, Reverse<Vec<u8>>, Reverse<Vec<u8>>, u32, u32);

/// Train a byte-level BPE vocabulary on `corpus` (one string per document).
///
/// Starts from the 256 single bytes (ranks 0..=255, in byte order) and adds
/// the most frequent adjacent pair until the vocabulary has `vocab_size`
/// tokens or no pair is left. Ties break on the pair's bytes, smallest first,
/// so the result is a pure function of the input. `vocab_size <= 256` returns
/// the bytes alone.
///
/// If two different pairs concatenate to the same bytes (`a|bc` and `ab|c`),
/// the second is merged into the token that already exists rather than adding
/// a duplicate, and uses no vocabulary slot.
pub fn train(corpus: &[&str], vocab_size: usize) -> Vocab {
    // Pre-token counts, in a sorted map so word indices are deterministic.
    let mut counts: BTreeMap<Vec<u8>, i64> = BTreeMap::new();
    for doc in corpus {
        for piece in pretokenize(doc) {
            *counts.entry(piece.as_bytes().to_vec()).or_insert(0) += 1;
        }
    }
    let mut words: Vec<(Vec<u32>, i64)> = counts
        .into_iter()
        .map(|(w, c)| (w.iter().map(|&b| u32::from(b)).collect(), c))
        .collect();

    let mut tokens: Vec<Vec<u8>> = (0..=255u8).map(|b| vec![b]).collect();
    let mut index: HashMap<Vec<u8>, u32> =
        tokens.iter().enumerate().map(|(i, t)| (t.clone(), i as u32)).collect();

    let mut pair_count: HashMap<(u32, u32), i64> = HashMap::new();
    let mut pair_words: HashMap<(u32, u32), HashSet<usize>> = HashMap::new();
    for (wi, (syms, c)) in words.iter().enumerate() {
        for p in syms.windows(2) {
            let key = (p[0], p[1]);
            *pair_count.entry(key).or_insert(0) += *c;
            pair_words.entry(key).or_default().insert(wi);
        }
    }

    let entry = |tokens: &[Vec<u8>], key: (u32, u32), count: i64| -> PairKey {
        (
            count,
            Reverse(tokens[key.0 as usize].clone()),
            Reverse(tokens[key.1 as usize].clone()),
            key.0,
            key.1,
        )
    };
    let mut heap: BinaryHeap<PairKey> = pair_count.iter().map(|(&k, &c)| entry(&tokens, k, c)).collect();

    while tokens.len() < vocab_size {
        let Some((count, _, _, a, b)) = heap.pop() else { break };
        let key = (a, b);
        if count <= 0 || pair_count.get(&key) != Some(&count) {
            continue; // stale entry
        }
        let mut merged = tokens[a as usize].clone();
        merged.extend_from_slice(&tokens[b as usize]);
        let new_id = match index.get(&merged) {
            Some(&id) => id,
            None => {
                let id = tokens.len() as u32;
                tokens.push(merged.clone());
                index.insert(merged, id);
                id
            }
        };

        let affected: Vec<usize> = pair_words.get(&key).map(|s| s.iter().copied().collect()).unwrap_or_default();
        let mut touched: HashSet<(u32, u32)> = HashSet::new();
        for wi in affected {
            let (syms, c) = &words[wi];
            if !syms.windows(2).any(|p| (p[0], p[1]) == key) {
                continue;
            }
            for p in syms.windows(2) {
                let k = (p[0], p[1]);
                *pair_count.entry(k).or_insert(0) -= *c;
                touched.insert(k);
            }
            let mut out: Vec<u32> = Vec::with_capacity(syms.len());
            let mut i = 0;
            while i < syms.len() {
                if i + 1 < syms.len() && syms[i] == a && syms[i + 1] == b {
                    out.push(new_id);
                    i += 2;
                } else {
                    out.push(syms[i]);
                    i += 1;
                }
            }
            for p in out.windows(2) {
                let k = (p[0], p[1]);
                *pair_count.entry(k).or_insert(0) += *c;
                pair_words.entry(k).or_default().insert(wi);
                touched.insert(k);
            }
            words[wi].0 = out;
        }
        let mut touched: Vec<(u32, u32)> = touched.into_iter().collect();
        touched.sort_unstable();
        for k in touched {
            match pair_count.get(&k) {
                Some(&c) if c > 0 => heap.push(entry(&tokens, k, c)),
                Some(_) => {
                    pair_count.remove(&k);
                }
                None => {}
            }
        }
    }

    let entries = tokens.into_iter().enumerate().map(|(i, t)| (t, i as u32)).collect();
    Vocab::from_entries(entries).expect("trainer output is a valid vocabulary")
}

// ---------------------------------------------------------------------------
// CLI (`mage-parse --bpe-count` / `--bpe-train`)
// ---------------------------------------------------------------------------

/// SHA-256 (hex) of a training corpus: each document as its length (u64,
/// little-endian) then its bytes. `--bpe-train` prints it, and
/// `data/mage_bpe_4096.corpus.sha256` records it, so a test can tell whether
/// the corpus on disk is still the one the shipped vocabulary was trained on.
pub fn corpus_digest(docs: &[&str]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for d in docs {
        h.update((d.len() as u64).to_le_bytes());
        h.update(d.as_bytes());
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// One-line usage for `--bpe-count`.
pub const COUNT_USAGE: &str = "Usage: mage-parse --bpe-count [--vocab FILE] [FILES|-]";
/// One-line usage for `--bpe-train`.
pub const TRAIN_USAGE: &str = "Usage: mage-parse --bpe-train --vocab-size N --out FILE FILES...";

fn read_text(path: &str) -> Result<String, String> {
    if path == "-" {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)
            .map_err(|e| format!("Error reading standard input: {e}"))?;
        Ok(s)
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("Error reading {path}: {e}"))
    }
}

/// Run a BPE mode. `mode` is `--bpe-count` or `--bpe-train`; `args` are the
/// arguments after it. Returns the text to print, or the error message.
///
/// Flags are parsed here, not in `main.rs`, so the binary's modifier-flag
/// filter and the published flag catalog stay about modes.
pub fn run_cli(mode: &str, args: &[String]) -> Result<String, String> {
    let mut vocab_path: Option<String> = None;
    let mut out_path: Option<String> = None;
    let mut size: Option<usize> = None;
    let mut files: Vec<&str> = Vec::new();
    let usage = if mode == "--bpe-train" { TRAIN_USAGE } else { COUNT_USAGE };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |name: &str| -> Result<String, String> {
            it.next().cloned().ok_or_else(|| format!("{name} needs a value\n{usage}"))
        };
        match a.as_str() {
            "--vocab" => vocab_path = Some(value("--vocab")?),
            "--out" if mode == "--bpe-train" => out_path = Some(value("--out")?),
            "--vocab-size" if mode == "--bpe-train" => {
                let v = value("--vocab-size")?;
                size = Some(v.parse().map_err(|_| format!("--vocab-size needs a whole number, got {v:?}"))?);
            }
            "-" => files.push("-"),
            f if f.starts_with('-') => return Err(format!("unknown option {f}\n{usage}")),
            f => files.push(f),
        }
    }
    match mode {
        "--bpe-count" => {
            let vocab_owned;
            let vocab = match &vocab_path {
                Some(p) => {
                    vocab_owned = Vocab::load(p).map_err(|e| format!("{p}: {e}"))?;
                    &vocab_owned
                }
                None => Vocab::mage(),
            };
            if files.is_empty() {
                files.push("-");
            }
            let mut total = 0usize;
            for f in files {
                total += vocab.count(&read_text(f)?);
            }
            Ok(total.to_string())
        }
        "--bpe-train" => {
            let (Some(size), Some(out)) = (size, out_path) else {
                return Err(TRAIN_USAGE.to_string());
            };
            if files.is_empty() {
                return Err(TRAIN_USAGE.to_string());
            }
            // CRLF to LF, so the vocabulary does not depend on how git checked
            // the corpus out.
            let docs: Vec<String> = files
                .iter()
                .map(|f| read_text(f).map(|t| t.replace("\r\n", "\n")))
                .collect::<Result<_, _>>()?;
            let refs: Vec<&str> = docs.iter().map(String::as_str).collect();
            let vocab = train(&refs, size);
            vocab.save(&out).map_err(|e| e.to_string())?;
            let bytes: usize = docs.iter().map(String::len).sum();
            let tokens: usize = docs.iter().map(|d| vocab.count(d)).sum();
            Ok(format!(
                "trained {} tokens on {} files ({} bytes, corpus sha256 {}): {} tokens, {:.4} tokens/byte -> {}",
                vocab.len(),
                docs.len(),
                bytes,
                corpus_digest(&refs),
                tokens,
                tokens as f64 / bytes.max(1) as f64,
                out
            ))
        }
        other => Err(format!("unknown mode: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> Vocab {
        // bytes + merges: "he"(256) "ll"(257) "hell"(258) "hello"(259) "lo"(260)
        let mut e: Vec<(Vec<u8>, u32)> = (0..=255u8).map(|b| (vec![b], u32::from(b))).collect();
        for (i, t) in ["he", "ll", "hell", "hello", "lo"].iter().enumerate() {
            e.push((t.as_bytes().to_vec(), 256 + i as u32));
        }
        Vocab::from_entries(e).unwrap()
    }

    #[test]
    fn base64_known_vectors_and_round_trip() {
        for (raw, enc) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(raw.as_bytes()), enc);
            assert_eq!(base64_decode(enc).unwrap(), raw.as_bytes());
        }
        let all: Vec<u8> = (0..=255u8).collect();
        for n in 0..all.len() {
            let enc = base64_encode(&all[..n]);
            assert_eq!(base64_decode(&enc).unwrap(), &all[..n]);
        }
    }

    #[test]
    fn base64_rejects_malformed_input() {
        for bad in ["Zg=", "Zg", "Z!==", "====", "Zm9v=", "Zh==", "Zm9=", "Zg==Zg=="] {
            assert!(base64_decode(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn vocab_text_round_trips() {
        let v = tiny();
        let text = v.to_text();
        let w = Vocab::parse(&text).unwrap();
        assert_eq!(w.len(), v.len());
        assert_eq!(w.to_text(), text);
        assert_eq!(w.rank(b"hello"), Some(259));
        assert_eq!(w.token(257), Some(&b"ll"[..]));
        // CRLF files load too.
        assert_eq!(Vocab::parse(&text.replace('\n', "\r\n")).unwrap().to_text(), text);
        let path = std::env::temp_dir().join("mage_bpe_roundtrip_test.tiktoken");
        let p = path.to_str().unwrap();
        v.save(p).unwrap();
        assert_eq!(Vocab::load(p).unwrap().to_text(), text);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn malformed_vocabs_are_errors_not_panics() {
        let good = tiny().to_text();
        let cases: Vec<(&str, String)> = vec![
            ("empty file", String::new()),
            ("no rank", "AA==\n".into()),
            ("extra field", format!("{good}aGU= 300 x\n")),
            ("bad base64", format!("{good}a*U= 300\n")),
            ("bad rank", format!("{good}aGU= -1\n")),
            ("rank overflow", format!("{good}aGU= 99999999999\n")),
            ("duplicate rank", format!("{good}aGk= 256\n")),
            ("duplicate token", format!("{good}aGU= 300\n")),
            ("empty token", format!("{good}= 300\n")),
            ("missing byte", good.lines().skip(1).collect::<Vec<_>>().join("\n")),
            ("double space", format!("{good}aGk=  300\n")),
        ];
        for (name, text) in cases {
            assert!(Vocab::parse(&text).is_err(), "{name} should be rejected");
        }
        assert!(Vocab::load("definitely/not/a/file.tiktoken").is_err());
        // The error carries the line number.
        let e = Vocab::parse("AA== x\n").unwrap_err();
        assert!(matches!(e, BpeError::Line { line: 1, .. }), "{e:?}");
    }

    #[test]
    fn merges_happen_in_rank_order() {
        let v = tiny();
        // "hello": whole word is a token.
        assert_eq!(v.encode("hello"), vec![259]);
        // "hell"+"o" via rank: he(256) < ll(257) < hell(258).
        assert_eq!(v.encode("hell"), vec![258]);
        // "helo": only "he" merges ("lo" 260 is higher rank but "el" is not a
        // token, so he|lo; he first then lo, no further pair).
        assert_eq!(v.encode("helo"), vec![256, 260]);
        // "hellol": he,ll first (lowest ranks) -> hell, then "ol"? no; "lol"
        // leaves hell|o|l -> hello is not formed because "hello" needs o
        // adjacent to hell: it is.  hell+o -> hello(259) then l.
        assert_eq!(v.encode("hellol"), vec![259, u32::from(b'l')]);
        // A lower-ranked pair wins over a higher one that overlaps it:
        // "ello": e|l|l|o -> ll(257) first, then no "el"/"lo" with "ll".
        assert_eq!(v.encode("ello"), vec![u32::from(b'e'), 257, u32::from(b'o')]);
        // "lo" (260) loses to "ll" (257) in "llo".
        assert_eq!(v.encode("llo"), vec![257, u32::from(b'o')]);
    }

    #[test]
    fn long_piece_is_not_quadratic() {
        // 200k identical bytes in one pre-token would be ~4e10 steps with the
        // scan-the-list algorithm; the heap version is instant.
        let v = tiny();
        let s = "l".repeat(200_000);
        let ids = v.encode(&s);
        assert_eq!(v.decode(&ids), s.as_bytes());
        assert_eq!(ids.len(), 100_000); // "ll" pairs
    }

    #[test]
    fn encode_decode_is_identity() {
        let texts = [
            "",
            "hello world",
            "Hello world's 1234567",
            "  leading and trailing  \n\n\t tabs\r\n",
            "caf\u{e9} na\u{ef}ve \u{4e2d}\u{6587} \u{43f}\u{440}\u{438}\u{432}\u{435}\u{442}",
            "emoji \u{1F600}\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} \u{2764}\u{FE0F} done",
            "line one\r\nline two\r\n\r\n",
            "x\u{0}y\u{7f}z \u{a0}nbsp",
            "fn main() { println!(\"hi\"); } // c'est la vie",
        ];
        let trained = train(&texts, 400);
        for v in [&tiny(), &trained, Vocab::mage()] {
            for t in texts {
                let ids = v.encode(t);
                assert_eq!(v.decode(&ids), t.as_bytes(), "text {t:?}");
                assert_eq!(v.try_decode(&ids).unwrap(), t.as_bytes());
                assert_eq!(v.count(t), ids.len());
            }
        }
        assert!(tiny().try_decode(&[9999]).is_err());
        assert_eq!(tiny().decode(&[9999, 259]), b"hello");
    }

    // ---- pre-tokenizer, one test per alternative -------------------------

    fn pt(s: &str) -> Vec<&str> {
        let p = pretokenize(s);
        assert_eq!(p.concat(), s, "pieces must concatenate back to the input");
        p
    }

    #[test]
    fn pretok_contractions() {
        assert_eq!(pt("it's"), ["it", "'s"]);
        assert_eq!(pt("we'LL they've I'm you're he'd can't"), [
            "we", "'LL", " they", "'ve", " I", "'m", " you", "'re", " he", "'d", " can", "'t"
        ]);
        // No word boundary needed after the suffix, as in the regex.
        assert_eq!(pt("'sx"), ["'s", "x"]);
        // A bare apostrophe falls through to the letter rule.
        assert_eq!(pt("'x"), ["'x"]);
    }

    #[test]
    fn pretok_words_with_one_optional_prefix() {
        assert_eq!(pt("Hello world"), ["Hello", " world"]);
        assert_eq!(pt("(foo"), ["(foo"]);
        assert_eq!(pt("a_b"), ["a", "_b"]);
        assert_eq!(pt("\tx"), ["\tx"]);
        // CR/LF never prefix a word.
        assert_eq!(pt("\nfoo"), ["\n", "foo"]);
        // A punctuation run is taken whole by the punctuation rule instead.
        assert_eq!(pt("--foo"), ["--", "foo"]);
        assert_eq!(pt("caf\u{e9} \u{4e2d}\u{6587}"), ["caf\u{e9}", " \u{4e2d}\u{6587}"]);
    }

    #[test]
    fn pretok_numbers_in_groups_of_three() {
        assert_eq!(pt("1234567"), ["123", "456", "7"]);
        assert_eq!(pt("a1b22"), ["a", "1", "b", "22"]);
        // A space before digits is not absorbed (the word rule needs a letter).
        assert_eq!(pt("x 12"), ["x", " ", "12"]);
    }

    #[test]
    fn pretok_hello_worlds_1234567() {
        assert_eq!(pt("Hello world's 1234567"), ["Hello", " world", "'s", " ", "123", "456", "7"]);
    }

    #[test]
    fn pretok_punctuation_runs() {
        assert_eq!(pt("!!!"), ["!!!"]);
        assert_eq!(pt("a !!! b"), ["a", " !!!", " b"]);
        assert_eq!(pt("foo();"), ["foo", "();"]);
        // Trailing newlines join the punctuation run.
        assert_eq!(pt("};\n\nx"), ["};\n\n", "x"]);
        assert_eq!(pt("=>\r\nx"), ["=>\r\n", "x"]);
        // Emoji are punctuation to the regex too.
        assert_eq!(pt("a \u{1F600}\u{1F600}"), ["a", " \u{1F600}\u{1F600}"]);
    }

    #[test]
    fn pretok_newlines() {
        assert_eq!(pt("a\n\nb"), ["a", "\n\n", "b"]);
        // `\s*[\r\n]+` takes blanks up to the last newline.
        assert_eq!(pt("a  \n  b"), ["a", "  \n", " ", " b"]);
        assert_eq!(pt("\n\n  x"), ["\n\n", " ", " x"]);
        assert_eq!(pt("x\r\n\r\ny"), ["x", "\r\n\r\n", "y"]);
    }

    #[test]
    fn pretok_whitespace_runs() {
        // Leading space joins its word.
        assert_eq!(pt(" word"), [" word"]);
        // In a run of spaces before a word, the last space joins the word.
        assert_eq!(pt("a    b"), ["a", "   ", " b"]);
        assert_eq!(pt("    x"), ["   ", " x"]);
        // At end of text the whole run is one token.
        assert_eq!(pt("a   "), ["a", "   "]);
        assert_eq!(pt("   "), ["   "]);
        // A lone space before a number stays alone.
        assert_eq!(pt("a 1"), ["a", " ", "1"]);
        assert_eq!(pt(""), Vec::<&str>::new());
        // Non-ASCII whitespace is whitespace.
        assert_eq!(pt("a\u{a0}\u{a0}b"), ["a", "\u{a0}", "\u{a0}b"]);
    }

    // ---- trainer ----------------------------------------------------------

    const CORPUS: [&str; 3] = [
        "fn add(a: i32, b: i32) -> i32 { a + b }\nfn sub(a: i32, b: i32) -> i32 { a - b }\n",
        "let total = add(1, 2);\nlet diff = sub(total, 1);\nlet total2 = add(total, diff);\n",
        "fn main() {\n    let x = add(1, 2);\n    let y = sub(x, 1);\n    println!(\"{}\", add(x, y));\n}\n",
    ];

    #[test]
    fn training_is_deterministic_and_compresses() {
        let a = train(&CORPUS, 300);
        let b = train(&CORPUS, 300);
        assert_eq!(a.to_text(), b.to_text());
        assert_eq!(a.len(), 300);
        // Base bytes keep ranks 0..=255; merges come after, in order.
        for byte in 0..=255u8 {
            assert_eq!(a.rank(&[byte]), Some(u32::from(byte)));
        }
        let bytes: usize = CORPUS.iter().map(|d| d.len()).sum();
        let tokens: usize = CORPUS.iter().map(|d| a.count(d)).sum();
        assert!(tokens < bytes, "{tokens} tokens for {bytes} bytes");
        // Doc order does not change the result (counts are sums).
        let rev = [CORPUS[2], CORPUS[1], CORPUS[0]];
        assert_eq!(train(&rev, 300).to_text(), a.to_text());
        // Small sizes give the bytes alone; an exhausted corpus stops early.
        assert_eq!(train(&CORPUS, 100).len(), 256);
        assert!(train(&["ab"], 1000).len() < 1000);
    }

    #[test]
    fn training_breaks_ties_on_smallest_pair_bytes() {
        // "ab" and "cd" are both seen once; "ab" < "cd" so it is merged first.
        let v = train(&["ab
cd"], 257);
        assert_eq!(v.token(256), Some(&b"ab"[..]));
        // Most frequent first, whatever the bytes.
        let v = train(&["zz zz zz ab"], 257);
        assert_eq!(v.token(256), Some(&b"zz"[..]));
    }

    #[test]
    fn training_never_produces_duplicate_tokens() {
        // Pairs a|bc and ab|c concatenate alike; the vocab must stay valid.
        let docs = ["abc abc abcabc ab c a bc bc ab"; 5];
        for size in [257, 260, 270, 300] {
            let v = train(&docs, size);
            let again = Vocab::parse(&v.to_text()).unwrap();
            assert_eq!(again.len(), v.len());
        }
    }

    // ---- shipped vocab -----------------------------------------------------

    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
    }

    #[test]
    fn shipped_vocab_is_valid_and_compresses_mage() {
        let v = Vocab::mage();
        assert_eq!(v.len(), 4096);
        let src = "pub fn add(a: i64, b: i64) -> i64 { a + b }\n";
        assert!(v.count(src) < src.len());
    }

    /// The vocabulary is reproducible from the corpus it was trained on. Any
    /// `.mg` edit in the repo changes that corpus, and that must not turn this
    /// test red (the vocabulary is still valid, just not retrained), so the
    /// strict retrain-and-compare runs only while the listed files still hash
    /// to the recorded digest; otherwise it is skipped with a note.
    #[test]
    fn shipped_vocab_is_reproduced_by_retraining() {
        let data = repo_root().join("prototype/data");
        let listing = std::fs::read_to_string(data.join("mage_bpe_4096.corpus.txt")).unwrap();
        let recorded = std::fs::read_to_string(data.join("mage_bpe_4096.corpus.sha256")).unwrap();
        let docs: Vec<String> = listing
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|rel| {
                std::fs::read_to_string(repo_root().join(rel))
                    .unwrap_or_else(|e| panic!("corpus file {rel}: {e}"))
                    .replace("\r\n", "\n")
            })
            .collect();
        assert!(docs.len() > 100, "corpus list looks truncated: {} files", docs.len());
        let refs: Vec<&str> = docs.iter().map(String::as_str).collect();
        if corpus_digest(&refs) != recorded.trim() {
            eprintln!("corpus changed since the shipped vocab was trained; skipping retrain check");
            return;
        }
        assert!(
            train(&refs, 4096).to_text() == Vocab::mage().to_text(),
            "data/mage_bpe_4096.tiktoken is not what the trainer produces from its corpus"
        );
    }

    // ---- CLI ---------------------------------------------------------------

    fn s(a: &[&str]) -> Vec<String> {
        a.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn cli_count_and_train_round_trip() {
        let dir = std::env::temp_dir().join("mage_bpe_cli_test");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("a.txt");
        std::fs::write(&src, CORPUS[0]).unwrap();
        let vocab = dir.join("v.tiktoken");
        let (src, vocab) = (src.to_str().unwrap(), vocab.to_str().unwrap());

        let msg = run_cli("--bpe-train", &s(&["--vocab-size", "300", "--out", vocab, src])).unwrap();
        assert!(msg.contains("tokens/byte"), "{msg}");
        let n: usize = run_cli("--bpe-count", &s(&["--vocab", vocab, src])).unwrap().parse().unwrap();
        assert_eq!(n, Vocab::load(vocab).unwrap().count(CORPUS[0]));
        assert!(n < CORPUS[0].len());
        // Several files sum; the default vocab is the shipped one.
        let twice: usize = run_cli("--bpe-count", &s(&[src, src])).unwrap().parse().unwrap();
        assert_eq!(twice, 2 * Vocab::mage().count(CORPUS[0]));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn cli_errors_are_messages() {
        assert!(run_cli("--bpe-train", &s(&[])).is_err());
        assert!(run_cli("--bpe-train", &s(&["--vocab-size", "x", "--out", "o", "f"])).is_err());
        assert!(run_cli("--bpe-train", &s(&["--vocab-size", "300", "f"])).is_err());
        assert!(run_cli("--bpe-count", &s(&["--bogus"])).is_err());
        assert!(run_cli("--bpe-count", &s(&["--vocab"])).is_err());
        assert!(run_cli("--bpe-count", &s(&["--vocab", "nope.tiktoken", "x"])).is_err());
        assert!(run_cli("--bpe-count", &s(&["no/such/file.mg"])).is_err());
        assert!(run_cli("--bpe-other", &s(&[])).is_err());
    }
}

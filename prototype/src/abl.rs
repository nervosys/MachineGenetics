//! Agentic Binary Language container codec — pure encode/decode for the `application/abl`
//! payload that wraps one-or-more Agentic Binary Language `Expr` blocks with names.
//!
//! Container layout (little-endian):
//! ```text
//!   magic    "Agentic Binary Language" (4 bytes)
//!   version  u16 = 3   (v3: per-item exprs are REPEAT-folded)
//!   count    u32           — number of items
//!   for each item:
//!     name_len u32
//!     name     UTF-8 bytes
//!     expr_len u32
//!     expr     codec::Encoder::encode_expr_only output
//! ```
//!
//! Used by both the CLI (`--target=abl-bytes`, `--from=abl-bytes`,
//! `--run=abl-bytes`) and the RAP server (`abl/encode`, `abl/decode`,
//! `abl/run`). Keeping the format in one place prevents drift.

use crate::ast;
use crate::abl_bridge;
use rmi::lang::Expr;

pub const ABL_MAGIC: &[u8; 4] = b"ABL1";
// v3: per-item exprs are REPEAT-folded (a `stack N { block }` ships as the block
// once + a count, so the artifact is O(1) in depth). Decode expands them back to
// the flat `Seq`, so the format change is invisible above this codec.
pub const ABL_VERSION: u16 = 3;

/// One decoded item from a Agentic Binary Language container.
#[derive(Debug)]
pub struct AblItem {
    pub name: String,
    pub expr: Expr,
    pub expr_bytes_len: usize,
}

/// Lower a MAGE module and encode every Agentic Binary Language-routed item into a single
/// Agentic Binary Language blob. Returns the blob plus per-item `(name, expr_bytes_len, content_hash)`
/// for summary printing; callers can ignore the summary tuple if not needed.
pub fn encode_module(module: &ast::Module) -> (Vec<u8>, Vec<(String, usize, u64)>) {
    let lowered = abl_bridge::lower_module(module);
    let mut blob: Vec<u8> = Vec::new();
    blob.extend_from_slice(ABL_MAGIC);
    blob.extend_from_slice(&ABL_VERSION.to_le_bytes());
    blob.extend_from_slice(&(lowered.items.len() as u32).to_le_bytes());
    let mut summary = Vec::with_capacity(lowered.items.len());
    for (name, expr) in &lowered.items {
        let name_bytes = name.as_bytes();
        // Fold contiguous repeats (e.g. `stack N { … }`) so the shipped expr is
        // O(1) in depth; the content hash below stays on the flat `expr`.
        let folded = abl_bridge::fold_repeats(expr);
        let expr_bytes = rmi::lang::codec::Encoder::encode_expr_only(&folded);
        blob.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        blob.extend_from_slice(name_bytes);
        blob.extend_from_slice(&(expr_bytes.len() as u32).to_le_bytes());
        blob.extend_from_slice(&expr_bytes);
        summary.push((name.clone(), expr_bytes.len(), expr.content_hash()));
    }
    // Symbol-table section (v2): every interned name, in id order. This makes a
    // symbolic (`kb`) artifact fully self-describing — predicate/rule NAMES are
    // recoverable on decode, not just arities. Deterministic (id order is fixed).
    let syms = &lowered.symbols;
    blob.extend_from_slice(&(syms.len() as u32).to_le_bytes());
    for i in 0..syms.len() {
        let name = syms.resolve(rmi::lang::Sym(i as u32));
        blob.extend_from_slice(&(name.len() as u32).to_le_bytes());
        blob.extend_from_slice(name.as_bytes());
    }
    (blob, summary)
}

fn take<'a>(buf: &'a [u8], pos: &mut usize, n: usize, what: &str) -> Result<&'a [u8], String> {
    if *pos + n > buf.len() {
        return Err(format!("{what}: unexpected EOF at offset {}", *pos));
    }
    let s = &buf[*pos..*pos + n];
    *pos += n;
    Ok(s)
}

fn read_u32(buf: &[u8], pos: &mut usize, what: &str) -> Result<usize, String> {
    Ok(u32::from_le_bytes(
        take(buf, pos, 4, what)?.try_into().map_err(|_| format!("{what} slice"))?,
    ) as usize)
}

/// Decode the header + items, returning them plus the offset just past the last
/// item (where the symbol-table section begins).
fn decode_items(blob: &[u8]) -> Result<(Vec<AblItem>, usize), String> {
    let mut pos = 0usize;
    let magic = take(blob, &mut pos, 4, "magic")?;
    if magic != ABL_MAGIC {
        return Err(format!("bad magic {magic:?} (expected Agentic Binary Language)"));
    }
    let ver = u16::from_le_bytes(
        take(blob, &mut pos, 2, "version")?
            .try_into()
            .map_err(|_| "version slice".to_string())?,
    );
    if ver != ABL_VERSION {
        return Err(format!("unsupported Agentic Binary Language version {ver}"));
    }
    let count = read_u32(blob, &mut pos, "count")?;
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let nl = read_u32(blob, &mut pos, "name_len")?;
        let name = std::str::from_utf8(take(blob, &mut pos, nl, "name")?)
            .map_err(|e| format!("item {i} name utf8: {e}"))?
            .to_string();
        let el = read_u32(blob, &mut pos, "expr_len")?;
        let expr_bytes = take(blob, &mut pos, el, "expr")?;
        let decoded = rmi::lang::codec::Decoder::decode_expr_only(expr_bytes)
            .map_err(|e| format!("item {i} ({name}): decode error: {e:?}"))?;
        // Expand REPEAT folds back to the flat `Seq` every consumer expects;
        // `expr_bytes_len` stays the on-wire (folded) size.
        let expr = abl_bridge::expand_repeats(&decoded);
        items.push(AblItem { name, expr, expr_bytes_len: el });
    }
    Ok((items, pos))
}

/// Decode a Agentic Binary Language container into its items. Returns a structured error
/// string rather than panicking, so the RAP layer can surface it as JSON.
pub fn decode_container(blob: &[u8]) -> Result<Vec<AblItem>, String> {
    Ok(decode_items(blob)?.0)
}

/// Decode the container's symbol table (names in id order). Empty if the
/// container has no symbol section. Lets a decoder resolve the `Sym` ids inside
/// decoded exprs back to names (e.g. kb predicate names) with NO execution.
pub fn decode_symbols(blob: &[u8]) -> Result<Vec<String>, String> {
    let (_items, mut pos) = decode_items(blob)?;
    if pos >= blob.len() {
        return Ok(Vec::new());
    }
    let count = read_u32(blob, &mut pos, "sym_count")?;
    let mut names = Vec::with_capacity(count);
    for i in 0..count {
        let nl = read_u32(blob, &mut pos, "sym_name_len")?;
        let name = std::str::from_utf8(take(blob, &mut pos, nl, "sym_name")?)
            .map_err(|e| format!("symbol {i} utf8: {e}"))?
            .to_string();
        names.push(name);
    }
    Ok(names)
}

/// Lowercase hex encoder — pure, no deps. Used by the RAP layer to ship
/// Agentic Binary Language bytes through a JSON channel.
pub fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Inverse of `to_hex`. Tolerates uppercase too; returns the offending
/// position on failure so RAP errors are easy to debug.
pub fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err(format!("hex length {} is not even", s.len()));
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    for i in (0..bytes.len()).step_by(2) {
        let hi = hex_nibble(bytes[i]).ok_or_else(|| format!("non-hex char at {i}"))?;
        let lo = hex_nibble(bytes[i + 1]).ok_or_else(|| format!("non-hex char at {}", i + 1))?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;

    const SAMPLE: &str = r#"
        net tiny {
            layer fc: Linear(8, 4);
            forward { fc }
        }
    "#;

    #[test]
    fn round_trip_encode_decode() {
        let tokens = lexer::lex(SAMPLE);
        let module = parser::parse(&tokens).expect("sample parses");
        let (blob, summary) = encode_module(&module);
        assert!(blob.starts_with(ABL_MAGIC));
        assert_eq!(summary.len(), 1, "tiny net should encode as one item");
        let items = decode_container(&blob).expect("round-trip decode");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, summary[0].0);
        assert_eq!(items[0].expr_bytes_len, summary[0].1);
    }

    const STACK_SRC: &str = r#"
        net DeepT {
            stack 12 {
                layer attn: MultiHeadAttention(256, 8);
                layer norm1: LayerNorm;
                layer ff1: Linear(256, 1024);
                layer act: GELU;
                layer ff2: Linear(1024, 256);
                layer norm2: LayerNorm;
            }
            forward { attn_0 }
        }
    "#;

    /// A `stack 12 { … }` net must ship as a small REPEAT-folded blob, yet decode
    /// back to the full flat expression — proving the artifact is O(1) in depth
    /// while the format change stays invisible above this codec.
    #[test]
    fn stack_net_ships_folded_decodes_flat() {
        let module = parser::parse(&lexer::lex(STACK_SRC)).expect("stack net parses");
        let (blob, summary) = encode_module(&module);
        // 12 six-layer blocks = 72 stages, but the folded item is tiny.
        let item_bytes = summary[0].1;
        assert!(item_bytes < 200, "folded DeepT item = {item_bytes} B (expected < 200)");

        // Decode expands REPEAT → the flat 72-stage pipeline the consumers expect.
        let items = decode_container(&blob).expect("round-trip decode");
        assert_eq!(items.len(), 1);
        let net = match &module.items[0].kind {
            ast::ItemKind::Net(n) => n.clone(),
            _ => panic!("first item is the net"),
        };
        let flat = abl_bridge::NetTranslator::translate(&net).expr;
        assert_eq!(items[0].expr, flat, "decoded expr must equal the flat translation");
    }

    #[test]
    fn hex_round_trip() {
        let raw = b"\x00\x01\xfe\xff hello \xab";
        let s = to_hex(raw);
        assert_eq!(s, "0001feff2068656c6c6f20ab");
        let back = from_hex(&s).expect("decodes");
        assert_eq!(back, raw);
    }

    #[test]
    fn hex_rejects_odd_length() {
        assert!(from_hex("abc").is_err());
    }

    #[test]
    fn hex_rejects_non_hex() {
        assert!(from_hex("zz").is_err());
    }

    #[test]
    fn decode_rejects_bad_magic() {
        let err = decode_container(b"NOPE\x01\x00\x00\x00\x00\x00").unwrap_err();
        assert!(err.contains("bad magic"), "got: {err}");
    }

    #[test]
    fn decode_rejects_short_blob() {
        let err = decode_container(b"RMI").unwrap_err();
        assert!(err.contains("EOF"), "got: {err}");
    }
}

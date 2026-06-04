//! RMIB container codec — pure encode/decode for the `application/rmib`
//! payload that wraps one-or-more RMIL `Expr` blocks with names.
//!
//! Container layout (little-endian):
//! ```text
//!   magic    "RMIB" (4 bytes)
//!   version  u16 = 1
//!   count    u32           — number of items
//!   for each item:
//!     name_len u32
//!     name     UTF-8 bytes
//!     expr_len u32
//!     expr     codec::Encoder::encode_expr_only output
//! ```
//!
//! Used by both the CLI (`--target=rmil-bytes`, `--from=rmil-bytes`,
//! `--run=rmil-bytes`) and the RAP server (`rmil/encode`, `rmil/decode`,
//! `rmil/run`). Keeping the format in one place prevents drift.

use crate::ast;
use crate::rmil_bridge;
use rmi::lang::Expr;

pub const RMIB_MAGIC: &[u8; 4] = b"RMIB";
pub const RMIB_VERSION: u16 = 1;

/// One decoded item from a RMIB container.
#[derive(Debug)]
pub struct RmibItem {
    pub name: String,
    pub expr: Expr,
    pub expr_bytes_len: usize,
}

/// Lower a MechGen module and encode every RMIL-routed item into a single
/// RMIB blob. Returns the blob plus per-item `(name, expr_bytes_len, content_hash)`
/// for summary printing; callers can ignore the summary tuple if not needed.
pub fn encode_module(module: &ast::Module) -> (Vec<u8>, Vec<(String, usize, u64)>) {
    let lowered = rmil_bridge::lower_module(module);
    let mut blob: Vec<u8> = Vec::new();
    blob.extend_from_slice(RMIB_MAGIC);
    blob.extend_from_slice(&RMIB_VERSION.to_le_bytes());
    blob.extend_from_slice(&(lowered.items.len() as u32).to_le_bytes());
    let mut summary = Vec::with_capacity(lowered.items.len());
    for (name, expr) in &lowered.items {
        let name_bytes = name.as_bytes();
        let expr_bytes = rmi::lang::codec::Encoder::encode_expr_only(expr);
        blob.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        blob.extend_from_slice(name_bytes);
        blob.extend_from_slice(&(expr_bytes.len() as u32).to_le_bytes());
        blob.extend_from_slice(&expr_bytes);
        summary.push((name.clone(), expr_bytes.len(), expr.content_hash()));
    }
    (blob, summary)
}

/// Decode a RMIB container into its items. Returns a structured error string
/// rather than panicking, so the RAP layer can surface it as JSON.
pub fn decode_container(blob: &[u8]) -> Result<Vec<RmibItem>, String> {
    let mut pos = 0usize;
    fn take<'a>(buf: &'a [u8], pos: &mut usize, n: usize, what: &str) -> Result<&'a [u8], String> {
        if *pos + n > buf.len() {
            return Err(format!("{what}: unexpected EOF at offset {}", *pos));
        }
        let s = &buf[*pos..*pos + n];
        *pos += n;
        Ok(s)
    }
    let magic = take(blob, &mut pos, 4, "magic")?;
    if magic != RMIB_MAGIC {
        return Err(format!("bad magic {magic:?} (expected RMIB)"));
    }
    let ver = u16::from_le_bytes(
        take(blob, &mut pos, 2, "version")?
            .try_into()
            .map_err(|_| "version slice".to_string())?,
    );
    if ver != RMIB_VERSION {
        return Err(format!("unsupported RMIB version {ver}"));
    }
    let count = u32::from_le_bytes(
        take(blob, &mut pos, 4, "count")?
            .try_into()
            .map_err(|_| "count slice".to_string())?,
    ) as usize;

    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let nl = u32::from_le_bytes(
            take(blob, &mut pos, 4, "name_len")?
                .try_into()
                .map_err(|_| "name_len slice".to_string())?,
        ) as usize;
        let name = std::str::from_utf8(take(blob, &mut pos, nl, "name")?)
            .map_err(|e| format!("item {i} name utf8: {e}"))?
            .to_string();
        let el = u32::from_le_bytes(
            take(blob, &mut pos, 4, "expr_len")?
                .try_into()
                .map_err(|_| "expr_len slice".to_string())?,
        ) as usize;
        let expr_bytes = take(blob, &mut pos, el, "expr")?;
        let expr = rmi::lang::codec::Decoder::decode_expr_only(expr_bytes)
            .map_err(|e| format!("item {i} ({name}): decode error: {e:?}"))?;
        items.push(RmibItem {
            name,
            expr,
            expr_bytes_len: el,
        });
    }
    Ok(items)
}

/// Lowercase hex encoder — pure, no deps. Used by the RAP layer to ship
/// RMIB bytes through a JSON channel.
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
        assert!(blob.starts_with(RMIB_MAGIC));
        assert_eq!(summary.len(), 1, "tiny net should encode as one item");
        let items = decode_container(&blob).expect("round-trip decode");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, summary[0].0);
        assert_eq!(items[0].expr_bytes_len, summary[0].1);
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

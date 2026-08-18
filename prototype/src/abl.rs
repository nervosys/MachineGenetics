//! Agentic Binary Language container codec — pure encode/decode for the `application/abl`
//! payload that wraps one-or-more Agentic Binary Language `Expr` blocks with names.
//!
//! Container layout (little-endian):
//! ```text
//!   magic    "ABL1" (4 bytes — the literal, not the language's name)
//!   version  u16 = 3   (v3: per-item exprs are REPEAT-folded)
//!   count    u32           — number of items
//!   for each item:
//!     name_len u32
//!     name     UTF-8 bytes
//!     expr_len u32
//!     expr     codec::Encoder::encode_expr_only output
//!   symbols  u32           — interned-name count (v2+)
//!   for each symbol:
//!     name_len u32
//!     name     UTF-8 bytes
//! ```
//!
//! The symbol table is what makes a `kb` artifact self-describing: without it
//! a decoder recovers predicate arities and not their names. It was added in
//! v2 and left out of every published description of the format until it was
//! measured — 100 of `unified.mg`'s 420 container bytes.
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
        // Said "expected Agentic Binary Language" — the language's name where
        // the four magic bytes belong, so the one message a caller gets when a
        // file is not a container told them to look for the wrong thing.
        // Formatted from the constant now, so it cannot drift again.
        return Err(format!(
            "bad magic {magic:?} (expected {:?})",
            std::str::from_utf8(ABL_MAGIC).unwrap_or("ABL1")
        ));
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
    if !s.len().is_multiple_of(2) {
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

    /// A decoder written from the *published* format consumes the whole file.
    ///
    /// The container has carried a symbol table since v2 — the section that
    /// makes a `kb` artifact self-describing, since without it a decoder
    /// recovers predicate arities and not their names. It was in none of the
    /// four places the format is described: `MAGE_ONTOLOGY.json`'s `abl.format`,
    /// `AGENT_PROTOCOL.md`, and the module docs here and in `main.rs`. An agent
    /// implementing a decoder from any of them parsed the items and stopped,
    /// leaving 100 of `unified.mg`'s 420 bytes unread and every interned name
    /// unrecovered.
    ///
    /// So this walks the bytes using only the published field list, and
    /// asserts it lands exactly on the end. A section added to the format and
    /// not to the description fails here, which is the only way to notice —
    /// `decode_container` would keep working, because it reads the real
    /// format rather than the documented one.
    #[test]
    fn the_published_format_accounts_for_every_byte() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/unified.mg"),
        )
        .expect("unified.mg");
        let module = parser::parse(&lexer::lex(&src)).expect("unified.mg parses");
        let (blob, _) = encode_module(&module);

        let published = crate::ontology::section("abl").expect("abl section");
        let format = published["format"]
            .as_array()
            .expect("format")
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            format.contains("symbols"),
            "the published format does not mention the symbol section: {format}"
        );

        // Walk exactly what the description says is there.
        let mut pos = 0usize;
        assert_eq!(&blob[..4], ABL_MAGIC.as_slice(), "magic");
        pos += 4;
        let ver = u16::from_le_bytes(blob[pos..pos + 2].try_into().unwrap());
        pos += 2;
        assert_eq!(ver, ABL_VERSION);
        let count = u32::from_le_bytes(blob[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        for _ in 0..count {
            let nl = u32::from_le_bytes(blob[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4 + nl;
            let el = u32::from_le_bytes(blob[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4 + el;
        }
        let syms = u32::from_le_bytes(blob[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        for _ in 0..syms {
            let nl = u32::from_le_bytes(blob[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4 + nl;
        }
        assert_eq!(
            pos,
            blob.len(),
            "a decoder following the published format stops {} bytes early — the \
             container has a section the description does not name",
            blob.len() - pos
        );
        assert!(syms > 0, "unified.mg interns names; the symbol table should not be empty");
    }

    /// A repeated stack is O(1) in depth, and comes back whole.
    ///
    /// `MEASUREMENTS.md` now claims a 128-layer net of identical `Linear(16,
    /// 16)` ships in 67 bytes and decompiles to 128 layers. Both halves matter
    /// and only together: a fold that loses layers would also produce a small
    /// artifact, and "compact at rest" would be measuring data loss.
    ///
    /// The size claim is a constant rather than a bound because it is
    /// machine-independent — if the encoding changes, this should be updated
    /// deliberately, and the document with it.
    #[test]
    fn identical_layers_fold_to_a_constant_and_survive_the_round_trip() {
        let net = |n: usize| {
            let layers: String =
                (0..n).map(|i| format!("  layer fc{i}: Linear(16, 16);\n")).collect();
            format!("net N {{\n{layers}  forward {{ fc0 }}\n}}\n")
        };
        let mut sizes = Vec::new();
        for n in [2usize, 8, 32, 128] {
            let module = parser::parse(&lexer::lex(&net(n))).expect("net parses");
            let (blob, _) = encode_module(&module);
            sizes.push(blob.len());

            let items = decode_container(&blob).expect("decode");
            assert_eq!(items.len(), 1);
            // The expr expands back to one op per layer; `Seq` length is the
            // structural count the decompiler renders.
            let layer_count =
                crate::abl_bridge::decompile(&items[0].expr, &items[0].name).net.layers.len();
            assert_eq!(
                layer_count, n,
                "a {n}-layer net came back with {layer_count} layers — the fold is lossy"
            );
        }
        assert!(
            sizes.iter().all(|&s| s == sizes[0]),
            "identical layers should fold to a constant size, got {sizes:?}"
        );
        assert_eq!(sizes[0], 67, "the documented constant is 67 bytes");
    }

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

#[cfg(test)]
mod documented_version_tests {
    use super::ABL_VERSION;

    /// `ARCHITECTURE.md` documents the container's wire version, and `decode`
    /// rejects a mismatch — so a stale number there is not cosmetic, it tells a
    /// reader to build artifacts this toolchain will refuse.
    ///
    /// It said "(currently 2)" from the 2026-06-12 bump to 3 until 2026-08-05,
    /// alongside a committed `MAGE_ONTOLOGY.json` advertising the same stale 2.
    /// Both were found by regenerating rather than reading. This asserts the
    /// prose against the constant so the next bump cannot leave it behind.
    #[test]
    fn architecture_md_documents_the_real_container_version() {
        let doc = include_str!("../../ARCHITECTURE.md");
        let want = format!("(currently {ABL_VERSION})");
        assert!(
            doc.contains(&want),
            "ARCHITECTURE.md must say `{want}` for the ABL container version; \
             bump the doc and regenerate MAGE_ONTOLOGY.json"
        );
    }
}

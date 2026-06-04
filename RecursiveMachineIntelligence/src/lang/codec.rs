//! Binary codec for RMIL programs.
//!
//! Wire format:
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │ Header (12 bytes)                   │
//! │  magic: [u8; 4] = "RMIL"           │
//! │  version: u16 = 1                  │
//! │  flags: u16                        │
//! │  root_nodes: u32                   │
//! ├─────────────────────────────────────┤
//! │ Symbol table                       │
//! │  count: u32                        │
//! │  for each: len:u16 + utf8 bytes    │
//! ├─────────────────────────────────────┤
//! │ Expression tree (recursive)        │
//! │  tag:u8 + variant-specific data    │
//! └─────────────────────────────────────┘
//! ```
//!
//! Design goals:
//! - **Compact**: variable-length integers, no padding, no alignment
//! - **Streamable**: can decode without seeking
//! - **Self-contained**: symbol table is inline
//!
//! A full transformer block encodes in ~60 bytes.

use crate::lang::expr::{Expr, Val};
use crate::lang::op::Op;
use crate::lang::sym::{Sym, SymbolTable};
use crate::lang::ty::{Dtype, Ty};

/// Magic bytes: "RMIL"
pub const MAGIC: [u8; 4] = [0x52, 0x4D, 0x49, 0x4C];
/// Current wire format version.
pub const VERSION: u16 = 1;

/// Encode error.
#[derive(Debug)]
pub enum CodecError {
    /// Buffer too short.
    Truncated,
    /// Invalid tag or discriminant.
    InvalidTag(u8),
    /// Bad magic bytes.
    BadMagic,
    /// Version mismatch.
    VersionMismatch(u16),
    /// Invalid UTF-8 in symbol table.
    BadUtf8,
    /// Expression nesting too deep (DoS guard).
    TooDeep,
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => f.write_str("truncated"),
            Self::InvalidTag(t) => write!(f, "invalid tag: {t}"),
            Self::BadMagic => f.write_str("bad magic"),
            Self::VersionMismatch(v) => write!(f, "version mismatch: {v}"),
            Self::BadUtf8 => f.write_str("bad utf8"),
            Self::TooDeep => f.write_str("nesting too deep"),
        }
    }
}

impl std::error::Error for CodecError {}

// ── Encoder ──────────────────────────────────────────────────────────────────

/// Binary encoder — writes RMIL expressions to a byte buffer.
pub struct Encoder {
    buf: Vec<u8>,
}

impl Encoder {
    /// New encoder with empty buffer.
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(256),
        }
    }

    /// Encode a complete RMIL program (header + symbol table + expression).
    pub fn encode_program(expr: &Expr, symbols: &SymbolTable) -> Vec<u8> {
        let mut enc = Self::new();
        // Header
        enc.buf.extend_from_slice(&MAGIC);
        enc.write_u16(VERSION);
        enc.write_u16(0); // flags (reserved)
        enc.write_u32(1); // root_nodes
        // Symbol table
        let sym_count = symbols.len() as u32;
        enc.write_u32(sym_count);
        for i in 0..sym_count {
            let s = symbols.resolve(Sym(i));
            let bytes = s.as_bytes();
            enc.write_u16(bytes.len() as u16);
            enc.buf.extend_from_slice(bytes);
        }
        // Expression
        enc.encode_expr(expr);
        enc.buf
    }

    /// Encode just an expression (no header/symbols).
    pub fn encode_expr_only(expr: &Expr) -> Vec<u8> {
        let mut enc = Self::new();
        enc.encode_expr(expr);
        enc.buf
    }

    fn encode_expr(&mut self, expr: &Expr) {
        self.buf.push(expr.tag());
        match expr {
            Expr::Lit(val) => self.encode_val(val),
            Expr::Ref(sym) => self.write_u32(sym.0),
            Expr::App(op, args) => {
                self.write_u16(op.0);
                self.write_u16(args.len() as u16);
                for a in args {
                    self.encode_expr(a);
                }
            }
            Expr::Seq(a, b) => {
                self.encode_expr(a);
                self.encode_expr(b);
            }
            Expr::Par(a, b) => {
                self.encode_expr(a);
                self.encode_expr(b);
            }
            Expr::Cond { pred, yes, no } => {
                self.encode_expr(pred);
                self.encode_expr(yes);
                self.encode_expr(no);
            }
            Expr::Let { name, val, body } => {
                self.write_u32(name.0);
                self.encode_expr(val);
                self.encode_expr(body);
            }
            Expr::Lam { params, body } => {
                self.write_u16(params.len() as u16);
                for (sym, ty) in params {
                    self.write_u32(sym.0);
                    self.encode_ty(ty);
                }
                self.encode_expr(body);
            }
            Expr::Call(func, args) => {
                self.encode_expr(func);
                self.write_u16(args.len() as u16);
                for a in args {
                    self.encode_expr(a);
                }
            }
            Expr::Block(exprs) => {
                self.write_u32(exprs.len() as u32);
                for e in exprs {
                    self.encode_expr(e);
                }
            }
        }
    }

    fn encode_val(&mut self, val: &Val) {
        self.buf.push(val.tag());
        match val {
            Val::Nil => {}
            Val::Bool(v) => self.buf.push(*v as u8),
            Val::I64(v) => self.write_i64(*v),
            Val::F32(bits) => self.write_u32(*bits),
            Val::F64(bits) => self.write_u64(*bits),
            Val::Tensor { dtype, shape, data } => {
                self.buf.push(*dtype as u8);
                self.write_u16(shape.len() as u16);
                for &d in shape {
                    self.write_u32(d as u32);
                }
                self.write_u32(data.len() as u32);
                self.buf.extend_from_slice(data);
            }
            Val::Sym(s) => self.write_u32(s.0),
            Val::Tuple(vs) => {
                self.write_u16(vs.len() as u16);
                for v in vs {
                    self.encode_val(v);
                }
            }
        }
    }

    fn encode_ty(&mut self, ty: &Ty) {
        self.buf.push(ty.tag());
        match ty {
            Ty::Void => {}
            Ty::Scalar(d) => self.buf.push(*d as u8),
            Ty::Tensor(d, shape) => {
                self.buf.push(*d as u8);
                self.write_u16(shape.len() as u16);
                for &dim in shape {
                    self.write_u32(dim as u32);
                }
            }
            Ty::Sym => {}
            Ty::Fn(inp, out) => {
                self.write_u16(inp.len() as u16);
                for t in inp {
                    self.encode_ty(t);
                }
                self.write_u16(out.len() as u16);
                for t in out {
                    self.encode_ty(t);
                }
            }
            Ty::Tuple(ts) | Ty::Union(ts) => {
                self.write_u16(ts.len() as u16);
                for t in ts {
                    self.encode_ty(t);
                }
            }
            Ty::Var(n) | Ty::Opaque(n) => self.write_u32(*n),
        }
    }

    // ── Primitives ───────────────────────────────────────────────────────

    fn write_u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn write_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn write_u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn write_i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

// ── Decoder ──────────────────────────────────────────────────────────────────

/// Binary decoder — reads RMIL programs from a byte buffer.
pub struct Decoder<'a> {
    data: &'a [u8],
    pos: usize,
    depth: usize,
    max_depth: usize,
}

impl<'a> Decoder<'a> {
    /// Decode a complete RMIL program (validates header, reads symbols + expr).
    pub fn decode_program(data: &'a [u8]) -> Result<(Expr, SymbolTable), CodecError> {
        let mut dec = Self {
            data,
            pos: 0,
            depth: 0,
            max_depth: 512,
        };

        // Header
        let magic = dec.read_bytes(4)?;
        if magic != MAGIC {
            return Err(CodecError::BadMagic);
        }
        let version = dec.read_u16()?;
        if version != VERSION {
            return Err(CodecError::VersionMismatch(version));
        }
        let _flags = dec.read_u16()?;
        let _root_nodes = dec.read_u32()?;

        // Symbol table
        let sym_count = dec.read_u32()?;
        let mut symbols = SymbolTable::new();
        // Skip slot 0 (nil, already in table)
        for i in 0..sym_count {
            let len = dec.read_u16()? as usize;
            let bytes = dec.read_bytes(len)?;
            let s = std::str::from_utf8(bytes).map_err(|_| CodecError::BadUtf8)?;
            if i > 0 {
                // slot 0 is already ""
                symbols.intern(s);
            }
        }

        // Expression
        let expr = dec.decode_expr()?;
        Ok((expr, symbols))
    }

    /// Decode just an expression (no header/symbols).
    pub fn decode_expr_only(data: &'a [u8]) -> Result<Expr, CodecError> {
        let mut dec = Self {
            data,
            pos: 0,
            depth: 0,
            max_depth: 512,
        };
        dec.decode_expr()
    }

    fn decode_expr(&mut self) -> Result<Expr, CodecError> {
        self.depth += 1;
        if self.depth > self.max_depth {
            return Err(CodecError::TooDeep);
        }
        let tag = self.read_u8()?;
        let expr = match tag {
            0 => {
                // Lit
                let val = self.decode_val()?;
                Expr::Lit(val)
            }
            1 => {
                // Ref
                let sym = Sym(self.read_u32()?);
                Expr::Ref(sym)
            }
            2 => {
                // App
                let op = Op(self.read_u16()?);
                let n = self.read_u16()? as usize;
                let mut args = Vec::with_capacity(n);
                for _ in 0..n {
                    args.push(self.decode_expr()?);
                }
                Expr::App(op, args)
            }
            3 => {
                // Seq
                let a = self.decode_expr()?;
                let b = self.decode_expr()?;
                Expr::Seq(Box::new(a), Box::new(b))
            }
            4 => {
                // Par
                let a = self.decode_expr()?;
                let b = self.decode_expr()?;
                Expr::Par(Box::new(a), Box::new(b))
            }
            5 => {
                // Cond
                let pred = self.decode_expr()?;
                let yes = self.decode_expr()?;
                let no = self.decode_expr()?;
                Expr::Cond {
                    pred: Box::new(pred),
                    yes: Box::new(yes),
                    no: Box::new(no),
                }
            }
            6 => {
                // Let
                let name = Sym(self.read_u32()?);
                let val = self.decode_expr()?;
                let body = self.decode_expr()?;
                Expr::Let {
                    name,
                    val: Box::new(val),
                    body: Box::new(body),
                }
            }
            7 => {
                // Lam
                let n = self.read_u16()? as usize;
                let mut params = Vec::with_capacity(n);
                for _ in 0..n {
                    let sym = Sym(self.read_u32()?);
                    let ty = self.decode_ty()?;
                    params.push((sym, ty));
                }
                let body = self.decode_expr()?;
                Expr::Lam {
                    params,
                    body: Box::new(body),
                }
            }
            8 => {
                // Call
                let func = self.decode_expr()?;
                let n = self.read_u16()? as usize;
                let mut args = Vec::with_capacity(n);
                for _ in 0..n {
                    args.push(self.decode_expr()?);
                }
                Expr::Call(Box::new(func), args)
            }
            9 => {
                // Block
                let n = self.read_u32()? as usize;
                let mut exprs = Vec::with_capacity(n);
                for _ in 0..n {
                    exprs.push(self.decode_expr()?);
                }
                Expr::Block(exprs)
            }
            _ => return Err(CodecError::InvalidTag(tag)),
        };
        self.depth -= 1;
        Ok(expr)
    }

    fn decode_val(&mut self) -> Result<Val, CodecError> {
        let tag = self.read_u8()?;
        match tag {
            0 => Ok(Val::Nil),
            1 => Ok(Val::Bool(self.read_u8()? != 0)),
            2 => Ok(Val::I64(self.read_i64()?)),
            3 => Ok(Val::F32(self.read_u32()?)),
            4 => Ok(Val::F64(self.read_u64()?)),
            5 => {
                let dtype_byte = self.read_u8()?;
                let dtype = Dtype::from_u8(dtype_byte).ok_or(CodecError::InvalidTag(dtype_byte))?;
                let ndim = self.read_u16()? as usize;
                let mut shape = Vec::with_capacity(ndim);
                for _ in 0..ndim {
                    shape.push(self.read_u32()? as usize);
                }
                let data_len = self.read_u32()? as usize;
                let data = self.read_bytes(data_len)?.to_vec();
                Ok(Val::Tensor { dtype, shape, data })
            }
            6 => Ok(Val::Sym(Sym(self.read_u32()?))),
            7 => {
                let n = self.read_u16()? as usize;
                let mut vs = Vec::with_capacity(n);
                for _ in 0..n {
                    vs.push(self.decode_val()?);
                }
                Ok(Val::Tuple(vs))
            }
            _ => Err(CodecError::InvalidTag(tag)),
        }
    }

    fn decode_ty(&mut self) -> Result<Ty, CodecError> {
        let tag = self.read_u8()?;
        match tag {
            0 => Ok(Ty::Void),
            1 => {
                let d = self.read_u8()?;
                Ok(Ty::Scalar(Dtype::from_u8(d).ok_or(CodecError::InvalidTag(d))?))
            }
            2 => {
                let d = self.read_u8()?;
                let dtype = Dtype::from_u8(d).ok_or(CodecError::InvalidTag(d))?;
                let ndim = self.read_u16()? as usize;
                let mut shape = Vec::with_capacity(ndim);
                for _ in 0..ndim {
                    shape.push(self.read_u32()? as usize);
                }
                Ok(Ty::Tensor(dtype, shape))
            }
            3 => Ok(Ty::Sym),
            4 => {
                let ni = self.read_u16()? as usize;
                let mut inp = Vec::with_capacity(ni);
                for _ in 0..ni {
                    inp.push(self.decode_ty()?);
                }
                let no = self.read_u16()? as usize;
                let mut out = Vec::with_capacity(no);
                for _ in 0..no {
                    out.push(self.decode_ty()?);
                }
                Ok(Ty::Fn(inp, out))
            }
            5 => {
                let n = self.read_u16()? as usize;
                let mut ts = Vec::with_capacity(n);
                for _ in 0..n {
                    ts.push(self.decode_ty()?);
                }
                Ok(Ty::Tuple(ts))
            }
            6 => {
                let n = self.read_u16()? as usize;
                let mut ts = Vec::with_capacity(n);
                for _ in 0..n {
                    ts.push(self.decode_ty()?);
                }
                Ok(Ty::Union(ts))
            }
            7 => Ok(Ty::Var(self.read_u32()?)),
            8 => Ok(Ty::Opaque(self.read_u32()?)),
            _ => Err(CodecError::InvalidTag(tag)),
        }
    }

    // ── Primitives ───────────────────────────────────────────────────────

    fn read_u8(&mut self) -> Result<u8, CodecError> {
        if self.pos >= self.data.len() {
            return Err(CodecError::Truncated);
        }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        if self.pos + n > self.data.len() {
            return Err(CodecError::Truncated);
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn read_u16(&mut self) -> Result<u16, CodecError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, CodecError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, CodecError> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_i64(&mut self) -> Result<i64, CodecError> {
        let bytes = self.read_bytes(8)?;
        Ok(i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }
}

// ── Convenience ──────────────────────────────────────────────────────────────

/// Encode an expression (without header) and return the byte count.
/// Useful for measuring wire size.
pub fn wire_size(expr: &Expr) -> usize {
    Encoder::encode_expr_only(expr).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::expr::{patterns, Expr, Val};
    use crate::lang::op::Op;
    use crate::lang::sym::SymbolTable;

    #[test]
    fn roundtrip_literal() {
        let expr = Expr::int(42);
        let bytes = Encoder::encode_expr_only(&expr);
        let decoded = Decoder::decode_expr_only(&bytes).unwrap();
        assert_eq!(expr, decoded);
    }

    #[test]
    fn roundtrip_seq() {
        let expr = Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU);
        let bytes = Encoder::encode_expr_only(&expr);
        let decoded = Decoder::decode_expr_only(&bytes).unwrap();
        assert_eq!(expr, decoded);
    }

    #[test]
    fn roundtrip_transformer_block() {
        let expr = patterns::transformer_block();
        let bytes = Encoder::encode_expr_only(&expr);
        let decoded = Decoder::decode_expr_only(&bytes).unwrap();
        assert_eq!(expr.content_hash(), decoded.content_hash());
    }

    #[test]
    fn roundtrip_program() {
        let mut syms = SymbolTable::new();
        let x = syms.intern("x");
        let expr = Expr::bind(x, Expr::float(3.15), Expr::sym(x));
        let bytes = Encoder::encode_program(&expr, &syms);
        let (decoded, decoded_syms) = Decoder::decode_program(&bytes).unwrap();
        assert_eq!(expr, decoded);
        assert_eq!(decoded_syms.resolve(x), "x");
    }

    #[test]
    fn roundtrip_val_tensor() {
        let val = Val::Tensor {
            dtype: Dtype::F32,
            shape: vec![2, 3],
            data: vec![0u8; 24], // 6 × 4 bytes
        };
        let expr = Expr::Lit(val);
        let bytes = Encoder::encode_expr_only(&expr);
        let decoded = Decoder::decode_expr_only(&bytes).unwrap();
        assert_eq!(expr, decoded);
    }

    #[test]
    fn compact_size() {
        // A transformer block should encode in well under 200 bytes
        let block = patterns::transformer_block();
        let size = wire_size(&block);
        assert!(
            size < 200,
            "transformer block encoded to {size} bytes (expected < 200)"
        );
    }

    #[test]
    fn bad_magic() {
        let mut data = vec![0x00, 0x00, 0x00, 0x00]; // wrong magic
        data.extend_from_slice(&1u16.to_le_bytes());
        assert!(matches!(
            Decoder::decode_program(&data),
            Err(CodecError::BadMagic)
        ));
    }

    #[test]
    fn truncated() {
        assert!(matches!(
            Decoder::decode_expr_only(&[]),
            Err(CodecError::Truncated)
        ));
    }

    #[test]
    fn roundtrip_cond() {
        let expr = Expr::Cond {
            pred: Box::new(Expr::boolean(true)),
            yes: Box::new(Expr::int(1)),
            no: Box::new(Expr::int(0)),
        };
        let bytes = Encoder::encode_expr_only(&expr);
        let decoded = Decoder::decode_expr_only(&bytes).unwrap();
        assert_eq!(expr, decoded);
    }

    #[test]
    fn roundtrip_par() {
        let expr = Expr::op1(Op::LINEAR) | Expr::op1(Op::CONV2D);
        let bytes = Encoder::encode_expr_only(&expr);
        let decoded = Decoder::decode_expr_only(&bytes).unwrap();
        assert_eq!(expr, decoded);
    }

    #[test]
    fn roundtrip_block() {
        let expr = Expr::Block(vec![Expr::int(1), Expr::int(2), Expr::int(3)]);
        let bytes = Encoder::encode_expr_only(&expr);
        let decoded = Decoder::decode_expr_only(&bytes).unwrap();
        assert_eq!(expr, decoded);
    }

    #[test]
    fn roundtrip_lambda() {
        let expr = Expr::lam(
            vec![(Sym(1), Ty::f32()), (Sym(2), Ty::f32t(&[3, 4]))],
            Expr::op2(Op::ADD, Expr::sym(Sym(1)), Expr::sym(Sym(2))),
        );
        let bytes = Encoder::encode_expr_only(&expr);
        let decoded = Decoder::decode_expr_only(&bytes).unwrap();
        assert_eq!(expr, decoded);
    }
}

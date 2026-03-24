// ── FFI Binding Generator ──────────────────────────────────────────
//
// Generates safe MechGen wrappers for foreign function interfaces.
//
// Supported targets:
//   - C headers  → MechGen extern declarations + safe wrappers
//   - Python     → .pyi stub files
//   - WASM       → .wit component bindings
//
// Each target produces:
//   1. Raw declarations (unsafe extern)
//   2. Safe wrapper functions with contract annotations
//   3. Type mappings from foreign types → MechGen types

use std::collections::BTreeMap;

// ── Foreign Type ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeignType {
    Void,
    Int(u8),      // bit width: 8, 16, 32, 64
    UInt(u8),
    Float(u8),    // 32, 64
    Bool,
    CString,      // *const c_char
    Ptr(Box<ForeignType>),
    Array(Box<ForeignType>, usize),
    Struct(String),
    Opaque(String),
}

impl ForeignType {
    pub fn to_c_type(&self) -> String {
        match self {
            ForeignType::Void => "void".into(),
            ForeignType::Int(8) => "int8_t".into(),
            ForeignType::Int(16) => "int16_t".into(),
            ForeignType::Int(32) => "int32_t".into(),
            ForeignType::Int(64) => "int64_t".into(),
            ForeignType::Int(w) => format!("int{w}_t"),
            ForeignType::UInt(8) => "uint8_t".into(),
            ForeignType::UInt(16) => "uint16_t".into(),
            ForeignType::UInt(32) => "uint32_t".into(),
            ForeignType::UInt(64) => "uint64_t".into(),
            ForeignType::UInt(w) => format!("uint{w}_t"),
            ForeignType::Float(32) => "float".into(),
            ForeignType::Float(64) => "double".into(),
            ForeignType::Float(w) => format!("float{w}"),
            ForeignType::Bool => "_Bool".into(),
            ForeignType::CString => "const char*".into(),
            ForeignType::Ptr(inner) => format!("{}*", inner.to_c_type()),
            ForeignType::Array(inner, n) => format!("{}[{}]", inner.to_c_type(), n),
            ForeignType::Struct(name) => format!("struct {name}"),
            ForeignType::Opaque(name) => name.clone(),
        }
    }

    pub fn to_redox_type(&self) -> String {
        match self {
            ForeignType::Void => "()".into(),
            ForeignType::Int(8) => "i8".into(),
            ForeignType::Int(16) => "i16".into(),
            ForeignType::Int(32) => "i32".into(),
            ForeignType::Int(64) => "i64".into(),
            ForeignType::Int(w) => format!("i{w}"),
            ForeignType::UInt(8) => "u8".into(),
            ForeignType::UInt(16) => "u16".into(),
            ForeignType::UInt(32) => "u32".into(),
            ForeignType::UInt(64) => "u64".into(),
            ForeignType::UInt(w) => format!("u{w}"),
            ForeignType::Float(32) => "f32".into(),
            ForeignType::Float(64) => "f64".into(),
            ForeignType::Float(w) => format!("f{w}"),
            ForeignType::Bool => "bool".into(),
            ForeignType::CString => "&str".into(),
            ForeignType::Ptr(inner) => format!("^{}", inner.to_redox_type()),
            ForeignType::Array(inner, n) => format!("[{}; {}]", inner.to_redox_type(), n),
            ForeignType::Struct(name) => name.clone(),
            ForeignType::Opaque(name) => name.clone(),
        }
    }

    pub fn to_python_type(&self) -> String {
        match self {
            ForeignType::Void => "None".into(),
            ForeignType::Int(_) | ForeignType::UInt(_) => "int".into(),
            ForeignType::Float(_) => "float".into(),
            ForeignType::Bool => "bool".into(),
            ForeignType::CString => "str".into(),
            ForeignType::Ptr(_) => "int".into(), // ctypes pointer
            ForeignType::Array(inner, _) => format!("list[{}]", inner.to_python_type()),
            ForeignType::Struct(name) | ForeignType::Opaque(name) => name.clone(),
        }
    }
}

// ── Foreign Function ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ForeignFunction {
    pub name: String,
    pub params: Vec<(String, ForeignType)>,
    pub return_type: ForeignType,
    pub library: String,
    pub doc: Option<String>,
}

// ── Foreign Struct ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ForeignStruct {
    pub name: String,
    pub fields: Vec<(String, ForeignType)>,
}

// ── Binding Target ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingTarget {
    C,
    Python,
    Wasm,
}

// ── FFI Generator ──────────────────────────────────────────────────

pub struct FfiGenerator {
    functions: Vec<ForeignFunction>,
    structs: Vec<ForeignStruct>,
    type_overrides: BTreeMap<String, String>, // foreign name → MechGen name
}

impl FfiGenerator {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            structs: Vec::new(),
            type_overrides: BTreeMap::new(),
        }
    }

    pub fn add_function(&mut self, func: ForeignFunction) {
        self.functions.push(func);
    }

    pub fn add_struct(&mut self, s: ForeignStruct) {
        self.structs.push(s);
    }

    pub fn add_type_override(&mut self, foreign: &str, MechGen: &str) {
        self.type_overrides.insert(foreign.into(), MechGen.into());
    }

    /// Generate raw extern declarations (MechGen syntax).
    pub fn generate_extern_decls(&self) -> String {
        let mut out = String::new();
        for f in &self.functions {
            let params: Vec<String> = f.params.iter()
                .map(|(n, t)| format!("{}: {}", n, self.map_type(t)))
                .collect();
            let ret = if matches!(f.return_type, ForeignType::Void) {
                String::new()
            } else {
                format!(" -> {}", self.map_type(&f.return_type))
            };
            out.push_str(&format!(
                "extern \"C\" fn {}({}){}\n",
                f.name, params.join(", "), ret
            ));
        }
        out
    }

    /// Generate safe wrapper functions.
    pub fn generate_safe_wrappers(&self) -> String {
        let mut out = String::new();
        for f in &self.functions {
            let params: Vec<String> = f.params.iter()
                .map(|(n, t)| format!("{}: {}", n, self.map_type(t)))
                .collect();
            let ret = if matches!(f.return_type, ForeignType::Void) {
                String::new()
            } else {
                format!(" -> {}", self.map_type(&f.return_type))
            };
            // Add @req for pointer params (non-null).
            let mut contracts = Vec::new();
            for (name, ty) in &f.params {
                if matches!(ty, ForeignType::Ptr(_) | ForeignType::CString) {
                    contracts.push(format!("@req({name} != null)"));
                }
            }
            for c in &contracts {
                out.push_str(&format!("{c}\n"));
            }
            out.push_str(&format!(
                "pub fn {}({}){} {{\n    unsafe {{ ffi::{}({}) }}\n}}\n\n",
                f.name,
                params.join(", "),
                ret,
                f.name,
                f.params.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", "),
            ));
        }
        out
    }

    /// Generate Python .pyi stubs.
    pub fn generate_python_stubs(&self) -> String {
        let mut out = String::new();
        for s in &self.structs {
            out.push_str(&format!("class {}:\n", s.name));
            for (name, ty) in &s.fields {
                out.push_str(&format!("    {}: {}\n", name, ty.to_python_type()));
            }
            out.push('\n');
        }
        for f in &self.functions {
            let params: Vec<String> = f.params.iter()
                .map(|(n, t)| format!("{}: {}", n, t.to_python_type()))
                .collect();
            let ret = f.return_type.to_python_type();
            out.push_str(&format!("def {}({}) -> {}: ...\n", f.name, params.join(", "), ret));
        }
        out
    }

    /// Generate WASM .wit interface.
    pub fn generate_wasm_wit(&self) -> String {
        let mut out = String::from("interface bindings {\n");
        for s in &self.structs {
            out.push_str(&format!("  record {} {{\n", s.name.to_lowercase()));
            for (name, ty) in &s.fields {
                out.push_str(&format!("    {}: {},\n", name, self.wit_type(ty)));
            }
            out.push_str("  }\n\n");
        }
        for f in &self.functions {
            let params: Vec<String> = f.params.iter()
                .map(|(n, t)| format!("{}: {}", n, self.wit_type(t)))
                .collect();
            let ret = if matches!(f.return_type, ForeignType::Void) {
                String::new()
            } else {
                format!(" -> {}", self.wit_type(&f.return_type))
            };
            out.push_str(&format!("  {}: func({}){}\n", f.name, params.join(", "), ret));
        }
        out.push_str("}\n");
        out
    }

    fn map_type(&self, ty: &ForeignType) -> String {
        let base = ty.to_redox_type();
        self.type_overrides.get(&base).cloned().unwrap_or(base)
    }

    fn wit_type(&self, ty: &ForeignType) -> String {
        match ty {
            ForeignType::Void => "unit".into(),
            ForeignType::Int(8) => "s8".into(),
            ForeignType::Int(16) => "s16".into(),
            ForeignType::Int(32) => "s32".into(),
            ForeignType::Int(64) => "s64".into(),
            ForeignType::Int(w) => format!("s{w}"),
            ForeignType::UInt(8) => "u8".into(),
            ForeignType::UInt(16) => "u16".into(),
            ForeignType::UInt(32) => "u32".into(),
            ForeignType::UInt(64) => "u64".into(),
            ForeignType::UInt(w) => format!("u{w}"),
            ForeignType::Float(32) => "float32".into(),
            ForeignType::Float(64) => "float64".into(),
            ForeignType::Float(w) => format!("float{w}"),
            ForeignType::Bool => "bool".into(),
            ForeignType::CString => "string".into(),
            ForeignType::Ptr(inner) => self.wit_type(inner), // WIT doesn't have raw pointers
            ForeignType::Array(inner, _) => format!("list<{}>", self.wit_type(inner)),
            ForeignType::Struct(name) => name.to_lowercase(),
            ForeignType::Opaque(name) => name.to_lowercase(),
        }
    }

    /// Summary stats.
    pub fn stats(&self) -> String {
        format!(
            "{{\"functions\":{},\"structs\":{},\"type_overrides\":{}}}",
            self.functions.len(),
            self.structs.len(),
            self.type_overrides.len()
        )
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_func() -> ForeignFunction {
        ForeignFunction {
            name: "add".into(),
            params: vec![("a".into(), ForeignType::Int(32)), ("b".into(), ForeignType::Int(32))],
            return_type: ForeignType::Int(32),
            library: "libmath".into(),
            doc: Some("Add two integers".into()),
        }
    }

    fn ptr_func() -> ForeignFunction {
        ForeignFunction {
            name: "strlen".into(),
            params: vec![("s".into(), ForeignType::CString)],
            return_type: ForeignType::UInt(64),
            library: "libc".into(),
            doc: None,
        }
    }

    fn sample_struct() -> ForeignStruct {
        ForeignStruct {
            name: "Point".into(),
            fields: vec![("x".into(), ForeignType::Float(64)), ("y".into(), ForeignType::Float(64))],
        }
    }

    // ── Type conversions ──────────────────────────────────────────

    #[test]
    fn c_type_conversions() {
        assert_eq!(ForeignType::Int(32).to_c_type(), "int32_t");
        assert_eq!(ForeignType::Float(64).to_c_type(), "double");
        assert_eq!(ForeignType::CString.to_c_type(), "const char*");
        assert_eq!(ForeignType::Void.to_c_type(), "void");
        assert_eq!(ForeignType::Ptr(Box::new(ForeignType::Int(32))).to_c_type(), "int32_t*");
    }

    #[test]
    fn redox_type_conversions() {
        assert_eq!(ForeignType::Int(32).to_redox_type(), "i32");
        assert_eq!(ForeignType::UInt(8).to_redox_type(), "u8");
        assert_eq!(ForeignType::Float(32).to_redox_type(), "f32");
        assert_eq!(ForeignType::CString.to_redox_type(), "&str");
        assert_eq!(ForeignType::Bool.to_redox_type(), "bool");
    }

    #[test]
    fn python_type_conversions() {
        assert_eq!(ForeignType::Int(32).to_python_type(), "int");
        assert_eq!(ForeignType::Float(64).to_python_type(), "float");
        assert_eq!(ForeignType::CString.to_python_type(), "str");
        assert_eq!(ForeignType::Bool.to_python_type(), "bool");
        assert_eq!(ForeignType::Void.to_python_type(), "None");
    }

    // ── Extern declarations ───────────────────────────────────────

    #[test]
    fn extern_decls() {
        let mut fg = FfiGenerator::new();
        fg.add_function(sample_func());
        let decls = fg.generate_extern_decls();
        assert!(decls.contains("extern \"C\" fn add(a: i32, b: i32) -> i32"));
    }

    #[test]
    fn extern_void_return() {
        let mut fg = FfiGenerator::new();
        fg.add_function(ForeignFunction {
            name: "init".into(),
            params: vec![],
            return_type: ForeignType::Void,
            library: "lib".into(),
            doc: None,
        });
        let decls = fg.generate_extern_decls();
        assert!(decls.contains("extern \"C\" fn init()"));
        assert!(!decls.contains("->"));
    }

    // ── Safe wrappers ─────────────────────────────────────────────

    #[test]
    fn safe_wrapper_basic() {
        let mut fg = FfiGenerator::new();
        fg.add_function(sample_func());
        let wrappers = fg.generate_safe_wrappers();
        assert!(wrappers.contains("pub fn add(a: i32, b: i32) -> i32"));
        assert!(wrappers.contains("unsafe { ffi::add(a, b) }"));
    }

    #[test]
    fn safe_wrapper_null_contract() {
        let mut fg = FfiGenerator::new();
        fg.add_function(ptr_func());
        let wrappers = fg.generate_safe_wrappers();
        assert!(wrappers.contains("@req(s != null)"));
    }

    // ── Python stubs ──────────────────────────────────────────────

    #[test]
    fn python_stubs_function() {
        let mut fg = FfiGenerator::new();
        fg.add_function(sample_func());
        let stubs = fg.generate_python_stubs();
        assert!(stubs.contains("def add(a: int, b: int) -> int: ..."));
    }

    #[test]
    fn python_stubs_struct() {
        let mut fg = FfiGenerator::new();
        fg.add_struct(sample_struct());
        let stubs = fg.generate_python_stubs();
        assert!(stubs.contains("class Point:"));
        assert!(stubs.contains("x: float"));
    }

    // ── WASM WIT ──────────────────────────────────────────────────

    #[test]
    fn wasm_wit_function() {
        let mut fg = FfiGenerator::new();
        fg.add_function(sample_func());
        let wit = fg.generate_wasm_wit();
        assert!(wit.contains("add: func(a: s32, b: s32) -> s32"));
    }

    #[test]
    fn wasm_wit_struct() {
        let mut fg = FfiGenerator::new();
        fg.add_struct(sample_struct());
        let wit = fg.generate_wasm_wit();
        assert!(wit.contains("record point {"));
        assert!(wit.contains("x: float64"));
    }

    // ── Type overrides ────────────────────────────────────────────

    #[test]
    fn type_override() {
        let mut fg = FfiGenerator::new();
        fg.add_type_override("i32", "MyInt");
        fg.add_function(sample_func());
        let decls = fg.generate_extern_decls();
        assert!(decls.contains("MyInt"));
    }

    // ── Stats ─────────────────────────────────────────────────────

    #[test]
    fn stats() {
        let mut fg = FfiGenerator::new();
        fg.add_function(sample_func());
        fg.add_struct(sample_struct());
        let s = fg.stats();
        assert!(s.contains("\"functions\":1"));
        assert!(s.contains("\"structs\":1"));
    }

    // ── struct + array types ──────────────────────────────────────

    #[test]
    fn array_type_conversions() {
        let arr = ForeignType::Array(Box::new(ForeignType::UInt(8)), 256);
        assert_eq!(arr.to_c_type(), "uint8_t[256]");
        assert_eq!(arr.to_redox_type(), "[u8; 256]");
        assert_eq!(arr.to_python_type(), "list[int]");
    }

    #[test]
    fn opaque_type() {
        let o = ForeignType::Opaque("FILE".into());
        assert_eq!(o.to_c_type(), "FILE");
        assert_eq!(o.to_redox_type(), "FILE");
    }
}

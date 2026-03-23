// redox_ffi: Zero-friction FFI — auto-binding generation for
// C, C++, Python, WASM, and CUDA foreign headers.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Target language
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FfiLang {
    C,
    Cpp,
    Python,
    Wasm,
    Cuda,
}

impl FfiLang {
    pub fn label(self) -> &'static str {
        match self {
            Self::C => "C",
            Self::Cpp => "C++",
            Self::Python => "Python",
            Self::Wasm => "WASM",
            Self::Cuda => "CUDA",
        }
    }

    pub fn file_extension(self) -> &'static str {
        match self {
            Self::C => ".h",
            Self::Cpp => ".hpp",
            Self::Python => ".pyi",
            Self::Wasm => ".wit",
            Self::Cuda => ".cuh",
        }
    }
}

// ---------------------------------------------------------------------------
// Type mapping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeMapping {
    pub foreign_type: String,
    pub redox_type: String,
    pub is_pointer: bool,
    pub is_nullable: bool,
}

// ---------------------------------------------------------------------------
// FFI signature
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfiParam {
    pub name: String,
    pub type_mapping: TypeMapping,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfiSignature {
    pub foreign_name: String,
    pub redox_name: String,
    pub params: Vec<FfiParam>,
    pub return_type: Option<TypeMapping>,
    pub is_unsafe: bool,
}

// ---------------------------------------------------------------------------
// Header descriptor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderDescriptor {
    pub path: String,
    pub lang: FfiLang,
    pub functions: Vec<FfiSignature>,
}

// ---------------------------------------------------------------------------
// Binding options
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingOptions {
    pub generate_safe_wrappers: bool,
    pub prefix: Option<String>,
    pub blocklist: Vec<String>,
}

impl Default for BindingOptions {
    fn default() -> Self {
        Self { generate_safe_wrappers: true, prefix: None, blocklist: Vec::new() }
    }
}

// ---------------------------------------------------------------------------
// Generated binding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedBinding {
    pub module_name: String,
    pub lang: FfiLang,
    pub code: String,
    pub signature_count: usize,
}

// ---------------------------------------------------------------------------
// Binding error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingError {
    UnsupportedType(String),
    ParseError(String),
    EmptyHeader(String),
    BlockedFunction(String),
}

impl std::fmt::Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedType(t) => write!(f, "unsupported type: {t}"),
            Self::ParseError(m) => write!(f, "parse error: {m}"),
            Self::EmptyHeader(p) => write!(f, "empty header: {p}"),
            Self::BlockedFunction(n) => write!(f, "blocked function: {n}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Code generators (per language)
// ---------------------------------------------------------------------------

fn generate_extern_fn(sig: &FfiSignature) -> String {
    let params: Vec<String> = sig.params.iter().map(|p| {
        let ty = if p.type_mapping.is_pointer {
            format!("*mut {}", p.type_mapping.redox_type)
        } else {
            p.type_mapping.redox_type.clone()
        };
        format!("{}: {}", p.name, ty)
    }).collect();
    let ret = sig.return_type.as_ref().map_or(String::new(), |r| {
        format!(" -> {}", r.redox_type)
    });
    format!("    fn {}({}){};", sig.redox_name, params.join(", "), ret)
}

fn generate_safe_wrapper(sig: &FfiSignature) -> String {
    let params: Vec<String> = sig.params.iter().map(|p| {
        format!("{}: {}", p.name, p.type_mapping.redox_type)
    }).collect();
    let ret = sig.return_type.as_ref().map_or(String::new(), |r| {
        format!(" -> {}", r.redox_type)
    });
    let args: Vec<String> = sig.params.iter().map(|p| p.name.clone()).collect();
    format!(
        "pub fn {}({}){} {{\n    unsafe {{ {}({}) }}\n}}",
        sig.redox_name, params.join(", "), ret, sig.redox_name, args.join(", ")
    )
}

// ---------------------------------------------------------------------------
// Binding generator
// ---------------------------------------------------------------------------

pub fn generate_binding(
    header: &HeaderDescriptor,
    opts: &BindingOptions,
) -> Result<GeneratedBinding, BindingError> {
    if header.functions.is_empty() {
        return Err(BindingError::EmptyHeader(header.path.clone()));
    }

    let mut extern_fns = Vec::new();
    let mut wrappers = Vec::new();
    let mut count = 0usize;

    for sig in &header.functions {
        if opts.blocklist.contains(&sig.foreign_name) {
            continue;
        }
        extern_fns.push(generate_extern_fn(sig));
        if opts.generate_safe_wrappers && sig.is_unsafe {
            wrappers.push(generate_safe_wrapper(sig));
        }
        count += 1;
    }

    if count == 0 {
        return Err(BindingError::EmptyHeader(header.path.clone()));
    }

    let link_name = header.path.replace('\\', "/").rsplit('/').next()
        .unwrap_or(&header.path).replace(header.lang.file_extension(), "");

    let mut code = format!("// Auto-generated FFI bindings for {}\n", header.path);
    code.push_str(&format!("// Language: {}\n\n", header.lang.label()));

    if let Some(ref pfx) = opts.prefix {
        code.push_str(&format!("// Prefix: {pfx}\n\n"));
    }

    code.push_str(&format!("#[link(name = \"{link_name}\")]\nextern \"C\" {{\n"));
    for f in &extern_fns {
        code.push_str(f);
        code.push('\n');
    }
    code.push_str("}\n");

    if !wrappers.is_empty() {
        code.push_str("\n// Safe wrappers\n");
        for w in &wrappers {
            code.push_str(w);
            code.push_str("\n\n");
        }
    }

    Ok(GeneratedBinding {
        module_name: link_name,
        lang: header.lang,
        code,
        signature_count: count,
    })
}

// ---------------------------------------------------------------------------
// Batch binding generator
// ---------------------------------------------------------------------------

pub fn generate_bindings(
    headers: &[HeaderDescriptor],
    opts: &BindingOptions,
) -> Vec<Result<GeneratedBinding, BindingError>> {
    headers.iter().map(|h| generate_binding(h, opts)).collect()
}

// ---------------------------------------------------------------------------
// Built-in type maps
// ---------------------------------------------------------------------------

pub fn default_c_types() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("int", "i32");
    m.insert("unsigned int", "u32");
    m.insert("long", "i64");
    m.insert("float", "f32");
    m.insert("double", "f64");
    m.insert("char", "i8");
    m.insert("void", "()");
    m.insert("size_t", "usize");
    m
}

pub fn default_python_types() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("int", "i64");
    m.insert("float", "f64");
    m.insert("str", "String");
    m.insert("bool", "bool");
    m.insert("bytes", "Vec<u8>");
    m.insert("None", "()");
    m
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfiSummary {
    pub total_headers: usize,
    pub total_signatures: usize,
    pub by_lang: HashMap<FfiLang, usize>,
}

pub fn summarize_bindings(bindings: &[GeneratedBinding]) -> FfiSummary {
    let mut by_lang: HashMap<FfiLang, usize> = HashMap::new();
    let mut total_sigs = 0usize;
    for b in bindings {
        *by_lang.entry(b.lang).or_insert(0) += b.signature_count;
        total_sigs += b.signature_count;
    }
    FfiSummary { total_headers: bindings.len(), total_signatures: total_sigs, by_lang }
}

// ---------------------------------------------------------------------------
// Pre-built example
// ---------------------------------------------------------------------------

fn sample_c_header() -> HeaderDescriptor {
    HeaderDescriptor {
        path: "math.h".to_string(),
        lang: FfiLang::C,
        functions: vec![
            FfiSignature {
                foreign_name: "sqrt".to_string(),
                redox_name: "c_sqrt".to_string(),
                params: vec![FfiParam {
                    name: "x".to_string(),
                    type_mapping: TypeMapping { foreign_type: "double".into(), redox_type: "f64".into(), is_pointer: false, is_nullable: false },
                }],
                return_type: Some(TypeMapping { foreign_type: "double".into(), redox_type: "f64".into(), is_pointer: false, is_nullable: false }),
                is_unsafe: true,
            },
            FfiSignature {
                foreign_name: "abs".to_string(),
                redox_name: "c_abs".to_string(),
                params: vec![FfiParam {
                    name: "n".to_string(),
                    type_mapping: TypeMapping { foreign_type: "int".into(), redox_type: "i32".into(), is_pointer: false, is_nullable: false },
                }],
                return_type: Some(TypeMapping { foreign_type: "int".into(), redox_type: "i32".into(), is_pointer: false, is_nullable: false }),
                is_unsafe: true,
            },
        ],
    }
}

fn sample_cuda_header() -> HeaderDescriptor {
    HeaderDescriptor {
        path: "kernels.cuh".to_string(),
        lang: FfiLang::Cuda,
        functions: vec![
            FfiSignature {
                foreign_name: "launch_kernel".to_string(),
                redox_name: "cuda_launch_kernel".to_string(),
                params: vec![
                    FfiParam {
                        name: "data".to_string(),
                        type_mapping: TypeMapping { foreign_type: "float*".into(), redox_type: "f32".into(), is_pointer: true, is_nullable: false },
                    },
                    FfiParam {
                        name: "len".to_string(),
                        type_mapping: TypeMapping { foreign_type: "size_t".into(), redox_type: "usize".into(), is_pointer: false, is_nullable: false },
                    },
                ],
                return_type: None,
                is_unsafe: true,
            },
        ],
    }
}

pub fn build_sample_headers() -> Vec<HeaderDescriptor> {
    vec![sample_c_header(), sample_cuda_header()]
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- FfiLang --
    #[test]
    fn test_lang_label() {
        assert_eq!(FfiLang::C.label(), "C");
        assert_eq!(FfiLang::Cuda.label(), "CUDA");
    }

    #[test]
    fn test_lang_extension() {
        assert_eq!(FfiLang::C.file_extension(), ".h");
        assert_eq!(FfiLang::Python.file_extension(), ".pyi");
    }

    // -- TypeMapping --
    #[test]
    fn test_type_mapping() {
        let tm = TypeMapping { foreign_type: "int".into(), redox_type: "i32".into(), is_pointer: false, is_nullable: false };
        assert_eq!(tm.redox_type, "i32");
    }

    // -- generate_binding --
    #[test]
    fn test_generate_c_binding() {
        let header = sample_c_header();
        let opts = BindingOptions::default();
        let binding = generate_binding(&header, &opts).unwrap();
        assert_eq!(binding.lang, FfiLang::C);
        assert_eq!(binding.signature_count, 2);
        assert!(binding.code.contains("extern \"C\""));
    }

    #[test]
    fn test_generate_cuda_binding() {
        let header = sample_cuda_header();
        let opts = BindingOptions::default();
        let binding = generate_binding(&header, &opts).unwrap();
        assert_eq!(binding.lang, FfiLang::Cuda);
        assert!(binding.code.contains("cuda_launch_kernel"));
    }

    #[test]
    fn test_empty_header_error() {
        let header = HeaderDescriptor { path: "empty.h".into(), lang: FfiLang::C, functions: vec![] };
        let err = generate_binding(&header, &BindingOptions::default()).unwrap_err();
        assert_eq!(err, BindingError::EmptyHeader("empty.h".into()));
    }

    #[test]
    fn test_blocklist() {
        let header = sample_c_header();
        let opts = BindingOptions { blocklist: vec!["sqrt".into(), "abs".into()], ..Default::default() };
        let err = generate_binding(&header, &opts).unwrap_err();
        assert!(matches!(err, BindingError::EmptyHeader(_)));
    }

    #[test]
    fn test_prefix_in_code() {
        let header = sample_c_header();
        let opts = BindingOptions { prefix: Some("my_".into()), ..Default::default() };
        let binding = generate_binding(&header, &opts).unwrap();
        assert!(binding.code.contains("Prefix: my_"));
    }

    #[test]
    fn test_safe_wrapper_generated() {
        let header = sample_c_header();
        let opts = BindingOptions { generate_safe_wrappers: true, ..Default::default() };
        let binding = generate_binding(&header, &opts).unwrap();
        assert!(binding.code.contains("Safe wrappers"));
    }

    #[test]
    fn test_no_safe_wrappers() {
        let header = sample_c_header();
        let opts = BindingOptions { generate_safe_wrappers: false, ..Default::default() };
        let binding = generate_binding(&header, &opts).unwrap();
        assert!(!binding.code.contains("Safe wrappers"));
    }

    // -- generate_bindings --
    #[test]
    fn test_batch_generate() {
        let headers = build_sample_headers();
        let results = generate_bindings(&headers, &BindingOptions::default());
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    // -- default type maps --
    #[test]
    fn test_c_types() {
        let m = default_c_types();
        assert_eq!(m.get("int"), Some(&"i32"));
        assert_eq!(m.get("double"), Some(&"f64"));
    }

    #[test]
    fn test_python_types() {
        let m = default_python_types();
        assert_eq!(m.get("int"), Some(&"i64"));
        assert_eq!(m.get("str"), Some(&"String"));
    }

    // -- summarize_bindings --
    #[test]
    fn test_summarize() {
        let headers = build_sample_headers();
        let bindings: Vec<GeneratedBinding> = generate_bindings(&headers, &BindingOptions::default())
            .into_iter().filter_map(|r| r.ok()).collect();
        let summary = summarize_bindings(&bindings);
        assert_eq!(summary.total_headers, 2);
        assert_eq!(summary.total_signatures, 3);
    }

    // -- BindingError display --
    #[test]
    fn test_error_display() {
        let e = BindingError::UnsupportedType("void*".into());
        assert_eq!(format!("{e}"), "unsupported type: void*");
    }

    #[test]
    fn test_parse_error_display() {
        let e = BindingError::ParseError("bad syntax".into());
        assert!(format!("{e}").contains("parse error"));
    }

    // -- partial blocklist --
    #[test]
    fn test_partial_blocklist() {
        let header = sample_c_header();
        let opts = BindingOptions { blocklist: vec!["sqrt".into()], ..Default::default() };
        let binding = generate_binding(&header, &opts).unwrap();
        assert_eq!(binding.signature_count, 1);
        assert!(binding.code.contains("c_abs"));
        assert!(!binding.code.contains("c_sqrt"));
    }

    // -- pointer type in extern --
    #[test]
    fn test_pointer_in_extern() {
        let header = sample_cuda_header();
        let binding = generate_binding(&header, &BindingOptions::default()).unwrap();
        assert!(binding.code.contains("*mut f32"));
    }

    // -- module name extraction --
    #[test]
    fn test_module_name() {
        let header = sample_c_header();
        let binding = generate_binding(&header, &BindingOptions::default()).unwrap();
        assert_eq!(binding.module_name, "math");
    }

    // -- build_sample_headers --
    #[test]
    fn test_sample_headers() {
        let headers = build_sample_headers();
        assert_eq!(headers.len(), 2);
    }

    // -- BindingOptions default --
    #[test]
    fn test_binding_options_default() {
        let opts = BindingOptions::default();
        assert!(opts.generate_safe_wrappers);
        assert!(opts.prefix.is_none());
        assert!(opts.blocklist.is_empty());
    }
}

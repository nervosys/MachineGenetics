// multilang-bindings — Cross-Language FFI Bridge Generator.
//
// Define a library API once in MAGE, then automatically generate
// bindings for C, C++, Python, and WebAssembly. Each target gets
// type-safe wrappers, proper memory management annotations, and
// a summary of the generated binding surface.
//
// Demonstrates:
//   - FFI language targets and type mappings
//   - Header descriptor parsing
//   - Binding generation with safe wrappers
//   - Multi-target code generation
//   - Effect annotations (/ io)
//   - Contract specs for type safety
//   - Enum-based dispatch per target language

use std::col;
use std::fmt;
use std::io;

// ─────────────────────────────────────────────────────────────────────
// §1 — Target languages and their type systems
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub data FfiTarget {
    C,
    Cpp,
    Python,
    Wasm,
}

extend FfiTarget {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FfiTarget::C      => write!(f, "C"),
            FfiTarget::Cpp    => write!(f, "C++"),
            FfiTarget::Python => write!(f, "Python"),
            FfiTarget::Wasm   => write!(f, "WebAssembly"),
        }
    }
}

extend FfiTarget {
    pub fn file_extension(&self) -> &String {
        match self {
            FfiTarget::C      => "h",
            FfiTarget::Cpp    => "hpp",
            FfiTarget::Python => "py",
            FfiTarget::Wasm   => "wat",
        }
    }

    pub fn comment_prefix(&self) -> &String {
        match self {
            FfiTarget::C | FfiTarget::Cpp => "//",
            FfiTarget::Python            => "#",
            FfiTarget::Wasm              => ";;",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// §2 — Type mapping: MAGE types → foreign types
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data TypeMapping {
    MAGE_type: String,
    foreign_type: String,
    is_pointer: bool,
    is_nullable: bool,
    needs_free: bool,
}

extend TypeMapping {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        val ptr = if self.is_pointer { "*" } else { "" };
        val null = if self.is_nullable { "?" } else { "" };
        write!(f, "{mg} → {ptr}{foreign}{null}",
            mg = self.MAGE_type,
            ptr = ptr,
            foreign = self.foreign_type,
            null = null)
    }
}

/// Build type mappings for a given target language.
///
/// @req  target is a valid FfiTarget
/// @ens  result contains mappings for all common MAGE types
/// @fx   pure
fn type_mappings_for(target: &FfiTarget) -> [TypeMapping]~ {
    match target {
        FfiTarget::C => vec![
            TypeMapping { MAGE_type: "i32".into(), foreign_type: "int32_t".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "i64".into(), foreign_type: "int64_t".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "f64".into(), foreign_type: "double".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "bool".into(), foreign_type: "bool".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "String".into(), foreign_type: "char".into(), is_pointer: true, is_nullable: false, needs_free: true },
            TypeMapping { MAGE_type: "&str".into(), foreign_type: "char".into(), is_pointer: true, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "?String".into(), foreign_type: "char".into(), is_pointer: true, is_nullable: true, needs_free: true },
            TypeMapping { MAGE_type: "[u8]~".into(), foreign_type: "uint8_t".into(), is_pointer: true, is_nullable: false, needs_free: true },
        ],
        FfiTarget::Cpp => vec![
            TypeMapping { MAGE_type: "i32".into(), foreign_type: "int32_t".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "i64".into(), foreign_type: "int64_t".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "f64".into(), foreign_type: "double".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "bool".into(), foreign_type: "bool".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "String".into(), foreign_type: "std::string".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "&str".into(), foreign_type: "std::string_view".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "?String".into(), foreign_type: "std::optional<std::string>".into(), is_pointer: false, is_nullable: true, needs_free: false },
            TypeMapping { MAGE_type: "[u8]~".into(), foreign_type: "std::vector<uint8_t>".into(), is_pointer: false, is_nullable: false, needs_free: false },
        ],
        FfiTarget::Python => vec![
            TypeMapping { MAGE_type: "i32".into(), foreign_type: "int".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "i64".into(), foreign_type: "int".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "f64".into(), foreign_type: "float".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "bool".into(), foreign_type: "bool".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "String".into(), foreign_type: "str".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "&str".into(), foreign_type: "str".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "?String".into(), foreign_type: "Optional[str]".into(), is_pointer: false, is_nullable: true, needs_free: false },
            TypeMapping { MAGE_type: "[u8]~".into(), foreign_type: "bytes".into(), is_pointer: false, is_nullable: false, needs_free: false },
        ],
        FfiTarget::Wasm => vec![
            TypeMapping { MAGE_type: "i32".into(), foreign_type: "i32".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "i64".into(), foreign_type: "i64".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "f64".into(), foreign_type: "f64".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "bool".into(), foreign_type: "i32".into(), is_pointer: false, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "String".into(), foreign_type: "i32".into(), is_pointer: true, is_nullable: false, needs_free: true },
            TypeMapping { MAGE_type: "&str".into(), foreign_type: "i32".into(), is_pointer: true, is_nullable: false, needs_free: false },
            TypeMapping { MAGE_type: "?String".into(), foreign_type: "i32".into(), is_pointer: true, is_nullable: true, needs_free: true },
            TypeMapping { MAGE_type: "[u8]~".into(), foreign_type: "i32".into(), is_pointer: true, is_nullable: false, needs_free: true },
        ],
    }
}

// ─────────────────────────────────────────────────────────────────────
// §3 — Function signatures to export
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data FfiParam {
    name: String,
    MAGE_type: String,
}

#[derive(Debug, Clone)]
pub data FfiFunction {
    name: String,
    params: [FfiParam]~,
    return_type: String,
    is_unsafe: bool,
    doc: String,
}

extend FfiFunction {
    pub fn new(name: String, doc: String) -> FfiFunction {
        FfiFunction {
            name: name,
            params: []~.new(),
            return_type: "()".to_string(),
            is_unsafe: false,
            doc: doc,
        }
    }

    pub fn param(mut self, name: String, ty: String) -> FfiFunction {
        self.params.push(FfiParam { name: name, MAGE_type: ty });
        self
    }

    pub fn returns(mut self, ty: String) -> FfiFunction {
        self.return_type = ty;
        self
    }

    pub fn unsafe_fn(mut self) -> FfiFunction {
        self.is_unsafe = true;
        self
    }
}

// ─────────────────────────────────────────────────────────────────────
// §4 — Library API: the interface we're exporting
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data LibraryApi {
    name: String,
    version: String,
    functions: [FfiFunction]~,
}

/// Define a sample image-processing library API.
///
/// @fx pure
fn define_image_api() -> LibraryApi {
    LibraryApi {
        name: "MAGE_image".to_string(),
        version: "1.0.0".to_string(),
        functions: vec![
            FfiFunction.new("image_open".to_string(), "Open an image file".to_string())
                .param("path".to_string(), "&str".to_string())
                .returns("?ImageHandle".to_string()),

            FfiFunction.new("image_width".to_string(), "Get image width in pixels".to_string())
                .param("handle".to_string(), "ImageHandle".to_string())
                .returns("i32".to_string()),

            FfiFunction.new("image_height".to_string(), "Get image height in pixels".to_string())
                .param("handle".to_string(), "ImageHandle".to_string())
                .returns("i32".to_string()),

            FfiFunction.new("image_resize".to_string(), "Resize image to target dimensions".to_string())
                .param("handle".to_string(), "ImageHandle".to_string())
                .param("width".to_string(), "i32".to_string())
                .param("height".to_string(), "i32".to_string())
                .returns("bool".to_string()),

            FfiFunction.new("image_to_grayscale".to_string(), "Convert image to grayscale".to_string())
                .param("handle".to_string(), "ImageHandle".to_string())
                .returns("bool".to_string()),

            FfiFunction.new("image_pixels".to_string(), "Get raw pixel data".to_string())
                .param("handle".to_string(), "ImageHandle".to_string())
                .returns("[u8]~".to_string())
                .unsafe_fn(),

            FfiFunction.new("image_save".to_string(), "Save image to file".to_string())
                .param("handle".to_string(), "ImageHandle".to_string())
                .param("path".to_string(), "&str".to_string())
                .param("format".to_string(), "&str".to_string())
                .returns("bool".to_string()),

            FfiFunction.new("image_close".to_string(), "Close and free image resources".to_string())
                .param("handle".to_string(), "ImageHandle".to_string())
                .returns("()".to_string()),
        ],
    }
}

// ─────────────────────────────────────────────────────────────────────
// §5 — Binding generator: produce output per target
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data GeneratedBinding {
    target: FfiTarget,
    filename: String,
    code: String,
    function_count: usize,
    line_count: usize,
}

extend GeneratedBinding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{target} → {file} ({funcs} functions, {lines} lines)",
            target = self.target,
            file = self.filename,
            funcs = self.function_count,
            lines = self.line_count)
    }
}

/// Look up the foreign type for a given MAGE type in a target.
///
/// @fx pure
fn map_type(MAGE_type: &String, mappings: &[TypeMapping]~) -> String {
    for mapping in mappings {
        if mapping.MAGE_type == *MAGE_type {
            val prefix = if mapping.is_pointer { "*" } else { "" };
            return format!("{prefix}{}", mapping.foreign_type);
        }
    }
    // Opaque handle — return as void pointer in C, int in Python, etc.
    format!("/* opaque */ void*")
}

/// Generate a C header file.
///
/// @ens  result.code contains proper extern "C" declarations
/// @fx   pure
fn generate_c_binding(api: &LibraryApi, prefix: &String) -> GeneratedBinding {
    var lines: [String]~ = []~.new();

    lines.push(format!("/* Auto-generated C bindings for {} v{} */", api.name, api.version));
    lines.push(format!("#ifndef {}{}_{}", prefix.to_uppercase(), api.name.to_uppercase(), "H"));
    lines.push(format!("#define {}{}_{}", prefix.to_uppercase(), api.name.to_uppercase(), "H"));
    lines.push("".to_string());
    lines.push("#include <stdint.h>".to_string());
    lines.push("#include <stdbool.h>".to_string());
    lines.push("".to_string());
    lines.push("typedef void* ImageHandle;".to_string());
    lines.push("".to_string());
    lines.push("#ifdef __cplusplus".to_string());
    lines.push("extern \"C\" {".to_string());
    lines.push("#endif".to_string());
    lines.push("".to_string());

    val mappings = type_mappings_for(&FfiTarget::C);

    for func in &api.functions {
        lines.push(format!("/* {} */", func.doc));
        val ret_type = map_type(&func.return_type, &mappings);
        var params_str: [String]~ = []~.new();
        for param in &func.params {
            val ty = map_type(&param.MAGE_type, &mappings);
            params_str.push(format!("{} {}", ty, param.name));
        }
        val params = if params_str.is_empty() { "void".to_string() } else { params_str.join(", ") };
        lines.push(format!("{} {}{}({});", ret_type, prefix, func.name, params));
        lines.push("".to_string());
    }

    lines.push("#ifdef __cplusplus".to_string());
    lines.push("}".to_string());
    lines.push("#endif".to_string());
    lines.push("".to_string());
    lines.push(format!("#endif /* {}{}_{} */", prefix.to_uppercase(), api.name.to_uppercase(), "H"));

    val code = lines.join("\n");
    val lc = lines.len();
    GeneratedBinding {
        target: FfiTarget::C,
        filename: format!("{}{}.h", prefix, api.name),
        code: code,
        function_count: api.functions.len(),
        line_count: lc,
    }
}

/// Generate a Python module using ctypes.
///
/// @fx pure
fn generate_python_binding(api: &LibraryApi, prefix: &String) -> GeneratedBinding {
    var lines: [String]~ = []~.new();

    lines.push(format!("# Auto-generated Python bindings for {} v{}", api.name, api.version));
    lines.push(format!("# Generated by MAGE FFI bridge"));
    lines.push("".to_string());
    lines.push("import ctypes".to_string());
    lines.push("from typing import Optional".to_string());
    lines.push("from pathlib import Path".to_string());
    lines.push("".to_string());
    lines.push(format!("_lib = ctypes.CDLL(\"lib{}{}.so\")", prefix, api.name));
    lines.push("".to_string());

    val mappings = type_mappings_for(&FfiTarget::Python);
    val ctypes_map = vec![
        ("int", "ctypes.c_int32"),
        ("float", "ctypes.c_double"),
        ("bool", "ctypes.c_bool"),
        ("str", "ctypes.c_char_p"),
        ("bytes", "ctypes.POINTER(ctypes.c_uint8)"),
    ];

    for func in &api.functions {
        lines.push(format!("# {}", func.doc));
        lines.push(format!("def {}(", func.name));
        for (i, param) in func.params.iter().enumerate() {
            val py_type = map_type(&param.MAGE_type, &mappings);
            val comma = if i < func.params.len() - 1 { "," } else { "" };
            lines.push(format!("    {}: {}{}", param.name, py_type, comma));
        }
        val ret_py = map_type(&func.return_type, &mappings);
        lines.push(format!(") -> {}:", ret_py));
        lines.push(format!("    return _lib.{}{}(", prefix, func.name));
        for (i, param) in func.params.iter().enumerate() {
            val comma = if i < func.params.len() - 1 { "," } else { "" };
            lines.push(format!("        {}{}", param.name, comma));
        }
        lines.push("    )".to_string());
        lines.push("".to_string());
    }

    val code = lines.join("\n");
    val lc = lines.len();
    GeneratedBinding {
        target: FfiTarget::Python,
        filename: format!("{}.py", api.name),
        code: code,
        function_count: api.functions.len(),
        line_count: lc,
    }
}

/// Generate WebAssembly text format imports.
///
/// @fx pure
fn generate_wasm_binding(api: &LibraryApi, prefix: &String) -> GeneratedBinding {
    var lines: [String]~ = []~.new();

    lines.push(format!(";; Auto-generated WASM imports for {} v{}", api.name, api.version));
    lines.push("(module".to_string());
    lines.push(format!("  ;; Import host functions from \"{}\"", api.name));
    lines.push("".to_string());

    val mappings = type_mappings_for(&FfiTarget::Wasm);

    for func in &api.functions {
        lines.push(format!("  ;; {}", func.doc));
        var params_wasm: [String]~ = []~.new();
        for param in &func.params {
            val wasm_ty = map_type(&param.MAGE_type, &mappings);
            params_wasm.push(format!("(param ${} {})", param.name, wasm_ty));
        }
        val ret_wasm = map_type(&func.return_type, &mappings);
        val result = if func.return_type == "()" { "".to_string() } else { format!(" (result {})", ret_wasm) };
        val params = params_wasm.join(" ");
        lines.push(format!("  (import \"{}\" \"{}{}\" (func ${} {}{}))", api.name, prefix, func.name, func.name, params, result));
        lines.push("".to_string());
    }

    lines.push(")".to_string());

    val code = lines.join("\n");
    val lc = lines.len();
    GeneratedBinding {
        target: FfiTarget::Wasm,
        filename: format!("{}.wat", api.name),
        code: code,
        function_count: api.functions.len(),
        line_count: lc,
    }
}

// ─────────────────────────────────────────────────────────────────────
// §6 — Entry point: generate bindings for all targets
// ─────────────────────────────────────────────────────────────────────

pub fn main() / io {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║  MAGE Multi-Language FFI Bridge Generator                ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!("");

    // Define the API.
    val api = define_image_api();
    val prefix = "mg_";

    println!("Library: {} v{}", api.name, api.version);
    println!("Functions: {}", api.functions.len());
    println!("Prefix: {}", prefix);
    println!("");

    println!("─── API Surface ──────────────────────────────────────────");
    for func in &api.functions {
        val params: [String]~ = func.params.iter()
            .map(|p| format!("{}: {}", p.name, p.MAGE_type))
            .collect();
        val unsafe_tag = if func.is_unsafe { " [unsafe]" } else { "" };
        println!("  pub fn {}({}) -> {}{}",
            func.name,
            params.join(", "),
            func.return_type,
            unsafe_tag);
    }
    println!("");

    // Generate bindings for each target.
    val targets = vec![FfiTarget::C, FfiTarget::Python, FfiTarget::Wasm];
    var bindings: [GeneratedBinding]~ = []~.new();

    for target in &targets {
        println!("─── Generating {} Bindings ─────────────────────", target);

        val binding = match target {
            FfiTarget::C      => generate_c_binding(&api, &prefix.to_string()),
            FfiTarget::Python => generate_python_binding(&api, &prefix.to_string()),
            FfiTarget::Wasm   => generate_wasm_binding(&api, &prefix.to_string()),
            _                => {
                println!("  Skipped: {} not yet supported", target);
                continue;
            },
        };

        println!("  {}", binding);
        println!("");

        // Show a preview (first 15 lines).
        val preview_lines: [&str]~ = binding.code.lines().take(15).collect();
        println!("  Preview:");
        for line in &preview_lines {
            println!("    {}", line);
        }
        if binding.line_count > 15 {
            println!("    ... ({} more lines)", binding.line_count - 15);
        }
        println!("");

        bindings.push(binding);
    }

    // Type mapping summary.
    println!("─── Type Mapping Summary ─────────────────────────────────");
    println!("  ┌──────────┬──────────────┬───────────────────────┬──────────┐");
    println!("  │ MAGE    │ C            │ Python                │ WASM     │");
    println!("  ├──────────┼──────────────┼───────────────────────┼──────────┤");
    val c_maps = type_mappings_for(&FfiTarget::C);
    val py_maps = type_mappings_for(&FfiTarget::Python);
    val wasm_maps = type_mappings_for(&FfiTarget::Wasm);
    for i in 0..c_maps.len() {
        println!("  │ {:<8} │ {:<12} │ {:<21} │ {:<8} │",
            c_maps[i].MAGE_type,
            c_maps[i].foreign_type,
            py_maps[i].foreign_type,
            wasm_maps[i].foreign_type);
    }
    println!("  └──────────┴──────────────┴───────────────────────┴──────────┘");

    // Final summary.
    println!("");
    println!("═══════════════════════════════════════════════════════════");
    val total_lines: usize = bindings.iter().map(|b| b.line_count).sum();
    val total_funcs: usize = bindings.iter().map(|b| b.function_count).sum();
    println!("  Generated {} binding files:", bindings.len());
    for b in &bindings {
        println!("    - {} ({})", b.filename, b.target);
    }
    println!("  Total: {} function bindings, {} lines of code", total_funcs, total_lines);
    println!("═══════════════════════════════════════════════════════════");
}

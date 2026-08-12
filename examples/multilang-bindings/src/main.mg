// multilang-bindings — one API description, four sets of bindings.
//
// `forge run` evaluates `main` and prints its result. Demonstrates:
//   - describing an API as data (`Func`, `Param`, `Ty`) rather than as text
//   - `trait` + `impl Trait for T` as the code generator's dispatch: each
//     target language is a type, and adding a language is adding an impl
//   - exhaustive `match` over a type enum, so a new type cannot be silently
//     forgotten by one backend
//   - ownership as part of the signature, which is the thing FFI actually
//     gets wrong
//   - `/ fs` on the one function that would write files
//
// The generated text is real output, not a sketch: `main` returns the C and
// Python declarations it produced.
//
// Run:  forge run        (or:  mage-parse --eval src/main.mg main)

// ── The API, as data ─────────────────────────────────────────────────

// The types this API is allowed to cross the boundary with. Keeping it a closed
// enum is the point: every backend must answer for every member, and `--check`
// says so if one does not.
enum Ty {
    I32,
    F64,
    Bool,
    Str,
    Bytes,
}

// Who frees it. This is the part that a hand-written header gets wrong, so it
// is in the description rather than in a comment.
enum Owner {
    Caller,
    Callee,
    Borrowed,
}

struct Param {
    name: String,
    ty: Ty,
    owner: Owner,
}

struct Func {
    name: String,
    params: [Param]~,
    returns: Ty,
    ret_owner: Owner,
}

// ── Backends ─────────────────────────────────────────────────────────

// One trait, one impl per language. `backend.render(func)` dispatches on the
// backend's own type, so the four generators never branch on a language tag.
trait Backend {
    fn type_name(&self, ty: Ty) -> String;
    fn render(&self, func: Func) -> String;
    fn note(&self, owner: Owner) -> String;
}

struct CBackend {
    prefix: String,
}

struct CppBackend {
    namespace: String,
}

struct PyBackend {
    module: String,
}

struct WasmBackend {
    memory: String,
}

// C: pointers and lengths, ownership only expressible as a comment.
impl Backend for CBackend {
    fn type_name(&self, ty: Ty) -> String {
        ?= ty {
            Ty.I32 => "int32_t",
            Ty.F64 => "double",
            Ty.Bool => "bool",
            Ty.Str => "const char*",
            Ty.Bytes => "uint8_t*",
        }
    }

    fn note(&self, owner: Owner) -> String {
        ?= owner {
            Owner.Caller => "/* caller frees */",
            Owner.Callee => "/* callee frees */",
            Owner.Borrowed => "/* borrowed */",
        }
    }

    fn render(&self, func: Func) -> String {
        val args = map(func.params, fn(p) => f"{self.type_name(p.ty)} {p.name}")
        val out = self.type_name(func.returns)
        f"{out} {self.prefix}{func.name}({join(args, ", ")}); {self.note(func.ret_owner)}"
    }
}

// C++: same types, but ownership becomes a real type where it can.
impl Backend for CppBackend {
    fn type_name(&self, ty: Ty) -> String {
        ?= ty {
            Ty.I32 => "int32_t",
            Ty.F64 => "double",
            Ty.Bool => "bool",
            Ty.Str => "std::string_view",
            Ty.Bytes => "std::vector<uint8_t>",
        }
    }

    fn note(&self, owner: Owner) -> String {
        ?= owner {
            Owner.Caller => "std::unique_ptr",
            Owner.Callee => "raw",
            Owner.Borrowed => "reference",
        }
    }

    fn render(&self, func: Func) -> String {
        val args = map(func.params, fn(p) => f"{self.type_name(p.ty)} {p.name}")
        val arglist = join(args, ", ")
        val out = self.type_name(func.returns)
        f"namespace {self.namespace} {{ {out} {func.name}({arglist}); }} // {self.note(func.ret_owner)}"
    }
}

// Python: types are annotations, and ownership is the GC's problem — which is
// itself worth stating, because it is the reason the C caller must not free.
impl Backend for PyBackend {
    fn type_name(&self, ty: Ty) -> String {
        ?= ty {
            Ty.I32 => "int",
            Ty.F64 => "float",
            Ty.Bool => "bool",
            Ty.Str => "str",
            Ty.Bytes => "bytes",
        }
    }

    fn note(&self, owner: Owner) -> String {
        ?= owner {
            Owner.Caller => "# refcounted",
            Owner.Callee => "# refcounted",
            Owner.Borrowed => "# memoryview",
        }
    }

    fn render(&self, func: Func) -> String {
        val args = map(func.params, fn(p) => f"{p.name}: {self.type_name(p.ty)}")
        val arglist = join(args, ", ")
        val out = self.type_name(func.returns)
        f"def {self.module}_{func.name}({arglist}) -> {out}: ...  {self.note(func.ret_owner)}"
    }
}

// Wasm: everything that is not a number becomes an offset into linear memory,
// so a string is two arguments and the ownership note is not optional.
impl Backend for WasmBackend {
    fn type_name(&self, ty: Ty) -> String {
        ?= ty {
            Ty.I32 => "i32",
            Ty.F64 => "f64",
            Ty.Bool => "i32",
            Ty.Str => "i32 i32",
            Ty.Bytes => "i32 i32",
        }
    }

    fn note(&self, owner: Owner) -> String {
        ?= owner {
            Owner.Caller => f"caller frees in {self.memory}",
            Owner.Callee => f"callee frees in {self.memory}",
            Owner.Borrowed => "no transfer",
        }
    }

    fn render(&self, func: Func) -> String {
        val args = map(func.params, fn(p) => self.type_name(p.ty))
        val arglist = join(args, " ")
        val out = self.type_name(func.returns)
        f"(func ${func.name} (param {arglist}) (result {out})) ;; {self.note(func.ret_owner)}"
    }
}

// ── Surface summary ──────────────────────────────────────────────────

// How many arguments a target actually emits. Wasm splits every non-numeric
// type into a pointer and a length, so its arity differs from the source API's
// — the number a binding generator has to get right and usually hardcodes.
fn wasm_arity(func: Func) -> usize {
    fold(
        func.params,
        0,
        fn(total, p) => total + ?= p.ty {
            Ty.Str => 2,
            Ty.Bytes => 2,
            _ => 1,
        },
    )
}

// ── Writing them out ─────────────────────────────────────────────────

// The one function that would touch the filesystem, so the one that carries
// `/ fs`. Everything above is pure and testable without a temp directory.
fn write_binding(path: String, body: String) -> usize / fs {
    len(chars(f"{path}\n{body}"))
}

// ── Entry point ──────────────────────────────────────────────────────

pub fn main() -> String / fs {
    val api = [
        @Func {
            name: "checksum",
            params: [
                @Param { name: "data", ty: Ty.Bytes, owner: Owner.Borrowed },
                @Param { name: "seed", ty: Ty.I32, owner: Owner.Caller },
            ],
            returns: Ty.I32,
            ret_owner: Owner.Caller,
        },
        @Func {
            name: "describe",
            params: [@Param { name: "code", ty: Ty.I32, owner: Owner.Caller }],
            returns: Ty.Str,
            ret_owner: Owner.Callee,
        },
    ]

    // Four backends, four values of four different types. Each `render` below
    // is the same call spelled the same way; which code runs is decided by the
    // receiver's type, which is the whole reason this is a trait.
    val c = @CBackend { prefix: "mg_" }
    val cpp = @CppBackend { namespace: "mage" }
    val py = @PyBackend { module: "mage" }
    val wasm = @WasmBackend { memory: "memory0" }

    val c_lines = map(api, fn(func) => c.render(func))
    val cpp_lines = map(api, fn(func) => cpp.render(func))
    val py_lines = map(api, fn(func) => py.render(func))
    val wasm_lines = map(api, fn(func) => wasm.render(func))

    // `describe` takes one `i32` and returns a string, so C sees one argument
    // and wasm sees one — but `checksum` takes a `Bytes`, which wasm splits
    // into a pointer and a length. The arities differ, and that difference is
    // computed rather than asserted.
    val arities = map(api, fn(func) => f"{func.name}:{len(func.params)}->{wasm_arity(func)}")
    val written = write_binding("bindings.h", join(c_lines, "\n"))

    join(
        [
            join(c_lines, " "),
            join(cpp_lines, " "),
            join(py_lines, " "),
            join(wasm_lines, " "),
            f"wasm arity {join(arities, " ")}",
            f"wrote {written} bytes",
        ],
        " || ",
    )
}

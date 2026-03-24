// live-compiler — Hot-Reload Development Server with Self-Healing.
//
// A development server that watches source files, incrementally
// recompiles on change, hot-patches running functions without restart,
// and automatically proposes fixes for common errors. Includes a
// rollback mechanism when patches introduce regressions.
//
// Demonstrates:
//   - Hot-reload runtime with function versioning
//   - Patch application and rollback
//   - Self-healing compiler diagnostics
//   - Repair candidate generation and confidence scoring
//   - Async file watching (/ io, / fs)
//   - Token budget monitoring
//   - Effect annotations throughout

use std::col;
use std::fmt;
use std::io;

// ─────────────────────────────────────────────────────────────────────
// §1 — Source file tracking
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SourceFile {
    path: String,
    content: String,
    version: u64,
    token_count: usize,
    last_modified: u64,
}

impl SourceFile {
    pub fn new(path: String, content: String) -> SourceFile {
        let tokens = count_tokens(&content);
        SourceFile {
            path: path,
            content: content,
            version: 1,
            token_count: tokens,
            last_modified: 0,
        }
    }

    pub fn update(&mut self, new_content: String) {
        self.content = new_content.clone();
        self.token_count = count_tokens(&new_content);
        self.version = self.version + 1;
        self.last_modified = self.last_modified + 1;
    }
}

/// Simple token counter — counts whitespace-separated tokens.
///
/// @fx pure
fn count_tokens(source: &String) -> usize {
    source.split_whitespace().count()
}

// ─────────────────────────────────────────────────────────────────────
// §2 — Diagnostics: errors the compiler finds
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    TypeMismatch,
    UndefinedVariable,
    MissingImport,
    SyntaxError,
    BorrowCheck,
    LifetimeError,
    UnusedVariable,
    AmbiguousType,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorKind::TypeMismatch     => write!(f, "type mismatch"),
            ErrorKind::UndefinedVariable => write!(f, "undefined variable"),
            ErrorKind::MissingImport    => write!(f, "missing import"),
            ErrorKind::SyntaxError      => write!(f, "syntax error"),
            ErrorKind::BorrowCheck      => write!(f, "borrow check"),
            ErrorKind::LifetimeError    => write!(f, "lifetime error"),
            ErrorKind::UnusedVariable   => write!(f, "unused variable"),
            ErrorKind::AmbiguousType    => write!(f, "ambiguous type"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Severity::Warning => write!(f, "warning"),
            Severity::Error   => write!(f, "error"),
        }
    }
}

impl ErrorKind {
    pub fn severity(&self) -> Severity {
        match self {
            ErrorKind::UnusedVariable => Severity::Warning,
            _ => Severity::Error,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    file: String,
    line: u32,
    column: u32,
    kind: ErrorKind,
    message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{sev}[{kind}]: {file}:{line}:{col} — {msg}",
            sev = self.kind.severity(),
            kind = self.kind,
            file = self.file,
            line = self.line,
            col = self.column,
            msg = self.message)
    }
}

// ─────────────────────────────────────────────────────────────────────
// §3 — Self-healing: propose fixes automatically
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RepairCandidate {
    description: String,
    patch_text: String,
    confidence: f64,
    kind: ErrorKind,
}

impl fmt::Display for RepairCandidate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{conf:.0}%] {desc}: {patch}",
            conf = self.confidence * 100.0,
            desc = self.description,
            patch = self.patch_text)
    }
}

/// Generate repair candidates for a diagnostic.
///
/// @req  diagnostic.kind.severity() == Severity::Error || Severity::Warning
/// @ens  result candidates sorted by confidence descending
/// @fx   pure
fn propose_repairs(diag: &Diagnostic) -> Vec<RepairCandidate> {
    let mut candidates: Vec<RepairCandidate> = Vec::new();

    match diag.kind {
        ErrorKind::TypeMismatch => {
            candidates.push(RepairCandidate {
                description: "Add explicit type cast".to_string(),
                patch_text: format!("({}) as TargetType", diag.message),
                confidence: 0.75,
                kind: ErrorKind::TypeMismatch,
            });
            candidates.push(RepairCandidate {
                description: "Use .into() conversion".to_string(),
                patch_text: "value.into()".to_string(),
                confidence: 0.60,
                kind: ErrorKind::TypeMismatch,
            });
        },
        ErrorKind::UndefinedVariable => {
            candidates.push(RepairCandidate {
                description: "Declare the variable".to_string(),
                patch_text: format!("let {}: _ = todo!();", diag.message),
                confidence: 0.50,
                kind: ErrorKind::UndefinedVariable,
            });
            candidates.push(RepairCandidate {
                description: "Check for typo in variable name".to_string(),
                patch_text: "// Did you mean a similar name?".to_string(),
                confidence: 0.70,
                kind: ErrorKind::UndefinedVariable,
            });
        },
        ErrorKind::MissingImport => {
            candidates.push(RepairCandidate {
                description: "Add import statement".to_string(),
                patch_text: format!("use std::{};", diag.message),
                confidence: 0.85,
                kind: ErrorKind::MissingImport,
            });
        },
        ErrorKind::UnusedVariable => {
            candidates.push(RepairCandidate {
                description: "Prefix with underscore".to_string(),
                patch_text: format!("let _{} = ...", diag.message),
                confidence: 0.95,
                kind: ErrorKind::UnusedVariable,
            });
            candidates.push(RepairCandidate {
                description: "Remove the variable".to_string(),
                patch_text: "// removed unused binding".to_string(),
                confidence: 0.80,
                kind: ErrorKind::UnusedVariable,
            });
        },
        _ => {
            candidates.push(RepairCandidate {
                description: "No automatic fix available".to_string(),
                patch_text: "// manual fix required".to_string(),
                confidence: 0.0,
                kind: diag.kind.clone(),
            });
        },
    };

    // Sort by confidence descending.
    candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
    candidates
}

// ─────────────────────────────────────────────────────────────────────
// §4 — Hot-reload runtime: patch functions without restart
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PatchStatus {
    Pending,
    Applied,
    RolledBack,
    Failed,
}

impl fmt::Display for PatchStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PatchStatus::Pending    => write!(f, "PENDING"),
            PatchStatus::Applied    => write!(f, "APPLIED"),
            PatchStatus::RolledBack => write!(f, "ROLLED BACK"),
            PatchStatus::Failed     => write!(f, "FAILED"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionPatch {
    id: u64,
    function_name: String,
    from_version: u64,
    to_version: u64,
    status: PatchStatus,
    description: String,
}

impl fmt::Display for FunctionPatch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Patch #{id}: {name} v{from}→v{to} [{status}]",
            id = self.id,
            name = self.function_name,
            from = self.from_version,
            to = self.to_version,
            status = self.status)
    }
}

#[derive(Debug)]
pub struct HotReloadRuntime {
    functions: HashMap<String, u64>,     // function name → current version
    patches: Vec<FunctionPatch>,
    rollbacks: Vec<String>,
    next_patch_id: u64,
}

impl HotReloadRuntime {
    pub fn new() -> HotReloadRuntime {
        HotReloadRuntime {
            functions: HashMap::new(),
            patches: Vec::new(),
            rollbacks: Vec::new(),
            next_patch_id: 1,
        }
    }

    pub fn register_function(&mut self, name: String, version: u64) {
        self.functions.insert(name, version);
    }

    /// Apply a hot patch to a running function.
    ///
    /// @req  self.functions.contains(function_name)
    /// @ens  patch is recorded in self.patches
    pub fn apply_patch(&mut self, function_name: &String, description: String) -> Result<u64, String> / io {
        let current = match self.functions.get(function_name) {
            Some(v) => *v,
            None => return Err(format!("Function '{}' not registered", function_name)),
        };

        let patch_id = self.next_patch_id;
        self.next_patch_id = self.next_patch_id + 1;
        let new_version = current + 1;

        let patch = FunctionPatch {
            id: patch_id,
            function_name: function_name.clone(),
            from_version: current,
            to_version: new_version,
            status: PatchStatus::Applied,
            description: description,
        };

        self.functions.insert(function_name.clone(), new_version);
        println!("    ⚡ {}", patch);
        self.patches.push(patch);
        Ok(patch_id)
    }

    /// Roll back the last patch for a function.
    ///
    /// @req  function has at least one applied patch
    pub fn rollback(&mut self, function_name: &String) -> Result<(), String> / io {
        // Find the last applied patch for this function.
        let mut found_idx: Option<usize> = None;
        let mut i = self.patches.len();
        for _ in 0..self.patches.len() {
            i = i - 1;
            if self.patches[i].function_name == *function_name
               && self.patches[i].status == PatchStatus::Applied {
                found_idx = Some(i);
                break;
            }
        }

        let idx = match found_idx {
            Some(i) => i,
            None => return Err(format!("No applied patches for '{}'", function_name)),
        };

        let patch = &mut self.patches[idx];
        patch.status = PatchStatus::RolledBack;
        self.functions.insert(function_name.clone(), patch.from_version);

        println!("    ↩ Rolled back: {} v{}→v{}", function_name, patch.to_version, patch.from_version);
        self.rollbacks.push(format!("Rolled back {}", function_name));
        Ok(())
    }

    pub fn version_of(&self, function_name: &String) -> Option<u64> {
        self.functions.get(function_name).copied()
    }

    pub fn patch_count(&self) -> usize {
        self.patches.len()
    }

    pub fn report(&self) / io {
        println!("");
        println!("── Hot-Reload Status ──────────────────────────────────");
        println!("  Registered functions: {}", self.functions.len());
        println!("  Applied patches:     {}", self.patches.iter().filter(|p| p.status == PatchStatus::Applied).count());
        println!("  Rolled back:         {}", self.rollbacks.len());
        println!("  ┌───────────────────────┬─────────┬────────────┐");
        println!("  │ Function              │ Version │ Status     │");
        println!("  ├───────────────────────┼─────────┼────────────┤");
        for (name, version) in &self.functions {
            println!("  │ {:<21} │ v{:<6} │ live       │", name, version);
        }
        println!("  └───────────────────────┴─────────┴────────────┘");
    }
}

// ─────────────────────────────────────────────────────────────────────
// §5 — Development server: ties it all together
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct DevServer {
    files: HashMap<String, SourceFile>,
    runtime: HotReloadRuntime,
    diagnostics: Vec<Diagnostic>,
    repairs_applied: u32,
    total_rebuilds: u32,
}

impl DevServer {
    pub fn new() -> DevServer {
        DevServer {
            files: HashMap::new(),
            runtime: HotReloadRuntime::new(),
            diagnostics: Vec::new(),
            repairs_applied: 0,
            total_rebuilds: 0,
        }
    }

    pub fn add_file(&mut self, path: String, content: String) / io {
        let file = SourceFile::new(path.clone(), content);
        println!("  📄 Loaded: {} ({} tokens, v{})", path, file.token_count, file.version);

        // Register all functions found in the file.
        // (simplified: register the file itself as a "function")
        self.runtime.register_function(path.clone(), file.version);
        self.files.insert(path, file);
    }

    /// Simulate a file change and trigger rebuild.
    pub fn on_file_changed(&mut self, path: &String, new_content: String) / io {
        println!("");
        println!("  🔄 File changed: {}", path);

        let file = match self.files.get_mut(path) {
            Some(f) => f,
            None => {
                println!("    ⚠ Unknown file: {}", path);
                return;
            },
        };

        let old_version = file.version;
        file.update(new_content);
        println!("    Updated: v{} → v{} ({} tokens)", old_version, file.version, file.token_count);
        self.total_rebuilds = self.total_rebuilds + 1;

        // Compile and check for errors.
        let errors = self.compile(path);

        if errors.is_empty() {
            // No errors — apply hot patch.
            println!("    ✓ Compilation successful");
            let _ = self.runtime.apply_patch(
                path,
                format!("Rebuild #{}", self.total_rebuilds),
            );
        } else {
            // Errors found — attempt self-healing.
            println!("    ✗ Found {} error(s) — attempting self-healing...", errors.len());
            self.heal_errors(&errors);
        }
    }

    /// Simulate compilation — returns diagnostics.
    fn compile(&mut self, path: &String) -> Vec<Diagnostic> / io {
        let mut errors: Vec<Diagnostic> = Vec::new();

        let file = match self.files.get(path) {
            Some(f) => f,
            None => return errors,
        };

        // Simulate detecting errors based on file content patterns.
        if file.content.contains("fn ") {
            errors.push(Diagnostic {
                file: path.clone(),
                line: 1,
                column: 1,
                kind: ErrorKind::SyntaxError,
                message: "Use MechGen `fn` keyword instead of Rust `fn`".to_string(),
            });
        }

        if file.content.contains("undefined_var") {
            errors.push(Diagnostic {
                file: path.clone(),
                line: 5,
                column: 10,
                kind: ErrorKind::UndefinedVariable,
                message: "undefined_var".to_string(),
            });
        }

        if file.content.contains("let ") {
            errors.push(Diagnostic {
                file: path.clone(),
                line: 3,
                column: 5,
                kind: ErrorKind::SyntaxError,
                message: "Use MechGen `let` keyword instead of Rust `let`".to_string(),
            });
        }

        if file.content.contains("unused_x") {
            errors.push(Diagnostic {
                file: path.clone(),
                line: 7,
                column: 5,
                kind: ErrorKind::UnusedVariable,
                message: "unused_x".to_string(),
            });
        }

        for diag in &errors {
            println!("    {}", diag);
            self.diagnostics.push(diag.clone());
        }
        errors
    }

    /// Attempt to heal errors automatically.
    fn heal_errors(&mut self, errors: &Vec<Diagnostic>) / io {
        for diag in errors {
            let repairs = propose_repairs(diag);
            if !repairs.is_empty() {
                let best = &repairs[0];
                println!("    💡 {}", best);
                if best.confidence > 0.5 {
                    println!("    ✓ Auto-applied repair (confidence {:.0}%)", best.confidence * 100.0);
                    self.repairs_applied = self.repairs_applied + 1;
                } else {
                    println!("    ⚠ Low confidence — manual review needed");
                }
            }
        }
    }

    /// Simulate detecting a regression and rolling back.
    pub fn simulate_regression(&mut self, path: &String) / io {
        println!("");
        println!("  ⚠ Regression detected in {}!", path);
        match self.runtime.rollback(path) {
            Ok(()) => println!("    ✓ Service restored to previous version"),
            Err(msg) => println!("    ✗ Rollback failed: {}", msg),
        }
    }

    pub fn summary(&self) / io {
        println!("");
        println!("── Development Server Summary ────────────────────────");
        println!("  Files tracked:    {}", self.files.len());
        println!("  Total rebuilds:   {}", self.total_rebuilds);
        println!("  Diagnostics:      {}", self.diagnostics.len());
        println!("  Auto-repairs:     {}", self.repairs_applied);
        let total_tokens: usize = self.files.values().map(|f| f.token_count).sum();
        println!("  Total tokens:     {}", total_tokens);
        self.runtime.report();
    }
}

// ─────────────────────────────────────────────────────────────────────
// §6 — Entry point: simulate a development session
// ─────────────────────────────────────────────────────────────────────

pub fn main() / io {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║  MechGen Live Compiler — Hot-Reload Dev Server             ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!("");

    let mut server = DevServer::new();

    // Load initial source files.
    println!("─── Loading Project ──────────────────────────────────────");
    server.add_file(
        "src/lib.mg".to_string(),
        "pub fn greet(name: &String) -> String {\n    format!(\"Hello, {name}!\")\n}".to_string(),
    );
    server.add_file(
        "src/math.mg".to_string(),
        "fn add(a: i32, b: i32) -> i32 { a + b }\nfn mul(a: i32, b: i32) -> i32 { a * b }".to_string(),
    );
    server.add_file(
        "src/api.mg".to_string(),
        "pub fn handle_request(req: &Request) / io + net -> Result<Response, ApiError> {\n    let body = req.body();\n    Ok(Response::ok(body))\n}".to_string(),
    );

    // Simulate editing a file — clean change.
    println!("");
    println!("─── Edit 1: Clean Change ─────────────────────────────────");
    server.on_file_changed(
        &"src/lib.mg".to_string(),
        "pub fn greet(name: &String) -> String {\n    let greeting = format!(\"Hello, {name}!\");\n    greeting\n}".to_string(),
    );

    // Simulate editing with an error — triggers self-healing.
    println!("");
    println!("─── Edit 2: Introduces Errors ────────────────────────────");
    server.on_file_changed(
        &"src/math.mg".to_string(),
        "fn add(a: i32, b: i32) -> i32 { a + b }\nlet unused_x = 42;\nfn mul(a: i32, b: i32) -> i32 { a * b }".to_string(),
    );

    // Another clean edit.
    println!("");
    println!("─── Edit 3: Fix and Improve ──────────────────────────────");
    server.on_file_changed(
        &"src/math.mg".to_string(),
        "fn add(a: i32, b: i32) -> i32 { a + b }\nfn mul(a: i32, b: i32) -> i32 { a * b }\nfn div(a: f64, b: f64) -> Result<f64, String>\n    @req b != 0.0\n{ Ok(a / b) }".to_string(),
    );

    // Simulate a regression — trigger rollback.
    println!("");
    println!("─── Edit 4: Regression Detected ──────────────────────────");
    server.on_file_changed(
        &"src/api.mg".to_string(),
        "pub fn handle_request(req: &Request) / io + net -> Result<Response, ApiError> {\n    let undefined_var = process(req);\n    Ok(Response::ok(undefined_var))\n}".to_string(),
    );
    server.simulate_regression(&"src/api.mg".to_string());

    // Final summary.
    server.summary();

    println!("");
    println!("═══════════════════════════════════════════════════════════");
    println!("  Dev server session complete.");
    println!("═══════════════════════════════════════════════════════════");
}

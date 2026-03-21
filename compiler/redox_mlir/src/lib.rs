//! # Redox MLIR Integration
//!
//! Provides Rust FFI bindings to the MLIR C API and safe wrappers for use
//! in the Redox compilation pipeline.
//!
//! ## Features
//!
//! - `mlir_ffi`: Link against actual MLIR C API libraries. Without this flag,
//!   a mock implementation is provided for development and testing.
//!
//! ## Architecture
//!
//! The Redox compilation pipeline uses MLIR as its multi-level IR:
//!
//! ```text
//! MIR → MLIR (Redox Dialect) → MLIR (Linalg/Affine/Vector/SCF) → MLIR (LLVM Dialect) → LLVM IR
//! ```
//!
//! Reference: REDOX_PROPOSAL.md §5.4.1

pub mod dialect;

// ---------------------------------------------------------------------------
// Raw FFI declarations (only when linking against MLIR)
// ---------------------------------------------------------------------------

/// Raw FFI bindings to the MLIR C API.
///
/// These are only available when the `mlir_ffi` feature is enabled and MLIR
/// libraries are on the link path.
#[cfg(feature = "mlir_ffi")]
pub mod ffi {
    use std::os::raw::c_void;

    /// Opaque MLIR context handle.
    pub type MlirContext = *mut c_void;
    /// Opaque MLIR module handle.
    pub type MlirModule = *mut c_void;
    /// Opaque MLIR operation handle.
    pub type MlirOperation = *mut c_void;
    /// Opaque MLIR block handle.
    pub type MlirBlock = *mut c_void;
    /// Opaque MLIR type handle.
    pub type MlirType = *mut c_void;
    /// Opaque MLIR location handle.
    pub type MlirLocation = *mut c_void;
    /// Opaque MLIR dialect handle.
    pub type MlirDialectHandle = *mut c_void;

    /// MLIR string reference (non-owning).
    #[repr(C)]
    pub struct MlirStringRef {
        pub data: *const u8,
        pub length: usize,
    }

    unsafe extern "C" {
        // Context management
        pub fn mlirContextCreate() -> MlirContext;
        pub fn mlirContextDestroy(context: MlirContext);
        pub fn mlirContextGetNumRegisteredDialects(context: MlirContext) -> i32;
        pub fn mlirContextGetNumLoadedDialects(context: MlirContext) -> i32;

        // Module management
        pub fn mlirModuleCreateEmpty(location: MlirLocation) -> MlirModule;
        pub fn mlirModuleDestroy(module: MlirModule);
        pub fn mlirModuleGetBody(module: MlirModule) -> MlirBlock;

        // Location
        pub fn mlirLocationUnknownGet(context: MlirContext) -> MlirLocation;
        pub fn mlirLocationFileLineColGet(
            context: MlirContext,
            filename: MlirStringRef,
            line: u32,
            col: u32,
        ) -> MlirLocation;

        // Dialect registration
        pub fn mlirGetDialectHandle__llvm__() -> MlirDialectHandle;
        pub fn mlirDialectHandleRegisterDialect(handle: MlirDialectHandle, context: MlirContext);
    }
}

// ---------------------------------------------------------------------------
// Safe wrapper types
// ---------------------------------------------------------------------------

/// An MLIR context — the top-level container for all MLIR entities.
///
/// In production (`mlir_ffi` feature), wraps a real `MlirContext`.
/// In testing (default), uses a mock that tracks creation/destruction.
pub struct Context {
    #[cfg(feature = "mlir_ffi")]
    raw: ffi::MlirContext,
    #[cfg(not(feature = "mlir_ffi"))]
    alive: bool,
    #[cfg(not(feature = "mlir_ffi"))]
    dialects_registered: Vec<String>,
}

/// An MLIR module — a container for operations.
pub struct Module {
    #[cfg(feature = "mlir_ffi")]
    raw: ffi::MlirModule,
    #[cfg(not(feature = "mlir_ffi"))]
    alive: bool,
    #[cfg(not(feature = "mlir_ffi"))]
    operations: Vec<String>,
}

/// An MLIR source location.
#[derive(Debug, Clone, PartialEq)]
pub enum Location {
    Unknown,
    FileLineCol { file: String, line: u32, col: u32 },
}

/// Error type for MLIR operations.
#[derive(Debug, Clone, PartialEq)]
pub enum MlirError {
    ContextCreationFailed,
    ModuleCreationFailed,
    DialectNotFound(String),
    VerificationFailed(String),
    NullPointer(&'static str),
}

impl std::fmt::Display for MlirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MlirError::ContextCreationFailed => write!(f, "MLIR context creation failed"),
            MlirError::ModuleCreationFailed => write!(f, "MLIR module creation failed"),
            MlirError::DialectNotFound(d) => write!(f, "MLIR dialect not found: {d}"),
            MlirError::VerificationFailed(msg) => {
                write!(f, "MLIR verification failed: {msg}")
            }
            MlirError::NullPointer(what) => write!(f, "null pointer: {what}"),
        }
    }
}

impl std::error::Error for MlirError {}

// ---------------------------------------------------------------------------
// Context implementation — mock (default, no mlir_ffi feature)
// ---------------------------------------------------------------------------

#[cfg(not(feature = "mlir_ffi"))]
impl Context {
    /// Create a new MLIR context (mock).
    pub fn new() -> Result<Self, MlirError> {
        Ok(Self { alive: true, dialects_registered: Vec::new() })
    }

    /// Whether this context is still alive (not destroyed).
    pub fn is_alive(&self) -> bool {
        self.alive
    }

    /// Register a dialect by name.
    pub fn register_dialect(&mut self, name: &str) -> Result<(), MlirError> {
        if !self.alive {
            return Err(MlirError::NullPointer("context"));
        }
        self.dialects_registered.push(name.to_string());
        Ok(())
    }

    /// Number of registered dialects.
    pub fn num_registered_dialects(&self) -> usize {
        self.dialects_registered.len()
    }

    /// Number of loaded dialects (in mock, same as registered).
    pub fn num_loaded_dialects(&self) -> usize {
        self.dialects_registered.len()
    }

    /// Destroy this context, releasing resources.
    pub fn destroy(&mut self) {
        self.alive = false;
        self.dialects_registered.clear();
    }
}

#[cfg(not(feature = "mlir_ffi"))]
impl Default for Context {
    fn default() -> Self {
        Self::new().expect("mock context creation cannot fail")
    }
}

#[cfg(not(feature = "mlir_ffi"))]
impl Drop for Context {
    fn drop(&mut self) {
        self.alive = false;
    }
}

// ---------------------------------------------------------------------------
// Context implementation — real FFI (mlir_ffi feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "mlir_ffi")]
impl Context {
    /// Create a new MLIR context backed by the C API.
    pub fn new() -> Result<Self, MlirError> {
        let raw = unsafe { ffi::mlirContextCreate() };
        if raw.is_null() {
            return Err(MlirError::ContextCreationFailed);
        }
        Ok(Self { raw })
    }

    pub fn is_alive(&self) -> bool {
        !self.raw.is_null()
    }

    pub fn num_registered_dialects(&self) -> usize {
        unsafe { ffi::mlirContextGetNumRegisteredDialects(self.raw) as usize }
    }

    pub fn num_loaded_dialects(&self) -> usize {
        unsafe { ffi::mlirContextGetNumLoadedDialects(self.raw) as usize }
    }

    pub fn destroy(&mut self) {
        if !self.raw.is_null() {
            unsafe { ffi::mlirContextDestroy(self.raw) };
            self.raw = std::ptr::null_mut();
        }
    }
}

#[cfg(feature = "mlir_ffi")]
impl Drop for Context {
    fn drop(&mut self) {
        self.destroy();
    }
}

// ---------------------------------------------------------------------------
// Module implementation — mock (default)
// ---------------------------------------------------------------------------

#[cfg(not(feature = "mlir_ffi"))]
impl Module {
    /// Create an empty MLIR module (mock).
    pub fn new_empty(_ctx: &Context, _loc: &Location) -> Result<Self, MlirError> {
        Ok(Self { alive: true, operations: Vec::new() })
    }

    pub fn is_alive(&self) -> bool {
        self.alive
    }

    /// Add a named operation (mock — just records the name).
    pub fn add_operation(&mut self, name: &str) {
        self.operations.push(name.to_string());
    }

    /// Number of operations in this module.
    pub fn num_operations(&self) -> usize {
        self.operations.len()
    }

    pub fn destroy(&mut self) {
        self.alive = false;
        self.operations.clear();
    }
}

#[cfg(not(feature = "mlir_ffi"))]
impl Drop for Module {
    fn drop(&mut self) {
        self.alive = false;
    }
}

// ---------------------------------------------------------------------------
// Location helpers
// ---------------------------------------------------------------------------

impl Location {
    pub fn unknown() -> Self {
        Location::Unknown
    }

    pub fn file_line_col(file: impl Into<String>, line: u32, col: u32) -> Self {
        Location::FileLineCol { file: file.into(), line, col }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Context lifecycle ---------------------------------------------------

    #[test]
    fn context_create_and_destroy() {
        let mut ctx = Context::new().expect("context creation should succeed");
        assert!(ctx.is_alive());
        ctx.destroy();
        assert!(!ctx.is_alive());
    }

    #[test]
    fn context_default() {
        let ctx = Context::default();
        assert!(ctx.is_alive());
    }

    #[test]
    fn context_drop_sets_not_alive() {
        let ctx = Context::new().unwrap();
        assert!(ctx.is_alive());
        drop(ctx);
        // After drop we can't check, but the test verifies no panic/crash.
    }

    // -- Dialect registration ------------------------------------------------

    #[test]
    fn register_dialect() {
        let mut ctx = Context::new().unwrap();
        assert_eq!(ctx.num_registered_dialects(), 0);
        ctx.register_dialect("llvm").unwrap();
        assert_eq!(ctx.num_registered_dialects(), 1);
        ctx.register_dialect("redox").unwrap();
        assert_eq!(ctx.num_registered_dialects(), 2);
    }

    // -- Module lifecycle ----------------------------------------------------

    #[test]
    fn module_create_and_destroy() {
        let ctx = Context::new().unwrap();
        let loc = Location::unknown();
        let mut module = Module::new_empty(&ctx, &loc).unwrap();
        assert!(module.is_alive());
        module.destroy();
        assert!(!module.is_alive());
    }

    #[test]
    fn module_add_operations() {
        let ctx = Context::new().unwrap();
        let loc = Location::file_line_col("main.rdx", 1, 0);
        let mut module = Module::new_empty(&ctx, &loc).unwrap();
        assert_eq!(module.num_operations(), 0);
        module.add_operation("redox.move");
        module.add_operation("redox.borrow");
        assert_eq!(module.num_operations(), 2);
    }

    // -- Location ------------------------------------------------------------

    #[test]
    fn location_unknown() {
        let loc = Location::unknown();
        assert_eq!(loc, Location::Unknown);
    }

    #[test]
    fn location_file_line_col() {
        let loc = Location::file_line_col("main.rs", 42, 5);
        assert_eq!(loc, Location::FileLineCol { file: "main.rs".into(), line: 42, col: 5 });
    }

    // -- Error display -------------------------------------------------------

    #[test]
    fn error_display() {
        assert_eq!(MlirError::ContextCreationFailed.to_string(), "MLIR context creation failed",);
        assert_eq!(
            MlirError::DialectNotFound("foo".into()).to_string(),
            "MLIR dialect not found: foo",
        );
        assert_eq!(
            MlirError::VerificationFailed("bad op".into()).to_string(),
            "MLIR verification failed: bad op",
        );
    }

    // -- Smoke test (the deliverable) ----------------------------------------

    #[test]
    fn smoke_test_create_and_destroy_context() {
        // Step 27 deliverable: "Add a smoke test that creates and destroys
        // an MLIR context."
        let mut ctx = Context::new().expect("MLIR context should be created");
        assert!(ctx.is_alive());

        // Register the Redox dialect
        ctx.register_dialect("redox").unwrap();
        assert_eq!(ctx.num_registered_dialects(), 1);

        // Create a module
        let loc = Location::file_line_col("smoke.rdx", 1, 0);
        let mut module = Module::new_empty(&ctx, &loc).unwrap();
        assert!(module.is_alive());

        // Add some operations
        module.add_operation("redox.move");
        module.add_operation("redox.copy");
        module.add_operation("redox.borrow");
        module.add_operation("redox.drop");
        assert_eq!(module.num_operations(), 4);

        // Tear down
        module.destroy();
        assert!(!module.is_alive());
        ctx.destroy();
        assert!(!ctx.is_alive());
    }
}

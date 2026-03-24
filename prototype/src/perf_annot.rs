// ── Performance Annotations ───────────────────────────────────────
//
// Processing for MechGen performance annotations (§5 of the spec):
//
//   @pi!           — force inline
//   @pnb           — no blocking (async-safe)
//   @pv(N)         — vectorization width hint (SIMD lanes)
//   @pt(target)    — target-specific optimization (e.g., "avx2", "neon")
//   @pa(N)         — alignment hint (bytes)
//   @pp            — pure function (no side effects, cacheable)
//   #[repr(target_optimal)] — layout optimization per target
//
// This module parses annotation strings, validates parameters,
// collects per-function annotation sets, and emits MLIR-compatible
// lowering hints.

use std::collections::BTreeMap;

// ── Annotation kinds ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PerfAnnotation {
    /// `@pi!` — force inline at all call sites.
    ForceInline,
    /// `@pnb` — function must not block.
    NoBlock,
    /// `@pv(N)` — preferred SIMD vectorization width.
    Vectorize(u32),
    /// `@pt(target)` — target-specific optimization hint.
    TargetHint(String),
    /// `@pa(N)` — alignment in bytes (must be power of 2).
    Alignment(u32),
    /// `@pp` — pure function: no side effects, result cacheable.
    Pure,
    /// `#[repr(target_optimal)]` — layout optimized per target.
    ReprTargetOptimal,
}

// ── Parse result ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    UnknownAnnotation(String),
    MissingArgument(String),
    InvalidArgument { annotation: String, reason: String },
}

/// Parse a single annotation string into a PerfAnnotation.
pub fn parse_annotation(s: &str) -> Result<PerfAnnotation, ParseError> {
    let s = s.trim();

    if s == "@pi!" {
        return Ok(PerfAnnotation::ForceInline);
    }
    if s == "@pnb" {
        return Ok(PerfAnnotation::NoBlock);
    }
    if s == "@pp" {
        return Ok(PerfAnnotation::Pure);
    }
    if s == "#[repr(target_optimal)]" {
        return Ok(PerfAnnotation::ReprTargetOptimal);
    }

    // @pv(N)
    if let Some(inner) = s.strip_prefix("@pv(").and_then(|r| r.strip_suffix(')')) {
        let n: u32 = inner.parse().map_err(|_| ParseError::InvalidArgument {
            annotation: "@pv".into(),
            reason: format!("expected integer, got '{}'", inner),
        })?;
        if !n.is_power_of_two() {
            return Err(ParseError::InvalidArgument {
                annotation: "@pv".into(),
                reason: format!("vectorization width must be power of 2, got {}", n),
            });
        }
        return Ok(PerfAnnotation::Vectorize(n));
    }

    // @pt(target)
    if let Some(inner) = s.strip_prefix("@pt(").and_then(|r| r.strip_suffix(')')) {
        if inner.is_empty() {
            return Err(ParseError::MissingArgument("@pt".into()));
        }
        return Ok(PerfAnnotation::TargetHint(inner.to_string()));
    }

    // @pa(N)
    if let Some(inner) = s.strip_prefix("@pa(").and_then(|r| r.strip_suffix(')')) {
        let n: u32 = inner.parse().map_err(|_| ParseError::InvalidArgument {
            annotation: "@pa".into(),
            reason: format!("expected integer, got '{}'", inner),
        })?;
        if !n.is_power_of_two() {
            return Err(ParseError::InvalidArgument {
                annotation: "@pa".into(),
                reason: format!("alignment must be power of 2, got {}", n),
            });
        }
        return Ok(PerfAnnotation::Alignment(n));
    }

    Err(ParseError::UnknownAnnotation(s.to_string()))
}

/// Parse multiple annotations from a whitespace-separated string.
pub fn parse_annotations(input: &str) -> Vec<Result<PerfAnnotation, ParseError>> {
    // Split on whitespace, but keep parenthesized groups together
    let mut results = Vec::new();
    let mut tokens = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();

    for ch in input.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ' ' | '\t' | '\n' if depth == 0 => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    for tok in tokens {
        results.push(parse_annotation(&tok));
    }
    results
}

// ── Annotation set ─────────────────────────────────────────────────

/// Collected performance annotations for a single function or struct.
#[derive(Debug, Clone, Default)]
pub struct PerfAnnotationSet {
    pub annotations: Vec<PerfAnnotation>,
}

impl PerfAnnotationSet {
    pub fn new() -> Self {
        Self { annotations: Vec::new() }
    }

    pub fn add(&mut self, ann: PerfAnnotation) {
        self.annotations.push(ann);
    }

    pub fn is_force_inline(&self) -> bool {
        self.annotations.iter().any(|a| matches!(a, PerfAnnotation::ForceInline))
    }

    pub fn is_no_block(&self) -> bool {
        self.annotations.iter().any(|a| matches!(a, PerfAnnotation::NoBlock))
    }

    pub fn is_pure(&self) -> bool {
        self.annotations.iter().any(|a| matches!(a, PerfAnnotation::Pure))
    }

    pub fn vectorize_width(&self) -> Option<u32> {
        self.annotations.iter().find_map(|a| match a {
            PerfAnnotation::Vectorize(n) => Some(*n),
            _ => None,
        })
    }

    pub fn alignment(&self) -> Option<u32> {
        self.annotations.iter().find_map(|a| match a {
            PerfAnnotation::Alignment(n) => Some(*n),
            _ => None,
        })
    }

    pub fn target_hints(&self) -> Vec<&str> {
        self.annotations
            .iter()
            .filter_map(|a| match a {
                PerfAnnotation::TargetHint(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    pub fn is_repr_target_optimal(&self) -> bool {
        self.annotations
            .iter()
            .any(|a| matches!(a, PerfAnnotation::ReprTargetOptimal))
    }

    /// Validate that annotations are mutually consistent.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        // Force-inline + no-block is fine
        // Multiple vectorize widths → conflict
        let vec_count = self
            .annotations
            .iter()
            .filter(|a| matches!(a, PerfAnnotation::Vectorize(_)))
            .count();
        if vec_count > 1 {
            errors.push("Multiple @pv annotations: conflicting vectorization widths".into());
        }

        // Multiple alignments → conflict
        let align_count = self
            .annotations
            .iter()
            .filter(|a| matches!(a, PerfAnnotation::Alignment(_)))
            .count();
        if align_count > 1 {
            errors.push("Multiple @pa annotations: conflicting alignments".into());
        }

        errors
    }
}

// ── Per-module registry ────────────────────────────────────────────

/// Tracks performance annotations for all functions/structs in a module.
pub struct PerfRegistry {
    /// Function qualified name → annotation set
    functions: BTreeMap<String, PerfAnnotationSet>,
    /// Struct name → annotation set (for repr hints)
    structs: BTreeMap<String, PerfAnnotationSet>,
}

impl PerfRegistry {
    pub fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
            structs: BTreeMap::new(),
        }
    }

    pub fn annotate_function(&mut self, name: &str, ann: PerfAnnotation) {
        self.functions.entry(name.into()).or_default().add(ann);
    }

    pub fn annotate_struct(&mut self, name: &str, ann: PerfAnnotation) {
        self.structs.entry(name.into()).or_default().add(ann);
    }

    pub fn get_function(&self, name: &str) -> Option<&PerfAnnotationSet> {
        self.functions.get(name)
    }

    pub fn get_struct(&self, name: &str) -> Option<&PerfAnnotationSet> {
        self.structs.get(name)
    }

    pub fn inline_functions(&self) -> Vec<&str> {
        self.functions
            .iter()
            .filter(|(_, set)| set.is_force_inline())
            .map(|(name, _)| name.as_str())
            .collect()
    }

    pub fn no_block_functions(&self) -> Vec<&str> {
        self.functions
            .iter()
            .filter(|(_, set)| set.is_no_block())
            .map(|(name, _)| name.as_str())
            .collect()
    }

    pub fn pure_functions(&self) -> Vec<&str> {
        self.functions
            .iter()
            .filter(|(_, set)| set.is_pure())
            .map(|(name, _)| name.as_str())
            .collect()
    }

    pub fn validate_all(&self) -> BTreeMap<String, Vec<String>> {
        let mut errors = BTreeMap::new();
        for (name, set) in &self.functions {
            let errs = set.validate();
            if !errs.is_empty() {
                errors.insert(name.clone(), errs);
            }
        }
        for (name, set) in &self.structs {
            let errs = set.validate();
            if !errs.is_empty() {
                errors.insert(name.clone(), errs);
            }
        }
        errors
    }

    /// Emit MLIR-compatible lowering hints.
    pub fn emit_mlir_hints(&self, function_name: &str) -> String {
        let mut out = String::new();
        if let Some(set) = self.functions.get(function_name) {
            for ann in &set.annotations {
                match ann {
                    PerfAnnotation::ForceInline => {
                        out.push_str(&format!(
                            "  MechGen.perf.inline @{} {{ always = true }}\n",
                            function_name
                        ));
                    }
                    PerfAnnotation::NoBlock => {
                        out.push_str(&format!(
                            "  MechGen.perf.noblock @{}\n",
                            function_name
                        ));
                    }
                    PerfAnnotation::Vectorize(n) => {
                        out.push_str(&format!(
                            "  MechGen.perf.vectorize @{} {{ width = {} }}\n",
                            function_name, n
                        ));
                    }
                    PerfAnnotation::TargetHint(target) => {
                        out.push_str(&format!(
                            "  MechGen.perf.target @{} {{ target = \"{}\" }}\n",
                            function_name, target
                        ));
                    }
                    PerfAnnotation::Alignment(n) => {
                        out.push_str(&format!(
                            "  MechGen.perf.align @{} {{ bytes = {} }}\n",
                            function_name, n
                        ));
                    }
                    PerfAnnotation::Pure => {
                        out.push_str(&format!(
                            "  MechGen.perf.pure @{}\n",
                            function_name
                        ));
                    }
                    PerfAnnotation::ReprTargetOptimal => {
                        out.push_str(&format!(
                            "  MechGen.perf.repr @{} {{ layout = \"target_optimal\" }}\n",
                            function_name
                        ));
                    }
                }
            }
        }
        out
    }

    pub fn stats(&self) -> String {
        format!(
            "{{\"functions\":{},\"structs\":{}}}",
            self.functions.len(),
            self.structs.len()
        )
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Parsing ───────────────────────────────────────────────────

    #[test]
    fn parse_force_inline() {
        assert_eq!(parse_annotation("@pi!").unwrap(), PerfAnnotation::ForceInline);
    }

    #[test]
    fn parse_no_block() {
        assert_eq!(parse_annotation("@pnb").unwrap(), PerfAnnotation::NoBlock);
    }

    #[test]
    fn parse_pure() {
        assert_eq!(parse_annotation("@pp").unwrap(), PerfAnnotation::Pure);
    }

    #[test]
    fn parse_vectorize() {
        assert_eq!(parse_annotation("@pv(4)").unwrap(), PerfAnnotation::Vectorize(4));
    }

    #[test]
    fn parse_vectorize_non_power_of_two() {
        assert!(parse_annotation("@pv(3)").is_err());
    }

    #[test]
    fn parse_target_hint() {
        assert_eq!(
            parse_annotation("@pt(avx2)").unwrap(),
            PerfAnnotation::TargetHint("avx2".into())
        );
    }

    #[test]
    fn parse_target_empty() {
        assert!(parse_annotation("@pt()").is_err());
    }

    #[test]
    fn parse_alignment() {
        assert_eq!(parse_annotation("@pa(64)").unwrap(), PerfAnnotation::Alignment(64));
    }

    #[test]
    fn parse_alignment_non_power_of_two() {
        assert!(parse_annotation("@pa(7)").is_err());
    }

    #[test]
    fn parse_repr_target_optimal() {
        assert_eq!(
            parse_annotation("#[repr(target_optimal)]").unwrap(),
            PerfAnnotation::ReprTargetOptimal
        );
    }

    #[test]
    fn parse_unknown() {
        assert!(parse_annotation("@xyz").is_err());
    }

    // ── Multi-parse ───────────────────────────────────────────────

    #[test]
    fn parse_multiple_annotations() {
        let results = parse_annotations("@pi! @pnb @pv(8) @pt(neon)");
        assert_eq!(results.len(), 4);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    // ── Annotation set ────────────────────────────────────────────

    #[test]
    fn annotation_set_queries() {
        let mut set = PerfAnnotationSet::new();
        set.add(PerfAnnotation::ForceInline);
        set.add(PerfAnnotation::NoBlock);
        set.add(PerfAnnotation::Vectorize(8));
        set.add(PerfAnnotation::Pure);
        assert!(set.is_force_inline());
        assert!(set.is_no_block());
        assert!(set.is_pure());
        assert_eq!(set.vectorize_width(), Some(8));
    }

    #[test]
    fn annotation_set_validate_conflict() {
        let mut set = PerfAnnotationSet::new();
        set.add(PerfAnnotation::Vectorize(4));
        set.add(PerfAnnotation::Vectorize(8));
        let errors = set.validate();
        assert!(!errors.is_empty());
    }

    #[test]
    fn annotation_set_validate_ok() {
        let mut set = PerfAnnotationSet::new();
        set.add(PerfAnnotation::ForceInline);
        set.add(PerfAnnotation::Pure);
        let errors = set.validate();
        assert!(errors.is_empty());
    }

    // ── Registry ──────────────────────────────────────────────────

    #[test]
    fn registry_annotate_and_query() {
        let mut reg = PerfRegistry::new();
        reg.annotate_function("math::add", PerfAnnotation::ForceInline);
        reg.annotate_function("math::add", PerfAnnotation::Pure);
        reg.annotate_function("io::read", PerfAnnotation::NoBlock);

        assert_eq!(reg.inline_functions(), vec!["math::add"]);
        assert_eq!(reg.pure_functions(), vec!["math::add"]);
        assert_eq!(reg.no_block_functions(), vec!["io::read"]);
    }

    #[test]
    fn registry_struct_annotation() {
        let mut reg = PerfRegistry::new();
        reg.annotate_struct("Vector4", PerfAnnotation::Alignment(16));
        reg.annotate_struct("Vector4", PerfAnnotation::ReprTargetOptimal);

        let set = reg.get_struct("Vector4").unwrap();
        assert_eq!(set.alignment(), Some(16));
        assert!(set.is_repr_target_optimal());
    }

    #[test]
    fn registry_validate_all() {
        let mut reg = PerfRegistry::new();
        reg.annotate_function("bad", PerfAnnotation::Vectorize(4));
        reg.annotate_function("bad", PerfAnnotation::Vectorize(8));
        let errors = reg.validate_all();
        assert!(errors.contains_key("bad"));
    }

    // ── MLIR hints ────────────────────────────────────────────────

    #[test]
    fn mlir_hints_force_inline() {
        let mut reg = PerfRegistry::new();
        reg.annotate_function("add", PerfAnnotation::ForceInline);
        let hints = reg.emit_mlir_hints("add");
        assert!(hints.contains("MechGen.perf.inline @add"));
        assert!(hints.contains("always = true"));
    }

    #[test]
    fn mlir_hints_vectorize() {
        let mut reg = PerfRegistry::new();
        reg.annotate_function("dot", PerfAnnotation::Vectorize(8));
        let hints = reg.emit_mlir_hints("dot");
        assert!(hints.contains("MechGen.perf.vectorize @dot"));
        assert!(hints.contains("width = 8"));
    }

    #[test]
    fn mlir_hints_multiple() {
        let mut reg = PerfRegistry::new();
        reg.annotate_function("f", PerfAnnotation::Pure);
        reg.annotate_function("f", PerfAnnotation::NoBlock);
        let hints = reg.emit_mlir_hints("f");
        assert!(hints.contains("MechGen.perf.pure @f"));
        assert!(hints.contains("MechGen.perf.noblock @f"));
    }

    // ── Stats ─────────────────────────────────────────────────────

    #[test]
    fn stats_json() {
        let mut reg = PerfRegistry::new();
        reg.annotate_function("a", PerfAnnotation::Pure);
        reg.annotate_struct("B", PerfAnnotation::Alignment(16));
        let s = reg.stats();
        assert!(s.contains("\"functions\":1"));
        assert!(s.contains("\"structs\":1"));
    }

    // ── Target hints ──────────────────────────────────────────────

    #[test]
    fn target_hints() {
        let mut set = PerfAnnotationSet::new();
        set.add(PerfAnnotation::TargetHint("avx2".into()));
        set.add(PerfAnnotation::TargetHint("neon".into()));
        let hints = set.target_hints();
        assert_eq!(hints, vec!["avx2", "neon"]);
    }
}

//! Compact performance annotations for the Redox compiler.
//!
//! - `@pi!` — force inline
//! - `@pi`  — suggest inline
//! - `@pnb` — no bounds check
//! - `@pv(N)` — vectorize with width N
//! - `@pt(target)` — target placement (e.g. gpu, simd, numa:0)
//! - `@pu` — unroll loop
//! - `@pf` — fuse adjacent loops
//! - `@pp` — parallelize

use std::fmt;

// ---------------------------------------------------------------------------
// Annotation types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum PerfAnnotation {
    /// `@pi!` — force inline
    InlineForce,
    /// `@pi` — suggest inline
    InlineHint,
    /// `@pnb` — disable bounds checking
    NoBoundsCheck,
    /// `@pv(N)` — vectorize with given width
    Vectorize(u32),
    /// `@pt(target)` — target placement
    TargetPlacement(TargetSpec),
    /// `@pu` — unroll loop
    Unroll(Option<u32>),
    /// `@pf` — fuse adjacent loops
    Fuse,
    /// `@pp` — parallelize
    Parallelize,
}

impl fmt::Display for PerfAnnotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PerfAnnotation::InlineForce => write!(f, "@pi!"),
            PerfAnnotation::InlineHint => write!(f, "@pi"),
            PerfAnnotation::NoBoundsCheck => write!(f, "@pnb"),
            PerfAnnotation::Vectorize(w) => write!(f, "@pv({w})"),
            PerfAnnotation::TargetPlacement(t) => write!(f, "@pt({t})"),
            PerfAnnotation::Unroll(None) => write!(f, "@pu"),
            PerfAnnotation::Unroll(Some(n)) => write!(f, "@pu({n})"),
            PerfAnnotation::Fuse => write!(f, "@pf"),
            PerfAnnotation::Parallelize => write!(f, "@pp"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TargetSpec {
    Gpu,
    Simd,
    Numa(u32),
    Cpu,
    Custom(String),
}

impl fmt::Display for TargetSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetSpec::Gpu => write!(f, "gpu"),
            TargetSpec::Simd => write!(f, "simd"),
            TargetSpec::Numa(node) => write!(f, "numa:{node}"),
            TargetSpec::Cpu => write!(f, "cpu"),
            TargetSpec::Custom(s) => write!(f, "{s}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnknownAnnotation(String),
    MissingArgument(String),
    InvalidArgument { annotation: String, arg: String, reason: String },
    UnexpectedEnd,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnknownAnnotation(s) => write!(f, "unknown annotation: {s}"),
            ParseError::MissingArgument(s) => write!(f, "missing argument for {s}"),
            ParseError::InvalidArgument { annotation, arg, reason } => {
                write!(f, "invalid argument '{arg}' for {annotation}: {reason}")
            }
            ParseError::UnexpectedEnd => write!(f, "unexpected end of annotation"),
        }
    }
}

/// Parse a performance annotation string.
pub fn parse_annotation(input: &str) -> Result<PerfAnnotation, ParseError> {
    let s = input.trim();
    if !s.starts_with('@') {
        return Err(ParseError::UnknownAnnotation(s.to_string()));
    }

    let body = &s[1..];

    if body == "pi!" {
        return Ok(PerfAnnotation::InlineForce);
    }
    if body == "pi" {
        return Ok(PerfAnnotation::InlineHint);
    }
    if body == "pnb" {
        return Ok(PerfAnnotation::NoBoundsCheck);
    }
    if body == "pf" {
        return Ok(PerfAnnotation::Fuse);
    }
    if body == "pp" {
        return Ok(PerfAnnotation::Parallelize);
    }
    if body == "pu" {
        return Ok(PerfAnnotation::Unroll(None));
    }

    // Annotations with arguments: @pv(N), @pt(target), @pu(N)
    if let Some(rest) = body.strip_prefix("pv(") {
        let rest = rest.strip_suffix(')').ok_or(ParseError::MissingArgument("@pv".to_string()))?;
        let n: u32 = rest.trim().parse().map_err(|_| ParseError::InvalidArgument {
            annotation: "@pv".to_string(),
            arg: rest.to_string(),
            reason: "expected positive integer".to_string(),
        })?;
        if n == 0 || !n.is_power_of_two() {
            return Err(ParseError::InvalidArgument {
                annotation: "@pv".to_string(),
                arg: n.to_string(),
                reason: "vectorize width must be a power of 2".to_string(),
            });
        }
        return Ok(PerfAnnotation::Vectorize(n));
    }

    if let Some(rest) = body.strip_prefix("pt(") {
        let rest = rest.strip_suffix(')').ok_or(ParseError::MissingArgument("@pt".to_string()))?;
        let target = parse_target(rest.trim())?;
        return Ok(PerfAnnotation::TargetPlacement(target));
    }

    if let Some(rest) = body.strip_prefix("pu(") {
        let rest = rest.strip_suffix(')').ok_or(ParseError::MissingArgument("@pu".to_string()))?;
        let n: u32 = rest.trim().parse().map_err(|_| ParseError::InvalidArgument {
            annotation: "@pu".to_string(),
            arg: rest.to_string(),
            reason: "expected positive integer".to_string(),
        })?;
        if n == 0 {
            return Err(ParseError::InvalidArgument {
                annotation: "@pu".to_string(),
                arg: "0".to_string(),
                reason: "unroll factor must be > 0".to_string(),
            });
        }
        return Ok(PerfAnnotation::Unroll(Some(n)));
    }

    Err(ParseError::UnknownAnnotation(s.to_string()))
}

fn parse_target(s: &str) -> Result<TargetSpec, ParseError> {
    match s {
        "gpu" => Ok(TargetSpec::Gpu),
        "simd" => Ok(TargetSpec::Simd),
        "cpu" => Ok(TargetSpec::Cpu),
        _ if s.starts_with("numa:") => {
            let num = &s[5..];
            let node: u32 = num.parse().map_err(|_| ParseError::InvalidArgument {
                annotation: "@pt".to_string(),
                arg: s.to_string(),
                reason: "invalid NUMA node number".to_string(),
            })?;
            Ok(TargetSpec::Numa(node))
        }
        _ => Ok(TargetSpec::Custom(s.to_string())),
    }
}

/// Parse multiple annotations from a line (space-separated).
pub fn parse_annotations(input: &str) -> Result<Vec<PerfAnnotation>, ParseError> {
    let mut result = Vec::new();
    let mut remaining = input.trim();

    while !remaining.is_empty() {
        if !remaining.starts_with('@') {
            remaining = remaining.trim_start();
            if remaining.is_empty() {
                break;
            }
            if !remaining.starts_with('@') {
                return Err(ParseError::UnknownAnnotation(remaining.to_string()));
            }
        }

        // Find the end of this annotation
        let end = find_annotation_end(remaining);
        let chunk = &remaining[..end];
        result.push(parse_annotation(chunk)?);
        remaining = remaining[end..].trim_start();
    }

    Ok(result)
}

fn find_annotation_end(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut i = 1; // skip '@'
    let mut depth = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            b' ' | b'\t' | b'\n' if depth == 0 => return i,
            _ => {}
        }
        i += 1;
    }
    i
}

// ---------------------------------------------------------------------------
// Annotation validation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationTarget {
    Function,
    Loop,
    Block,
    Expression,
}

impl fmt::Display for AnnotationTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnnotationTarget::Function => write!(f, "function"),
            AnnotationTarget::Loop => write!(f, "loop"),
            AnnotationTarget::Block => write!(f, "block"),
            AnnotationTarget::Expression => write!(f, "expression"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub annotation: PerfAnnotation,
    pub target: AnnotationTarget,
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} not valid on {}: {}", self.annotation, self.target, self.message)
    }
}

/// Returns which targets an annotation can be applied to.
pub fn valid_targets(ann: &PerfAnnotation) -> Vec<AnnotationTarget> {
    match ann {
        PerfAnnotation::InlineForce | PerfAnnotation::InlineHint => {
            vec![AnnotationTarget::Function]
        }
        PerfAnnotation::NoBoundsCheck => {
            vec![AnnotationTarget::Function, AnnotationTarget::Block, AnnotationTarget::Expression]
        }
        PerfAnnotation::Vectorize(_) => {
            vec![AnnotationTarget::Loop]
        }
        PerfAnnotation::TargetPlacement(_) => {
            vec![AnnotationTarget::Function, AnnotationTarget::Block, AnnotationTarget::Loop]
        }
        PerfAnnotation::Unroll(_) => {
            vec![AnnotationTarget::Loop]
        }
        PerfAnnotation::Fuse => {
            vec![AnnotationTarget::Loop]
        }
        PerfAnnotation::Parallelize => {
            vec![AnnotationTarget::Loop, AnnotationTarget::Block]
        }
    }
}

/// Validate an annotation applied to a given target.
pub fn validate_annotation(
    ann: &PerfAnnotation,
    target: &AnnotationTarget,
) -> Result<(), ValidationError> {
    let targets = valid_targets(ann);
    if targets.contains(target) {
        Ok(())
    } else {
        Err(ValidationError {
            annotation: ann.clone(),
            target: target.clone(),
            message: format!(
                "{} can only be applied to: {}",
                ann,
                targets.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", "),
            ),
        })
    }
}

// ---------------------------------------------------------------------------
// Annotation set (attached to an IR node)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnnotationSet {
    annotations: Vec<PerfAnnotation>,
}

impl AnnotationSet {
    pub fn new() -> Self {
        Self { annotations: Vec::new() }
    }

    pub fn add(&mut self, ann: PerfAnnotation) {
        self.annotations.push(ann);
    }

    pub fn has_inline_force(&self) -> bool {
        self.annotations.iter().any(|a| matches!(a, PerfAnnotation::InlineForce))
    }

    pub fn has_inline_hint(&self) -> bool {
        self.annotations.iter().any(|a| matches!(a, PerfAnnotation::InlineHint))
    }

    pub fn has_no_bounds_check(&self) -> bool {
        self.annotations.iter().any(|a| matches!(a, PerfAnnotation::NoBoundsCheck))
    }

    pub fn vectorize_width(&self) -> Option<u32> {
        self.annotations.iter().find_map(|a| {
            if let PerfAnnotation::Vectorize(w) = a { Some(*w) } else { None }
        })
    }

    pub fn target_placement(&self) -> Option<&TargetSpec> {
        self.annotations.iter().find_map(|a| {
            if let PerfAnnotation::TargetPlacement(t) = a { Some(t) } else { None }
        })
    }

    pub fn unroll_factor(&self) -> Option<Option<u32>> {
        self.annotations.iter().find_map(|a| {
            if let PerfAnnotation::Unroll(n) = a { Some(*n) } else { None }
        })
    }

    pub fn has_fuse(&self) -> bool {
        self.annotations.iter().any(|a| matches!(a, PerfAnnotation::Fuse))
    }

    pub fn has_parallelize(&self) -> bool {
        self.annotations.iter().any(|a| matches!(a, PerfAnnotation::Parallelize))
    }

    pub fn iter(&self) -> impl Iterator<Item = &PerfAnnotation> {
        self.annotations.iter()
    }

    pub fn len(&self) -> usize {
        self.annotations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.annotations.is_empty()
    }

    /// Validate all annotations against a target.
    pub fn validate_all(&self, target: &AnnotationTarget) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        for ann in &self.annotations {
            if let Err(e) = validate_annotation(ann, target) {
                errors.push(e);
            }
        }
        errors
    }

    /// Check for conflicting annotations.
    pub fn check_conflicts(&self) -> Vec<String> {
        let mut conflicts = Vec::new();
        let has_force = self.has_inline_force();
        let has_hint = self.has_inline_hint();
        if has_force && has_hint {
            conflicts.push("@pi! and @pi are redundant; @pi! already forces inline".to_string());
        }

        let vec_count = self.annotations.iter()
            .filter(|a| matches!(a, PerfAnnotation::Vectorize(_)))
            .count();
        if vec_count > 1 {
            conflicts.push("multiple @pv annotations conflict".to_string());
        }

        let target_count = self.annotations.iter()
            .filter(|a| matches!(a, PerfAnnotation::TargetPlacement(_)))
            .count();
        if target_count > 1 {
            conflicts.push("multiple @pt annotations conflict".to_string());
        }

        conflicts
    }
}

impl fmt::Display for AnnotationSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.annotations.iter().map(|a| a.to_string()).collect();
        write!(f, "{}", parts.join(" "))
    }
}

// ---------------------------------------------------------------------------
// IR lowering integration
// ---------------------------------------------------------------------------

/// Lowered annotation — ready for codegen.
#[derive(Debug, Clone, PartialEq)]
pub enum LoweredAnnotation {
    /// LLVM inline attribute
    LlvmInline { force: bool },
    /// Disable bounds check for a region
    NoBoundsCheck { region_id: u64 },
    /// LLVM/MLIR vectorization hint
    VectorizeHint { width: u32, region_id: u64 },
    /// Target placement metadata
    Placement { target: TargetSpec, region_id: u64 },
    /// Loop unroll metadata
    UnrollHint { factor: Option<u32>, region_id: u64 },
    /// Loop fusion hint
    FuseHint { region_id: u64 },
    /// Parallelization hint
    ParallelHint { region_id: u64 },
}

/// Lower an annotation set to codegen-ready annotations.
pub fn lower_annotations(
    set: &AnnotationSet,
    region_id: u64,
) -> Vec<LoweredAnnotation> {
    let mut lowered = Vec::new();
    for ann in set.iter() {
        match ann {
            PerfAnnotation::InlineForce => {
                lowered.push(LoweredAnnotation::LlvmInline { force: true });
            }
            PerfAnnotation::InlineHint => {
                lowered.push(LoweredAnnotation::LlvmInline { force: false });
            }
            PerfAnnotation::NoBoundsCheck => {
                lowered.push(LoweredAnnotation::NoBoundsCheck { region_id });
            }
            PerfAnnotation::Vectorize(w) => {
                lowered.push(LoweredAnnotation::VectorizeHint {
                    width: *w,
                    region_id,
                });
            }
            PerfAnnotation::TargetPlacement(t) => {
                lowered.push(LoweredAnnotation::Placement {
                    target: t.clone(),
                    region_id,
                });
            }
            PerfAnnotation::Unroll(n) => {
                lowered.push(LoweredAnnotation::UnrollHint {
                    factor: *n,
                    region_id,
                });
            }
            PerfAnnotation::Fuse => {
                lowered.push(LoweredAnnotation::FuseHint { region_id });
            }
            PerfAnnotation::Parallelize => {
                lowered.push(LoweredAnnotation::ParallelHint { region_id });
            }
        }
    }
    lowered
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Display tests --

    #[test]
    fn test_display_inline_force() {
        assert_eq!(PerfAnnotation::InlineForce.to_string(), "@pi!");
    }

    #[test]
    fn test_display_vectorize() {
        assert_eq!(PerfAnnotation::Vectorize(4).to_string(), "@pv(4)");
    }

    #[test]
    fn test_display_target() {
        assert_eq!(
            PerfAnnotation::TargetPlacement(TargetSpec::Gpu).to_string(),
            "@pt(gpu)"
        );
        assert_eq!(
            PerfAnnotation::TargetPlacement(TargetSpec::Numa(2)).to_string(),
            "@pt(numa:2)"
        );
    }

    #[test]
    fn test_display_unroll() {
        assert_eq!(PerfAnnotation::Unroll(None).to_string(), "@pu");
        assert_eq!(PerfAnnotation::Unroll(Some(4)).to_string(), "@pu(4)");
    }

    // -- Parsing tests --

    #[test]
    fn test_parse_inline_force() {
        assert_eq!(parse_annotation("@pi!"), Ok(PerfAnnotation::InlineForce));
    }

    #[test]
    fn test_parse_inline_hint() {
        assert_eq!(parse_annotation("@pi"), Ok(PerfAnnotation::InlineHint));
    }

    #[test]
    fn test_parse_no_bounds_check() {
        assert_eq!(parse_annotation("@pnb"), Ok(PerfAnnotation::NoBoundsCheck));
    }

    #[test]
    fn test_parse_vectorize() {
        assert_eq!(parse_annotation("@pv(8)"), Ok(PerfAnnotation::Vectorize(8)));
    }

    #[test]
    fn test_parse_vectorize_non_power_of_two() {
        assert!(parse_annotation("@pv(3)").is_err());
    }

    #[test]
    fn test_parse_target_gpu() {
        assert_eq!(
            parse_annotation("@pt(gpu)"),
            Ok(PerfAnnotation::TargetPlacement(TargetSpec::Gpu))
        );
    }

    #[test]
    fn test_parse_target_numa() {
        assert_eq!(
            parse_annotation("@pt(numa:0)"),
            Ok(PerfAnnotation::TargetPlacement(TargetSpec::Numa(0)))
        );
    }

    #[test]
    fn test_parse_target_custom() {
        assert_eq!(
            parse_annotation("@pt(fpga)"),
            Ok(PerfAnnotation::TargetPlacement(TargetSpec::Custom("fpga".to_string())))
        );
    }

    #[test]
    fn test_parse_unroll_no_arg() {
        assert_eq!(parse_annotation("@pu"), Ok(PerfAnnotation::Unroll(None)));
    }

    #[test]
    fn test_parse_unroll_with_arg() {
        assert_eq!(parse_annotation("@pu(8)"), Ok(PerfAnnotation::Unroll(Some(8))));
    }

    #[test]
    fn test_parse_parallelize() {
        assert_eq!(parse_annotation("@pp"), Ok(PerfAnnotation::Parallelize));
    }

    #[test]
    fn test_parse_fuse() {
        assert_eq!(parse_annotation("@pf"), Ok(PerfAnnotation::Fuse));
    }

    #[test]
    fn test_parse_unknown() {
        assert!(parse_annotation("@xyz").is_err());
    }

    #[test]
    fn test_parse_multiple() {
        let anns = parse_annotations("@pi! @pnb @pv(4)").unwrap();
        assert_eq!(anns.len(), 3);
        assert_eq!(anns[0], PerfAnnotation::InlineForce);
        assert_eq!(anns[1], PerfAnnotation::NoBoundsCheck);
        assert_eq!(anns[2], PerfAnnotation::Vectorize(4));
    }

    // -- Validation tests --

    #[test]
    fn test_validate_inline_on_function() {
        assert!(validate_annotation(&PerfAnnotation::InlineForce, &AnnotationTarget::Function).is_ok());
    }

    #[test]
    fn test_validate_inline_on_loop_fails() {
        assert!(validate_annotation(&PerfAnnotation::InlineForce, &AnnotationTarget::Loop).is_err());
    }

    #[test]
    fn test_validate_vectorize_on_loop() {
        assert!(validate_annotation(&PerfAnnotation::Vectorize(4), &AnnotationTarget::Loop).is_ok());
    }

    #[test]
    fn test_validate_vectorize_on_function_fails() {
        assert!(validate_annotation(&PerfAnnotation::Vectorize(4), &AnnotationTarget::Function).is_err());
    }

    // -- Annotation set tests --

    #[test]
    fn test_annotation_set_queries() {
        let mut set = AnnotationSet::new();
        set.add(PerfAnnotation::InlineForce);
        set.add(PerfAnnotation::Vectorize(8));
        set.add(PerfAnnotation::TargetPlacement(TargetSpec::Gpu));

        assert!(set.has_inline_force());
        assert!(!set.has_inline_hint());
        assert_eq!(set.vectorize_width(), Some(8));
        assert_eq!(set.target_placement(), Some(&TargetSpec::Gpu));
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_annotation_set_conflicts() {
        let mut set = AnnotationSet::new();
        set.add(PerfAnnotation::InlineForce);
        set.add(PerfAnnotation::InlineHint);
        let conflicts = set.check_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].contains("redundant"));
    }

    #[test]
    fn test_annotation_set_validate_all() {
        let mut set = AnnotationSet::new();
        set.add(PerfAnnotation::InlineForce);
        set.add(PerfAnnotation::Vectorize(4));
        let errors = set.validate_all(&AnnotationTarget::Function);
        assert_eq!(errors.len(), 1); // Vectorize not valid on function
    }

    #[test]
    fn test_annotation_set_display() {
        let mut set = AnnotationSet::new();
        set.add(PerfAnnotation::InlineForce);
        set.add(PerfAnnotation::NoBoundsCheck);
        assert_eq!(set.to_string(), "@pi! @pnb");
    }

    // -- Lowering tests --

    #[test]
    fn test_lower_annotations() {
        let mut set = AnnotationSet::new();
        set.add(PerfAnnotation::InlineForce);
        set.add(PerfAnnotation::Vectorize(4));
        set.add(PerfAnnotation::Fuse);

        let lowered = lower_annotations(&set, 42);
        assert_eq!(lowered.len(), 3);
        assert!(matches!(lowered[0], LoweredAnnotation::LlvmInline { force: true }));
        assert!(matches!(lowered[1], LoweredAnnotation::VectorizeHint { width: 4, region_id: 42 }));
        assert!(matches!(lowered[2], LoweredAnnotation::FuseHint { region_id: 42 }));
    }

    // -- Error display tests --

    #[test]
    fn test_parse_error_display() {
        let e = ParseError::UnknownAnnotation("@bad".to_string());
        assert!(e.to_string().contains("unknown annotation"));
    }

    #[test]
    fn test_validation_error_display() {
        let e = ValidationError {
            annotation: PerfAnnotation::Vectorize(4),
            target: AnnotationTarget::Function,
            message: "not valid".to_string(),
        };
        assert!(e.to_string().contains("@pv(4)"));
        assert!(e.to_string().contains("function"));
    }
}

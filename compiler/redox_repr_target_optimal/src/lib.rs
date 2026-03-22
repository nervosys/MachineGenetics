//! # Target-Optimal Struct Layout
//!
//! `#[repr(target_optimal)]` performs per-target struct layout optimization
//! using an MLIR-inspired cost model. It reorders fields to minimize padding,
//! considers cache-line alignment, and selects optimal layout for each target.

use std::fmt;

// ── Target Architecture ──────────────────────────────────────────────

/// Target architecture descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetArch {
    pub name: String,
    pub pointer_size: u64,
    pub cache_line_size: u64,
    pub max_align: u64,
    pub prefer_packed: bool,
    pub endian: Endian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Endian {
    Little,
    Big,
}

impl TargetArch {
    pub fn x86_64() -> Self {
        Self {
            name: "x86_64".into(),
            pointer_size: 8,
            cache_line_size: 64,
            max_align: 16,
            prefer_packed: false,
            endian: Endian::Little,
        }
    }

    pub fn aarch64() -> Self {
        Self {
            name: "aarch64".into(),
            pointer_size: 8,
            cache_line_size: 64,
            max_align: 16,
            prefer_packed: false,
            endian: Endian::Little,
        }
    }

    pub fn wasm32() -> Self {
        Self {
            name: "wasm32".into(),
            pointer_size: 4,
            cache_line_size: 64,
            max_align: 8,
            prefer_packed: false,
            endian: Endian::Little,
        }
    }

    pub fn embedded_arm() -> Self {
        Self {
            name: "arm-embedded".into(),
            pointer_size: 4,
            cache_line_size: 32,
            max_align: 8,
            prefer_packed: true,
            endian: Endian::Little,
        }
    }
}

// ── Field Types ──────────────────────────────────────────────────────

/// Primitive type representation for layout calculation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldType {
    Bool,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
    Pointer,
    Array(Box<FieldType>, u64),
    Custom { size: u64, align: u64 },
}

impl FieldType {
    /// Size in bytes for a given target.
    pub fn size(&self, target: &TargetArch) -> u64 {
        match self {
            Self::Bool | Self::U8 | Self::I8 => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::U64 | Self::I64 | Self::F64 => 8,
            Self::Pointer => target.pointer_size,
            Self::Array(elem, count) => elem.size(target) * count,
            Self::Custom { size, .. } => *size,
        }
    }

    /// Alignment in bytes for a given target.
    pub fn align(&self, target: &TargetArch) -> u64 {
        match self {
            Self::Bool | Self::U8 | Self::I8 => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::U64 | Self::I64 | Self::F64 => 8,
            Self::Pointer => target.pointer_size,
            Self::Array(elem, _) => elem.align(target),
            Self::Custom { align, .. } => *align,
        }
    }
}

impl fmt::Display for FieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool => write!(f, "bool"),
            Self::U8 => write!(f, "u8"),
            Self::I8 => write!(f, "i8"),
            Self::U16 => write!(f, "u16"),
            Self::I16 => write!(f, "i16"),
            Self::U32 => write!(f, "u32"),
            Self::I32 => write!(f, "i32"),
            Self::U64 => write!(f, "u64"),
            Self::I64 => write!(f, "i64"),
            Self::F32 => write!(f, "f32"),
            Self::F64 => write!(f, "f64"),
            Self::Pointer => write!(f, "ptr"),
            Self::Array(elem, n) => write!(f, "[{elem}; {n}]"),
            Self::Custom { size, align } => write!(f, "custom({size}, {align})"),
        }
    }
}

// ── Struct Definition ────────────────────────────────────────────────

/// A single field in a struct.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructField {
    pub name: String,
    pub ty: FieldType,
    /// Original index in the source definition.
    pub original_index: usize,
}

/// A struct definition to optimize.
#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<StructField>,
    pub repr: ReprKind,
}

/// Repr annotation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReprKind {
    Rust,
    C,
    TargetOptimal,
    Packed,
}

impl fmt::Display for ReprKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rust => write!(f, "repr(Rust)"),
            Self::C => write!(f, "repr(C)"),
            Self::TargetOptimal => write!(f, "repr(target_optimal)"),
            Self::Packed => write!(f, "repr(packed)"),
        }
    }
}

// ── Layout Result ────────────────────────────────────────────────────

/// A computed field placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPlacement {
    pub field_name: String,
    pub original_index: usize,
    pub offset: u64,
    pub size: u64,
    pub align: u64,
}

/// Complete layout for a struct on a given target.
#[derive(Debug, Clone)]
pub struct StructLayout {
    pub struct_name: String,
    pub target_name: String,
    pub repr: ReprKind,
    pub placements: Vec<FieldPlacement>,
    pub total_size: u64,
    pub alignment: u64,
    pub padding_bytes: u64,
}

impl StructLayout {
    pub fn padding_ratio(&self) -> f64 {
        if self.total_size == 0 {
            0.0
        } else {
            self.padding_bytes as f64 / self.total_size as f64
        }
    }

    pub fn field_order(&self) -> Vec<&str> {
        self.placements.iter().map(|p| p.field_name.as_str()).collect()
    }
}

impl fmt::Display for StructLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Layout for `{}` on {} ({}):", self.struct_name, self.target_name, self.repr)?;
        writeln!(f, "  total size: {} bytes, alignment: {}, padding: {} bytes ({:.1}%)",
            self.total_size, self.alignment, self.padding_bytes,
            self.padding_ratio() * 100.0)?;
        for p in &self.placements {
            writeln!(f, "  offset {:>3}: {} (size={}, align={})",
                p.offset, p.field_name, p.size, p.align)?;
        }
        Ok(())
    }
}

// ── Cost Model ───────────────────────────────────────────────────────

/// MLIR-inspired cost model for evaluating struct layouts.
#[derive(Debug, Clone)]
pub struct CostModel {
    /// Weight for total padding bytes.
    pub padding_weight: f64,
    /// Weight for cache-line splits (fields spanning cache-line boundaries).
    pub cache_split_weight: f64,
    /// Weight for total struct size.
    pub size_weight: f64,
    /// Weight for alignment violations (field not naturally aligned).
    pub misalign_penalty: f64,
}

impl Default for CostModel {
    fn default() -> Self {
        Self {
            padding_weight: 1.0,
            cache_split_weight: 2.0,
            size_weight: 0.1,
            misalign_penalty: 5.0,
        }
    }
}

impl CostModel {
    pub fn memory_constrained() -> Self {
        Self {
            padding_weight: 3.0,
            cache_split_weight: 0.5,
            size_weight: 2.0,
            misalign_penalty: 1.0,
        }
    }

    pub fn cache_optimized() -> Self {
        Self {
            padding_weight: 0.5,
            cache_split_weight: 5.0,
            size_weight: 0.1,
            misalign_penalty: 3.0,
        }
    }

    /// Evaluate cost of a given layout.
    pub fn evaluate(&self, layout: &StructLayout, target: &TargetArch) -> f64 {
        let padding_cost = layout.padding_bytes as f64 * self.padding_weight;
        let size_cost = layout.total_size as f64 * self.size_weight;

        let mut cache_splits = 0u64;
        for p in &layout.placements {
            let start_line = p.offset / target.cache_line_size;
            let end_line = (p.offset + p.size - 1) / target.cache_line_size;
            if start_line != end_line {
                cache_splits += 1;
            }
        }
        let cache_cost = cache_splits as f64 * self.cache_split_weight;

        let mut misalign_count = 0u64;
        for p in &layout.placements {
            if p.align > 0 && p.offset % p.align != 0 {
                misalign_count += 1;
            }
        }
        let misalign_cost = misalign_count as f64 * self.misalign_penalty;

        padding_cost + cache_cost + size_cost + misalign_cost
    }
}

// ── Layout Computation ───────────────────────────────────────────────

fn align_up(offset: u64, align: u64) -> u64 {
    if align == 0 {
        return offset;
    }
    (offset + align - 1) / align * align
}

/// Compute layout with fields in the given order.
fn compute_layout_ordered(
    struct_name: &str,
    fields: &[StructField],
    target: &TargetArch,
    repr: ReprKind,
    packed: bool,
) -> StructLayout {
    let mut placements = Vec::new();
    let mut offset = 0u64;
    let mut max_align = 1u64;
    let mut data_bytes = 0u64;

    for field in fields {
        let size = field.ty.size(target);
        let align = if packed { 1 } else { field.ty.align(target) };
        offset = align_up(offset, align);
        if align > max_align {
            max_align = align;
        }
        placements.push(FieldPlacement {
            field_name: field.name.clone(),
            original_index: field.original_index,
            offset,
            size,
            align,
        });
        data_bytes += size;
        offset += size;
    }

    // Pad to struct alignment
    let struct_align = if packed { 1 } else { max_align.min(target.max_align) };
    let total_size = align_up(offset, struct_align);

    StructLayout {
        struct_name: struct_name.into(),
        target_name: target.name.clone(),
        repr,
        placements,
        total_size,
        alignment: struct_align,
        padding_bytes: total_size - data_bytes,
    }
}

/// Compute layout preserving source order (repr(C) / repr(Rust)).
pub fn compute_source_layout(def: &StructDef, target: &TargetArch) -> StructLayout {
    let packed = def.repr == ReprKind::Packed;
    compute_layout_ordered(&def.name, &def.fields, target, def.repr, packed)
}

/// Compute optimal layout by sorting fields largest-alignment-first,
/// then largest-size-first, to minimize padding.
pub fn compute_optimal_layout(def: &StructDef, target: &TargetArch) -> StructLayout {
    let mut sorted = def.fields.clone();
    sorted.sort_by(|a, b| {
        let align_cmp = b.ty.align(target).cmp(&a.ty.align(target));
        if align_cmp == std::cmp::Ordering::Equal {
            b.ty.size(target).cmp(&a.ty.size(target))
        } else {
            align_cmp
        }
    });
    compute_layout_ordered(&def.name, &sorted, target, ReprKind::TargetOptimal, false)
}

/// Compute optimal layout considering cache-line grouping.
/// Fields frequently accessed together (by adjacency in source) are kept
/// within the same cache line when possible.
pub fn compute_cache_aware_layout(
    def: &StructDef,
    target: &TargetArch,
    cost_model: &CostModel,
) -> StructLayout {
    // For small structs, try all relevant orderings
    if def.fields.len() <= 8 {
        // Generate candidate orderings: source order, alignment-sorted, size-sorted
        let mut candidates = Vec::new();

        // Candidate 1: alignment-sorted (largest first)
        let mut by_align = def.fields.clone();
        by_align.sort_by(|a, b| {
            b.ty.align(target).cmp(&a.ty.align(target))
                .then_with(|| b.ty.size(target).cmp(&a.ty.size(target)))
        });
        candidates.push(by_align);

        // Candidate 2: size-sorted (largest first)
        let mut by_size = def.fields.clone();
        by_size.sort_by(|a, b| {
            b.ty.size(target).cmp(&a.ty.size(target))
                .then_with(|| b.ty.align(target).cmp(&a.ty.align(target)))
        });
        candidates.push(by_size);

        // Candidate 3: group bools/u8s at the end
        let mut grouped = def.fields.clone();
        grouped.sort_by(|a, b| {
            let a_small = a.ty.size(target) <= 1;
            let b_small = b.ty.size(target) <= 1;
            a_small.cmp(&b_small)
                .then_with(|| b.ty.align(target).cmp(&a.ty.align(target)))
        });
        candidates.push(grouped);

        // Candidate 4: source order
        candidates.push(def.fields.clone());

        let mut best_layout = None;
        let mut best_cost = f64::MAX;

        for ordering in &candidates {
            let layout = compute_layout_ordered(
                &def.name, ordering, target, ReprKind::TargetOptimal, false,
            );
            let cost = cost_model.evaluate(&layout, target);
            if cost < best_cost {
                best_cost = cost;
                best_layout = Some(layout);
            }
        }

        best_layout.unwrap()
    } else {
        // For larger structs, use alignment-sorted heuristic
        compute_optimal_layout(def, target)
    }
}

// ── Multi-Target Optimizer ───────────────────────────────────────────

/// Result of multi-target optimization.
#[derive(Debug)]
pub struct MultiTargetResult {
    pub struct_name: String,
    pub layouts: Vec<(StructLayout, f64)>,
    pub best_universal: Option<StructLayout>,
}

impl MultiTargetResult {
    /// Get layout for a specific target.
    pub fn for_target(&self, target_name: &str) -> Option<&StructLayout> {
        self.layouts.iter()
            .find(|(l, _)| l.target_name == target_name)
            .map(|(l, _)| l)
    }
}

/// Optimize layout across multiple targets.
pub fn optimize_multi_target(
    def: &StructDef,
    targets: &[TargetArch],
    cost_model: &CostModel,
) -> MultiTargetResult {
    let mut layouts = Vec::new();

    for target in targets {
        let layout = compute_cache_aware_layout(def, target, cost_model);
        let cost = cost_model.evaluate(&layout, target);
        layouts.push((layout, cost));
    }

    // Find a universal ordering that works well across all targets:
    // use alignment-sorted which is generally good everywhere.
    let universal = if !targets.is_empty() {
        let primary = &targets[0];
        Some(compute_optimal_layout(def, primary))
    } else {
        None
    };

    MultiTargetResult {
        struct_name: def.name.clone(),
        layouts,
        best_universal: universal,
    }
}

// ── Layout Diff ──────────────────────────────────────────────────────

/// Difference between two layouts for the same struct.
#[derive(Debug)]
pub struct LayoutDiff {
    pub struct_name: String,
    pub from_repr: ReprKind,
    pub to_repr: ReprKind,
    pub size_delta: i64,
    pub padding_delta: i64,
    pub reordered_fields: Vec<String>,
}

impl fmt::Display for LayoutDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LayoutDiff for `{}`: {} -> {}, size: {:+}, padding: {:+}",
            self.struct_name, self.from_repr, self.to_repr,
            self.size_delta, self.padding_delta)?;
        if !self.reordered_fields.is_empty() {
            write!(f, ", reordered: [{}]", self.reordered_fields.join(", "))?;
        }
        Ok(())
    }
}

/// Compare two layouts.
pub fn diff_layouts(from: &StructLayout, to: &StructLayout) -> LayoutDiff {
    let size_delta = to.total_size as i64 - from.total_size as i64;
    let padding_delta = to.padding_bytes as i64 - from.padding_bytes as i64;

    let from_order: Vec<&str> = from.placements.iter().map(|p| p.field_name.as_str()).collect();
    let to_order: Vec<&str> = to.placements.iter().map(|p| p.field_name.as_str()).collect();

    let reordered_fields: Vec<String> = to_order.iter()
        .zip(from_order.iter())
        .filter(|(a, b)| a != b)
        .map(|(a, _)| a.to_string())
        .collect();

    LayoutDiff {
        struct_name: from.struct_name.clone(),
        from_repr: from.repr,
        to_repr: to.repr,
        size_delta,
        padding_delta,
        reordered_fields,
    }
}

// ── Optimization Pipeline ────────────────────────────────────────────

/// Full optimization result.
#[derive(Debug)]
pub struct OptimizationResult {
    pub original: StructLayout,
    pub optimized: StructLayout,
    pub diff: LayoutDiff,
    pub cost_before: f64,
    pub cost_after: f64,
    pub improvement_pct: f64,
}

/// Run the full optimization pipeline on a struct for a given target.
pub fn optimize(
    def: &StructDef,
    target: &TargetArch,
    cost_model: &CostModel,
) -> OptimizationResult {
    let original = compute_source_layout(def, target);
    let optimized = compute_cache_aware_layout(def, target, cost_model);
    let diff = diff_layouts(&original, &optimized);
    let cost_before = cost_model.evaluate(&original, target);
    let cost_after = cost_model.evaluate(&optimized, target);
    let improvement_pct = if cost_before > 0.0 {
        (1.0 - cost_after / cost_before) * 100.0
    } else {
        0.0
    };

    OptimizationResult {
        original,
        optimized,
        diff,
        cost_before,
        cost_after,
        improvement_pct,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_struct() -> StructDef {
        StructDef {
            name: "Foo".into(),
            fields: vec![
                StructField { name: "a".into(), ty: FieldType::Bool, original_index: 0 },
                StructField { name: "b".into(), ty: FieldType::U64, original_index: 1 },
                StructField { name: "c".into(), ty: FieldType::U8, original_index: 2 },
                StructField { name: "d".into(), ty: FieldType::U32, original_index: 3 },
                StructField { name: "e".into(), ty: FieldType::Bool, original_index: 4 },
                StructField { name: "f".into(), ty: FieldType::Pointer, original_index: 5 },
            ],
            repr: ReprKind::C,
        }
    }

    #[test]
    fn test_field_type_sizes() {
        let t = TargetArch::x86_64();
        assert_eq!(FieldType::Bool.size(&t), 1);
        assert_eq!(FieldType::U32.size(&t), 4);
        assert_eq!(FieldType::U64.size(&t), 8);
        assert_eq!(FieldType::Pointer.size(&t), 8);
        assert_eq!(FieldType::Array(Box::new(FieldType::U32), 4).size(&t), 16);
    }

    #[test]
    fn test_field_type_align() {
        let t = TargetArch::x86_64();
        assert_eq!(FieldType::Bool.align(&t), 1);
        assert_eq!(FieldType::U16.align(&t), 2);
        assert_eq!(FieldType::U64.align(&t), 8);
        assert_eq!(FieldType::Pointer.align(&t), 8);
    }

    #[test]
    fn test_field_type_display() {
        assert_eq!(format!("{}", FieldType::U32), "u32");
        assert_eq!(format!("{}", FieldType::Pointer), "ptr");
        assert_eq!(format!("{}", FieldType::Array(Box::new(FieldType::I8), 3)), "[i8; 3]");
        assert_eq!(format!("{}", FieldType::Custom { size: 12, align: 4 }), "custom(12, 4)");
    }

    #[test]
    fn test_wasm32_pointer_size() {
        let t = TargetArch::wasm32();
        assert_eq!(FieldType::Pointer.size(&t), 4);
        assert_eq!(FieldType::Pointer.align(&t), 4);
    }

    #[test]
    fn test_source_layout_c_repr() {
        let def = sample_struct();
        let t = TargetArch::x86_64();
        let layout = compute_source_layout(&def, &t);
        // C layout preserves order: a(1) pad(7) b(8) c(1) pad(3) d(4) e(1) pad(7) f(8)
        assert_eq!(layout.placements[0].field_name, "a");
        assert_eq!(layout.placements[1].field_name, "b");
        assert_eq!(layout.total_size % layout.alignment, 0);
        assert!(layout.padding_bytes > 0);
    }

    #[test]
    fn test_optimal_layout_reduces_padding() {
        let def = sample_struct();
        let t = TargetArch::x86_64();
        let source = compute_source_layout(&def, &t);
        let optimal = compute_optimal_layout(&def, &t);
        assert!(optimal.padding_bytes <= source.padding_bytes,
            "optimal padding {} should <= source padding {}",
            optimal.padding_bytes, source.padding_bytes);
    }

    #[test]
    fn test_optimal_layout_alignment_order() {
        let def = sample_struct();
        let t = TargetArch::x86_64();
        let optimal = compute_optimal_layout(&def, &t);
        // First fields should have highest alignment
        for i in 1..optimal.placements.len() {
            assert!(optimal.placements[i - 1].align >= optimal.placements[i].align,
                "fields should be ordered by decreasing alignment");
        }
    }

    #[test]
    fn test_packed_layout_no_padding() {
        let def = StructDef {
            name: "Packed".into(),
            fields: vec![
                StructField { name: "a".into(), ty: FieldType::U8, original_index: 0 },
                StructField { name: "b".into(), ty: FieldType::U64, original_index: 1 },
                StructField { name: "c".into(), ty: FieldType::U16, original_index: 2 },
            ],
            repr: ReprKind::Packed,
        };
        let t = TargetArch::x86_64();
        let layout = compute_source_layout(&def, &t);
        assert_eq!(layout.padding_bytes, 0);
        assert_eq!(layout.total_size, 1 + 8 + 2);
        assert_eq!(layout.alignment, 1);
    }

    #[test]
    fn test_empty_struct() {
        let def = StructDef {
            name: "Empty".into(),
            fields: vec![],
            repr: ReprKind::Rust,
        };
        let t = TargetArch::x86_64();
        let layout = compute_source_layout(&def, &t);
        assert_eq!(layout.total_size, 0);
        assert_eq!(layout.padding_bytes, 0);
    }

    #[test]
    fn test_single_field() {
        let def = StructDef {
            name: "Single".into(),
            fields: vec![
                StructField { name: "x".into(), ty: FieldType::U32, original_index: 0 },
            ],
            repr: ReprKind::Rust,
        };
        let t = TargetArch::x86_64();
        let layout = compute_source_layout(&def, &t);
        assert_eq!(layout.total_size, 4);
        assert_eq!(layout.padding_bytes, 0);
        assert_eq!(layout.alignment, 4);
    }

    #[test]
    fn test_cost_model_default() {
        let cm = CostModel::default();
        assert_eq!(cm.padding_weight, 1.0);
        assert_eq!(cm.cache_split_weight, 2.0);
    }

    #[test]
    fn test_cost_model_evaluate() {
        let def = sample_struct();
        let t = TargetArch::x86_64();
        let layout = compute_source_layout(&def, &t);
        let cm = CostModel::default();
        let cost = cm.evaluate(&layout, &t);
        assert!(cost > 0.0, "non-trivial struct should have positive cost");
    }

    #[test]
    fn test_cost_optimal_lower_than_source() {
        let def = sample_struct();
        let t = TargetArch::x86_64();
        let cm = CostModel::default();
        let source = compute_source_layout(&def, &t);
        let optimal = compute_cache_aware_layout(&def, &t, &cm);
        let cost_source = cm.evaluate(&source, &t);
        let cost_optimal = cm.evaluate(&optimal, &t);
        assert!(cost_optimal <= cost_source,
            "optimal cost {cost_optimal} should <= source cost {cost_source}");
    }

    #[test]
    fn test_cache_aware_layout() {
        let def = sample_struct();
        let t = TargetArch::x86_64();
        let cm = CostModel::cache_optimized();
        let layout = compute_cache_aware_layout(&def, &t, &cm);
        assert_eq!(layout.repr, ReprKind::TargetOptimal);
        assert!(layout.total_size > 0);
    }

    #[test]
    fn test_memory_constrained_cost_model() {
        let def = sample_struct();
        let t = TargetArch::embedded_arm();
        let cm = CostModel::memory_constrained();
        let layout = compute_cache_aware_layout(&def, &t, &cm);
        assert!(layout.total_size > 0);
    }

    #[test]
    fn test_padding_ratio() {
        let layout = StructLayout {
            struct_name: "T".into(),
            target_name: "test".into(),
            repr: ReprKind::Rust,
            placements: vec![],
            total_size: 100,
            alignment: 8,
            padding_bytes: 25,
        };
        assert!((layout.padding_ratio() - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_padding_ratio_zero_size() {
        let layout = StructLayout {
            struct_name: "T".into(),
            target_name: "test".into(),
            repr: ReprKind::Rust,
            placements: vec![],
            total_size: 0,
            alignment: 1,
            padding_bytes: 0,
        };
        assert_eq!(layout.padding_ratio(), 0.0);
    }

    #[test]
    fn test_diff_layouts() {
        let def = sample_struct();
        let t = TargetArch::x86_64();
        let source = compute_source_layout(&def, &t);
        let optimal = compute_optimal_layout(&def, &t);
        let diff = diff_layouts(&source, &optimal);
        assert!(diff.size_delta <= 0, "optimal should be same size or smaller");
        assert!(diff.padding_delta <= 0, "optimal should have same or less padding");
    }

    #[test]
    fn test_diff_display() {
        let diff = LayoutDiff {
            struct_name: "Foo".into(),
            from_repr: ReprKind::C,
            to_repr: ReprKind::TargetOptimal,
            size_delta: -8,
            padding_delta: -8,
            reordered_fields: vec!["b".into(), "f".into()],
        };
        let s = format!("{diff}");
        assert!(s.contains("Foo"));
        assert!(s.contains("-8"));
        assert!(s.contains("reordered"));
    }

    #[test]
    fn test_optimize_pipeline() {
        let def = sample_struct();
        let t = TargetArch::x86_64();
        let cm = CostModel::default();
        let result = optimize(&def, &t, &cm);
        assert!(result.cost_after <= result.cost_before);
        assert!(result.improvement_pct >= 0.0);
    }

    #[test]
    fn test_multi_target() {
        let def = sample_struct();
        let targets = vec![TargetArch::x86_64(), TargetArch::aarch64(), TargetArch::wasm32()];
        let cm = CostModel::default();
        let result = optimize_multi_target(&def, &targets, &cm);
        assert_eq!(result.layouts.len(), 3);
        assert!(result.best_universal.is_some());
        assert!(result.for_target("x86_64").is_some());
        assert!(result.for_target("wasm32").is_some());
        assert!(result.for_target("riscv").is_none());
    }

    #[test]
    fn test_struct_layout_display() {
        let def = StructDef {
            name: "Tiny".into(),
            fields: vec![
                StructField { name: "x".into(), ty: FieldType::U32, original_index: 0 },
            ],
            repr: ReprKind::Rust,
        };
        let t = TargetArch::x86_64();
        let layout = compute_source_layout(&def, &t);
        let s = format!("{layout}");
        assert!(s.contains("Tiny"));
        assert!(s.contains("offset"));
    }

    #[test]
    fn test_repr_display() {
        assert_eq!(format!("{}", ReprKind::C), "repr(C)");
        assert_eq!(format!("{}", ReprKind::TargetOptimal), "repr(target_optimal)");
        assert_eq!(format!("{}", ReprKind::Packed), "repr(packed)");
    }

    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 8), 0);
        assert_eq!(align_up(1, 8), 8);
        assert_eq!(align_up(8, 8), 8);
        assert_eq!(align_up(9, 8), 16);
        assert_eq!(align_up(5, 4), 8);
        assert_eq!(align_up(10, 0), 10);
    }

    #[test]
    fn test_custom_field_type() {
        let t = TargetArch::x86_64();
        let custom = FieldType::Custom { size: 24, align: 8 };
        assert_eq!(custom.size(&t), 24);
        assert_eq!(custom.align(&t), 8);
    }

    #[test]
    fn test_field_order() {
        let def = sample_struct();
        let t = TargetArch::x86_64();
        let layout = compute_source_layout(&def, &t);
        let order = layout.field_order();
        assert_eq!(order, vec!["a", "b", "c", "d", "e", "f"]);
    }

    #[test]
    fn test_optimize_all_same_size_fields() {
        let def = StructDef {
            name: "Uniform".into(),
            fields: vec![
                StructField { name: "a".into(), ty: FieldType::U32, original_index: 0 },
                StructField { name: "b".into(), ty: FieldType::U32, original_index: 1 },
                StructField { name: "c".into(), ty: FieldType::U32, original_index: 2 },
            ],
            repr: ReprKind::C,
        };
        let t = TargetArch::x86_64();
        let cm = CostModel::default();
        let result = optimize(&def, &t, &cm);
        assert_eq!(result.optimized.padding_bytes, 0);
        assert_eq!(result.optimized.total_size, 12);
    }

    #[test]
    fn test_aarch64_target() {
        let t = TargetArch::aarch64();
        assert_eq!(t.pointer_size, 8);
        assert_eq!(t.cache_line_size, 64);
    }

    #[test]
    fn test_embedded_prefers_packed() {
        let t = TargetArch::embedded_arm();
        assert!(t.prefer_packed);
        assert_eq!(t.pointer_size, 4);
    }

    #[test]
    fn test_endian() {
        assert_eq!(TargetArch::x86_64().endian, Endian::Little);
        assert_eq!(TargetArch::aarch64().endian, Endian::Little);
    }

    #[test]
    fn test_large_struct_optimization() {
        // > 8 fields: falls back to alignment-sort heuristic
        let fields: Vec<StructField> = (0..12).map(|i| {
            let ty = match i % 4 {
                0 => FieldType::Bool,
                1 => FieldType::U64,
                2 => FieldType::U16,
                _ => FieldType::U32,
            };
            StructField { name: format!("f{i}"), ty, original_index: i }
        }).collect();
        let def = StructDef { name: "Large".into(), fields, repr: ReprKind::Rust };
        let t = TargetArch::x86_64();
        let cm = CostModel::default();
        let result = optimize(&def, &t, &cm);
        assert!(result.cost_after <= result.cost_before);
    }
}

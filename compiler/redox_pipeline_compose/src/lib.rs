//! # Pipeline Composition from Specs
//!
//! `pipeline` blocks that chain function contracts with compile-time
//! verification of contract compatibility.
//!
//! ```text
//! pipeline process_data {
//!     read_input |> validate |> transform |> write_output
//! }
//! ```
//!
//! Each stage has a contract. The pipeline verifier checks that each stage's
//! postconditions imply the next stage's preconditions.

use std::collections::HashMap;
use std::fmt;

// ── Contract Expressions ─────────────────────────────────────────────

/// Expression in a contract clause.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    BoolLit(bool),
    IntLit(i64),
    Var(String),
    Result,
    Field(Box<Expr>, String),
    MethodCall(Box<Expr>, String, Vec<Expr>),
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    UnOp(UnOp, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnOp {
    Not,
    Neg,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add => write!(f, "+"),
            Self::Sub => write!(f, "-"),
            Self::Mul => write!(f, "*"),
            Self::Div => write!(f, "/"),
            Self::Eq => write!(f, "=="),
            Self::Ne => write!(f, "!="),
            Self::Lt => write!(f, "<"),
            Self::Le => write!(f, "<="),
            Self::Gt => write!(f, ">"),
            Self::Ge => write!(f, ">="),
            Self::And => write!(f, "&&"),
            Self::Or => write!(f, "||"),
        }
    }
}

impl fmt::Display for UnOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Not => write!(f, "!"),
            Self::Neg => write!(f, "-"),
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoolLit(b) => write!(f, "{b}"),
            Self::IntLit(n) => write!(f, "{n}"),
            Self::Var(v) => write!(f, "{v}"),
            Self::Result => write!(f, "result"),
            Self::Field(e, name) => write!(f, "{e}.{name}"),
            Self::MethodCall(e, name, args) => {
                let a: Vec<String> = args.iter().map(|x| format!("{x}")).collect();
                write!(f, "{e}.{name}({})", a.join(", "))
            }
            Self::BinOp(l, op, r) => write!(f, "({l} {op} {r})"),
            Self::UnOp(op, e) => write!(f, "{op}{e}"),
        }
    }
}

// ── Types ────────────────────────────────────────────────────────────

/// Simple type for pipeline stages.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StageType {
    Int,
    Bool,
    String,
    Array(Box<StageType>),
    Record(Vec<(String, StageType)>),
    Named(String),
    Unit,
}

impl fmt::Display for StageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => write!(f, "int"),
            Self::Bool => write!(f, "bool"),
            Self::String => write!(f, "string"),
            Self::Array(t) => write!(f, "[{t}]"),
            Self::Record(fields) => {
                let fs: Vec<String> = fields.iter().map(|(n, t)| format!("{n}: {t}")).collect();
                write!(f, "{{{}}}", fs.join(", "))
            }
            Self::Named(n) => write!(f, "{n}"),
            Self::Unit => write!(f, "()"),
        }
    }
}

// ── Function Contract ────────────────────────────────────────────────

/// A contract for a single function / pipeline stage.
#[derive(Debug, Clone)]
pub struct FunctionContract {
    pub name: String,
    pub input_type: StageType,
    pub output_type: StageType,
    pub preconditions: Vec<Expr>,
    pub postconditions: Vec<Expr>,
}

impl FunctionContract {
    pub fn new(name: impl Into<String>, input: StageType, output: StageType) -> Self {
        Self {
            name: name.into(),
            input_type: input,
            output_type: output,
            preconditions: Vec::new(),
            postconditions: Vec::new(),
        }
    }

    pub fn pre(mut self, expr: Expr) -> Self {
        self.preconditions.push(expr);
        self
    }

    pub fn post(mut self, expr: Expr) -> Self {
        self.postconditions.push(expr);
        self
    }
}

impl fmt::Display for FunctionContract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({}) -> {}", self.name, self.input_type, self.output_type)?;
        if !self.preconditions.is_empty() {
            let pres: Vec<String> = self.preconditions.iter().map(|e| format!("{e}")).collect();
            write!(f, " @req {}", pres.join(", "))?;
        }
        if !self.postconditions.is_empty() {
            let posts: Vec<String> = self.postconditions.iter().map(|e| format!("{e}")).collect();
            write!(f, " @ens {}", posts.join(", "))?;
        }
        Ok(())
    }
}

// ── Pipeline Definition ──────────────────────────────────────────────

/// A pipeline stage.
#[derive(Debug, Clone)]
pub struct PipelineStage {
    pub function_name: String,
    pub index: usize,
}

/// A pipeline definition.
#[derive(Debug, Clone)]
pub struct Pipeline {
    pub name: String,
    pub stages: Vec<PipelineStage>,
}

impl Pipeline {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), stages: Vec::new() }
    }

    pub fn stage(mut self, function_name: impl Into<String>) -> Self {
        let index = self.stages.len();
        self.stages.push(PipelineStage { function_name: function_name.into(), index });
        self
    }

    pub fn len(&self) -> usize {
        self.stages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }
}

impl fmt::Display for Pipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pipeline {} {{ ", self.name)?;
        let names: Vec<&str> = self.stages.iter().map(|s| s.function_name.as_str()).collect();
        write!(f, "{}", names.join(" |> "))?;
        write!(f, " }}")
    }
}

// ── Contract Registry ────────────────────────────────────────────────

/// Registry of function contracts.
#[derive(Debug, Default)]
pub struct ContractRegistry {
    contracts: HashMap<String, FunctionContract>,
}

impl ContractRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, contract: FunctionContract) {
        self.contracts.insert(contract.name.clone(), contract);
    }

    pub fn get(&self, name: &str) -> Option<&FunctionContract> {
        self.contracts.get(name)
    }

    pub fn len(&self) -> usize {
        self.contracts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.contracts.is_empty()
    }
}

// ── Compatibility Checking ───────────────────────────────────────────

/// Error from pipeline verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError {
    /// A stage has no registered contract.
    MissingContract { stage_index: usize, function_name: String },
    /// Type mismatch between output of stage i and input of stage i+1.
    TypeMismatch {
        from_stage: usize,
        from_name: String,
        from_output: String,
        to_stage: usize,
        to_name: String,
        to_input: String,
    },
    /// Postcondition of stage i does not imply precondition of stage i+1.
    ContractIncompatible {
        from_stage: usize,
        from_name: String,
        to_stage: usize,
        to_name: String,
        postcondition: String,
        precondition: String,
    },
    /// Pipeline is empty.
    EmptyPipeline,
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingContract { stage_index, function_name } => {
                write!(f, "stage {stage_index}: no contract for `{function_name}`")
            }
            Self::TypeMismatch {
                from_stage,
                from_name,
                from_output,
                to_stage,
                to_name,
                to_input,
            } => write!(
                f,
                "type mismatch: stage {from_stage} `{from_name}` outputs {from_output}, but stage {to_stage} `{to_name}` expects {to_input}"
            ),
            Self::ContractIncompatible {
                from_stage,
                from_name,
                to_stage,
                to_name,
                postcondition,
                precondition,
            } => write!(
                f,
                "contract mismatch: stage {from_stage} `{from_name}` postcondition ({postcondition}) may not imply stage {to_stage} `{to_name}` precondition ({precondition})"
            ),
            Self::EmptyPipeline => write!(f, "pipeline is empty"),
        }
    }
}

/// Check if two stage types are compatible.
pub fn types_compatible(output: &StageType, input: &StageType) -> bool {
    match (output, input) {
        (a, b) if a == b => true,
        // Named types match if names match
        (StageType::Named(a), StageType::Named(b)) => a == b,
        // Array subtyping
        (StageType::Array(a), StageType::Array(b)) => types_compatible(a, b),
        // Record subtyping: output record has at least all fields of input record
        (StageType::Record(out_fields), StageType::Record(in_fields)) => in_fields
            .iter()
            .all(|(name, ty)| out_fields.iter().any(|(n, t)| n == name && types_compatible(t, ty))),
        _ => false,
    }
}

/// Check structural compatibility of expressions.
/// A postcondition `P(result)` is compatible with a precondition `Q(input)` if
/// they have a matching structure. This is a conservative static check.
pub fn exprs_structurally_compatible(post: &Expr, pre: &Expr) -> bool {
    // Simple structural heuristic:
    // If postcondition ensures result >= N and precondition requires input >= M where N >= M,
    // it's compatible.
    match (post, pre) {
        // Trivial: pre is just `true`
        (_, Expr::BoolLit(true)) => true,
        // post is `true` but pre is non-trivial => cannot guarantee
        (Expr::BoolLit(true), _) => false,
        // Both are comparison with same operator against literals
        (Expr::BinOp(_, post_op, post_rhs), Expr::BinOp(_, pre_op, pre_rhs)) => {
            match (post_op, pre_op) {
                // post: result > N, pre: input > M => compatible if N >= M
                (BinOp::Gt, BinOp::Gt) | (BinOp::Ge, BinOp::Ge) | (BinOp::Gt, BinOp::Ge) => {
                    match (post_rhs.as_ref(), pre_rhs.as_ref()) {
                        (Expr::IntLit(n), Expr::IntLit(m)) => n >= m,
                        _ => false,
                    }
                }
                // post: result < N, pre: input < M => compatible if N <= M
                (BinOp::Lt, BinOp::Lt) | (BinOp::Le, BinOp::Le) | (BinOp::Lt, BinOp::Le) => {
                    match (post_rhs.as_ref(), pre_rhs.as_ref()) {
                        (Expr::IntLit(n), Expr::IntLit(m)) => n <= m,
                        _ => false,
                    }
                }
                // Same string expressions
                _ => format!("{post}") == format!("{pre}"),
            }
        }
        // Fallback: string comparison (same expression text)
        _ => format!("{post}") == format!("{pre}"),
    }
}

/// Verify a pipeline against a contract registry.
pub fn verify_pipeline(pipeline: &Pipeline, registry: &ContractRegistry) -> Vec<PipelineError> {
    let mut errors = Vec::new();

    if pipeline.stages.is_empty() {
        errors.push(PipelineError::EmptyPipeline);
        return errors;
    }

    // Check all stages have contracts
    let mut contracts: Vec<Option<&FunctionContract>> = Vec::new();
    for stage in &pipeline.stages {
        match registry.get(&stage.function_name) {
            Some(c) => contracts.push(Some(c)),
            None => {
                errors.push(PipelineError::MissingContract {
                    stage_index: stage.index,
                    function_name: stage.function_name.clone(),
                });
                contracts.push(None);
            }
        }
    }

    // Check adjacent stage compatibility
    for i in 0..pipeline.stages.len() - 1 {
        let (from_c, to_c) = match (&contracts[i], &contracts[i + 1]) {
            (Some(a), Some(b)) => (a, b),
            _ => continue, // Skip if missing contract
        };

        // Type compatibility
        if !types_compatible(&from_c.output_type, &to_c.input_type) {
            errors.push(PipelineError::TypeMismatch {
                from_stage: i,
                from_name: from_c.name.clone(),
                from_output: format!("{}", from_c.output_type),
                to_stage: i + 1,
                to_name: to_c.name.clone(),
                to_input: format!("{}", to_c.input_type),
            });
        }

        // Contract compatibility: each postcondition of `from` should
        // support each precondition of `to`
        for pre in &to_c.preconditions {
            let mut supported = false;
            for post in &from_c.postconditions {
                if exprs_structurally_compatible(post, pre) {
                    supported = true;
                    break;
                }
            }
            if !supported && !from_c.postconditions.is_empty() {
                // Find the most relevant postcondition for error reporting
                let post_str = if !from_c.postconditions.is_empty() {
                    format!("{}", from_c.postconditions[0])
                } else {
                    "(none)".into()
                };
                errors.push(PipelineError::ContractIncompatible {
                    from_stage: i,
                    from_name: from_c.name.clone(),
                    to_stage: i + 1,
                    to_name: to_c.name.clone(),
                    postcondition: post_str,
                    precondition: format!("{pre}"),
                });
            }
        }
    }

    errors
}

// ── Pipeline Compilation ─────────────────────────────────────────────

/// Compiled pipeline stage.
#[derive(Debug, Clone)]
pub struct CompiledStage {
    pub function_name: String,
    pub input_type: StageType,
    pub output_type: StageType,
    pub index: usize,
}

/// Compiled pipeline ready for execution.
#[derive(Debug, Clone)]
pub struct CompiledPipeline {
    pub name: String,
    pub stages: Vec<CompiledStage>,
    pub input_type: StageType,
    pub output_type: StageType,
}

impl fmt::Display for CompiledPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "compiled pipeline `{}`: {} -> {} ({} stages)",
            self.name,
            self.input_type,
            self.output_type,
            self.stages.len()
        )
    }
}

/// Compile a verified pipeline.
pub fn compile_pipeline(
    pipeline: &Pipeline,
    registry: &ContractRegistry,
) -> Result<CompiledPipeline, Vec<PipelineError>> {
    let errors = verify_pipeline(pipeline, registry);
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut compiled_stages = Vec::new();
    for stage in &pipeline.stages {
        let contract = registry.get(&stage.function_name).unwrap();
        compiled_stages.push(CompiledStage {
            function_name: stage.function_name.clone(),
            input_type: contract.input_type.clone(),
            output_type: contract.output_type.clone(),
            index: stage.index,
        });
    }

    let input_type =
        compiled_stages.first().map(|s| s.input_type.clone()).unwrap_or(StageType::Unit);
    let output_type =
        compiled_stages.last().map(|s| s.output_type.clone()).unwrap_or(StageType::Unit);

    Ok(CompiledPipeline {
        name: pipeline.name.clone(),
        stages: compiled_stages,
        input_type,
        output_type,
    })
}

// ── Pipeline Analysis ────────────────────────────────────────────────

/// Summary of a pipeline for reporting.
#[derive(Debug)]
pub struct PipelineAnalysis {
    pub name: String,
    pub stage_count: usize,
    pub type_chain: Vec<String>,
    pub contract_coverage: f64,
    pub errors: Vec<PipelineError>,
    pub is_valid: bool,
}

impl fmt::Display for PipelineAnalysis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Pipeline `{}`:", self.name)?;
        writeln!(f, "  stages: {}", self.stage_count)?;
        writeln!(f, "  types:  {}", self.type_chain.join(" -> "))?;
        writeln!(f, "  coverage: {:.0}%", self.contract_coverage * 100.0)?;
        writeln!(f, "  valid: {}", self.is_valid)?;
        if !self.errors.is_empty() {
            writeln!(f, "  errors:")?;
            for e in &self.errors {
                writeln!(f, "    - {e}")?;
            }
        }
        Ok(())
    }
}

/// Analyze a pipeline.
pub fn analyze_pipeline(pipeline: &Pipeline, registry: &ContractRegistry) -> PipelineAnalysis {
    let errors = verify_pipeline(pipeline, registry);
    let stage_count = pipeline.stages.len();

    let mut type_chain = Vec::new();
    let mut contracts_found = 0usize;
    for stage in &pipeline.stages {
        if let Some(c) = registry.get(&stage.function_name) {
            if type_chain.is_empty() {
                type_chain.push(format!("{}", c.input_type));
            }
            type_chain.push(format!("{}", c.output_type));
            contracts_found += 1;
        } else {
            type_chain.push("?".into());
        }
    }

    let coverage = if stage_count > 0 { contracts_found as f64 / stage_count as f64 } else { 0.0 };

    PipelineAnalysis {
        name: pipeline.name.clone(),
        stage_count,
        type_chain,
        contract_coverage: coverage,
        errors: errors.clone(),
        is_valid: errors.is_empty(),
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_registry() -> ContractRegistry {
        let mut reg = ContractRegistry::new();
        reg.register(FunctionContract::new("read_input", StageType::Unit, StageType::String).post(
            Expr::BinOp(
                Box::new(Expr::MethodCall(Box::new(Expr::Result), "len".into(), vec![])),
                BinOp::Gt,
                Box::new(Expr::IntLit(0)),
            ),
        ));
        reg.register(
            FunctionContract::new("validate", StageType::String, StageType::String)
                .pre(Expr::BinOp(
                    Box::new(Expr::MethodCall(
                        Box::new(Expr::Var("input".into())),
                        "len".into(),
                        vec![],
                    )),
                    BinOp::Gt,
                    Box::new(Expr::IntLit(0)),
                ))
                .post(Expr::BinOp(
                    Box::new(Expr::MethodCall(Box::new(Expr::Result), "len".into(), vec![])),
                    BinOp::Gt,
                    Box::new(Expr::IntLit(0)),
                )),
        );
        reg.register(
            FunctionContract::new("transform", StageType::String, StageType::Int)
                .pre(Expr::BinOp(
                    Box::new(Expr::MethodCall(
                        Box::new(Expr::Var("input".into())),
                        "len".into(),
                        vec![],
                    )),
                    BinOp::Gt,
                    Box::new(Expr::IntLit(0)),
                ))
                .post(Expr::BinOp(Box::new(Expr::Result), BinOp::Ge, Box::new(Expr::IntLit(0)))),
        );
        reg.register(FunctionContract::new("write_output", StageType::Int, StageType::Unit).pre(
            Expr::BinOp(Box::new(Expr::Var("input".into())), BinOp::Ge, Box::new(Expr::IntLit(0))),
        ));
        reg
    }

    fn valid_pipeline() -> Pipeline {
        Pipeline::new("process_data")
            .stage("read_input")
            .stage("validate")
            .stage("transform")
            .stage("write_output")
    }

    #[test]
    fn test_pipeline_builder() {
        let p = valid_pipeline();
        assert_eq!(p.name, "process_data");
        assert_eq!(p.len(), 4);
        assert!(!p.is_empty());
    }

    #[test]
    fn test_pipeline_display() {
        let p = valid_pipeline();
        let s = format!("{p}");
        assert!(s.contains("process_data"));
        assert!(s.contains("|>"));
    }

    #[test]
    fn test_contract_registry() {
        let reg = setup_registry();
        assert_eq!(reg.len(), 4);
        assert!(!reg.is_empty());
        assert!(reg.get("validate").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_contract_display() {
        let c = FunctionContract::new("inc", StageType::Int, StageType::Int)
            .pre(Expr::BoolLit(true))
            .post(Expr::BinOp(Box::new(Expr::Result), BinOp::Gt, Box::new(Expr::IntLit(0))));
        let s = format!("{c}");
        assert!(s.contains("inc"));
        assert!(s.contains("@req"));
        assert!(s.contains("@ens"));
    }

    #[test]
    fn test_types_compatible_same() {
        assert!(types_compatible(&StageType::Int, &StageType::Int));
        assert!(types_compatible(&StageType::String, &StageType::String));
    }

    #[test]
    fn test_types_compatible_mismatch() {
        assert!(!types_compatible(&StageType::Int, &StageType::String));
        assert!(!types_compatible(&StageType::Bool, &StageType::Int));
    }

    #[test]
    fn test_types_compatible_array() {
        let a = StageType::Array(Box::new(StageType::Int));
        let b = StageType::Array(Box::new(StageType::Int));
        assert!(types_compatible(&a, &b));
    }

    #[test]
    fn test_types_compatible_record_subtype() {
        let out = StageType::Record(vec![
            ("name".into(), StageType::String),
            ("age".into(), StageType::Int),
            ("active".into(), StageType::Bool),
        ]);
        let inp = StageType::Record(vec![
            ("name".into(), StageType::String),
            ("age".into(), StageType::Int),
        ]);
        assert!(types_compatible(&out, &inp));
    }

    #[test]
    fn test_types_compatible_record_missing_field() {
        let out = StageType::Record(vec![("name".into(), StageType::String)]);
        let inp = StageType::Record(vec![
            ("name".into(), StageType::String),
            ("age".into(), StageType::Int),
        ]);
        assert!(!types_compatible(&out, &inp));
    }

    #[test]
    fn test_verify_valid_pipeline() {
        let reg = setup_registry();
        let p = valid_pipeline();
        let errors = verify_pipeline(&p, &reg);
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_verify_empty_pipeline() {
        let reg = setup_registry();
        let p = Pipeline::new("empty");
        let errors = verify_pipeline(&p, &reg);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], PipelineError::EmptyPipeline));
    }

    #[test]
    fn test_verify_missing_contract() {
        let reg = setup_registry();
        let p = Pipeline::new("bad").stage("read_input").stage("nonexistent");
        let errors = verify_pipeline(&p, &reg);
        assert!(errors.iter().any(|e| matches!(e, PipelineError::MissingContract { .. })));
    }

    #[test]
    fn test_verify_type_mismatch() {
        let mut reg = ContractRegistry::new();
        reg.register(FunctionContract::new("a", StageType::Unit, StageType::Int));
        reg.register(FunctionContract::new("b", StageType::String, StageType::Unit));
        let p = Pipeline::new("bad").stage("a").stage("b");
        let errors = verify_pipeline(&p, &reg);
        assert!(errors.iter().any(|e| matches!(e, PipelineError::TypeMismatch { .. })));
    }

    #[test]
    fn test_verify_contract_incompatible() {
        let mut reg = ContractRegistry::new();
        reg.register(FunctionContract::new("a", StageType::Int, StageType::Int).post(Expr::BinOp(
            Box::new(Expr::Result),
            BinOp::Ge,
            Box::new(Expr::IntLit(0)),
        )));
        reg.register(FunctionContract::new("b", StageType::Int, StageType::Int).pre(Expr::BinOp(
            Box::new(Expr::Var("input".into())),
            BinOp::Gt,
            Box::new(Expr::IntLit(100)),
        )));
        let p = Pipeline::new("bad").stage("a").stage("b");
        let errors = verify_pipeline(&p, &reg);
        assert!(errors.iter().any(|e| matches!(e, PipelineError::ContractIncompatible { .. })));
    }

    #[test]
    fn test_exprs_structurally_compatible_trivial() {
        assert!(exprs_structurally_compatible(&Expr::BoolLit(true), &Expr::BoolLit(true)));
        assert!(exprs_structurally_compatible(
            &Expr::BinOp(Box::new(Expr::Result), BinOp::Gt, Box::new(Expr::IntLit(5))),
            &Expr::BoolLit(true),
        ));
    }

    #[test]
    fn test_exprs_compatible_ge_chain() {
        let post = Expr::BinOp(Box::new(Expr::Result), BinOp::Ge, Box::new(Expr::IntLit(10)));
        let pre =
            Expr::BinOp(Box::new(Expr::Var("x".into())), BinOp::Ge, Box::new(Expr::IntLit(5)));
        assert!(exprs_structurally_compatible(&post, &pre));
    }

    #[test]
    fn test_exprs_compatible_ge_fail() {
        let post = Expr::BinOp(Box::new(Expr::Result), BinOp::Ge, Box::new(Expr::IntLit(3)));
        let pre =
            Expr::BinOp(Box::new(Expr::Var("x".into())), BinOp::Ge, Box::new(Expr::IntLit(10)));
        assert!(!exprs_structurally_compatible(&post, &pre));
    }

    #[test]
    fn test_compile_valid_pipeline() {
        let reg = setup_registry();
        let p = valid_pipeline();
        let compiled = compile_pipeline(&p, &reg).unwrap();
        assert_eq!(compiled.name, "process_data");
        assert_eq!(compiled.stages.len(), 4);
        assert_eq!(compiled.input_type, StageType::Unit);
        assert_eq!(compiled.output_type, StageType::Unit);
    }

    #[test]
    fn test_compile_invalid_pipeline() {
        let reg = setup_registry();
        let p = Pipeline::new("bad").stage("nonexistent");
        let result = compile_pipeline(&p, &reg);
        assert!(result.is_err());
    }

    #[test]
    fn test_compiled_pipeline_display() {
        let reg = setup_registry();
        let p = valid_pipeline();
        let compiled = compile_pipeline(&p, &reg).unwrap();
        let s = format!("{compiled}");
        assert!(s.contains("process_data"));
        assert!(s.contains("4 stages"));
    }

    #[test]
    fn test_analyze_valid_pipeline() {
        let reg = setup_registry();
        let p = valid_pipeline();
        let analysis = analyze_pipeline(&p, &reg);
        assert!(analysis.is_valid);
        assert_eq!(analysis.stage_count, 4);
        assert_eq!(analysis.contract_coverage, 1.0);
    }

    #[test]
    fn test_analyze_partial_coverage() {
        let reg = setup_registry();
        let p = Pipeline::new("partial").stage("read_input").stage("unknown_stage");
        let analysis = analyze_pipeline(&p, &reg);
        assert!(!analysis.is_valid);
        assert!(analysis.contract_coverage < 1.0);
    }

    #[test]
    fn test_analysis_display() {
        let reg = setup_registry();
        let p = valid_pipeline();
        let analysis = analyze_pipeline(&p, &reg);
        let s = format!("{analysis}");
        assert!(s.contains("process_data"));
        assert!(s.contains("valid: true"));
    }

    #[test]
    fn test_pipeline_error_display() {
        let e = PipelineError::EmptyPipeline;
        assert_eq!(format!("{e}"), "pipeline is empty");

        let e = PipelineError::MissingContract { stage_index: 1, function_name: "foo".into() };
        assert!(format!("{e}").contains("foo"));
    }

    #[test]
    fn test_type_display() {
        assert_eq!(format!("{}", StageType::Int), "int");
        assert_eq!(format!("{}", StageType::Unit), "()");
        assert_eq!(format!("{}", StageType::Array(Box::new(StageType::Bool))), "[bool]");
        let rec = StageType::Record(vec![("x".into(), StageType::Int)]);
        assert!(format!("{rec}").contains("x: int"));
    }

    #[test]
    fn test_expr_display() {
        let e = Expr::BinOp(Box::new(Expr::Var("x".into())), BinOp::Add, Box::new(Expr::IntLit(1)));
        assert_eq!(format!("{e}"), "(x + 1)");
    }

    #[test]
    fn test_binop_display() {
        assert_eq!(format!("{}", BinOp::Eq), "==");
        assert_eq!(format!("{}", BinOp::And), "&&");
    }

    #[test]
    fn test_single_stage_pipeline() {
        let mut reg = ContractRegistry::new();
        reg.register(FunctionContract::new("identity", StageType::Int, StageType::Int));
        let p = Pipeline::new("single").stage("identity");
        let errors = verify_pipeline(&p, &reg);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_named_type_compatibility() {
        assert!(
            types_compatible(&StageType::Named("Foo".into()), &StageType::Named("Foo".into()),)
        );
        assert!(!types_compatible(
            &StageType::Named("Foo".into()),
            &StageType::Named("Bar".into()),
        ));
    }

    #[test]
    fn test_contract_no_conditions() {
        let c = FunctionContract::new("noop", StageType::Unit, StageType::Unit);
        assert!(c.preconditions.is_empty());
        assert!(c.postconditions.is_empty());
    }

    #[test]
    fn test_true_post_nontrivial_pre() {
        // post: true, pre: x > 0 => not compatible
        assert!(!exprs_structurally_compatible(
            &Expr::BoolLit(true),
            &Expr::BinOp(Box::new(Expr::Var("x".into())), BinOp::Gt, Box::new(Expr::IntLit(0))),
        ));
    }
}

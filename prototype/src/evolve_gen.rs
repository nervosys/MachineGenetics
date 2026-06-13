/// MAGE Evolve Codegen — compiles `evolve` blocks into population-loop MLIR.
///
/// Translates EvolveDef AST nodes into a generational evolutionary algorithm:
///
///   1. Initialise population of `genome_type` values.
///   2. For each generation:
///      a. Evaluate fitness   (`fitness` block)
///      b. Select parents     (`select_fn` or default tournament selection)
///      c. Crossover children (`crossover_fn` or default single-point)
///      d. Mutate             (`mutate_fn` or default bit-flip)
///   3. Return the fittest individual.
///
/// Generates `MAGE.evolve.*` MLIR dialect ops.
use crate::ast;
use crate::hir::{Diagnostic, DiagnosticCategory, Severity};

// ── IR for evolutionary algorithm loop ──────────────────────────────

/// Selection strategy.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionStrategy {
    Tournament { size: usize },
    RouletteWheel,
    Rank,
    Elitist { keep: usize },
}

impl Default for SelectionStrategy {
    fn default() -> Self {
        SelectionStrategy::Tournament { size: 3 }
    }
}

/// Crossover strategy.
#[derive(Debug, Clone, PartialEq)]
pub enum CrossoverStrategy {
    SinglePoint,
    TwoPoint,
    Uniform { probability: f64 },
}

impl Default for CrossoverStrategy {
    fn default() -> Self {
        CrossoverStrategy::SinglePoint
    }
}

/// Mutation strategy.
#[derive(Debug, Clone, PartialEq)]
pub enum MutationStrategy {
    BitFlip { rate: f64 },
    Gaussian { stddev: f64 },
    Swap,
}

impl Default for MutationStrategy {
    fn default() -> Self {
        MutationStrategy::BitFlip { rate: 0.01 }
    }
}

/// The compiled evolution plan — a structured IR for one `evolve` block.
#[derive(Debug, Clone)]
pub struct EvolvePlan {
    pub name: String,
    pub genome_type: String,
    pub population_size: usize,
    pub generations: usize,
    pub selection: SelectionStrategy,
    pub crossover: CrossoverStrategy,
    pub mutation: MutationStrategy,
    /// Whether the user provided custom blocks.
    pub has_custom_fitness: bool,
    pub has_custom_mutate: bool,
    pub has_custom_crossover: bool,
    pub has_custom_select: bool,
}

impl EvolvePlan {
    /// Generate the MLIR operation sequence for this evolutionary algorithm.
    pub fn emit_mlir(&self) -> Vec<String> {
        let mut ops = Vec::new();

        ops.push(format!(
            "MAGE.evolve @{} : {} {{",
            self.name, self.genome_type
        ));

        // Population init.
        ops.push(format!(
            "  %pop = MAGE.evolve.init_population({}) : !MAGE.population<{}>",
            self.population_size, self.genome_type
        ));

        // Generation loop.
        ops.push(format!(
            "  MAGE.evolve.generation_loop {} {{",
            self.generations
        ));

        // Fitness evaluation.
        if self.has_custom_fitness {
            ops.push("    %fitness = MAGE.evolve.eval_fitness_custom(%pop) : !MAGE.fitness_vec".into());
        } else {
            ops.push("    %fitness = MAGE.evolve.eval_fitness(%pop) : !MAGE.fitness_vec".into());
        }

        // Selection.
        let select_op = match &self.selection {
            SelectionStrategy::Tournament { size } => {
                format!("    %parents = MAGE.evolve.select_tournament(%pop, %fitness, {size}) : !MAGE.population<{}>", self.genome_type)
            }
            SelectionStrategy::RouletteWheel => {
                format!("    %parents = MAGE.evolve.select_roulette(%pop, %fitness) : !MAGE.population<{}>", self.genome_type)
            }
            SelectionStrategy::Rank => {
                format!("    %parents = MAGE.evolve.select_rank(%pop, %fitness) : !MAGE.population<{}>", self.genome_type)
            }
            SelectionStrategy::Elitist { keep } => {
                format!("    %parents = MAGE.evolve.select_elitist(%pop, %fitness, {keep}) : !MAGE.population<{}>", self.genome_type)
            }
        };
        ops.push(select_op);

        // Crossover.
        let crossover_op = match &self.crossover {
            CrossoverStrategy::SinglePoint => {
                format!("    %children = MAGE.evolve.crossover_single_point(%parents) : !MAGE.population<{}>", self.genome_type)
            }
            CrossoverStrategy::TwoPoint => {
                format!("    %children = MAGE.evolve.crossover_two_point(%parents) : !MAGE.population<{}>", self.genome_type)
            }
            CrossoverStrategy::Uniform { probability } => {
                format!("    %children = MAGE.evolve.crossover_uniform(%parents, {probability}) : !MAGE.population<{}>", self.genome_type)
            }
        };
        ops.push(crossover_op);

        // Mutation.
        let mutate_op = match &self.mutation {
            MutationStrategy::BitFlip { rate } => {
                format!("    %pop_next = MAGE.evolve.mutate_bitflip(%children, {rate}) : !MAGE.population<{}>", self.genome_type)
            }
            MutationStrategy::Gaussian { stddev } => {
                format!("    %pop_next = MAGE.evolve.mutate_gaussian(%children, {stddev}) : !MAGE.population<{}>", self.genome_type)
            }
            MutationStrategy::Swap => {
                format!("    %pop_next = MAGE.evolve.mutate_swap(%children) : !MAGE.population<{}>", self.genome_type)
            }
        };
        ops.push(mutate_op);

        ops.push("  }".to_string()); // end generation loop

        // Extract best individual.
        ops.push(format!(
            "  %best = MAGE.evolve.best(%pop) : {}",
            self.genome_type
        ));

        ops.push("}".to_string()); // end evolve block

        ops
    }
}

// ── AST → EvolvePlan builder ────────────────────────────────────────

/// Build an EvolvePlan from an AST EvolveDef.
pub fn build_evolve_plan(def: &ast::EvolveDef) -> Result<EvolvePlan, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    let genome_type = type_to_string(&def.genome_type);

    let population_size = def.population_size.as_ref()
        .and_then(|e| extract_usize(e))
        .unwrap_or_else(|| {
            diagnostics.push(Diagnostic::categorized(
                Severity::Warning,
                format!("evolve `{}`: no population_size specified, defaulting to 100", def.name),
                DiagnosticCategory::TypeMismatch,
                None,
            ));
            100
        });

    let generations = def.generations.as_ref()
        .and_then(|e| extract_usize(e))
        .unwrap_or_else(|| {
            diagnostics.push(Diagnostic::categorized(
                Severity::Warning,
                format!("evolve `{}`: no generations specified, defaulting to 50", def.name),
                DiagnosticCategory::TypeMismatch,
                None,
            ));
            50
        });

    if population_size == 0 {
        diagnostics.push(Diagnostic::categorized(
            Severity::Error,
            format!("evolve `{}`: population_size must be > 0", def.name),
            DiagnosticCategory::TypeMismatch,
            None,
        ));
    }

    if generations == 0 {
        diagnostics.push(Diagnostic::categorized(
            Severity::Error,
            format!("evolve `{}`: generations must be > 0", def.name),
            DiagnosticCategory::TypeMismatch,
            None,
        ));
    }

    let has_errors = diagnostics.iter().any(|d| d.severity == Severity::Error);
    if has_errors {
        return Err(diagnostics);
    }

    Ok(EvolvePlan {
        name: def.name.clone(),
        genome_type,
        population_size,
        generations,
        selection: SelectionStrategy::default(),
        crossover: CrossoverStrategy::default(),
        mutation: MutationStrategy::default(),
        has_custom_fitness: true, // Fitness block is always user-provided.
        has_custom_mutate: def.mutate_fn.is_some(),
        has_custom_crossover: def.crossover_fn.is_some(),
        has_custom_select: def.select_fn.is_some(),
    })
}

fn type_to_string(ty: &ast::Type) -> String {
    match ty {
        ast::Type::Path { segments, type_args } => {
            let base = segments.join("::");
            if type_args.is_empty() {
                base
            } else {
                let arg_strs: Vec<String> = type_args.iter().map(|a| type_to_string(a)).collect();
                format!("{}<{}>", base, arg_strs.join(", "))
            }
        }
        ast::Type::Tensor { inner, shape } => {
            let dim_strs: Vec<String> = shape.iter().map(|d| format!("{d:?}")).collect();
            format!("Tensor<{}, {}>", type_to_string(inner), dim_strs.join(", "))
        }
        ast::Type::Fn { params, ret } => {
            let p_strs: Vec<String> = params.iter().map(|p| type_to_string(p)).collect();
            match ret {
                Some(r) => format!("({}) -> {}", p_strs.join(", "), type_to_string(r)),
                None => format!("({})", p_strs.join(", ")),
            }
        }
        _ => "unknown".to_string(),
    }
}

fn extract_usize(expr: &ast::Expr) -> Option<usize> {
    match expr {
        ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => {
            value.parse().ok()
        }
        _ => None,
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_plan() {
        let plan = EvolvePlan {
            name: "test_opt".into(),
            genome_type: "Vec<f64>".into(),
            population_size: 50,
            generations: 20,
            selection: SelectionStrategy::default(),
            crossover: CrossoverStrategy::default(),
            mutation: MutationStrategy::default(),
            has_custom_fitness: true,
            has_custom_mutate: false,
            has_custom_crossover: false,
            has_custom_select: false,
        };

        let mlir = plan.emit_mlir();
        assert!(mlir[0].contains("MAGE.evolve @test_opt"));
        assert!(mlir.iter().any(|op| op.contains("init_population(50)")));
        assert!(mlir.iter().any(|op| op.contains("generation_loop 20")));
        assert!(mlir.iter().any(|op| op.contains("select_tournament")));
        assert!(mlir.iter().any(|op| op.contains("crossover_single_point")));
        assert!(mlir.iter().any(|op| op.contains("mutate_bitflip")));
        assert!(mlir.iter().any(|op| op.contains("MAGE.evolve.best")));
    }

    #[test]
    fn elitist_selection() {
        let plan = EvolvePlan {
            name: "elitist_test".into(),
            genome_type: "Genome".into(),
            population_size: 100,
            generations: 30,
            selection: SelectionStrategy::Elitist { keep: 10 },
            crossover: CrossoverStrategy::TwoPoint,
            mutation: MutationStrategy::Gaussian { stddev: 0.1 },
            has_custom_fitness: true,
            has_custom_mutate: false,
            has_custom_crossover: false,
            has_custom_select: false,
        };

        let mlir = plan.emit_mlir();
        assert!(mlir.iter().any(|op| op.contains("select_elitist")));
        assert!(mlir.iter().any(|op| op.contains("crossover_two_point")));
        assert!(mlir.iter().any(|op| op.contains("mutate_gaussian")));
    }

    #[test]
    fn roulette_uniform() {
        let plan = EvolvePlan {
            name: "uw".into(),
            genome_type: "Bits".into(),
            population_size: 200,
            generations: 100,
            selection: SelectionStrategy::RouletteWheel,
            crossover: CrossoverStrategy::Uniform { probability: 0.5 },
            mutation: MutationStrategy::Swap,
            has_custom_fitness: true,
            has_custom_mutate: true,
            has_custom_crossover: true,
            has_custom_select: true,
        };

        let mlir = plan.emit_mlir();
        assert!(mlir.iter().any(|op| op.contains("select_roulette")));
        assert!(mlir.iter().any(|op| op.contains("crossover_uniform")));
        assert!(mlir.iter().any(|op| op.contains("mutate_swap")));
    }

    #[test]
    fn rank_selection() {
        let plan = EvolvePlan {
            name: "rank_test".into(),
            genome_type: "Float".into(),
            population_size: 64,
            generations: 10,
            selection: SelectionStrategy::Rank,
            crossover: CrossoverStrategy::default(),
            mutation: MutationStrategy::default(),
            has_custom_fitness: true,
            has_custom_mutate: false,
            has_custom_crossover: false,
            has_custom_select: false,
        };

        let mlir = plan.emit_mlir();
        assert!(mlir.iter().any(|op| op.contains("select_rank")));
    }

    #[test]
    fn zero_population_error() {
        let def = ast::EvolveDef {
            name: "bad".into(),
            genome_type: ast::Type::Path { segments: vec!["X".into()], type_args: vec![] },
            population_size: Some(ast::Expr::Literal {
                value: "0".into(),
                kind: ast::LiteralKind::Int,
            }),
            generations: Some(ast::Expr::Literal {
                value: "10".into(),
                kind: ast::LiteralKind::Int,
            }),
            fitness: ast::Block { stmts: vec![], tail_expr: None },
            mutate_fn: None,
            crossover_fn: None,
            select_fn: None,
        };

        let result = build_evolve_plan(&def);
        assert!(result.is_err());
        let diags = result.unwrap_err();
        assert!(diags.iter().any(|d| d.message.contains("population_size must be > 0")));
    }

    #[test]
    fn default_population_and_generations() {
        let def = ast::EvolveDef {
            name: "defaults".into(),
            genome_type: ast::Type::Path { segments: vec!["Vec".into()], type_args: vec![] },
            population_size: None,
            generations: None,
            fitness: ast::Block { stmts: vec![], tail_expr: None },
            mutate_fn: None,
            crossover_fn: None,
            select_fn: None,
        };

        let plan = build_evolve_plan(&def).unwrap();
        assert_eq!(plan.population_size, 100);
        assert_eq!(plan.generations, 50);
    }

    #[test]
    fn mlir_structure() {
        let plan = EvolvePlan {
            name: "structure_test".into(),
            genome_type: "G".into(),
            population_size: 10,
            generations: 5,
            selection: SelectionStrategy::default(),
            crossover: CrossoverStrategy::default(),
            mutation: MutationStrategy::default(),
            has_custom_fitness: true,
            has_custom_mutate: false,
            has_custom_crossover: false,
            has_custom_select: false,
        };

        let mlir = plan.emit_mlir();
        // First line opens the block, last closes it.
        assert!(mlir.first().unwrap().starts_with("MAGE.evolve"));
        assert_eq!(mlir.last().unwrap(), "}");
        // Has init, loop, fitness, select, crossover, mutate, best.
        let joined = mlir.join("\n");
        assert!(joined.contains("init_population"));
        assert!(joined.contains("generation_loop"));
        assert!(joined.contains("eval_fitness"));
        assert!(joined.contains("select_"));
        assert!(joined.contains("crossover_"));
        assert!(joined.contains("mutate_"));
        assert!(joined.contains("best"));
    }
}

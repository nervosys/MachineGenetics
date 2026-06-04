// autonomous-pipeline — AI-Driven Code Generation Pipeline.
//
// A production-grade scenario: an AI system receives a natural-language
// specification ("build a REST API for user management"), decomposes it
// into verified tasks, generates code through a contracted pipeline,
// estimates costs per target, and delivers optimized output.
//
// Demonstrates:
//   - Task decomposition into dependency DAGs
//   - Function contracts with pre/post conditions
//   - Pipeline composition with type-safe stages
//   - Cost oracle queries for multi-target optimization
//   - Token budget tracking for LLM-aware compilation
//   - Memory recall for caching intermediate results
//   - Effect annotations (/ io, / db, / net)

use std::col;
use std::fmt;
use std::io;

// ─────────────────────────────────────────────────────────────────────
// §1 — Specification: the user's high-level request
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data Specification {
    description: String,
    language: String,
    targets: [String]~,
    max_token_budget: usize,
    safety_level: SafetyLevel,
}

#[derive(Debug, Clone, PartialEq)]
pub data SafetyLevel {
    Prototype,
    Development,
    Production,
    SafetyCritical,
}

extend SafetyLevel {
    fn requires_contracts(&self) -> bool {
        match self {
            SafetyLevel::Production | SafetyLevel::SafetyCritical => true,
            _ => false,
        }
    }

    fn requires_formal_verification(&self) -> bool {
        match self {
            SafetyLevel::SafetyCritical => true,
            _ => false,
        }
    }

    fn min_test_coverage(&self) -> f64 {
        match self {
            SafetyLevel::Prototype => 0.0,
            SafetyLevel::Development => 0.60,
            SafetyLevel::Production => 0.85,
            SafetyLevel::SafetyCritical => 0.95,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// §2 — Task decomposition: break spec into executable units
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data TaskKind {
    DesignApi,
    DefineContracts,
    ImplementEndpoint,
    ImplementModel,
    GenerateTests,
    OptimizeForTarget,
    VerifyContracts,
    IntegrationTest,
}

#[derive(Debug, Clone)]
pub data Task {
    id: u64,
    name: String,
    kind: TaskKind,
    dependencies: [u64]~,
    token_estimate: usize,
    completed: bool,
}

extend Task {
    pub fn new(id: u64, name: String, kind: TaskKind, token_est: usize) -> Task {
        Task {
            id: id,
            name: name,
            kind: kind,
            dependencies: []~.new(),
            token_estimate: token_est,
            completed: false,
        }
    }

    pub fn depends_on(mut self, deps: [u64]~) -> Task {
        self.dependencies = deps;
        self
    }

    pub fn is_ready(&self, completed: &{u64}) -> bool {
        self.dependencies.iter().all(|d| completed.contains(d))
    }
}

/// Decompose a specification into a task DAG.
///
/// @req  spec.targets.len() > 0            "must target at least one platform"
/// @ens  result.len() >= 3                 "minimal pipeline has design + implement + test"
/// @fx   pure
fn decompose(spec: &Specification) -> [Task]~ {
    var tasks: [Task]~ = []~.new();
    var next_id: u64 = 1;

    // Phase 1: Design the API surface.
    val design = Task.new(next_id, "Design REST API surface".to_string(), TaskKind::DesignApi, 200);
    tasks.push(design);
    val design_id = next_id;
    next_id = next_id + 1;

    // Phase 2: Define contracts (if safety level requires it).
    var contract_id: ?u64 = None;
    if spec.safety_level.requires_contracts() {
        val contracts = Task.new(
            next_id,
            "Define function contracts".to_string(),
            TaskKind::DefineContracts,
            150,
        ).depends_on(vec![design_id]);
        contract_id = Some(next_id);
        tasks.push(contracts);
        next_id = next_id + 1;
    }

    // Phase 3: Implement models and endpoints.
    val model_deps = match contract_id {
        Some(cid) => vec![design_id, cid],
        None => vec![design_id],
    };

    val model = Task.new(
        next_id,
        "Implement User model".to_string(),
        TaskKind::ImplementModel,
        300,
    ).depends_on(model_deps.clone());
    val model_id = next_id;
    tasks.push(model);
    next_id = next_id + 1;

    // Endpoints depend on model.
    val endpoints = ["create_user", "get_user", "update_user", "delete_user"];
    var endpoint_ids: [u64]~ = []~.new();
    for name in &endpoints {
        val ep = Task.new(
            next_id,
            format!("Implement {name} endpoint"),
            TaskKind::ImplementEndpoint,
            250,
        ).depends_on(vec![model_id]);
        endpoint_ids.push(next_id);
        tasks.push(ep);
        next_id = next_id + 1;
    }

    // Phase 4: Generate tests.
    val test_task = Task.new(
        next_id,
        "Generate test suite".to_string(),
        TaskKind::GenerateTests,
        400,
    ).depends_on(endpoint_ids.clone());
    val test_id = next_id;
    tasks.push(test_task);
    next_id = next_id + 1;

    // Phase 5: Optimize for each target.
    for target in &spec.targets {
        val opt = Task.new(
            next_id,
            format!("Optimize for {target}"),
            TaskKind::OptimizeForTarget,
            180,
        ).depends_on(vec![test_id]);
        tasks.push(opt);
        next_id = next_id + 1;
    }

    // Phase 6: Verify contracts (if safety-critical).
    if spec.safety_level.requires_formal_verification() {
        val verify = Task.new(
            next_id,
            "Formal contract verification".to_string(),
            TaskKind::VerifyContracts,
            500,
        ).depends_on(endpoint_ids.clone());
        tasks.push(verify);
    }

    tasks
}

// ─────────────────────────────────────────────────────────────────────
// §3 — Contracts: specify what each generated function must satisfy
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data ContractSpec {
    fn_name: String,
    preconditions: [String]~,
    postconditions: [String]~,
}

extend ContractSpec {
    pub fn new(name: &String) -> ContractSpec {
        ContractSpec {
            fn_name: name.clone(),
            preconditions: []~.new(),
            postconditions: []~.new(),
        }
    }

    pub fn pre(mut self, cond: String) -> ContractSpec {
        self.preconditions.push(cond);
        self
    }

    pub fn post(mut self, cond: String) -> ContractSpec {
        self.postconditions.push(cond);
        self
    }
}

/// Build contracts for a user-management REST API. Each endpoint gets
/// pre/postconditions that the generated code must satisfy.
///
/// @fx pure
fn build_api_contracts() -> [ContractSpec]~ {
    vec![
        ContractSpec.new(&"create_user".to_string())
            .pre("email.len() > 0".to_string())
            .pre("email.contains('@')".to_string())
            .pre("password.len() >= 8".to_string())
            .post("result.is_ok() => db.contains(email)".to_string())
            .post("result.is_err() => db.unchanged()".to_string()),

        ContractSpec.new(&"get_user".to_string())
            .pre("id > 0".to_string())
            .post("result.is_some() => result.unwrap().id == id".to_string()),

        ContractSpec.new(&"update_user".to_string())
            .pre("id > 0".to_string())
            .pre("db.contains(id)".to_string())
            .post("result.is_ok() => db.get(id).version > old(db.get(id).version)".to_string()),

        ContractSpec.new(&"delete_user".to_string())
            .pre("id > 0".to_string())
            .post("result.is_ok() => !db.contains(id)".to_string()),
    ]
}

// ─────────────────────────────────────────────────────────────────────
// §4 — Pipeline: staged code generation
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data StageResult {
    Ok(String),
    Err(String),
}

#[derive(Debug, Clone)]
pub data PipelineStage {
    name: String,
    input_type: String,
    output_type: String,
}

#[derive(Debug, Clone)]
pub data GenerationPipeline {
    stages: [PipelineStage]~,
    results: {String: StageResult},
}

extend GenerationPipeline {
    pub fn new() -> GenerationPipeline {
        GenerationPipeline { stages: []~.new(), results: {}.new() }
    }

    pub fn stage(mut self, name: String, input: String, output: String) -> GenerationPipeline {
        self.stages.push(PipelineStage {
            name: name,
            input_type: input,
            output_type: output,
        });
        self
    }

    /// Run all stages sequentially. Each stage's output becomes the
    /// next stage's input — verified by type compatibility.
    ///
    /// @req  self.stages.len() > 0          "pipeline must have stages"
    /// @ens  self.results.len() == self.stages.len()
    pub fn run(&mut self, initial_input: String) / io {
        println!("");
        println!("╔═══════════════════════════════════════════╗");
        println!("║     Code Generation Pipeline              ║");
        println!("╚═══════════════════════════════════════════╝");

        var current_input = initial_input;
        for (i, stage) in self.stages.iter().enumerate() {
            println!("  ┌─ Stage {}: {}", i + 1, stage.name);
            println!("  │  Input:  {} ({} chars)", stage.input_type, current_input.len());

            // Simulate generation — in production this calls the LLM.
            val output = format!("// Generated by stage: {}\n{}", stage.name, current_input);

            println!("  │  Output: {} ({} chars)", stage.output_type, output.len());
            println!("  └─ ✓ Complete");

            self.results.insert(stage.name.clone(), StageResult::Ok(output.clone()));
            current_input = output;
        }
        println!("");
        println!("Pipeline complete: {} stages executed.", self.results.len());
    }
}

/// Build the standard code-generation pipeline.
///
/// @fx pure
fn build_generation_pipeline() -> GenerationPipeline {
    GenerationPipeline.new()
        .stage(
            "parse_spec".to_string(),
            "NaturalLanguage".to_string(),
            "StructuredAST".to_string(),
        )
        .stage(
            "generate_types".to_string(),
            "StructuredAST".to_string(),
            "TypeDefinitions".to_string(),
        )
        .stage(
            "generate_endpoints".to_string(),
            "TypeDefinitions".to_string(),
            "EndpointCode".to_string(),
        )
        .stage(
            "attach_contracts".to_string(),
            "EndpointCode".to_string(),
            "ContractedCode".to_string(),
        )
        .stage(
            "generate_tests".to_string(),
            "ContractedCode".to_string(),
            "TestedModule".to_string(),
        )
}

// ─────────────────────────────────────────────────────────────────────
// §5 — Cost estimation: choose the best target
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data TargetCost {
    target: String,
    latency_ms: f64,
    memory_mb: f64,
    energy_score: f64,
}

extend TargetCost {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {:.1}ms latency, {:.1}MB memory, energy={:.2}",
            self.target, self.latency_ms, self.memory_mb, self.energy_score)
    }
}

/// Estimate compilation cost for each target.
///
/// @req  targets.len() > 0
/// @ens  result.len() == targets.len()
/// @fx   pure
fn estimate_costs(targets: &[String]~, code_size_tokens: usize) -> [TargetCost]~ {
    var costs: [TargetCost]~ = []~.new();
    val base = code_size_tokens as f64;

    for target in targets {
        val (lat, mem, energy) = match target.as_str() {
            "x86_64"  => (base * 0.012, base * 0.08,  0.85),
            "aarch64" => (base * 0.010, base * 0.07,  0.65),
            "wasm32"  => (base * 0.018, base * 0.12,  0.40),
            "riscv64" => (base * 0.015, base * 0.09,  0.55),
            _         => (base * 0.020, base * 0.15,  1.00),
        };
        costs.push(TargetCost {
            target: target.clone(),
            latency_ms: lat,
            memory_mb: mem,
            energy_score: energy,
        });
    }
    costs
}

/// Select the optimal target for the given workload.
///
/// @req  costs.len() > 0
/// @fx   pure
fn select_optimal_target(costs: &[TargetCost]~) -> &TargetCost {
    // Multi-objective scoring: prefer low latency + low memory + low energy.
    var best = &costs[0];
    var best_score = best.latency_ms + best.memory_mb + best.energy_score * 100.0;

    for cost in costs.iter().skip(1) {
        val score = cost.latency_ms + cost.memory_mb + cost.energy_score * 100.0;
        if score < best_score {
            best = cost;
            best_score = score;
        }
    }
    best
}

// ─────────────────────────────────────────────────────────────────────
// §6 — Token budget tracking
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub data TokenBudget {
    max_tokens: usize,
    used_tokens: usize,
    allocations: [(String, usize)]~,
}

extend TokenBudget {
    pub fn new(max: usize) -> TokenBudget {
        TokenBudget { max_tokens: max, used_tokens: 0, allocations: []~.new() }
    }

    pub fn allocate(&mut self, label: String, tokens: usize) -> bool {
        if self.used_tokens + tokens > self.max_tokens {
            return false;
        }
        self.used_tokens = self.used_tokens + tokens;
        self.allocations.push((label, tokens));
        true
    }

    pub fn remaining(&self) -> usize {
        self.max_tokens - self.used_tokens
    }

    pub fn utilization(&self) -> f64 {
        self.used_tokens as f64 / self.max_tokens as f64 * 100.0
    }

    pub fn report(&self) / io {
        println!("");
        println!("── Token Budget Report ──────────────────────");
        println!("  Total:     {}", self.max_tokens);
        println!("  Used:      {}", self.used_tokens);
        println!("  Remaining: {}", self.remaining());
        println!("  Util:      {:.1}%", self.utilization());
        println!("  ┌───────────────────────────┬────────┐");
        println!("  │ Allocation                │ Tokens │");
        println!("  ├───────────────────────────┼────────┤");
        for (label, count) in &self.allocations {
            println!("  │ {:<25} │ {:>6} │", label, count);
        }
        println!("  └───────────────────────────┴────────┘");
    }
}

// ─────────────────────────────────────────────────────────────────────
// §7 — Memory: cache intermediate generation results
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data CacheEntry {
    key: String,
    value: String,
    hit_count: u64,
}

#[derive(Debug)]
pub data GenerationCache {
    entries: {String: CacheEntry},
}

extend GenerationCache {
    pub fn new() -> GenerationCache {
        GenerationCache { entries: {}.new() }
    }

    pub fn store(&mut self, key: String, value: String) {
        self.entries.insert(key.clone(), CacheEntry {
            key: key,
            value: value,
            hit_count: 0,
        });
    }

    pub fn recall(&mut self, key: &String) -> ?&String {
        match self.entries.get_mut(key) {
            Some(entry) => {
                entry.hit_count = entry.hit_count + 1;
                Some(&entry.value)
            },
            None => None,
        }
    }

    pub fn stats(&self) / io {
        println!("  Cache entries: {}", self.entries.len());
        for (key, entry) in &self.entries {
            println!("    {}: {} hits", key, entry.hit_count);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// §8 — Entry point: orchestrate the full pipeline
// ─────────────────────────────────────────────────────────────────────

pub fn main() / io {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║  MechGen Autonomous Code Generation Pipeline               ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!("");

    // Define the specification.
    val spec = Specification {
        description: "Build a REST API for user management with CRUD operations".to_string(),
        language: "MechGen".to_string(),
        targets: vec!["x86_64", "aarch64", "wasm32"].iter().map(|s| s.to_string()).collect(),
        max_token_budget: 4096,
        safety_level: SafetyLevel::Production,
    };

    println!("Specification:");
    println!("  {}", spec.description);
    println!("  Language: {}", spec.language);
    println!("  Targets:  {:?}", spec.targets);
    println!("  Safety:   {:?}", spec.safety_level);
    println!("  Budget:   {} tokens", spec.max_token_budget);
    println!("");

    // Step 1: Decompose into tasks.
    println!("─── Step 1: Task Decomposition ───────────────────────────");
    val tasks = decompose(&spec);
    println!("  Decomposed into {} tasks:", tasks.len());
    for task in &tasks {
        val deps = match task.dependencies.is_empty() {
            false => format!(" (depends on: {:?})", task.dependencies),
            true => "".to_string(),
        };
        println!("    [{}] {} (~{} tokens){}", task.id, task.name, task.token_estimate, deps);
    }
    println!("");

    // Step 2: Build contracts.
    println!("─── Step 2: Contract Specification ───────────────────────");
    val contracts = build_api_contracts();
    for contract in &contracts {
        println!("  {}:", contract.fn_name);
        for pre in &contract.preconditions {
            println!("    @req {}", pre);
        }
        for post in &contract.postconditions {
            println!("    @ens {}", post);
        }
    }
    println!("");

    // Step 3: Track token budget across tasks.
    println!("─── Step 3: Token Budget Allocation ──────────────────────");
    var budget = TokenBudget.new(spec.max_token_budget);
    for task in &tasks {
        val ok = budget.allocate(task.name.clone(), task.token_estimate);
        if !ok {
            println!("  ⚠  Budget exceeded at task: {}", task.name);
        }
    }
    budget.report();
    println!("");

    // Step 4: Run the generation pipeline.
    println!("─── Step 4: Generation Pipeline ──────────────────────────");
    var pipeline = build_generation_pipeline();
    pipeline.run(spec.description.clone());
    println!("");

    // Step 5: Estimate costs and select target.
    println!("─── Step 5: Cost Estimation ──────────────────────────────");
    val total_tokens = tasks.iter().map(|t| t.token_estimate).sum::<usize>();
    val costs = estimate_costs(&spec.targets, total_tokens);
    for cost in &costs {
        println!("  {}", cost);
    }
    val optimal = select_optimal_target(&costs);
    println!("");
    println!("  ★ Optimal target: {}", optimal);
    println!("");

    // Step 6: Cache results for future recall.
    println!("─── Step 6: Result Caching ───────────────────────────────");
    var cache = GenerationCache.new();
    cache.store("api_design".to_string(), "User { id, email, name }".to_string());
    cache.store("contracts".to_string(), format!("{} contracts defined", contracts.len()));
    cache.store("optimal_target".to_string(), optimal.target.clone());

    // Simulate recall.
    match cache.recall(&"api_design".to_string()) {
        Some(val) => println!("  Recalled api_design: {}", val),
        None => println!("  Cache miss"),
    };
    cache.stats();
    println!("");

    println!("═══════════════════════════════════════════════════════════");
    println!("  Pipeline complete. {} tasks, {} contracts,", tasks.len(), contracts.len());
    println!("  {} targets evaluated. Optimal: {}", costs.len(), optimal.target);
    println!("═══════════════════════════════════════════════════════════");
}

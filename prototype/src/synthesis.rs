// ── Synthesis Oracle ───────────────────────────────────────────────
//
// Generates candidate implementations from specs, ranks them by cost,
// and verifies them against contract clauses.
//
// Components:
//   - `SynthesisSpec`  — extracted specification (contracts + effects + perf)
//   - `Candidate`      — a generated implementation candidate
//   - `SynthesisOracle`— the engine: generate, rank, verify, select

use std::collections::BTreeMap;

// ── Specification ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SynthesisSpec {
    pub name: String,
    pub params: Vec<(String, String)>, // (name, type)
    pub return_type: Option<String>,
    pub preconditions: Vec<String>,         // @req conditions
    pub postconditions: Vec<String>,        // @ens conditions
    pub invariants: Vec<String>,            // @inv conditions
    pub effects: Vec<String>,               // declared effects
    pub perf_bounds: Vec<(String, String)>, // (metric, bound)
}

impl SynthesisSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            params: Vec::new(),
            return_type: None,
            preconditions: Vec::new(),
            postconditions: Vec::new(),
            invariants: Vec::new(),
            effects: Vec::new(),
            perf_bounds: Vec::new(),
        }
    }

    pub fn with_param(mut self, name: &str, ty: &str) -> Self {
        self.params.push((name.into(), ty.into()));
        self
    }

    pub fn with_return(mut self, ty: &str) -> Self {
        self.return_type = Some(ty.into());
        self
    }

    pub fn with_req(mut self, condition: &str) -> Self {
        self.preconditions.push(condition.into());
        self
    }

    pub fn with_ens(mut self, condition: &str) -> Self {
        self.postconditions.push(condition.into());
        self
    }

    pub fn with_inv(mut self, condition: &str) -> Self {
        self.invariants.push(condition.into());
        self
    }

    pub fn with_effect(mut self, effect: &str) -> Self {
        self.effects.push(effect.into());
        self
    }

    pub fn with_perf(mut self, metric: &str, bound: &str) -> Self {
        self.perf_bounds.push((metric.into(), bound.into()));
        self
    }

    /// Total constraint count.
    pub fn constraint_count(&self) -> usize {
        self.preconditions.len()
            + self.postconditions.len()
            + self.invariants.len()
            + self.effects.len()
            + self.perf_bounds.len()
    }
}

// ── Strategy ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Strategy {
    /// Direct/imperative approach.
    Imperative,
    /// Recursive decomposition.
    Recursive,
    /// Iterator/functional pipeline.
    Functional,
    /// Table-driven/lookup.
    TableDriven,
    /// Speculative (generate-and-test).
    Speculative,
}

impl std::fmt::Display for Strategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Strategy::Imperative => write!(f, "imperative"),
            Strategy::Recursive => write!(f, "recursive"),
            Strategy::Functional => write!(f, "functional"),
            Strategy::TableDriven => write!(f, "table-driven"),
            Strategy::Speculative => write!(f, "speculative"),
        }
    }
}

// ── Candidate ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: u64,
    pub strategy: Strategy,
    pub body: String,
    pub cost: CostEstimate,
    pub verification: VerificationResult,
}

#[derive(Debug, Clone, Default)]
pub struct CostEstimate {
    pub token_count: usize,
    pub cyclomatic_complexity: usize,
    pub allocation_count: usize,
    pub effect_count: usize,
}

impl CostEstimate {
    /// Composite score (lower is better).
    pub fn score(&self) -> f64 {
        (self.token_count as f64) * 1.0
            + (self.cyclomatic_complexity as f64) * 5.0
            + (self.allocation_count as f64) * 3.0
            + (self.effect_count as f64) * 2.0
    }
}

// ── Verification ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// The candidate is one MAGE function the real parser accepts. Its
    /// effects are inferred from the parse, so one that fails is not checked.
    pub parses: bool,
    pub preconditions_met: bool,
    pub postconditions_met: bool,
    pub invariants_met: bool,
    pub effects_contained: bool,
    pub perf_bounds_met: bool,
    pub violations: Vec<String>,
}

impl Default for VerificationResult {
    fn default() -> Self {
        Self {
            parses: true,
            preconditions_met: true,
            postconditions_met: true,
            invariants_met: true,
            effects_contained: true,
            perf_bounds_met: true,
            violations: Vec::new(),
        }
    }
}

impl VerificationResult {
    pub fn is_valid(&self) -> bool {
        self.parses
            && self.preconditions_met
            && self.postconditions_met
            && self.invariants_met
            && self.effects_contained
            && self.perf_bounds_met
    }
}

// ── Synthesis Oracle ───────────────────────────────────────────────

pub struct SynthesisOracle {
    next_id: u64,
    strategies: Vec<Strategy>,
    /// Strategy-specific templates: strategy → (body_template, base_cost).
    templates: BTreeMap<String, (String, CostEstimate)>,
}

impl Default for SynthesisOracle {
    fn default() -> Self {
        Self::new()
    }
}

impl SynthesisOracle {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            strategies: vec![
                Strategy::Imperative,
                Strategy::Recursive,
                Strategy::Functional,
                Strategy::TableDriven,
                Strategy::Speculative,
            ],
            templates: BTreeMap::new(),
        }
    }

    /// Register a template for a strategy.
    pub fn register_template(
        &mut self,
        strategy: &Strategy,
        body_template: &str,
        cost: CostEstimate,
    ) {
        self.templates.insert(strategy.to_string(), (body_template.into(), cost));
    }

    /// Generate candidates for a spec using all strategies.
    pub fn generate(&mut self, spec: &SynthesisSpec) -> Vec<Candidate> {
        let mut candidates = Vec::new();
        for strategy in &self.strategies.clone() {
            let (body, cost) = self.generate_one(spec, strategy);
            let verification = self.verify(spec, &body, &cost);
            let id = self.next_id;
            self.next_id += 1;
            candidates.push(Candidate { id, strategy: strategy.clone(), body, cost, verification });
        }
        candidates
    }

    fn generate_one(&self, spec: &SynthesisSpec, strategy: &Strategy) -> (String, CostEstimate) {
        // Use registered template if available.
        if let Some((tmpl, cost)) = self.templates.get(&strategy.to_string()) {
            let body = tmpl
                .replace("{name}", &spec.name)
                .replace(
                    "{params}",
                    &spec
                        .params
                        .iter()
                        .map(|(n, t)| format!("{n}: {t}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                )
                .replace("{return}", spec.return_type.as_deref().unwrap_or("()"));
            return (body, cost.clone());
        }

        // Otherwise, synthesize stub based on strategy.
        let params_str: String =
            spec.params.iter().map(|(n, t)| format!("{n}: {t}")).collect::<Vec<_>>().join(", ");
        let ret = spec.return_type.as_deref().unwrap_or("()");

        match strategy {
            Strategy::Imperative => {
                let body = format!(
                    "fn {}({}) -> {} {{ todo(\"imperative\") }}",
                    spec.name, params_str, ret
                );
                let cost = CostEstimate {
                    token_count: 8 + spec.params.len() * 3,
                    cyclomatic_complexity: 1,
                    allocation_count: 0,
                    effect_count: spec.effects.len(),
                };
                (body, cost)
            }
            Strategy::Recursive => {
                let body = format!(
                    "fn {}({}) -> {} {{ if base_case {{ base }} else {{ {}(smaller) }} }}",
                    spec.name, params_str, ret, spec.name
                );
                let cost = CostEstimate {
                    token_count: 15 + spec.params.len() * 3,
                    cyclomatic_complexity: 2,
                    allocation_count: 0,
                    effect_count: spec.effects.len(),
                };
                (body, cost)
            }
            Strategy::Functional => {
                let body = format!(
                    "fn {}({}) -> {} {{ fold(input, init, |acc, x| combine(acc, x)) }}",
                    spec.name, params_str, ret
                );
                let cost = CostEstimate {
                    token_count: 12 + spec.params.len() * 3,
                    cyclomatic_complexity: 1,
                    allocation_count: 1,
                    effect_count: spec.effects.len(),
                };
                (body, cost)
            }
            Strategy::TableDriven => {
                let body = format!("fn {}({}) -> {} {{ TABLE[key] }}", spec.name, params_str, ret);
                let cost = CostEstimate {
                    token_count: 6 + spec.params.len() * 3,
                    cyclomatic_complexity: 1,
                    allocation_count: 0,
                    effect_count: spec.effects.len(),
                };
                (body, cost)
            }
            Strategy::Speculative => {
                let body = format!(
                    "fn {}({}) -> {} {{ val c = guess(); if verify(c) {{ c }} else {{ fallback() }} }}",
                    spec.name, params_str, ret
                );
                let cost = CostEstimate {
                    token_count: 18 + spec.params.len() * 3,
                    cyclomatic_complexity: 2,
                    allocation_count: 1,
                    effect_count: spec.effects.len(),
                };
                (body, cost)
            }
        }
    }

    /// Verify a candidate body against the spec.
    fn verify(&self, spec: &SynthesisSpec, body: &str, cost: &CostEstimate) -> VerificationResult {
        let mut result = VerificationResult::default();

        // Check preconditions: body should reference each param (simplified check).
        for (param_name, _) in &spec.params {
            if !body.contains(param_name.as_str())
                && !body.contains("todo(")
                && !body.contains("{params}")
            {
                result.preconditions_met = false;
                result.violations.push(format!("param `{param_name}` unused"));
            }
        }

        // Check postconditions: body must have a return expression (not just todo()).
        if body.contains("todo(") {
            for ens in &spec.postconditions {
                result.postconditions_met = false;
                result.violations.push(format!("postcondition `{ens}` unverified (stub)"));
            }
        }

        // Check effects with the real inference, not by searching the text:
        // every effect the candidate performs must be one the spec declares.
        // It searched for Rust spellings (`println!`, `File::`), which a MAGE
        // body never contains, so `println("…")` passed as pure.
        match crate::parser::parse(&crate::lexer::lex(body)) {
            Ok(module) => {
                let inferred = crate::effects::infer_effects(&module);
                let mut found: Vec<String> = module
                    .items
                    .iter()
                    .filter_map(|item| match &item.kind {
                        crate::ast::ItemKind::Function(f) => Some(inferred.effects_of(&f.name)),
                        _ => None,
                    })
                    .flat_map(|set| set.iter().map(|e| e.to_string()).collect::<Vec<_>>())
                    .collect();
                found.sort();
                found.dedup();
                for effect in found {
                    if !spec.effects.contains(&effect) {
                        result.effects_contained = false;
                        result.violations.push(format!("{effect} effect detected but not declared"));
                    }
                }
            }
            Err(e) => {
                result.parses = false;
                result.violations.push(format!("candidate does not parse: {}", e.message));
            }
        }

        // Check perf bounds.
        for (metric, bound) in &spec.perf_bounds {
            if metric == "complexity"
                && let Ok(max) = bound.parse::<usize>()
                    && cost.cyclomatic_complexity > max {
                        result.perf_bounds_met = false;
                        result.violations.push(format!(
                            "complexity {} exceeds bound {}",
                            cost.cyclomatic_complexity, max
                        ));
                    }
            if metric == "tokens"
                && let Ok(max) = bound.parse::<usize>()
                    && cost.token_count > max {
                        result.perf_bounds_met = false;
                        result
                            .violations
                            .push(format!("tokens {} exceeds bound {}", cost.token_count, max));
                    }
        }

        result
    }

    /// Rank candidates: valid first, then by cost score ascending.
    pub fn rank(&self, candidates: &mut [Candidate]) {
        candidates.sort_by(|a, b| {
            let a_valid = a.verification.is_valid();
            let b_valid = b.verification.is_valid();
            b_valid
                .cmp(&a_valid) // valid first
                .then_with(|| {
                    a.cost.score().partial_cmp(&b.cost.score()).unwrap_or(std::cmp::Ordering::Equal)
                })
        });
    }

    /// Generate, verify, rank, and return the best candidate (if any valid).
    pub fn synthesize(&mut self, spec: &SynthesisSpec) -> Option<Candidate> {
        let mut candidates = self.generate(spec);
        self.rank(&mut candidates);
        candidates.into_iter().find(|c| c.verification.is_valid())
    }

    /// JSON summary of synthesis run.
    pub fn synthesize_report(&mut self, spec: &SynthesisSpec) -> String {
        let mut candidates = self.generate(spec);
        self.rank(&mut candidates);
        let entries: Vec<String> = candidates.iter().map(|c| {
            format!(
                "{{\"id\":{},\"strategy\":\"{}\",\"score\":{:.1},\"valid\":{},\"violations\":{}}}",
                c.id,
                c.strategy,
                c.cost.score(),
                c.verification.is_valid(),
                c.verification.violations.len(),
            )
        }).collect();
        format!("{{\"spec\":\"{}\",\"candidates\":[{}]}}", spec.name, entries.join(","))
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_spec() -> SynthesisSpec {
        SynthesisSpec::new("add")
            .with_param("a", "i32")
            .with_param("b", "i32")
            .with_return("i32")
            .with_req("a >= 0")
            .with_ens("result == a + b")
    }

    // ── Spec construction ─────────────────────────────────────────

    #[test]
    fn spec_constraint_count() {
        let s = simple_spec().with_inv("result >= 0").with_perf("complexity", "5");
        assert_eq!(s.constraint_count(), 4); // 1 req + 1 ens + 1 inv + 1 perf
    }

    #[test]
    fn spec_empty() {
        let s = SynthesisSpec::new("noop");
        assert_eq!(s.constraint_count(), 0);
    }

    // ── Candidate generation ──────────────────────────────────────

    #[test]
    fn generates_all_strategies() {
        let mut oracle = SynthesisOracle::new();
        let candidates = oracle.generate(&simple_spec());
        assert_eq!(candidates.len(), 5);
        let strategies: Vec<_> = candidates.iter().map(|c| &c.strategy).collect();
        assert!(strategies.contains(&&Strategy::Imperative));
        assert!(strategies.contains(&&Strategy::Recursive));
        assert!(strategies.contains(&&Strategy::Functional));
    }

    #[test]
    fn candidate_ids_unique() {
        let mut oracle = SynthesisOracle::new();
        let c1 = oracle.generate(&simple_spec());
        let c2 = oracle.generate(&simple_spec());
        let ids1: Vec<u64> = c1.iter().map(|c| c.id).collect();
        let ids2: Vec<u64> = c2.iter().map(|c| c.id).collect();
        for id in &ids1 {
            assert!(!ids2.contains(id));
        }
    }

    // ── Cost estimation ───────────────────────────────────────────

    #[test]
    fn cost_score_ordering() {
        let low = CostEstimate {
            token_count: 5,
            cyclomatic_complexity: 1,
            allocation_count: 0,
            effect_count: 0,
        };
        let high = CostEstimate {
            token_count: 20,
            cyclomatic_complexity: 3,
            allocation_count: 2,
            effect_count: 1,
        };
        assert!(low.score() < high.score());
    }

    // ── Verification ──────────────────────────────────────────────

    #[test]
    fn valid_candidate_passes() {
        let mut oracle = SynthesisOracle::new();
        let spec = simple_spec();
        let candidates = oracle.generate(&spec);
        // Table-driven references params in the template implicitly
        let table = candidates.iter().find(|c| c.strategy == Strategy::TableDriven).unwrap();
        assert!(table.verification.is_valid());
    }

    #[test]
    fn perf_bound_violation() {
        let mut oracle = SynthesisOracle::new();
        let spec = SynthesisSpec::new("f")
            .with_param("x", "i32")
            .with_return("i32")
            .with_perf("complexity", "1");
        let candidates = oracle.generate(&spec);
        // Recursive and speculative have complexity 2
        let recursive = candidates.iter().find(|c| c.strategy == Strategy::Recursive).unwrap();
        assert!(!recursive.verification.perf_bounds_met);
    }

    #[test]
    fn token_bound_violation() {
        let mut oracle = SynthesisOracle::new();
        let spec = SynthesisSpec::new("f")
            .with_param("x", "i32")
            .with_return("i32")
            .with_perf("tokens", "5");
        let candidates = oracle.generate(&spec);
        // All candidates have >5 tokens
        assert!(candidates.iter().all(|c| !c.verification.perf_bounds_met));
    }

    #[test]
    fn effect_violation() {
        let mut oracle = SynthesisOracle::new();
        oracle.register_template(
            &Strategy::Imperative,
            "f {name}({params}) -> {return} { println(\"hello\"); 42 }",
            CostEstimate {
                token_count: 10,
                cyclomatic_complexity: 1,
                allocation_count: 0,
                effect_count: 1,
            },
        );
        let spec = SynthesisSpec::new("f").with_param("x", "i32").with_return("i32"); // no IO effect declared
        let candidates = oracle.generate(&spec);
        let imp = candidates.iter().find(|c| c.strategy == Strategy::Imperative).unwrap();
        assert!(imp.verification.parses, "{:?}", imp.verification.violations);
        assert!(!imp.verification.effects_contained);
        assert_eq!(imp.verification.violations, ["IO effect detected but not declared"]);
    }

    #[test]
    fn effect_allowed() {
        let mut oracle = SynthesisOracle::new();
        oracle.register_template(
            &Strategy::Imperative,
            "f {name}({params}) -> {return} { println(\"hello\"); 42 }",
            CostEstimate {
                token_count: 10,
                cyclomatic_complexity: 1,
                allocation_count: 0,
                effect_count: 1,
            },
        );
        let spec =
            SynthesisSpec::new("f").with_param("x", "i32").with_return("i32").with_effect("IO"); // IO allowed
        let candidates = oracle.generate(&spec);
        let imp = candidates.iter().find(|c| c.strategy == Strategy::Imperative).unwrap();
        assert!(imp.verification.effects_contained);
        assert!(imp.verification.is_valid(), "{:?}", imp.verification.violations);
    }

    #[test]
    fn an_effect_is_found_through_a_capability_handle_and_a_panic() {
        let mut oracle = SynthesisOracle::new();
        oracle.register_template(
            &Strategy::Imperative,
            "f {name}({params}) -> {return} { io.print(\"x\"); if x < 0 { panic(\"neg\") }; 42 }",
            CostEstimate { token_count: 10, cyclomatic_complexity: 1, allocation_count: 0, effect_count: 2 },
        );
        let spec = SynthesisSpec::new("f").with_param("x", "i32").with_return("i32");
        let candidates = oracle.generate(&spec);
        let imp = candidates.iter().find(|c| c.strategy == Strategy::Imperative).unwrap();
        assert!(imp.verification.parses, "{:?}", imp.verification.violations);
        assert_eq!(
            imp.verification.violations,
            ["IO effect detected but not declared", "Panic effect detected but not declared"]
        );
    }

    #[test]
    fn a_candidate_that_does_not_parse_is_not_valid() {
        let mut oracle = SynthesisOracle::new();
        oracle.register_template(
            &Strategy::Imperative,
            "fn {name}({params}) -> {return} { println!(\"hello\"); 42 }",
            CostEstimate { token_count: 10, cyclomatic_complexity: 1, allocation_count: 0, effect_count: 1 },
        );
        let spec = SynthesisSpec::new("f").with_param("x", "i32").with_return("i32").with_effect("IO");
        let candidates = oracle.generate(&spec);
        let imp = candidates.iter().find(|c| c.strategy == Strategy::Imperative).unwrap();
        assert!(!imp.verification.parses);
        assert!(!imp.verification.is_valid());
        assert!(imp.verification.violations.iter().any(|v| v.starts_with("candidate does not parse")));
    }

    // ── Ranking ───────────────────────────────────────────────────

    #[test]
    fn ranking_valid_first() {
        let mut oracle = SynthesisOracle::new();
        let spec = SynthesisSpec::new("f")
            .with_param("x", "i32")
            .with_return("i32")
            .with_perf("complexity", "1");
        let mut candidates = oracle.generate(&spec);
        oracle.rank(&mut candidates);
        // Valid candidates (complexity <= 1) come before invalid ones
        let first_invalid = candidates.iter().position(|c| !c.verification.is_valid());
        let last_valid = candidates.iter().rposition(|c| c.verification.is_valid());
        if let (Some(fi), Some(lv)) = (first_invalid, last_valid) {
            assert!(lv < fi);
        }
    }

    #[test]
    fn ranking_by_cost_within_valid() {
        let mut oracle = SynthesisOracle::new();
        let spec = simple_spec();
        let mut candidates = oracle.generate(&spec);
        oracle.rank(&mut candidates);
        let valid: Vec<_> = candidates.iter().filter(|c| c.verification.is_valid()).collect();
        for pair in valid.windows(2) {
            assert!(pair[0].cost.score() <= pair[1].cost.score());
        }
    }

    // ── Synthesize (end-to-end) ───────────────────────────────────

    #[test]
    fn synthesize_returns_best() {
        let mut oracle = SynthesisOracle::new();
        let best = oracle.synthesize(&simple_spec());
        assert!(best.is_some());
        assert!(best.unwrap().verification.is_valid());
    }

    #[test]
    fn synthesize_report_json() {
        let mut oracle = SynthesisOracle::new();
        let report = oracle.synthesize_report(&simple_spec());
        assert!(report.contains("\"spec\":\"add\""));
        assert!(report.contains("\"candidates\":["));
    }

    // ── Custom templates ──────────────────────────────────────────

    #[test]
    fn custom_template() {
        let mut oracle = SynthesisOracle::new();
        oracle.register_template(
            &Strategy::Imperative,
            "f {name}({params}) -> {return} { a + b }",
            CostEstimate {
                token_count: 5,
                cyclomatic_complexity: 1,
                allocation_count: 0,
                effect_count: 0,
            },
        );
        let spec = simple_spec();
        let candidates = oracle.generate(&spec);
        let imp = candidates.iter().find(|c| c.strategy == Strategy::Imperative).unwrap();
        assert!(imp.body.contains("a + b"));
        assert_eq!(imp.cost.token_count, 5);
    }

    // ── Strategy display ──────────────────────────────────────────

    #[test]
    fn strategy_display() {
        assert_eq!(format!("{}", Strategy::Imperative), "imperative");
        assert_eq!(format!("{}", Strategy::TableDriven), "table-driven");
    }

    // ── Verification result ───────────────────────────────────────

    #[test]
    fn verification_default_valid() {
        let v = VerificationResult::default();
        assert!(v.is_valid());
        assert!(v.violations.is_empty());
    }

    // ── Generated text is MAGE ────────────────────────────────────

    /// Every strategy's generated function lexes and parses as MAGE, and the
    /// parsed item is a function named for the spec. The stubs name
    /// placeholders (`base_case`, `combine`), so this holds them to the
    /// parser, not the type checker.
    #[test]
    fn every_strategy_stub_parses_as_mage() {
        use crate::ast::ItemKind;
        let mut oracle = SynthesisOracle::new();
        let candidates = oracle.generate(&simple_spec());
        assert_eq!(candidates.len(), 5, "one candidate per strategy");
        for c in &candidates {
            let tokens = crate::lexer::lex(&c.body);
            let module = crate::parser::parse(&tokens).unwrap_or_else(|e| {
                panic!("strategy {} does not parse: {}\n{}", c.strategy, e.message, c.body)
            });
            assert_eq!(module.items.len(), 1, "strategy {}: {}", c.strategy, c.body);
            let ItemKind::Function(f) = &module.items[0].kind else {
                panic!("strategy {} is not a function: {}", c.strategy, c.body);
            };
            assert_eq!(f.name, "add", "strategy {}", c.strategy);
            assert_eq!(f.params.len(), 2, "strategy {}", c.strategy);
        }
    }
}

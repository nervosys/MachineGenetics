//! Solver choice from a convexity certificate (plan 6.3).
//!
//! Minimising (or maximising) a function over a box of parameters can be done
//! by two methods, and the convexity pass decides which one is safe to use:
//!
//! - **Projected gradient** (`Solver::ProjectedGradient`) is exact on a convex
//!   problem: every local minimum is global, so descent reaches the optimum.
//!   It runs only when `convexity::function_curvature` certifies the function
//!   with the right curvature for the sense (`Convex` to minimise, `Concave` to
//!   maximise; `Affine` and `Constant` fit both), and every domain is `Real`.
//! - **Evolution strategy** (`Solver::Es`), a (1+1)-ES with the 1/5th success
//!   rule, is the fallback. It makes no assumption about the shape of the
//!   function, so it is safe on anything, at a higher cost in evaluations.
//!
//! The choice rule is `choose`. Its reason string names every check that
//! failed, so the journal records exactly why a solver ran.
//!
//! **`Unknown` falls back conservatively.** A `Convex` or `Concave` verdict is
//! a proof, but `Unknown` only means DCP cannot certify the function. `2 * x * x`
//! is convex and still gets ES, because the DCP rules do not reassociate
//! products. A non-convex function is never sent to projected gradient: the
//! only route to it is a certificate.
//!
//! **Fuel is the cost measure.** Every evaluation runs through
//! `eval::run_metered`, and the fuel it spends is the crate's proxy for joules.
//! A search stops when its budget is spent. Each evaluation is capped at the
//! budget that remains, and a search stops cleanly when that cap reaches zero.
//!
//! **The journal.** `Solution` derives `serde::Serialize`. Its JSON record
//! names the solver that ran (`solver`) and the reason it was chosen
//! (`reason`), so a run can be audited after the fact.

use crate::ast::{self, ItemKind};
use crate::convexity;
use crate::eval::{self, BoundedError, Value};
use crate::params::{self, Domain};

/// Whether the objective is minimised or maximised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sense {
    Minimise,
    Maximise,
}

/// The method that ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Solver {
    /// Projected gradient descent, exact on a certified convex problem.
    ProjectedGradient,
    /// A (1+1)-evolution strategy, for everything else.
    Es,
}

/// The result of a solve, and the journal record of how it was obtained.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Solution {
    /// The solver that ran.
    pub solver: Solver,
    /// Why that solver was chosen.
    pub reason: String,
    /// The best point found, one coordinate per parameter.
    pub x: Vec<f64>,
    /// The objective at `x`, in the caller's sense (not negated when maximising).
    pub value: f64,
    /// The number of objective evaluations.
    pub evals: u64,
    /// The fuel spent across all evaluations.
    pub fuel: u64,
}

/// Choose the solver for `fd` from its convexity certificate.
///
/// Projected gradient only if the curvature fits the sense (`Convex` or an
/// affine/constant function to minimise, `Concave` or affine/constant to
/// maximise) and every domain is `Real`. Otherwise ES, and the reason says
/// which check failed.
pub fn choose(fd: &ast::FunctionDef, sense: Sense, domains: &[Domain]) -> (Solver, String) {
    let curvature = convexity::function_curvature(fd);
    let mut failures = Vec::new();

    let fits = match sense {
        Sense::Minimise => curvature.is_convex(),
        Sense::Maximise => curvature.is_concave(),
    };
    if !fits {
        let needed = match sense {
            Sense::Minimise => "convex",
            Sense::Maximise => "concave",
        };
        failures.push(format!(
            "curvature is {curvature:?}, not certified {needed} for {}",
            sense_name(sense)
        ));
    }
    if let Some(i) = domains.iter().position(|d| !matches!(d, Domain::Real { .. })) {
        failures.push(format!("domain {i} is not real"));
    }

    if failures.is_empty() {
        let reason = format!(
            "curvature is {curvature:?}, certified for {}, and every domain is real",
            sense_name(sense)
        );
        (Solver::ProjectedGradient, reason)
    } else {
        (Solver::Es, failures.join("; "))
    }
}

fn sense_name(sense: Sense) -> &'static str {
    match sense {
        Sense::Minimise => "minimisation",
        Sense::Maximise => "maximisation",
    }
}

/// Solve `name` over the box given by `domains`, within `budget_fuel`.
///
/// Errors when the function is missing, when its arity differs from
/// `domains.len()`, when an evaluation fails or returns a non-number, or when
/// the budget runs out before the first evaluation.
pub fn solve(
    module: &ast::Module,
    name: &str,
    sense: Sense,
    domains: &[Domain],
    budget_fuel: u64,
    seed: u64,
) -> Result<Solution, String> {
    let fd = find_function(module, name)?;
    if fd.params.len() != domains.len() {
        return Err(format!(
            "`{name}` takes {} parameter(s), but {} domain(s) were given",
            fd.params.len(),
            domains.len()
        ));
    }
    let (solver, reason) = choose(fd, sense, domains);
    execute(module, name, sense, domains, budget_fuel, seed, solver, reason, None)
}

/// Solve `name` with the domains taken from the `@param` consts whose names
/// match the function's parameters, case-insensitively.
pub fn solve_params(
    module: &ast::Module,
    name: &str,
    sense: Sense,
    budget_fuel: u64,
    seed: u64,
) -> Result<Solution, String> {
    let fd = find_function(module, name)?;
    let (decls, diags) = params::params(module);
    if let Some(d) = diags.first() {
        return Err(format!("invalid @param declaration: {}", d.message));
    }
    let mut domains = Vec::with_capacity(fd.params.len());
    for p in &fd.params {
        match decls.iter().find(|d| d.name.eq_ignore_ascii_case(&p.name)) {
            Some(d) => domains.push(d.domain.clone()),
            None => {
                return Err(format!(
                    "parameter `{}` of `{name}` has no matching @param const",
                    p.name
                ))
            }
        }
    }
    solve(module, name, sense, &domains, budget_fuel, seed)
}

fn find_function<'m>(module: &'m ast::Module, name: &str) -> Result<&'m ast::FunctionDef, String> {
    module
        .items
        .iter()
        .find_map(|item| match &item.kind {
            ItemKind::Function(fd) if fd.name == name => Some(fd),
            _ => None,
        })
        .ok_or_else(|| format!("no function named `{name}`"))
}

/// Run `solver` directly, bypassing the choice. `target`, when given, stops
/// the search once the minimised objective reaches it (the test helper uses
/// this to measure how much fuel ES needs).
#[allow(clippy::too_many_arguments)]
fn execute(
    module: &ast::Module,
    name: &str,
    sense: Sense,
    domains: &[Domain],
    budget_fuel: u64,
    seed: u64,
    solver: Solver,
    reason: String,
    target: Option<f64>,
) -> Result<Solution, String> {
    let mut meter = Meter { module, name, budget: budget_fuel, spent: 0, evals: 0 };
    let found = match solver {
        Solver::ProjectedGradient => projected_gradient(&mut meter, domains, sense)?,
        Solver::Es => evolution_strategy(&mut meter, domains, sense, seed, target)?,
    };
    let (x, objective) = found.ok_or_else(|| {
        "the fuel budget ran out before the first evaluation".to_string()
    })?;
    let value = match sense {
        Sense::Minimise => objective,
        Sense::Maximise => -objective,
    };
    Ok(Solution {
        solver,
        reason,
        x,
        value,
        evals: meter.evals,
        fuel: meter.spent,
    })
}

/// The objective as the searches see it: minimised, whatever the sense.
fn objective_of(sense: Sense, value: f64) -> f64 {
    match sense {
        Sense::Minimise => value,
        Sense::Maximise => -value,
    }
}

/// Runs the function under a fuel budget, and keeps count.
struct Meter<'a> {
    module: &'a ast::Module,
    name: &'a str,
    budget: u64,
    spent: u64,
    evals: u64,
}

impl Meter<'_> {
    /// Evaluate the function at `x`. `Ok(None)` means the budget is spent, or
    /// this evaluation ran out of it: the search should stop there.
    fn eval(&mut self, x: &[f64]) -> Result<Option<f64>, String> {
        let left = self.budget - self.spent;
        // Each run should cost at least one fuel. The second test stops a
        // search that somehow costs nothing, so it cannot loop forever.
        if left == 0 || self.evals > self.budget {
            return Ok(None);
        }
        let args: Vec<Value> = x.iter().map(|v| Value::Float(*v)).collect();
        let (result, used) = eval::run_metered(self.module, self.name, args, left);
        self.spent += used.min(left);
        self.evals += 1;
        match result {
            Ok(Value::Float(v)) => Ok(Some(v)),
            Ok(Value::Int(i)) => Ok(Some(i as f64)),
            Ok(other) => Err(format!("`{}` returned {other:?}, not a number", self.name)),
            Err(BoundedError::FuelExhausted) => Ok(None),
            Err(BoundedError::Error(msg)) => Err(msg),
        }
    }
}

/// The bounds of each domain, which projected gradient needs.
fn real_bounds(domains: &[Domain]) -> Result<Vec<(f64, f64)>, String> {
    domains
        .iter()
        .enumerate()
        .map(|(i, d)| match d {
            Domain::Real { lo, hi } => Ok((*lo, *hi)),
            _ => Err(format!("domain {i} is not real, so projected gradient cannot run")),
        })
        .collect()
}

/// Projected gradient descent with central-difference gradients and Armijo
/// backtracking. The result is `(x, minimised objective)`, or `None` if the
/// budget ran out before the first evaluation.
fn projected_gradient(
    meter: &mut Meter,
    domains: &[Domain],
    sense: Sense,
) -> Result<Option<(Vec<f64>, f64)>, String> {
    let bounds = real_bounds(domains)?;
    let project = |p: f64, (lo, hi): (f64, f64)| p.max(lo).min(hi);
    let mut x: Vec<f64> = bounds.iter().map(|(lo, hi)| 0.5 * (lo + hi)).collect();
    let mut fx = match meter.eval(&x)? {
        Some(v) => objective_of(sense, v),
        None => return Ok(None),
    };

    loop {
        // Central-difference gradient. A budget stop mid-gradient keeps the
        // current point, which is the best one found so far.
        let mut grad = vec![0.0; x.len()];
        for i in 0..x.len() {
            let h = 1e-6 * x[i].abs().max(1.0);
            let mut plus = x.clone();
            plus[i] += h;
            let mut minus = x.clone();
            minus[i] -= h;
            let (Some(fp), Some(fm)) = (meter.eval(&plus)?, meter.eval(&minus)?) else {
                return Ok(Some((x, fx)));
            };
            grad[i] = (objective_of(sense, fp) - objective_of(sense, fm)) / (2.0 * h);
        }

        // Armijo backtracking along the projected steepest descent.
        let mut step = 1.0;
        let mut accepted = None;
        for _ in 0..=40 {
            let candidate: Vec<f64> = x
                .iter()
                .zip(&grad)
                .zip(&bounds)
                .map(|((xi, gi), b)| project(xi - step * gi, *b))
                .collect();
            let fc = match meter.eval(&candidate)? {
                Some(v) => objective_of(sense, v),
                None => return Ok(Some((x, fx))),
            };
            let decrease: f64 = grad
                .iter()
                .zip(candidate.iter().zip(&x))
                .map(|(g, (c, xi))| g * (c - xi))
                .sum();
            if fc <= fx + 1e-4 * decrease {
                accepted = Some((candidate, fc));
                break;
            }
            step *= 0.5;
        }

        // No sufficient decrease in 41 trials: the point is as good as the
        // local model allows, so the search stops.
        let Some((candidate, fc)) = accepted else {
            return Ok(Some((x, fx)));
        };
        let largest_move = candidate
            .iter()
            .zip(&x)
            .map(|(c, xi)| (c - xi).abs())
            .fold(0.0_f64, f64::max);
        x = candidate;
        fx = fc;
        if largest_move < 1e-9 {
            return Ok(Some((x, fx)));
        }
    }
}

/// A (1+1)-evolution strategy with the 1/5th success rule.
///
/// The step is `sigma` times each box width, so one scale serves every
/// coordinate. It is adapted every ten evaluations: up when more than a fifth
/// of the mutations improved, down when fewer did. Integer domains round the
/// candidate, and set domains snap it to the nearest member. The search stops
/// when the budget runs out, or when the minimised objective reaches `target`.
fn evolution_strategy(
    meter: &mut Meter,
    domains: &[Domain],
    sense: Sense,
    seed: u64,
    target: Option<f64>,
) -> Result<Option<(Vec<f64>, f64)>, String> {
    let widths: Vec<f64> = domains.iter().map(domain_width).collect();
    let mut rng = Rng::new(seed);
    let mut x: Vec<f64> = domains.iter().map(|d| snap(d, midpoint(d))).collect();
    let mut fx = match meter.eval(&x)? {
        Some(v) => objective_of(sense, v),
        None => return Ok(None),
    };

    let mut sigma = 0.3;
    let mut window = 0_u32;
    let mut successes = 0_u32;
    while target.is_none_or(|t| fx > t) {
        let candidate: Vec<f64> = x
            .iter()
            .zip(&widths)
            .zip(domains)
            .map(|((xi, w), d)| snap(d, xi + sigma * w * rng.normal()))
            .collect();
        let fc = match meter.eval(&candidate)? {
            Some(v) => objective_of(sense, v),
            None => break,
        };
        window += 1;
        if fc < fx {
            successes += 1;
        }
        if fc <= fx {
            x = candidate;
            fx = fc;
        }
        if window == 10 {
            let rate = f64::from(successes) / 10.0;
            sigma = if rate > 0.2 { sigma * 1.5 } else if rate < 0.2 { sigma * 0.82 } else { sigma };
            sigma = sigma.clamp(1e-9, 1.0);
            window = 0;
            successes = 0;
        }
    }
    Ok(Some((x, fx)))
}

/// The width of a domain, used to scale ES steps.
fn domain_width(d: &Domain) -> f64 {
    match d {
        Domain::Real { lo, hi } => hi - lo,
        Domain::Int { lo, hi } => (hi - lo) as f64,
        Domain::Set(members) => {
            let lo = members.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = members.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            hi - lo
        }
    }
}

fn midpoint(d: &Domain) -> f64 {
    match d {
        Domain::Real { lo, hi } => 0.5 * (lo + hi),
        Domain::Int { lo, hi } => 0.5 * (*lo as f64 + *hi as f64),
        Domain::Set(members) => {
            let lo = members.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = members.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            0.5 * (lo + hi)
        }
    }
}

/// Move `v` into the domain: clamp a real, round then clamp an integer, and
/// snap a set to its nearest member. An empty set returns `v` unchanged.
fn snap(d: &Domain, v: f64) -> f64 {
    match d {
        Domain::Real { lo, hi } => v.max(*lo).min(*hi),
        Domain::Int { lo, hi } => v.round().max(*lo as f64).min(*hi as f64),
        Domain::Set(members) => members
            .iter()
            .copied()
            .min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs()))
            .unwrap_or(v),
    }
}

/// A xorshift64* generator with Box-Muller normals. Deterministic for a seed,
/// and needs no external crate.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A uniform value in the open interval (0, 1).
    fn unit(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 0.5) / (1_u64 << 53) as f64
    }

    /// A standard normal value.
    fn normal(&mut self) -> f64 {
        let u1 = self.unit();
        let u2 = self.unit();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// Run the evolution strategy to a target, with no choice step. The test
/// suite uses this to measure how much fuel ES needs on a problem.
#[cfg(test)]
fn es_until(
    module: &ast::Module,
    name: &str,
    sense: Sense,
    domains: &[Domain],
    budget_fuel: u64,
    seed: u64,
    target: f64,
) -> Result<Solution, String> {
    execute(
        module,
        name,
        sense,
        domains,
        budget_fuel,
        seed,
        Solver::Es,
        "forced for a comparison in the tests".to_string(),
        Some(target),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, parser};

    fn module(src: &str) -> ast::Module {
        parser::parse(&lexer::lex(src)).expect("parse failed")
    }

    fn real(lo: f64, hi: f64) -> Domain {
        Domain::Real { lo, hi }
    }

    const QUADRATIC: &str =
        "f quad(x: f64, y: f64) -> f64 { (x - 1.5) * (x - 1.5) + (y + 0.5) * (y + 0.5) }";

    #[test]
    fn convex_quadratic_takes_projected_gradient_to_the_optimum() {
        let m = module(QUADRATIC);
        let domains = [real(-4.0, 4.0), real(-4.0, 4.0)];
        let s = solve(&m, "quad", Sense::Minimise, &domains, 1_000_000, 1).unwrap();
        assert_eq!(s.solver, Solver::ProjectedGradient, "{}", s.reason);
        assert!((s.x[0] - 1.5).abs() < 1e-4, "x = {:?}", s.x);
        assert!((s.x[1] + 0.5).abs() < 1e-4, "x = {:?}", s.x);
        assert!(s.value < 1e-8, "value = {}", s.value);
    }

    #[test]
    fn projected_gradient_spends_less_fuel_than_es_on_the_same_problem() {
        let m = module(QUADRATIC);
        let domains = [real(-4.0, 4.0), real(-4.0, 4.0)];
        let pg = solve(&m, "quad", Sense::Minimise, &domains, 1_000_000, 1).unwrap();
        // ES must reach the optimum value within 1e-4. A large budget is the
        // cap: if ES never gets there, it has still spent more than the gradient.
        let es = es_until(&m, "quad", Sense::Minimise, &domains, 5_000_000, 1, 1e-4).unwrap();
        assert!(es.value <= 1e-4, "ES did not reach the optimum: {es:?}");
        assert!(
            pg.fuel < es.fuel,
            "gradient fuel {} is not below ES fuel {}",
            pg.fuel,
            es.fuel
        );
        eprintln!("test 1 fuel: projected gradient {} ({} evals), ES {} ({} evals)", pg.fuel, pg.evals, es.fuel, es.evals);
    }

    #[test]
    fn an_active_bound_is_respected() {
        let m = module("f g(x: f64) -> f64 { (x - 10.0) * (x - 10.0) }");
        let s = solve(&m, "g", Sense::Minimise, &[real(0.0, 4.0)], 1_000_000, 1).unwrap();
        assert_eq!(s.solver, Solver::ProjectedGradient);
        assert!((s.x[0] - 4.0).abs() < 1e-9, "x = {:?}", s.x);
    }

    #[test]
    fn exponential_minimum_is_ln_two() {
        let m = module("f h(x: f64) -> f64 { exp(x) - 2.0 * x }");
        let s = solve(&m, "h", Sense::Minimise, &[real(-3.0, 3.0)], 1_000_000, 1).unwrap();
        assert_eq!(s.solver, Solver::ProjectedGradient);
        assert!((s.x[0] - 2.0_f64.ln()).abs() < 1e-4, "x = {:?}", s.x);
    }

    #[test]
    fn maximising_a_concave_function_takes_projected_gradient() {
        let m = module("f p(x: f64) -> f64 { -(x - 1.0) * (x - 1.0) }");
        let s = solve(&m, "p", Sense::Maximise, &[real(-3.0, 3.0)], 1_000_000, 1).unwrap();
        assert_eq!(s.solver, Solver::ProjectedGradient, "{}", s.reason);
        assert!((s.x[0] - 1.0).abs() < 1e-4, "x = {:?}", s.x);
        assert!(s.value.abs() < 1e-8, "value = {}", s.value);
    }

    #[test]
    fn a_non_convex_or_uncertified_function_never_takes_the_convex_route() {
        // (name, source, sense, domains). Each is Unknown or the wrong curvature
        // for its sense, or has a non-real domain, so none may use projected gradient.
        let cases: Vec<(&str, &str, Sense, Vec<Domain>)> = vec![
            ("x*y", "f a(x: f64, y: f64) -> f64 { x * y }", Sense::Minimise, vec![real(-2.0, 2.0), real(-2.0, 2.0)]),
            ("-(x*x) minimised", "f b(x: f64) -> f64 { -(x * x) }", Sense::Minimise, vec![real(-2.0, 2.0)]),
            ("2*x*x", "f c(x: f64) -> f64 { 2.0 * x * x }", Sense::Minimise, vec![real(-2.0, 2.0)]),
            (
                "convex with an Int domain",
                "f d(x: f64) -> f64 { (x - 1.5) * (x - 1.5) }",
                Sense::Minimise,
                vec![Domain::Int { lo: -4, hi: 4 }],
            ),
            (
                "convex maximised",
                "f e(x: f64) -> f64 { (x - 1.5) * (x - 1.5) }",
                Sense::Maximise,
                vec![real(-4.0, 4.0)],
            ),
        ];
        for (label, src, sense, domains) in cases {
            let m = module(src);
            let fd = find_fn(&m);
            let (solver, reason) = choose(fd, sense, &domains);
            assert_eq!(solver, Solver::Es, "{label}: {reason}");
            let s = solve(&m, &fd.name, sense, &domains, 20_000, 3).unwrap();
            assert_eq!(s.solver, Solver::Es, "{label}: {}", s.reason);
        }
    }

    fn find_fn(m: &ast::Module) -> &ast::FunctionDef {
        match &m.items[0].kind {
            ItemKind::Function(fd) => fd,
            _ => panic!("not a function"),
        }
    }

    #[test]
    fn es_finds_the_minimum_of_a_non_convex_double_well() {
        // Two equal minima at x = -1 and x = +1; no certificate applies.
        let m = module("f w(x: f64) -> f64 { (x * x - 1.0) * (x * x - 1.0) }");
        let s = solve(&m, "w", Sense::Minimise, &[real(-2.0, 2.0)], 200_000, 7).unwrap();
        assert_eq!(s.solver, Solver::Es);
        assert!((s.x[0].abs() - 1.0).abs() < 1e-2, "x = {:?}", s.x);
        assert!(s.value < 1e-3, "value = {}", s.value);
    }

    #[test]
    fn es_finds_an_integer_minimum_and_is_deterministic_for_a_seed() {
        let m = module("f k(x: f64) -> f64 { abs(x - 2.0) }");
        let domains = [Domain::Int { lo: -10, hi: 10 }];
        let a = solve(&m, "k", Sense::Minimise, &domains, 50_000, 42).unwrap();
        let b = solve(&m, "k", Sense::Minimise, &domains, 50_000, 42).unwrap();
        assert_eq!(a.solver, Solver::Es);
        assert_eq!(a.x, vec![2.0], "x = {:?}", a.x);
        assert_eq!(a.value, 0.0);
        assert_eq!((a.x.clone(), a.fuel, a.evals), (b.x.clone(), b.fuel, b.evals));
    }

    #[test]
    fn solve_params_reads_domains_from_param_consts() {
        // The consts are upper case and the parameters lower case, so the
        // match must be case-insensitive. `Y` is a keyword, hence `a` and `b`.
        let src = "@param(-4.0, 4.0)\nconst A: f64 = 0.0;\n@param(-4.0, 4.0)\nconst B: f64 = 0.0;\n\
            f quad(a: f64, b: f64) -> f64 { (a - 1.5) * (a - 1.5) + (b + 0.5) * (b + 0.5) }";
        let m = module(src);
        let s = solve_params(&m, "quad", Sense::Minimise, 1_000_000, 1).unwrap();
        assert_eq!(s.solver, Solver::ProjectedGradient, "{}", s.reason);
        assert!((s.x[0] - 1.5).abs() < 1e-4 && (s.x[1] + 0.5).abs() < 1e-4, "x = {:?}", s.x);
    }

    #[test]
    fn solve_params_errors_when_a_parameter_has_no_param_const() {
        let m = module(QUADRATIC);
        let err = solve_params(&m, "quad", Sense::Minimise, 1_000, 1).unwrap_err();
        assert!(err.contains("no matching @param"), "{err}");
    }

    #[test]
    fn solution_journal_names_the_solver_and_the_reason() {
        let m = module(QUADRATIC);
        let domains = [real(-4.0, 4.0), real(-4.0, 4.0)];
        let s = solve(&m, "quad", Sense::Minimise, &domains, 1_000_000, 1).unwrap();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"solver\""), "{json}");
        assert!(json.contains("\"reason\""), "{json}");
    }
}

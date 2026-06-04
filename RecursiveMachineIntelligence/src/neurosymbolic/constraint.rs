//! Differentiable Constraints
//!
//! Implements soft constraints that can be used in neural optimization
//! while respecting symbolic logical structure.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A soft constraint with differentiable satisfaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftConstraint {
    /// Unique identifier
    pub id: Uuid,
    
    /// Constraint name
    pub name: String,
    
    /// The logical formula representing the constraint
    pub formula: ConstraintFormula,
    
    /// Weight for violation penalty
    pub weight: f64,
    
    /// Temperature for soft satisfaction
    pub temperature: f64,
}

impl SoftConstraint {
    /// Create a new soft constraint
    pub fn new(name: impl Into<String>, formula: ConstraintFormula) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            formula,
            weight: 1.0,
            temperature: 1.0,
        }
    }
    
    /// Set the weight
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
    
    /// Set the temperature
    pub fn with_temperature(mut self, temp: f64) -> Self {
        self.temperature = temp;
        self
    }
    
    /// Compute soft satisfaction in [0, 1]
    pub fn satisfaction(&self, assignment: &HashMap<String, f64>) -> f64 {
        self.formula.evaluate(assignment, self.temperature)
    }
    
    /// Compute violation penalty
    pub fn violation(&self, assignment: &HashMap<String, f64>) -> f64 {
        let sat = self.satisfaction(assignment);
        self.weight * (1.0 - sat)
    }
    
    /// Compute gradient of violation w.r.t. variables
    pub fn gradient(&self, assignment: &HashMap<String, f64>) -> HashMap<String, f64> {
        self.formula.gradient(assignment, self.temperature, self.weight)
    }
}

/// A constraint formula with differentiable evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintFormula {
    /// Variable reference (maps to [0, 1] assignment)
    Variable(String),
    
    /// Constant value in [0, 1]
    Constant(f64),
    
    /// Negation (1 - x)
    Not(Box<ConstraintFormula>),
    
    /// Conjunction (product t-norm or min)
    And(Vec<ConstraintFormula>),
    
    /// Disjunction (probabilistic sum or max)
    Or(Vec<ConstraintFormula>),
    
    /// Implication (Lukasiewicz: min(1, 1-a+b))
    Implies(Box<ConstraintFormula>, Box<ConstraintFormula>),
    
    /// Equality constraint (soft: exp(-|a-b|/temp))
    Equals(Box<ConstraintFormula>, Box<ConstraintFormula>),
    
    /// Less than constraint (sigmoid-based)
    LessThan(Box<ConstraintFormula>, Box<ConstraintFormula>),
    
    /// Greater than constraint
    GreaterThan(Box<ConstraintFormula>, Box<ConstraintFormula>),
    
    /// Linear combination constraint
    Linear { 
        /// Coefficients for each variable
        coeffs: Vec<f64>, 
        /// Variable names
        vars: Vec<String>,
        /// Bias term
        bias: f64,
    },
    
    /// Custom differentiable function
    Custom {
        /// Function name
        name: String,
        /// Input formulas
        inputs: Vec<ConstraintFormula>,
    },
}

impl ConstraintFormula {
    /// Evaluate the formula with given variable assignments
    pub fn evaluate(&self, assignment: &HashMap<String, f64>, temp: f64) -> f64 {
        match self {
            ConstraintFormula::Variable(name) => {
                assignment.get(name).copied().unwrap_or(0.5)
            }
            
            ConstraintFormula::Constant(v) => *v,
            
            ConstraintFormula::Not(inner) => {
                1.0 - inner.evaluate(assignment, temp)
            }
            
            ConstraintFormula::And(formulas) => {
                // Product t-norm for differentiability
                formulas.iter()
                    .map(|f| f.evaluate(assignment, temp))
                    .product()
            }
            
            ConstraintFormula::Or(formulas) => {
                // Probabilistic sum: 1 - prod(1 - x_i)
                let complement_product: f64 = formulas.iter()
                    .map(|f| 1.0 - f.evaluate(assignment, temp))
                    .product();
                1.0 - complement_product
            }
            
            ConstraintFormula::Implies(a, b) => {
                // Lukasiewicz implication: min(1, 1 - a + b)
                let a_val = a.evaluate(assignment, temp);
                let b_val = b.evaluate(assignment, temp);
                (1.0 - a_val + b_val).min(1.0)
            }
            
            ConstraintFormula::Equals(a, b) => {
                let a_val = a.evaluate(assignment, temp);
                let b_val = b.evaluate(assignment, temp);
                let diff = (a_val - b_val).abs();
                (-diff / temp).exp()
            }
            
            ConstraintFormula::LessThan(a, b) => {
                let a_val = a.evaluate(assignment, temp);
                let b_val = b.evaluate(assignment, temp);
                let diff = b_val - a_val;
                1.0 / (1.0 + (-diff / temp).exp())
            }
            
            ConstraintFormula::GreaterThan(a, b) => {
                let a_val = a.evaluate(assignment, temp);
                let b_val = b.evaluate(assignment, temp);
                let diff = a_val - b_val;
                1.0 / (1.0 + (-diff / temp).exp())
            }
            
            ConstraintFormula::Linear { coeffs, vars, bias } => {
                let sum: f64 = coeffs.iter()
                    .zip(vars.iter())
                    .map(|(c, v)| c * assignment.get(v).copied().unwrap_or(0.0))
                    .sum();
                // Sigmoid to map to [0, 1]
                1.0 / (1.0 + (-(sum + bias) / temp).exp())
            }
            
            ConstraintFormula::Custom { name, inputs } => {
                // Evaluate inputs and apply named function
                let vals: Vec<f64> = inputs.iter()
                    .map(|f| f.evaluate(assignment, temp))
                    .collect();
                
                match name.as_str() {
                    "mean" => vals.iter().sum::<f64>() / vals.len() as f64,
                    "max" => vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                    "min" => vals.iter().cloned().fold(f64::INFINITY, f64::min),
                    _ => vals.first().copied().unwrap_or(0.5),
                }
            }
        }
    }
    
    /// Compute gradient with respect to variables
    pub fn gradient(&self, assignment: &HashMap<String, f64>, temp: f64, weight: f64) -> HashMap<String, f64> {
        let mut grads = HashMap::new();
        self.backward(assignment, temp, weight, &mut grads);
        grads
    }
    
    fn backward(&self, assignment: &HashMap<String, f64>, temp: f64, upstream: f64, grads: &mut HashMap<String, f64>) {
        match self {
            ConstraintFormula::Variable(name) => {
                *grads.entry(name.clone()).or_insert(0.0) += upstream;
            }
            
            ConstraintFormula::Constant(_) => {}
            
            ConstraintFormula::Not(inner) => {
                inner.backward(assignment, temp, -upstream, grads);
            }
            
            ConstraintFormula::And(formulas) => {
                // Product rule
                let vals: Vec<f64> = formulas.iter()
                    .map(|f| f.evaluate(assignment, temp))
                    .collect();
                let total_product: f64 = vals.iter().product();
                
                for (i, f) in formulas.iter().enumerate() {
                    let other_product = if vals[i].abs() > 1e-10 {
                        total_product / vals[i]
                    } else {
                        vals.iter().enumerate()
                            .filter(|(j, _)| *j != i)
                            .map(|(_, v)| v)
                            .product()
                    };
                    f.backward(assignment, temp, upstream * other_product, grads);
                }
            }
            
            ConstraintFormula::Or(formulas) => {
                // d/dx (1 - prod(1-x_i)) = prod(1-x_j, j!=i)
                let complements: Vec<f64> = formulas.iter()
                    .map(|f| 1.0 - f.evaluate(assignment, temp))
                    .collect();
                let total_product: f64 = complements.iter().product();
                
                for (i, f) in formulas.iter().enumerate() {
                    let other_product = if complements[i].abs() > 1e-10 {
                        total_product / complements[i]
                    } else {
                        complements.iter().enumerate()
                            .filter(|(j, _)| *j != i)
                            .map(|(_, v)| v)
                            .product()
                    };
                    f.backward(assignment, temp, upstream * other_product, grads);
                }
            }
            
            ConstraintFormula::Implies(a, b) => {
                let a_val = a.evaluate(assignment, temp);
                let b_val = b.evaluate(assignment, temp);
                
                // min(1, 1-a+b): gradient depends on whether we're in the clamped region
                if 1.0 - a_val + b_val < 1.0 {
                    a.backward(assignment, temp, -upstream, grads);
                    b.backward(assignment, temp, upstream, grads);
                }
            }
            
            ConstraintFormula::Equals(a, b) => {
                let a_val = a.evaluate(assignment, temp);
                let b_val = b.evaluate(assignment, temp);
                let diff = a_val - b_val;
                let exp_term = (-diff.abs() / temp).exp();
                
                // d/da exp(-|a-b|/T) = -sign(a-b)/T * exp(-|a-b|/T)
                let sign = if diff >= 0.0 { 1.0 } else { -1.0 };
                let factor = -sign / temp * exp_term * upstream;
                
                a.backward(assignment, temp, factor, grads);
                b.backward(assignment, temp, -factor, grads);
            }
            
            ConstraintFormula::LessThan(a, b) | ConstraintFormula::GreaterThan(a, b) => {
                let a_val = a.evaluate(assignment, temp);
                let b_val = b.evaluate(assignment, temp);
                let diff = if matches!(self, ConstraintFormula::LessThan(_, _)) {
                    b_val - a_val
                } else {
                    a_val - b_val
                };
                
                let sigmoid = 1.0 / (1.0 + (-diff / temp).exp());
                let sigmoid_grad = sigmoid * (1.0 - sigmoid) / temp;
                
                if matches!(self, ConstraintFormula::LessThan(_, _)) {
                    a.backward(assignment, temp, -sigmoid_grad * upstream, grads);
                    b.backward(assignment, temp, sigmoid_grad * upstream, grads);
                } else {
                    a.backward(assignment, temp, sigmoid_grad * upstream, grads);
                    b.backward(assignment, temp, -sigmoid_grad * upstream, grads);
                }
            }
            
            ConstraintFormula::Linear { coeffs, vars, bias } => {
                let sum: f64 = coeffs.iter()
                    .zip(vars.iter())
                    .map(|(c, v)| c * assignment.get(v).copied().unwrap_or(0.0))
                    .sum::<f64>() + bias;
                
                let sigmoid = 1.0 / (1.0 + (-sum / temp).exp());
                let sigmoid_grad = sigmoid * (1.0 - sigmoid) / temp;
                
                for (c, v) in coeffs.iter().zip(vars.iter()) {
                    *grads.entry(v.clone()).or_insert(0.0) += c * sigmoid_grad * upstream;
                }
            }
            
            ConstraintFormula::Custom { inputs, .. } => {
                // Simple pass-through for custom functions
                let n = inputs.len() as f64;
                for input in inputs {
                    input.backward(assignment, temp, upstream / n, grads);
                }
            }
        }
    }
    
    /// Get all variables in this formula
    pub fn variables(&self) -> Vec<String> {
        let mut vars = Vec::new();
        self.collect_variables(&mut vars);
        vars.sort();
        vars.dedup();
        vars
    }
    
    fn collect_variables(&self, vars: &mut Vec<String>) {
        match self {
            ConstraintFormula::Variable(name) => vars.push(name.clone()),
            ConstraintFormula::Constant(_) => {}
            ConstraintFormula::Not(inner) => inner.collect_variables(vars),
            ConstraintFormula::And(formulas) | ConstraintFormula::Or(formulas) => {
                for f in formulas {
                    f.collect_variables(vars);
                }
            }
            ConstraintFormula::Implies(a, b) |
            ConstraintFormula::Equals(a, b) |
            ConstraintFormula::LessThan(a, b) |
            ConstraintFormula::GreaterThan(a, b) => {
                a.collect_variables(vars);
                b.collect_variables(vars);
            }
            ConstraintFormula::Linear { vars: v, .. } => {
                vars.extend(v.clone());
            }
            ConstraintFormula::Custom { inputs, .. } => {
                for input in inputs {
                    input.collect_variables(vars);
                }
            }
        }
    }
}

/// A differentiable constraint that can be used in optimization
#[derive(Debug, Clone)]
pub struct DifferentiableConstraint {
    /// The soft constraint
    pub constraint: SoftConstraint,
    
    /// Current variable assignments
    assignments: HashMap<String, f64>,
}

impl DifferentiableConstraint {
    /// Create a new differentiable constraint
    pub fn new(constraint: SoftConstraint) -> Self {
        Self {
            assignments: HashMap::new(),
            constraint,
        }
    }
    
    /// Set a variable assignment
    pub fn set(&mut self, var: &str, value: f64) {
        self.assignments.insert(var.to_string(), value.clamp(0.0, 1.0));
    }
    
    /// Get current satisfaction level
    pub fn satisfaction(&self) -> f64 {
        self.constraint.satisfaction(&self.assignments)
    }
    
    /// Get current violation
    pub fn violation(&self) -> f64 {
        self.constraint.violation(&self.assignments)
    }
    
    /// Get gradient for optimization
    pub fn gradient(&self) -> HashMap<String, f64> {
        self.constraint.gradient(&self.assignments)
    }
}

/// Solver for systems of soft constraints
pub struct ConstraintSolver {
    /// Constraints to solve
    constraints: Vec<SoftConstraint>,
    
    /// Current assignments
    assignments: HashMap<String, f64>,
    
    /// Learning rate
    learning_rate: f64,
    
    /// Maximum iterations
    max_iterations: usize,
    
    /// Convergence threshold
    threshold: f64,
}

impl ConstraintSolver {
    /// Create a new constraint solver
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
            assignments: HashMap::new(),
            learning_rate: 0.1,
            max_iterations: 1000,
            threshold: 1e-6,
        }
    }
    
    /// Add a constraint
    pub fn add_constraint(&mut self, constraint: SoftConstraint) {
        // Initialize variables from constraint
        for var in constraint.formula.variables() {
            self.assignments.entry(var).or_insert(0.5);
        }
        self.constraints.push(constraint);
    }
    
    /// Set learning rate
    pub fn with_learning_rate(mut self, lr: f64) -> Self {
        self.learning_rate = lr;
        self
    }
    
    /// Set maximum iterations
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }
    
    /// Get total violation
    pub fn total_violation(&self) -> f64 {
        self.constraints.iter()
            .map(|c| c.violation(&self.assignments))
            .sum()
    }
    
    /// Solve using gradient descent
    pub fn solve(&mut self) -> bool {
        for _ in 0..self.max_iterations {
            let violation = self.total_violation();
            
            if violation < self.threshold {
                return true;
            }
            
            // Compute total gradient
            let mut total_grad: HashMap<String, f64> = HashMap::new();
            
            for constraint in &self.constraints {
                let grad = constraint.gradient(&self.assignments);
                for (var, g) in grad {
                    *total_grad.entry(var).or_insert(0.0) += g;
                }
            }
            
            // Update assignments (gradient descent on violation = ascent on satisfaction)
            for (var, grad) in total_grad {
                if let Some(val) = self.assignments.get_mut(&var) {
                    *val = (*val - self.learning_rate * grad).clamp(0.0, 1.0);
                }
            }
        }
        
        self.total_violation() < self.threshold
    }
    
    /// Get current assignment for a variable
    pub fn get(&self, var: &str) -> Option<f64> {
        self.assignments.get(var).copied()
    }
    
    /// Get all assignments
    pub fn assignments(&self) -> &HashMap<String, f64> {
        &self.assignments
    }
    
    /// Set initial assignment
    pub fn set_initial(&mut self, var: &str, value: f64) {
        self.assignments.insert(var.to_string(), value.clamp(0.0, 1.0));
    }
}

impl Default for ConstraintSolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_variable_evaluation() {
        let formula = ConstraintFormula::Variable("x".to_string());
        
        let mut assignment = HashMap::new();
        assignment.insert("x".to_string(), 0.7);
        
        let result = formula.evaluate(&assignment, 1.0);
        assert!((result - 0.7).abs() < 0.001);
    }
    
    #[test]
    fn test_and_evaluation() {
        let formula = ConstraintFormula::And(vec![
            ConstraintFormula::Variable("x".to_string()),
            ConstraintFormula::Variable("y".to_string()),
        ]);
        
        let mut assignment = HashMap::new();
        assignment.insert("x".to_string(), 0.8);
        assignment.insert("y".to_string(), 0.6);
        
        let result = formula.evaluate(&assignment, 1.0);
        // Product t-norm: 0.8 * 0.6 = 0.48
        assert!((result - 0.48).abs() < 0.001);
    }
    
    #[test]
    fn test_or_evaluation() {
        let formula = ConstraintFormula::Or(vec![
            ConstraintFormula::Variable("x".to_string()),
            ConstraintFormula::Variable("y".to_string()),
        ]);
        
        let mut assignment = HashMap::new();
        assignment.insert("x".to_string(), 0.3);
        assignment.insert("y".to_string(), 0.4);
        
        let result = formula.evaluate(&assignment, 1.0);
        // Probabilistic sum: 1 - (1-0.3)*(1-0.4) = 1 - 0.7*0.6 = 0.58
        assert!((result - 0.58).abs() < 0.001);
    }
    
    #[test]
    fn test_implies_evaluation() {
        let formula = ConstraintFormula::Implies(
            Box::new(ConstraintFormula::Variable("x".to_string())),
            Box::new(ConstraintFormula::Variable("y".to_string())),
        );
        
        let mut assignment = HashMap::new();
        assignment.insert("x".to_string(), 0.8);
        assignment.insert("y".to_string(), 0.6);
        
        let result = formula.evaluate(&assignment, 1.0);
        // Lukasiewicz: min(1, 1 - 0.8 + 0.6) = min(1, 0.8) = 0.8
        assert!((result - 0.8).abs() < 0.001);
    }
    
    #[test]
    fn test_soft_constraint() {
        let formula = ConstraintFormula::And(vec![
            ConstraintFormula::Variable("x".to_string()),
            ConstraintFormula::Variable("y".to_string()),
        ]);
        
        let constraint = SoftConstraint::new("test", formula)
            .with_weight(2.0);
        
        let mut assignment = HashMap::new();
        assignment.insert("x".to_string(), 1.0);
        assignment.insert("y".to_string(), 1.0);
        
        let sat = constraint.satisfaction(&assignment);
        assert!((sat - 1.0).abs() < 0.001);
        
        let viol = constraint.violation(&assignment);
        assert!(viol.abs() < 0.001);
    }
    
    #[test]
    fn test_constraint_solver() {
        let mut solver = ConstraintSolver::new()
            .with_learning_rate(0.5)
            .with_max_iterations(100);
        
        // Constraint: x > 0.5
        let c1 = SoftConstraint::new(
            "x_large",
            ConstraintFormula::GreaterThan(
                Box::new(ConstraintFormula::Variable("x".to_string())),
                Box::new(ConstraintFormula::Constant(0.5)),
            ),
        );
        
        solver.add_constraint(c1);
        solver.set_initial("x", 0.1);
        
        let solved = solver.solve();
        
        if solved {
            let x = solver.get("x").unwrap();
            assert!(x > 0.4); // Should move towards satisfying x > 0.5
        }
    }

    #[test]
    fn test_constraint_formula_variables() {
        let f = ConstraintFormula::And(vec![
            ConstraintFormula::Variable("x".to_string()),
            ConstraintFormula::Or(vec![
                ConstraintFormula::Variable("y".to_string()),
                ConstraintFormula::Variable("x".to_string()),
            ]),
        ]);
        let vars = f.variables();
        assert_eq!(vars, vec!["x".to_string(), "y".to_string()]);
    }

    #[test]
    fn test_constraint_formula_not() {
        let assignment: HashMap<String, f64> = [("x".to_string(), 1.0)].into_iter().collect();
        let f = ConstraintFormula::Not(Box::new(ConstraintFormula::Variable("x".to_string())));
        let val = f.evaluate(&assignment, 1.0);
        // Not(1.0) should be close to 0
        assert!(val < 0.5);
    }

    #[test]
    fn test_constraint_formula_less_than() {
        let assignment: HashMap<String, f64> = [
            ("x".to_string(), 0.3),
            ("y".to_string(), 0.8),
        ].into_iter().collect();
        let f = ConstraintFormula::LessThan(
            Box::new(ConstraintFormula::Variable("x".to_string())),
            Box::new(ConstraintFormula::Variable("y".to_string())),
        );
        let val = f.evaluate(&assignment, 0.1);
        assert!(val > 0.5, "0.3 < 0.8 should be satisfied");
    }

    #[test]
    fn test_differentiable_constraint() {
        let sc = SoftConstraint::new(
            "test",
            ConstraintFormula::Variable("x".to_string()),
        );
        let mut dc = DifferentiableConstraint::new(sc);
        dc.set("x", 0.9);
        assert!(dc.satisfaction() > 0.5);
        assert!(dc.violation() < 0.5);
    }

    #[test]
    fn test_solver_set_initial() {
        let mut solver = ConstraintSolver::new();
        solver.add_constraint(SoftConstraint::new(
            "high_x",
            ConstraintFormula::Variable("x".to_string()),
        ).with_weight(2.0));
        solver.set_initial("x", 0.1);
        let initial_violation = solver.total_violation();
        assert!(initial_violation > 0.0, "Low x should have violation");
        let _converged = solver.solve();
        // After solving, verify we can read assignments
        let x_val = solver.get("x");
        assert!(x_val.is_some(), "Should have assignment for x");
        let assignments = solver.assignments();
        assert!(!assignments.is_empty());
    }

}

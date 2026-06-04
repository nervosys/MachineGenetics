//! AI Planning
//!
//! Provides classical AI planning capabilities for agents to
//! reason about actions, states, and goals.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use uuid::Uuid;

use super::logic::{Predicate, Term};

/// A state in the planning domain (set of ground predicates)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// Ground predicates that are true in this state
    pub facts: HashSet<Predicate>,
}

impl State {
    /// Create a new empty state
    pub fn new() -> Self {
        Self {
            facts: HashSet::new(),
        }
    }

    /// Create a state from a set of facts
    pub fn from_facts(facts: impl IntoIterator<Item = Predicate>) -> Self {
        Self {
            facts: facts.into_iter().collect(),
        }
    }

    /// Add a fact
    pub fn add(&mut self, fact: Predicate) {
        self.facts.insert(fact);
    }

    /// Remove a fact
    pub fn remove(&mut self, fact: &Predicate) {
        self.facts.remove(fact);
    }

    /// Check if a fact is true
    pub fn holds(&self, fact: &Predicate) -> bool {
        self.facts.contains(fact)
    }

    /// Check if all facts in a set are true
    pub fn holds_all(&self, facts: &[Predicate]) -> bool {
        facts.iter().all(|f| self.holds(f))
    }

    /// Check if any fact in a set is true
    pub fn holds_any(&self, facts: &[Predicate]) -> bool {
        facts.iter().any(|f| self.holds(f))
    }

    /// Get all facts matching a predicate name
    pub fn get_matching(&self, name: &str) -> Vec<&Predicate> {
        self.facts.iter().filter(|p| p.name == name).collect()
    }

    /// Apply a substitution to all facts
    pub fn apply_substitution(&self, subst: &HashMap<String, Term>) -> State {
        State {
            facts: self
                .facts
                .iter()
                .map(|f| f.apply_substitution(subst))
                .collect(),
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

/// A goal specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    /// Predicates that must be true
    pub positive: Vec<Predicate>,

    /// Predicates that must be false
    pub negative: Vec<Predicate>,
}

impl Goal {
    /// Create a new goal with only positive conditions
    pub fn new(positive: Vec<Predicate>) -> Self {
        Self {
            positive,
            negative: Vec::new(),
        }
    }

    /// Create a goal with both positive and negative conditions
    pub fn with_negative(positive: Vec<Predicate>, negative: Vec<Predicate>) -> Self {
        Self { positive, negative }
    }

    /// Check if the goal is satisfied in a state
    pub fn satisfied(&self, state: &State) -> bool {
        self.positive.iter().all(|p| state.holds(p))
            && self.negative.iter().all(|p| !state.holds(p))
    }

    /// Count how many conditions are satisfied
    pub fn satisfied_count(&self, state: &State) -> usize {
        let pos = self.positive.iter().filter(|p| state.holds(p)).count();
        let neg = self.negative.iter().filter(|p| !state.holds(p)).count();
        pos + neg
    }

    /// Get unsatisfied positive conditions
    pub fn unsatisfied(&self, state: &State) -> Vec<&Predicate> {
        self.positive.iter().filter(|p| !state.holds(p)).collect()
    }
}

/// An action schema (parameterized action)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Unique identifier
    pub id: Uuid,

    /// Action name
    pub name: String,

    /// Parameters (variables)
    pub parameters: Vec<String>,

    /// Preconditions that must hold
    pub preconditions: Vec<Predicate>,

    /// Effects to add (positive effects)
    pub add_effects: Vec<Predicate>,

    /// Effects to delete (negative effects)
    pub delete_effects: Vec<Predicate>,

    /// Cost of the action
    pub cost: f64,
}

impl Action {
    /// Create a new action
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            parameters: Vec::new(),
            preconditions: Vec::new(),
            add_effects: Vec::new(),
            delete_effects: Vec::new(),
            cost: 1.0,
        }
    }

    /// Add a parameter
    pub fn with_parameter(mut self, param: impl Into<String>) -> Self {
        self.parameters.push(param.into());
        self
    }

    /// Add parameters
    pub fn with_parameters(mut self, params: Vec<String>) -> Self {
        self.parameters = params;
        self
    }

    /// Add a precondition
    pub fn with_precondition(mut self, pred: Predicate) -> Self {
        self.preconditions.push(pred);
        self
    }

    /// Add an add effect
    pub fn with_add_effect(mut self, pred: Predicate) -> Self {
        self.add_effects.push(pred);
        self
    }

    /// Add a delete effect
    pub fn with_delete_effect(mut self, pred: Predicate) -> Self {
        self.delete_effects.push(pred);
        self
    }

    /// Set the cost
    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost = cost;
        self
    }

    /// Check if preconditions are satisfied in a state
    pub fn applicable(&self, state: &State, bindings: &HashMap<String, Term>) -> bool {
        self.preconditions
            .iter()
            .map(|p| p.apply_substitution(bindings))
            .all(|p| state.holds(&p))
    }

    /// Apply the action to a state
    pub fn apply(&self, state: &State, bindings: &HashMap<String, Term>) -> State {
        let mut new_state = state.clone();

        // Remove delete effects
        for effect in &self.delete_effects {
            let ground = effect.apply_substitution(bindings);
            new_state.remove(&ground);
        }

        // Add add effects
        for effect in &self.add_effects {
            let ground = effect.apply_substitution(bindings);
            new_state.add(ground);
        }

        new_state
    }

    /// Get a ground instance of this action
    pub fn instantiate(&self, bindings: &HashMap<String, Term>) -> GroundAction {
        GroundAction {
            name: self.name.clone(),
            arguments: self
                .parameters
                .iter()
                .filter_map(|p| bindings.get(p).cloned())
                .collect(),
            cost: self.cost,
        }
    }
}

/// A ground (instantiated) action
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundAction {
    /// Action name
    pub name: String,

    /// Ground arguments
    pub arguments: Vec<Term>,

    /// Action cost
    #[serde(default = "default_cost")]
    pub cost: f64,
}

fn default_cost() -> f64 {
    1.0
}

impl Eq for GroundAction {}

impl std::hash::Hash for GroundAction {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.arguments.hash(state);
        self.cost.to_bits().hash(state);
    }
}

impl GroundAction {
    /// Create a new ground action with given name and arguments
    pub fn new(name: impl Into<String>, arguments: Vec<Term>) -> Self {
        Self {
            name: name.into(),
            arguments,
            cost: 1.0,
        }
    }
}

impl std::fmt::Display for GroundAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.name)?;
        for (i, arg) in self.arguments.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", arg)?;
        }
        write!(f, ")")
    }
}

/// A plan (sequence of ground actions)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// Unique identifier
    pub id: Uuid,

    /// Actions in the plan
    pub actions: Vec<GroundAction>,

    /// Total cost
    pub total_cost: f64,

    /// Whether the plan is valid
    pub valid: bool,
}

impl Plan {
    /// Create a new empty plan
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            actions: Vec::new(),
            total_cost: 0.0,
            valid: false,
        }
    }

    /// Create a plan from actions
    pub fn from_actions(actions: Vec<GroundAction>) -> Self {
        let total_cost = actions.iter().map(|a| a.cost).sum();
        Self {
            id: Uuid::new_v4(),
            actions,
            total_cost,
            valid: true,
        }
    }

    /// Add an action to the plan
    pub fn add(&mut self, action: GroundAction) {
        self.total_cost += action.cost;
        self.actions.push(action);
    }

    /// Length of the plan
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

impl Default for Plan {
    fn default() -> Self {
        Self::new()
    }
}

/// Planning domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    /// Domain name
    pub name: String,

    /// Available actions
    pub actions: Vec<Action>,

    /// Type hierarchy (for typed planning)
    pub types: HashMap<String, Vec<String>>, // type -> objects

    /// Predicates in the domain
    pub predicates: Vec<String>,
}

impl Domain {
    /// Create a new domain
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            actions: Vec::new(),
            types: HashMap::new(),
            predicates: Vec::new(),
        }
    }

    /// Add an action
    pub fn add_action(&mut self, action: Action) {
        self.actions.push(action);
    }

    /// Add a type with objects
    pub fn add_type(&mut self, type_name: impl Into<String>, objects: Vec<String>) {
        self.types.insert(type_name.into(), objects);
    }

    /// Get all objects of a type
    pub fn objects_of_type(&self, type_name: &str) -> &[String] {
        self.types
            .get(type_name)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

/// Planning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerConfig {
    /// Maximum search nodes
    pub max_nodes: usize,

    /// Timeout in milliseconds
    pub timeout_ms: u64,

    /// Whether to use heuristic search
    pub use_heuristic: bool,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            max_nodes: 100000,
            timeout_ms: 60000,
            use_heuristic: true,
        }
    }
}

/// A search node for planning
#[derive(Debug, Clone)]
struct SearchNode {
    state: State,
    plan: Vec<GroundAction>,
    cost: f64,
    heuristic: f64,
}

impl PartialEq for SearchNode {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
    }
}

impl Eq for SearchNode {}

impl PartialOrd for SearchNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap
        let self_f = self.cost + self.heuristic;
        let other_f = other.cost + other.heuristic;
        other_f.partial_cmp(&self_f).unwrap_or(Ordering::Equal)
    }
}

/// The planner
pub struct Planner {
    config: PlannerConfig,
}

impl Planner {
    /// Create a new planner
    pub fn new(config: PlannerConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn default_planner() -> Self {
        Self::new(PlannerConfig::default())
    }

    /// Plan using A* search
    pub fn plan(&self, domain: &Domain, initial: &State, goal: &Goal) -> Option<Plan> {
        if goal.satisfied(initial) {
            return Some(Plan::from_actions(vec![]));
        }

        let mut open = BinaryHeap::new();
        let mut closed: HashSet<Vec<u8>> = HashSet::new();

        let initial_h = self.heuristic(initial, goal);
        open.push(SearchNode {
            state: initial.clone(),
            plan: vec![],
            cost: 0.0,
            heuristic: initial_h,
        });

        let mut nodes_expanded = 0;

        while let Some(node) = open.pop() {
            nodes_expanded += 1;

            if nodes_expanded > self.config.max_nodes {
                break;
            }

            // Check goal
            if goal.satisfied(&node.state) {
                return Some(Plan::from_actions(node.plan));
            }

            // State hash for duplicate detection
            let state_hash = self.state_hash(&node.state);
            if closed.contains(&state_hash) {
                continue;
            }
            closed.insert(state_hash);

            // Expand
            for action in &domain.actions {
                for bindings in self.get_applicable_bindings(action, &node.state, domain) {
                    let new_state = action.apply(&node.state, &bindings);
                    let ground_action = action.instantiate(&bindings);

                    let mut new_plan = node.plan.clone();
                    new_plan.push(ground_action);

                    let new_cost = node.cost + action.cost;
                    let new_h = if self.config.use_heuristic {
                        self.heuristic(&new_state, goal)
                    } else {
                        0.0
                    };

                    open.push(SearchNode {
                        state: new_state,
                        plan: new_plan,
                        cost: new_cost,
                        heuristic: new_h,
                    });
                }
            }
        }

        None
    }

    /// Simple goal-count heuristic
    fn heuristic(&self, state: &State, goal: &Goal) -> f64 {
        let unsatisfied = goal.positive.len() - goal.satisfied_count(state);
        unsatisfied as f64
    }

    /// Hash a state for duplicate detection
    fn state_hash(&self, state: &State) -> Vec<u8> {
        let mut facts: Vec<_> = state.facts.iter().map(|f| format!("{}", f)).collect();
        facts.sort();

        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        facts.hash(&mut hasher);
        hasher.finish().to_le_bytes().to_vec()
    }

    /// Get all applicable bindings for an action
    fn get_applicable_bindings(
        &self,
        action: &Action,
        state: &State,
        _domain: &Domain,
    ) -> Vec<HashMap<String, Term>> {
        if action.parameters.is_empty() {
            // No parameters - check if preconditions hold
            if action.applicable(state, &HashMap::new()) {
                return vec![HashMap::new()];
            }
            return vec![];
        }

        // Get all possible bindings
        let mut bindings_list = vec![HashMap::new()];

        for param in &action.parameters {
            let mut new_bindings = Vec::new();

            // Get possible objects for this parameter
            // Simplified: try all constants from state
            let objects: HashSet<Term> = state
                .facts
                .iter()
                .flat_map(|f| f.args.iter().cloned())
                .filter(|t| matches!(t, Term::Constant(_)))
                .collect();

            for bindings in bindings_list {
                for obj in &objects {
                    let mut new_b = bindings.clone();
                    new_b.insert(param.clone(), obj.clone());
                    new_bindings.push(new_b);
                }
            }

            bindings_list = new_bindings;
        }

        // Filter to applicable bindings
        bindings_list
            .into_iter()
            .filter(|b| action.applicable(state, b))
            .collect()
    }

    /// Plan using breadth-first search (guaranteed shortest plan)
    pub fn plan_bfs(&self, domain: &Domain, initial: &State, goal: &Goal) -> Option<Plan> {
        if goal.satisfied(initial) {
            return Some(Plan::from_actions(vec![]));
        }

        let mut queue = VecDeque::new();
        let mut visited: HashSet<Vec<u8>> = HashSet::new();

        queue.push_back((initial.clone(), vec![]));
        visited.insert(self.state_hash(initial));

        let mut nodes = 0;

        while let Some((state, plan)) = queue.pop_front() {
            nodes += 1;
            if nodes > self.config.max_nodes {
                break;
            }

            for action in &domain.actions {
                for bindings in self.get_applicable_bindings(action, &state, domain) {
                    let new_state = action.apply(&state, &bindings);
                    let state_hash = self.state_hash(&new_state);

                    if visited.contains(&state_hash) {
                        continue;
                    }
                    visited.insert(state_hash);

                    let mut new_plan = plan.clone();
                    new_plan.push(action.instantiate(&bindings));

                    if goal.satisfied(&new_state) {
                        return Some(Plan::from_actions(new_plan));
                    }

                    queue.push_back((new_state, new_plan));
                }
            }
        }

        None
    }
}

impl Default for Planner {
    fn default() -> Self {
        Self::default_planner()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks_world_domain() -> Domain {
        let mut domain = Domain::new("blocks-world");

        // Pick up action
        let pickup = Action::new("pickup")
            .with_parameter("block")
            .with_precondition(Predicate::new("clear", vec![Term::var("block")]))
            .with_precondition(Predicate::new("ontable", vec![Term::var("block")]))
            .with_precondition(Predicate::new("handempty", vec![]))
            .with_add_effect(Predicate::new("holding", vec![Term::var("block")]))
            .with_delete_effect(Predicate::new("ontable", vec![Term::var("block")]))
            .with_delete_effect(Predicate::new("clear", vec![Term::var("block")]))
            .with_delete_effect(Predicate::new("handempty", vec![]));

        // Put down action
        let putdown = Action::new("putdown")
            .with_parameter("block")
            .with_precondition(Predicate::new("holding", vec![Term::var("block")]))
            .with_add_effect(Predicate::new("ontable", vec![Term::var("block")]))
            .with_add_effect(Predicate::new("clear", vec![Term::var("block")]))
            .with_add_effect(Predicate::new("handempty", vec![]))
            .with_delete_effect(Predicate::new("holding", vec![Term::var("block")]));

        // Stack action
        let stack = Action::new("stack")
            .with_parameter("block")
            .with_parameter("onto")
            .with_precondition(Predicate::new("holding", vec![Term::var("block")]))
            .with_precondition(Predicate::new("clear", vec![Term::var("onto")]))
            .with_add_effect(Predicate::new(
                "on",
                vec![Term::var("block"), Term::var("onto")],
            ))
            .with_add_effect(Predicate::new("clear", vec![Term::var("block")]))
            .with_add_effect(Predicate::new("handempty", vec![]))
            .with_delete_effect(Predicate::new("holding", vec![Term::var("block")]))
            .with_delete_effect(Predicate::new("clear", vec![Term::var("onto")]));

        // Unstack action
        let unstack = Action::new("unstack")
            .with_parameter("block")
            .with_parameter("from")
            .with_precondition(Predicate::new(
                "on",
                vec![Term::var("block"), Term::var("from")],
            ))
            .with_precondition(Predicate::new("clear", vec![Term::var("block")]))
            .with_precondition(Predicate::new("handempty", vec![]))
            .with_add_effect(Predicate::new("holding", vec![Term::var("block")]))
            .with_add_effect(Predicate::new("clear", vec![Term::var("from")]))
            .with_delete_effect(Predicate::new(
                "on",
                vec![Term::var("block"), Term::var("from")],
            ))
            .with_delete_effect(Predicate::new("clear", vec![Term::var("block")]))
            .with_delete_effect(Predicate::new("handempty", vec![]));

        domain.add_action(pickup);
        domain.add_action(putdown);
        domain.add_action(stack);
        domain.add_action(unstack);

        domain
    }

    #[test]
    fn test_state() {
        let mut state = State::new();
        state.add(Predicate::new(
            "on",
            vec![Term::constant("a"), Term::constant("b")],
        ));
        state.add(Predicate::new("clear", vec![Term::constant("a")]));

        assert!(state.holds(&Predicate::new(
            "on",
            vec![Term::constant("a"), Term::constant("b")]
        )));
        assert!(state.holds(&Predicate::new("clear", vec![Term::constant("a")])));
        assert!(!state.holds(&Predicate::new("clear", vec![Term::constant("b")])));
    }

    #[test]
    fn test_action_apply() {
        let action = Action::new("test")
            .with_add_effect(Predicate::new("result", vec![Term::var("X")]))
            .with_delete_effect(Predicate::new("input", vec![Term::var("X")]));

        let mut initial = State::new();
        initial.add(Predicate::new("input", vec![Term::constant("a")]));

        let mut bindings = HashMap::new();
        bindings.insert("X".to_string(), Term::constant("a"));

        let result = action.apply(&initial, &bindings);

        assert!(!result.holds(&Predicate::new("input", vec![Term::constant("a")])));
        assert!(result.holds(&Predicate::new("result", vec![Term::constant("a")])));
    }

    #[test]
    fn test_simple_planning() {
        let domain = blocks_world_domain();

        // Initial: a is on table, hand empty, a is clear
        let mut initial = State::new();
        initial.add(Predicate::new("ontable", vec![Term::constant("a")]));
        initial.add(Predicate::new("clear", vec![Term::constant("a")]));
        initial.add(Predicate::new("handempty", vec![]));

        // Goal: holding a
        let goal = Goal::new(vec![Predicate::new("holding", vec![Term::constant("a")])]);

        let planner = Planner::default_planner();
        let plan = planner.plan(&domain, &initial, &goal);

        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert!(!plan.is_empty());
        assert_eq!(plan.actions[0].name, "pickup");
    }

    #[test]
    fn test_goal_satisfied() {
        let goal = Goal::new(vec![Predicate::new(
            "at",
            vec![Term::constant("a"), Term::constant("loc1")],
        )]);

        let mut state = State::new();
        assert!(!goal.satisfied(&state));

        state.add(Predicate::new(
            "at",
            vec![Term::constant("a"), Term::constant("loc1")],
        ));
        assert!(goal.satisfied(&state));
    }

    #[test]
    fn test_state_from_facts() {
        let facts = vec![
            Predicate::new(
                "at",
                vec![
                    Term::Constant("robot".to_string()),
                    Term::Constant("A".to_string()),
                ],
            ),
            Predicate::new("clear", vec![Term::Constant("B".to_string())]),
        ];
        let state = State::from_facts(facts);
        assert!(state.holds(&Predicate::new(
            "clear",
            vec![Term::Constant("B".to_string())]
        )));
        assert_eq!(state.get_matching("at").len(), 1);
    }

    #[test]
    fn test_state_holds_all_any() {
        let mut state = State::new();
        let p1 = Predicate::new("on", vec![Term::Constant("A".to_string())]);
        let p2 = Predicate::new("on", vec![Term::Constant("B".to_string())]);
        let p3 = Predicate::new("on", vec![Term::Constant("C".to_string())]);
        state.add(p1.clone());
        state.add(p2.clone());

        assert!(state.holds_all(&[p1.clone(), p2.clone()]));
        assert!(!state.holds_all(&[p1.clone(), p3.clone()]));
        assert!(state.holds_any(&[p1.clone(), p3.clone()]));
        assert!(!state.holds_any(&[p3]));
    }

    #[test]
    fn test_goal_with_negative() {
        let pos = vec![Predicate::new("at", vec![Term::Constant("B".to_string())])];
        let neg = vec![Predicate::new("at", vec![Term::Constant("A".to_string())])];
        let goal = Goal::with_negative(pos, neg);

        let mut state = State::new();
        state.add(Predicate::new("at", vec![Term::Constant("B".to_string())]));
        assert!(goal.satisfied(&state));

        state.add(Predicate::new("at", vec![Term::Constant("A".to_string())]));
        assert!(!goal.satisfied(&state), "Negative condition should fail");
    }

    #[test]
    fn test_goal_unsatisfied() {
        let pos = vec![
            Predicate::new("a", vec![]),
            Predicate::new("b", vec![]),
            Predicate::new("c", vec![]),
        ];
        let goal = Goal::new(pos);
        let mut state = State::new();
        state.add(Predicate::new("a", vec![]));
        let unsat = goal.unsatisfied(&state);
        assert_eq!(unsat.len(), 2);
    }

    #[test]
    fn test_plan_from_actions() {
        let a1 = GroundAction::new(
            "move".to_string(),
            vec![
                Term::Constant("A".to_string()),
                Term::Constant("B".to_string()),
            ],
        );
        let a2 = GroundAction::new("pick".to_string(), vec![Term::Constant("box".to_string())]);
        let plan = Plan::from_actions(vec![a1, a2]);
        assert_eq!(plan.len(), 2);
        assert!(!plan.is_empty());
        assert!((plan.total_cost - 2.0).abs() < 1e-5);
    }

    #[test]
    fn test_plan_already_satisfied() {
        let domain = blocks_world_domain();
        let mut initial = State::new();
        initial.add(Predicate::new("holding", vec![Term::constant("a")]));

        let goal = Goal::new(vec![Predicate::new("holding", vec![Term::constant("a")])]);

        let planner = Planner::default_planner();
        let plan = planner.plan(&domain, &initial, &goal);
        assert!(plan.is_some());
        assert!(plan.unwrap().is_empty(), "Plan should be empty when goal already met");
    }

    #[test]
    fn test_bfs_finds_plan() {
        let domain = blocks_world_domain();
        let mut initial = State::new();
        initial.add(Predicate::new("ontable", vec![Term::constant("a")]));
        initial.add(Predicate::new("clear", vec![Term::constant("a")]));
        initial.add(Predicate::new("handempty", vec![]));

        let goal = Goal::new(vec![Predicate::new("holding", vec![Term::constant("a")])]);

        let planner = Planner::default_planner();
        let plan = planner.plan_bfs(&domain, &initial, &goal);
        assert!(plan.is_some());
        assert_eq!(plan.unwrap().actions[0].name, "pickup");
    }

    #[test]
    fn test_bfs_already_satisfied() {
        let domain = blocks_world_domain();
        let mut initial = State::new();
        initial.add(Predicate::new("holding", vec![Term::constant("a")]));

        let goal = Goal::new(vec![Predicate::new("holding", vec![Term::constant("a")])]);

        let planner = Planner::default_planner();
        let plan = planner.plan_bfs(&domain, &initial, &goal);
        assert!(plan.is_some());
        assert!(plan.unwrap().is_empty());
    }

    #[test]
    fn test_domain_types() {
        let mut domain = Domain::new("logistics");
        domain.add_type("city", vec!["A".into(), "B".into(), "C".into()]);
        domain.add_type("vehicle", vec!["truck1".into()]);

        assert_eq!(domain.objects_of_type("city").len(), 3);
        assert_eq!(domain.objects_of_type("vehicle").len(), 1);
        assert_eq!(domain.objects_of_type("nonexistent").len(), 0);
    }

    #[test]
    fn test_state_default() {
        let state = State::default();
        assert_eq!(state.facts.len(), 0);
    }

    #[test]
    fn test_plan_default() {
        let plan = Plan::default();
        assert!(plan.is_empty());
        assert_eq!(plan.len(), 0);
        assert!((plan.total_cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_plan_add() {
        let mut plan = Plan::new();
        plan.add(GroundAction::new("act1", vec![]));
        plan.add(GroundAction::new("act2", vec![]));
        assert_eq!(plan.len(), 2);
        assert!((plan.total_cost - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ground_action_display() {
        let ga = GroundAction::new(
            "move",
            vec![Term::constant("robot"), Term::constant("locB")],
        );
        let s = format!("{}", ga);
        assert!(s.contains("move"));
        assert!(s.contains("robot"));
        assert!(s.contains("locB"));
    }

    #[test]
    fn test_action_cost() {
        let action = Action::new("expensive_move").with_cost(5.0);
        assert!((action.cost - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_goal_satisfied_count() {
        let goal = Goal::new(vec![
            Predicate::new("a", vec![]),
            Predicate::new("b", vec![]),
            Predicate::new("c", vec![]),
        ]);
        let mut state = State::new();
        assert_eq!(goal.satisfied_count(&state), 0);

        state.add(Predicate::new("a", vec![]));
        state.add(Predicate::new("c", vec![]));
        assert_eq!(goal.satisfied_count(&state), 2);
    }

    #[test]
    fn test_planner_config_defaults() {
        let config = PlannerConfig::default();
        assert_eq!(config.max_nodes, 100000);
        assert!(config.use_heuristic);
    }

    #[test]
    fn test_state_remove() {
        let mut state = State::new();
        let fact = Predicate::new("on", vec![Term::constant("a")]);
        state.add(fact.clone());
        assert!(state.holds(&fact));

        state.remove(&fact);
        assert!(!state.holds(&fact));
    }
}

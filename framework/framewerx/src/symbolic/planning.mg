// framewerx::symbolic::planning — classical and probabilistic planning.

// PDDL-style action: (preconditions, effects).
S PDDLAction {
    name: s,
    parameters: [s]~,
    preconditions: [s]~,
    effects: [s]~,
}

// STRIPS planner: forward search over add/delete lists.
S STRIPSPlanner { heuristic: s, search: s }
I STRIPSPlanner {
    +f astar() -> STRIPSPlanner {
        @STRIPSPlanner { heuristic: "h_max", search: "A*" }
    }
}

// Hierarchical Task Network planner.
S HTNPlanner { method_library: s, max_depth: usize }

// Partial-Order Planner.
S POPPlanner { ordering: s, threat_resolution: s }

// Monte Carlo Tree Search (used in AlphaGo / MuZero).
S MCTS {
    exploration_c: f32,
    rollouts: usize,
    tree_policy: s,
    default_policy: s,
}

I MCTS {
    +f uct(c: f32, rollouts: usize) -> MCTS {
        @MCTS {
            exploration_c: c,
            rollouts: rollouts,
            tree_policy: "UCT",
            default_policy: "uniform",
        }
    }
}

// Markov Decision Process spec.
S MDP { states: usize, actions: usize, gamma: f32 }

// Partially-Observable MDP.
S POMDP { states: usize, actions: usize, observations: usize, gamma: f32 }

// Value iteration / policy iteration solvers.
S ValueIteration { theta: f32, max_iters: usize }
S PolicyIteration { evaluation_iters: usize }

// framewerx::symbolic::solvers — constraint, SAT, SMT, theorem provers.

// SAT solver interface (CDCL).
S SATSolver {
    decision_heuristic: s,
    restart_strategy: s,
    learning: bool,
}

I SATSolver {
    +f cdcl() -> SATSolver {
        @SATSolver { decision_heuristic: "VSIDS", restart_strategy: "luby", learning: 1b }
    }
}

// SMT solver: SAT + theory combination (LIA, LRA, BV, arrays, UF).
S SMTSolver {
    theories: [s]~,
    incremental: bool,
}

I SMTSolver {
    +f z3_like() -> SMTSolver {
        @SMTSolver {
            theories: ["LIA", "LRA", "BV", "arrays", "UF"],
            incremental: 1b,
        }
    }
}

// Constraint Satisfaction Problem solver (AC-3 / backtracking).
S CSPSolver { propagation: s, search: s }

// Mixed Integer Programming.
S MILPSolver { method: s, presolve: bool }

// Theorem prover (resolution / paramodulation).
S TheoremProver {
    calculus: s,
    redundancy: s,
    max_clauses: usize,
}

I TheoremProver {
    +f resolution() -> TheoremProver {
        @TheoremProver { calculus: "resolution", redundancy: "subsumption", max_clauses: 100000 }
    }
    +f superposition() -> TheoremProver {
        @TheoremProver { calculus: "superposition", redundancy: "AVATAR", max_clauses: 100000 }
    }
}

// Equational rewriting / term-rewriting system.
S TermRewriteSystem { strategy: s, confluent: bool, terminating: bool }

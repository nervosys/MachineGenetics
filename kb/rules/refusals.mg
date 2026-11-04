// Why the arena's gates refuse a program. Each refusal class names one rule.
//
// `arena/src/substrate.rs` has ten classes and `arena/src/physics_task.rs` six,
// of which five are the same classes under the same names. `refused_by` holds
// both: a test in each file holds its enum to these facts.
//
// A class is coarser than a diagnostic. `type` is whatever the type checker
// said first, so it is not E0308, and `effect` is also a candidate that is not
// pure, so it is not E0550. Those have a gate rule of their own, and the
// refusal's message carries the detail. `depth` and `parse` are exact, so they
// name the rule that decides them. `budget` is decided in `arena.rs`, not
// the substrate: the variant is built only for its evidence entry.

kb Refusals {
    fact instance_of("substrate", SymbolicComponent);
    fact instance_of("arena", SymbolicComponent);
    fact instance_of("physics_task", SymbolicComponent);

    // The arena's generators, under the kind each is named by in the evidence
    // log (arena/src/evidence.rs). Only the grammar agent's policy is learned;
    // `command` runs whatever program it is given.
    fact instance_of("grammar", NeuralComponent);
    fact instance_of("uniform", Component);
    fact instance_of("mutator", Component);
    fact instance_of("attacker", Component);
    fact instance_of("command", Component);

    fact instance_of("gate_signature", GateRule);
    fact gate_row("gate_signature", "A candidate is exactly one function of the required shape and role, in MAGE-core");
    fact enforced_by("gate_signature", "substrate");
    fact enforced_by("gate_signature", "physics_task");

    fact instance_of("gate_type", GateRule);
    fact gate_row("gate_type", "A candidate passes the static checks");
    fact enforced_by("gate_type", "substrate");
    fact enforced_by("gate_type", "physics_task");

    fact instance_of("gate_effect", GateRule);
    fact gate_row("gate_effect", "A candidate is pure");
    fact enforced_by("gate_effect", "substrate");

    fact instance_of("gate_fuel", GateRule);
    fact gate_row("gate_fuel", "A run ends within its fuel");
    fact enforced_by("gate_fuel", "substrate");
    fact enforced_by("gate_fuel", "physics_task");

    fact instance_of("gate_runtime", GateRule);
    fact gate_row("gate_runtime", "A run returns a value of the required kind, without an error");
    fact enforced_by("gate_runtime", "substrate");
    fact enforced_by("gate_runtime", "physics_task");

    fact instance_of("gate_empty", GateRule);
    fact gate_row("gate_empty", "A run produces some output");
    fact enforced_by("gate_empty", "substrate");

    fact instance_of("gate_budget", GateRule);
    fact gate_row("gate_budget", "A run is not started once the energy budget or the allowance is spent");
    fact enforced_by("gate_budget", "arena");

    fact instance_of("gate_canary", GateRule);
    fact gate_row("gate_canary", "Output carrying a held-out canary halts the run");
    fact enforced_by("gate_canary", "arena");

    fact instance_of("gate_contract", GateRule);
    fact gate_row("gate_contract", "The task's contracts hold at run time");
    fact enforced_by("gate_contract", "physics_task");

    fact instance_of("depth", Refusal);
    fact refused_by("depth", "max_depth");
    fact instance_of("parse", Refusal);
    fact refused_by("parse", "E0001");
    fact instance_of("signature", Refusal);
    fact refused_by("signature", "gate_signature");
    fact instance_of("type", Refusal);
    fact refused_by("type", "gate_type");
    fact instance_of("effect", Refusal);
    fact refused_by("effect", "gate_effect");
    fact instance_of("fuel", Refusal);
    fact refused_by("fuel", "gate_fuel");
    fact instance_of("runtime", Refusal);
    fact refused_by("runtime", "gate_runtime");
    fact instance_of("empty", Refusal);
    fact refused_by("empty", "gate_empty");
    fact instance_of("budget", Refusal);
    fact refused_by("budget", "gate_budget");
    fact instance_of("canary", Refusal);
    fact refused_by("canary", "gate_canary");
    fact instance_of("contract", Refusal);
    fact refused_by("contract", "gate_contract");
}

// Why the knowledge-base gate refuses a proposal (NEUROSYMBOLIC_PLAN.md, Phase 8).
// `prototype/src/kb_proposal.rs` decides the first four; `scripts/kb-gate.sh`
// runs the conformance suite and the benchmark against the changed files.

kb Admission {
    fact instance_of("kb_gate", SymbolicComponent);
    // The extension registry in prototype/src/grammar.rs, which proposes promotions.
    fact instance_of("grammar_extensions", SymbolicComponent);
    // Widens a profile limit from a run's evidence (arena/src/limit_proposer.rs).
    fact instance_of("limit_proposer", SymbolicComponent);

    fact instance_of("kb_base", GateRule);
    fact gate_row("kb_base", "A proposal is written against the knowledge base it changes");
    fact enforced_by("kb_base", "kb_gate");

    fact instance_of("kb_author", GateRule);
    fact gate_row("kb_author", "A proposal's author is a stated component or a named person");
    fact enforced_by("kb_author", "kb_gate");

    fact instance_of("kb_edit", GateRule);
    fact gate_row("kb_edit", "A proposal says what it changes, and each edit applies exactly once to a knowledge-base file");
    fact enforced_by("kb_edit", "kb_gate");

    fact instance_of("kb_schema", GateRule);
    fact gate_row("kb_schema", "The changed knowledge base fits its schema and keeps every invariant");
    fact enforced_by("kb_schema", "kb_gate");

    fact instance_of("kb_conformance", GateRule);
    fact gate_row("kb_conformance", "The conformance suite passes against the changed knowledge base");
    fact enforced_by("kb_conformance", "kb_gate");

    fact instance_of("kb_cost", GateRule);
    fact gate_row("kb_cost", "A fixed benchmark costs no more under the changed knowledge base");
    fact enforced_by("kb_cost", "kb_gate");
}

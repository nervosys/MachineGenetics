// framewerx::symbolic::logic — first-order and propositional logic.
//
// Lowers to RMI Symbolic family (high byte 0x01): UNIFY (0x0100),
// RESOLVE (0x0101), INFER (0x0102), PLAN (0x0103).

// A logical term: variable, constant, or function application.
E Term {
    Var(s),
    Const(s),
    Func(s),
}

// A Horn clause: head :- body1, body2, ...
S HornClause {
    head: Term,
    body: [Term]~,
}

// Unification engine: returns most-general unifier or fails.
S Unifier {}
I Unifier {
    +f new() -> Unifier { @Unifier {} }
}

// SLD resolution (Prolog-style backward chaining).
S SLDResolver { max_depth: usize, max_solutions: usize }
I SLDResolver {
    +f new() -> SLDResolver {
        @SLDResolver { max_depth: 100, max_solutions: 10 }
    }
}

// Forward chainer (production rule system: data -> conclusions).
S ForwardChainer { max_iterations: usize }

// Backward chainer (goal -> proof tree).
S BackwardChainer { max_depth: usize }

// Negation as failure / closed-world assumption flag.
S CWA { enabled: bool }

// Description Logic ontology (OWL-style):
// concept hierarchy + role hierarchy + assertions (TBox/ABox/RBox).
S DLOntology {
    namespace: s,
    concepts: [s]~,
    roles: [s]~,
}

// Tableau reasoner for description logic.
S TableauReasoner { logic_fragment: s, blocking: s }

I TableauReasoner {
    +f alc() -> TableauReasoner { @TableauReasoner { logic_fragment: "ALC", blocking: "equality" } }
    +f shoiq() -> TableauReasoner { @TableauReasoner { logic_fragment: "SHOIQ", blocking: "double" } }
}

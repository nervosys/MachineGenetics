// Constraints on programs that kb/grammar.mg's productions do not express.
//
// `constraint_row` is name, value, meaning: the value is a number where the
// constraint has one, and "" otherwise. Each constraint has a witness in
// prototype/tests/rules.rs, or for the arena's in arena/src/grammar.rs. A
// `gap` is input the compiler accepts when it arguably should not, or a form
// the grammar does not reach. Run-time limits such as eval's call depth are
// not here: they do not decide what a program is.

kb GrammarRules {
    fact instance_of("arena_grammar", SymbolicComponent);

    fact instance_of("max_depth", GrammarConstraint);
    fact constraint_row("max_depth", "5", "An arena list node nests at most this deep; integer nodes two more");
    fact enforced_by("max_depth", "arena_grammar");

    fact instance_of("max_nest", GrammarConstraint);
    fact constraint_row("max_nest", "64", "The arena parser's nesting, and the method calls in one chain");
    fact enforced_by("max_nest", "arena_grammar");

    fact instance_of("reserved_item_keyword", GrammarConstraint);
    fact constraint_row("reserved_item_keyword", "", "`extern`, `U` and `unsafe` are reserved, and refused as items");
    fact enforced_by("reserved_item_keyword", "parser");

    fact instance_of("reserved_pattern", GrammarConstraint);
    fact constraint_row("reserved_pattern", "", "The swarm patterns and `grammar_extension` are refused as expressions");
    fact enforced_by("reserved_pattern", "parser");

    fact instance_of("keyword_binding", GrammarConstraint);
    fact constraint_row("keyword_binding", "", "A keyword is refused as a binding name");
    fact enforced_by("keyword_binding", "parser");

    fact instance_of("macro_call", GrammarConstraint);
    fact constraint_row("macro_call", "", "`name!(...)` is refused: MAGE has no macros");
    fact enforced_by("macro_call", "parser");

    fact instance_of("unit_letters_only", GrammarConstraint);
    fact constraint_row("unit_letters_only", "", "A unit name is letters only");
    fact enforced_by("unit_letters_only", "parser");

    fact instance_of("agent_fields", GrammarConstraint);
    fact constraint_row("agent_fields", "", "An agent has only `capabilities` and `requires_approval`");
    fact enforced_by("agent_fields", "parser");

    fact instance_of("swarm_labels", GrammarConstraint);
    fact constraint_row("swarm_labels", "", "`dispatch`, `aggregate` and `on_failure` are refused in a swarm");
    fact enforced_by("swarm_labels", "parser");

    fact instance_of("evolve_fields", GrammarConstraint);
    fact constraint_row("evolve_fields", "", "An evolve block has only its seven fields");
    fact enforced_by("evolve_fields", "parser");

    fact instance_of("kb_entries", GrammarConstraint);
    fact constraint_row("kb_entries", "", "A kb block holds only `fact` and `rule`");
    fact enforced_by("kb_entries", "parser");

    fact instance_of("clause_on_other_item", GrammarConstraint);
    fact constraint_row("clause_on_other_item", "", "A contract clause above an item that is not a function or struct");
    fact gap("clause_on_other_item", "accepted and silently dropped: the parser keeps clauses only for functions and structs");

    fact instance_of("layout_blocks", GrammarConstraint);
    fact constraint_row("layout_blocks", "", "The offside rule: a block by indentation");
    fact gap("layout_blocks", "the parser accepts layout and kb/grammar.mg omits it, so recognise.rs refuses a valid layout program");

    fact instance_of("overflowing_dimension", GrammarConstraint);
    fact constraint_row("overflowing_dimension", "", "A tensor dimension too large for its integer");
    fact gap("overflowing_dimension", "accepted and silently read as 0");
}

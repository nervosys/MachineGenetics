// The top-level schema of MAGE's knowledge base, v1 (NEUROSYMBOLIC_PLAN.md §5).
//
// Every other file under kb/ states facts in this vocabulary, and
// prototype/src/kb.rs refuses to load a knowledge base that breaks it:
//
// - a fact's predicate must be one of the schema's own (`class`,
//   `subclass_of`, `relation`, `arg_class`, `invariant`, `instance_of`) or a
//   declared `relation`;
// - each argument of a relation fact must be an instance of the class its
//   `arg_class` names (`Text` takes any constant);
// - every `invariant` names a rule here whose results are violations, and
//   each must derive nothing.
//
// Changing this file changes the schema: bump `schema_version` with it.

kb Schema {
    // v2 (2026-10-29): `TokenKind` and `lexes_as`, for kb/lexicon.mg.
    // v3 (2026-10-29): `sigil_two_keywords` dropped: `:` abbreviates both
    // `in` and `else`, and the grammar, not the spelling, says which.
    fact schema_version(3);

    // ── Classes ─────────────────────────────────────────────────────────
    // Seven roots. The twelve domains of MAGE_ONTOLOGY.md group facts into
    // files; they are not classes.
    fact class(Form);
    fact class(Construct);
    fact class(Meaning);
    fact class(Rule);
    fact class(Component);
    fact class(Artifact);
    fact class(Evidence);
    // Not in the §5 draft's table, but named by `in_profile`'s range.
    fact class(Profile);
    // Free text: any constant is a `Text`.
    fact class(Text);

    fact class(HumanSource);
    fact class(SigilSource);
    fact class(Declarative);
    fact class(Ast);
    fact class(Hir);
    fact class(IrOp);
    fact class(BinaryContainer);
    fact subclass_of(HumanSource, Form);
    fact subclass_of(SigilSource, Form);
    fact subclass_of(Declarative, Form);
    fact subclass_of(Ast, Form);
    fact subclass_of(Hir, Form);
    fact subclass_of(IrOp, Form);
    fact subclass_of(BinaryContainer, Form);

    fact class(Token);
    fact class(Keyword);
    fact class(Sigil);
    fact class(Production);
    fact class(AstKind);
    fact class(TokenKind);
    fact subclass_of(Token, Construct);
    fact subclass_of(Keyword, Token);
    fact subclass_of(Sigil, Token);
    fact subclass_of(Production, Construct);
    fact subclass_of(AstKind, Construct);
    // What the lexer produces; a `Token` is a spelling that lexes as one.
    fact subclass_of(TokenKind, Construct);

    fact class(Type);
    fact class(Effect);
    fact class(Op);
    fact class(Layer);
    fact subclass_of(Type, Meaning);
    fact subclass_of(Effect, Meaning);
    fact subclass_of(Op, Meaning);
    fact subclass_of(Layer, Meaning);

    fact class(TypeRule);
    fact class(SafetyRule);
    fact class(Contract);
    fact class(GrammarConstraint);
    fact class(Diagnostic);
    fact subclass_of(TypeRule, Rule);
    fact subclass_of(SafetyRule, Rule);
    fact subclass_of(Contract, Rule);
    fact subclass_of(GrammarConstraint, Rule);
    fact subclass_of(Diagnostic, Rule);

    fact class(NeuralComponent);
    fact class(SymbolicComponent);
    fact subclass_of(NeuralComponent, Component);
    fact subclass_of(SymbolicComponent, Component);

    fact class(Program);
    fact class(Block);
    fact class(Container);
    fact subclass_of(Program, Artifact);
    fact subclass_of(Block, Artifact);
    fact subclass_of(Container, Artifact);

    fact class(Refusal);
    fact class(Measurement);
    fact subclass_of(Refusal, Evidence);
    fact subclass_of(Measurement, Evidence);

    // ── Relations: name, then the class of each argument by position ────
    fact relation(lexes_as);
    fact arg_class(lexes_as, 1, Token);
    fact arg_class(lexes_as, 2, TokenKind);
    fact relation(abbreviates);
    fact arg_class(abbreviates, 1, Sigil);
    fact arg_class(abbreviates, 2, Keyword);
    fact relation(derives);
    fact arg_class(derives, 1, Production);
    fact arg_class(derives, 2, Construct);
    fact relation(start_symbol);
    fact arg_class(start_symbol, 1, Production);
    fact relation(parses_as);
    fact arg_class(parses_as, 1, Production);
    fact arg_class(parses_as, 2, AstKind);
    fact relation(lowers_to);
    fact arg_class(lowers_to, 1, Form);
    fact arg_class(lowers_to, 2, Form);
    fact relation(round_trips);
    fact arg_class(round_trips, 1, Form);
    fact arg_class(round_trips, 2, Form);
    fact relation(loses);
    fact arg_class(loses, 1, Form);
    fact arg_class(loses, 2, Form);
    fact arg_class(loses, 3, Text);
    fact relation(has_type);
    fact arg_class(has_type, 1, Construct);
    fact arg_class(has_type, 2, Type);
    fact relation(has_effect);
    fact arg_class(has_effect, 1, Construct);
    fact arg_class(has_effect, 2, Effect);
    fact relation(constrained_by);
    fact arg_class(constrained_by, 1, Construct);
    fact arg_class(constrained_by, 2, Rule);
    fact relation(enforced_by);
    fact arg_class(enforced_by, 1, Rule);
    fact arg_class(enforced_by, 2, SymbolicComponent);
    fact relation(proposed_by);
    fact arg_class(proposed_by, 1, Artifact);
    fact arg_class(proposed_by, 2, NeuralComponent);
    fact relation(refused_by);
    fact arg_class(refused_by, 1, Refusal);
    fact arg_class(refused_by, 2, Rule);
    fact relation(in_profile);
    fact arg_class(in_profile, 1, Production);
    fact arg_class(in_profile, 2, Profile);
    fact relation(tested_by);
    fact arg_class(tested_by, 1, Diagnostic);
    fact arg_class(tested_by, 2, Text);

    // ── Membership ──────────────────────────────────────────────────────
    rule below(a: str, b: str) { subclass_of(a, b) }
    rule below(a: str, c: str) { subclass_of(a, b) && below(b, c) }
    rule is_a(x: str, c: str) { instance_of(x, c) }
    rule is_a(x: str, c: str) { instance_of(x, d) && below(d, c) }

    // ── Invariants: each rule's results are violations ──────────────────
    fact invariant(class_cycle, "the class hierarchy has no cycle");
    rule class_cycle(c: str) { below(c, c) }

    fact invariant(undeclared_class, "every class named by subclass_of, arg_class or instance_of is declared");
    rule undeclared_class(c: str) { subclass_of(c, _) && !class(c) }
    rule undeclared_class(c: str) { subclass_of(_, c) && !class(c) }
    rule undeclared_class(c: str) { arg_class(_, _, c) && !class(c) }
    rule undeclared_class(c: str) { instance_of(_, c) && !class(c) }

    fact invariant(unenforced_rule, "every Rule has an enforced_by");
    rule enforced(r: str) { enforced_by(r, _) }
    rule unenforced_rule(r: str) { is_a(r, "Rule") && !enforced(r) }

    fact invariant(rule_enforced_twice, "no Rule has two enforced_by");
    rule rule_enforced_twice(r: str) { enforced_by(r, a) && enforced_by(r, b) && a != b }

    fact invariant(spelling_two_kinds, "a Token lexes as one TokenKind");
    rule spelling_two_kinds(t: str) { lexes_as(t, a) && lexes_as(t, b) && a != b }

    fact invariant(keyword_two_sigils, "a Keyword has at most one Sigil");
    rule keyword_two_sigils(k: str) { abbreviates(a, k) && abbreviates(b, k) && a != b }

    fact invariant(unaccounted_lowering, "every lowers_to round-trips or states what it loses");
    rule accounted(x: str, y: str) { round_trips(x, y) }
    rule accounted(x: str, y: str) { loses(x, y, _) }
    rule unaccounted_lowering(x: str, y: str) { lowers_to(x, y) && !accounted(x, y) }

    fact invariant(unreachable_production, "every Production is reachable from a start_symbol");
    rule reachable(p: str) { start_symbol(p) }
    rule reachable(q: str) { reachable(p) && derives(p, q) }
    rule unreachable_production(p: str) { is_a(p, "Production") && !reachable(p) }

    fact invariant(unbuilt_ast_kind, "every AstKind is built by some Production");
    rule built(k: str) { parses_as(_, k) }
    rule unbuilt_ast_kind(k: str) { is_a(k, "AstKind") && !built(k) }

    fact invariant(untested_diagnostic, "every Diagnostic is emitted by some test");
    rule tested(d: str) { tested_by(d, _) }
    rule untested_diagnostic(d: str) { is_a(d, "Diagnostic") && !tested(d) }

    fact invariant(unnamed_refusal, "every Refusal names a Rule");
    rule names_rule(x: str) { refused_by(x, r) && is_a(r, "Rule") }
    rule unnamed_refusal(x: str) { is_a(x, "Refusal") && !names_rule(x) }
}

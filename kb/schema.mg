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
    // v4 (2026-10-29): `rhs`, for kb/grammar.mg. `derives` is now read off
    // the `rhs` facts by the loader, not stated.
    // v5 (2026-10-30): `summary` and `sigil_row`, so the ontology's `sigils`,
    // `keywords` and `ast_kinds` sections are generated from facts.
    // v6 (2026-10-30): `OpFamily` and the op, layer, type and effect rows of
    // kb/meaning.mg, so the ontology's `ir_ops`, `op_families`, `layer_map`,
    // `types` and `effects` sections can be checked against facts.
    // v7 (2026-10-30): `canonical_layer`, for the ontology's `layer_map`
    // section, and `abl_field` and `abl_media_type`, for its `abl` section.
    // v8 (2026-10-31): `safety_rule`, `rule_fix`, `rule_tag`, `RuleDatabase`
    // and `Severity`, for kb/rules/safety.mg. `unenforced_rule` exempts
    // SafetyRule: those rules are knowledge, and nothing executes them.
    // v9 (2026-10-31): `diagnostic_row`, `warning_carries` and `gap`, for
    // kb/rules/diagnostics.mg. `rule_enforced_twice` dropped: E0308 has three
    // emitters, and a code with several still traces to one rule.
    // v10 (2026-10-31): `contract_row`, `clause_checked` and `constraint_row`,
    // for kb/rules/contracts.mg and kb/rules/grammar.mg. A clause_checked
    // counts as enforcing its Contract.
    // v11 (2026-10-31): `GateRule` and `gate_row`, for kb/rules/refusals.mg.
    // v12 (2026-10-31): `ProfileForm` and the `form_row`, `leaf_form`,
    // `gated_form`, `profile_limit`, `profile_constraint` and `profile_signature`
    // relations, for kb/profiles/arena.mg.
    // v13 (2026-11-01): `form_example` and `profile_text`, the arena's prompt.
    // v14 (2026-11-03): `block_name` and `block_signature`, for the forge block
    // store's facts (`BlockStore::facts` in forge/src/registry/blocks.rs).
    // v15 (2026-11-04): `BuildRule` and `build_rule_row`, for kb/rules/builder.mg.
    // v16 (2026-11-04): `build_op`, `build_topology` and `build_consensus`, for
    // kb/rules/builder.mg, and `capability_namespace`, for kb/meaning.mg.
    // v17 (2026-11-04): `agent_role`, `role_grant` and `base_unit`, for
    // kb/meaning.mg: the agent roles of hir.rs and the base units of units.rs.
    // v18 (2026-11-04): `vocabulary`, `layer_grade` and `attribute_abbreviation`.
    // v19 (2026-11-04): `builtin_function` and `builtin_grade`, for kb/meaning.mg:
    // the builtins of resolve.rs's STD_FNS and differentiable.rs's grade lists.
    // v20 (2026-11-06): `scalar_math`, for kb/meaning.mg: the scalar math functions of
    // resolve.rs's SCALAR_MATH.
    // v21 (2026-11-06): `builtin_effect`, for kb/meaning.mg: the builtin names that
    // effects.rs's BUILTIN_EFFECTS attributes an effect on call.
    fact schema_version(21);

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
    // One choice of a profile's generator. Coarser than a `Production`.
    fact class(ProfileForm);
    fact subclass_of(ProfileForm, Construct);
    // What the lexer produces; a `Token` is a spelling that lexes as one.
    fact subclass_of(TokenKind, Construct);

    fact class(Type);
    fact class(Effect);
    fact class(Op);
    fact class(Layer);
    fact class(OpFamily);
    fact subclass_of(Type, Meaning);
    fact subclass_of(Effect, Meaning);
    fact subclass_of(Op, Meaning);
    fact subclass_of(Layer, Meaning);
    fact subclass_of(OpFamily, Meaning);

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
    fact class(GateRule);
    fact subclass_of(GateRule, Rule);
    // A builder's error code: refused before any artifact exists, so not a Diagnostic.
    fact class(BuildRule);
    fact subclass_of(BuildRule, Rule);
    fact class(RuleDatabase);
    fact class(Severity);
    fact subclass_of(RuleDatabase, Meaning);
    fact subclass_of(Severity, Meaning);
    fact instance_of("Ownership", RuleDatabase);
    fact instance_of("Borrow", RuleDatabase);
    fact instance_of("Lifetime", RuleDatabase);
    fact instance_of("TypeSafety", RuleDatabase);
    fact instance_of("Concurrency", RuleDatabase);
    fact instance_of("FFI", RuleDatabase);
    fact instance_of("AgentElision", RuleDatabase);
    fact instance_of("SwarmSafety", RuleDatabase);
    fact instance_of("Error", Severity);
    fact instance_of("Warning", Severity);
    fact instance_of("Info", Severity);
    fact instance_of("Hint", Severity);

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
    // One alternative of a production, in EBNF over production and
    // `TokenKind` names (`recognise.rs` reads it).
    fact relation(rhs);
    fact arg_class(rhs, 1, Production);
    fact arg_class(rhs, 2, Text);
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
    // A forge block, by its content address: its handle and its parameter signature.
    fact relation(block_name);
    fact arg_class(block_name, 1, Block);
    fact arg_class(block_name, 2, Text);
    fact relation(block_signature);
    fact arg_class(block_signature, 1, Block);
    fact arg_class(block_signature, 2, Text);
    fact relation(refused_by);
    fact arg_class(refused_by, 1, Refusal);
    fact arg_class(refused_by, 2, Rule);
    // id, database, category, severity, description, rationale, confidence.
    fact relation(safety_rule);
    fact arg_class(safety_rule, 1, SafetyRule);
    fact arg_class(safety_rule, 2, RuleDatabase);
    fact arg_class(safety_rule, 3, Text);
    fact arg_class(safety_rule, 4, Severity);
    fact arg_class(safety_rule, 5, Text);
    fact arg_class(safety_rule, 6, Text);
    fact arg_class(safety_rule, 7, Text);
    fact relation(rule_fix);
    fact arg_class(rule_fix, 1, SafetyRule);
    fact arg_class(rule_fix, 2, Text);
    // rule, position, tag: a rule's tags are ordered.
    fact relation(rule_tag);
    fact arg_class(rule_tag, 1, SafetyRule);
    fact arg_class(rule_tag, 2, Text);
    fact arg_class(rule_tag, 3, Text);
    // id, severity, category, summary.
    fact relation(diagnostic_row);
    fact arg_class(diagnostic_row, 1, Diagnostic);
    fact arg_class(diagnostic_row, 2, Severity);
    fact arg_class(diagnostic_row, 3, Text);
    fact arg_class(diagnostic_row, 4, Text);
    // A warning class, and the code it goes out under.
    fact relation(warning_carries);
    fact arg_class(warning_carries, 1, Diagnostic);
    fact arg_class(warning_carries, 2, Diagnostic);
    // A rule nothing checks, and why, in place of an `enforced_by`.
    fact relation(gap);
    fact arg_class(gap, 1, Rule);
    fact arg_class(gap, 2, Text);
    // name, spelling, meaning.
    fact relation(contract_row);
    fact arg_class(contract_row, 1, Contract);
    fact arg_class(contract_row, 2, Text);
    fact arg_class(contract_row, 3, Text);
    // clause, placement, the component that checks it there.
    fact relation(clause_checked);
    fact arg_class(clause_checked, 1, Contract);
    fact arg_class(clause_checked, 2, Text);
    fact arg_class(clause_checked, 3, SymbolicComponent);
    fact relation(gate_row);
    fact arg_class(gate_row, 1, GateRule);
    fact arg_class(gate_row, 2, Text);
    // id, family (the schema part that serves it), when, fix.
    fact relation(build_rule_row);
    fact arg_class(build_rule_row, 1, BuildRule);
    fact arg_class(build_rule_row, 2, Text);
    fact arg_class(build_rule_row, 3, Text);
    fact arg_class(build_rule_row, 4, Text);
    // The constructible ops of prototype/src/builder.rs: a name, its arity, whether
    // it transforms the running dim, and its doc. Arity and transforms are Text.
    fact relation(build_op);
    fact arg_class(build_op, 1, Text);
    fact arg_class(build_op, 2, Text);
    fact arg_class(build_op, 3, Text);
    fact arg_class(build_op, 4, Text);
    fact relation(build_topology);
    fact arg_class(build_topology, 1, Text);
    fact relation(build_consensus);
    fact arg_class(build_consensus, 1, Text);
    // The capability namespaces of prototype/src/hir.rs, and the effect each is
    // attributed to: its Effect's name, or `none` for a namespace with no effect.
    fact relation(capability_namespace);
    fact arg_class(capability_namespace, 1, Text);
    fact arg_class(capability_namespace, 2, Text);
    // The agent roles of prototype/src/hir.rs, and the effects each is granted:
    // a role's name, and an effect's name as `hir::Effect` displays it.
    fact relation(agent_role);
    fact arg_class(agent_role, 1, Text);
    fact relation(role_grant);
    fact arg_class(role_grant, 1, Text);
    fact arg_class(role_grant, 2, Text);
    // The base units of prototype/src/units.rs: a name, and its position in a Dim.
    fact relation(base_unit);
    fact arg_class(base_unit, 1, Text);
    fact arg_class(base_unit, 2, Text);
    // The standard vocabulary of prototype/src/resolve.rs: a name, its signature
    // (display only; nothing parses it) and its doc.
    fact relation(vocabulary);
    fact arg_class(vocabulary, 1, Text);
    fact arg_class(vocabulary, 2, Text);
    fact arg_class(vocabulary, 3, Text);
    // The attribute abbreviations of prototype/src/elision.rs: a short name and
    // the full name it expands to.
    fact relation(attribute_abbreviation);
    fact arg_class(attribute_abbreviation, 1, Text);
    fact arg_class(attribute_abbreviation, 2, Text);
    // The grade of each surface layer type in prototype/src/differentiable.rs:
    // `smooth` or `kinked`. A layer with no grade is Unknown.
    fact relation(layer_grade);
    fact arg_class(layer_grade, 1, Layer);
    fact arg_class(layer_grade, 2, Text);
    // The builtin functions of prototype/src/resolve.rs's STD_FNS: a name each.
    fact relation(builtin_function);
    fact arg_class(builtin_function, 1, Text);
    // The grade of each builtin in differentiable.rs's KINKED, SMOOTH_BUILTINS and
    // DISCRETE_BUILTINS: `kinked`, `smooth` or `discrete`.
    fact relation(builtin_grade);
    fact arg_class(builtin_grade, 1, Text);
    fact arg_class(builtin_grade, 2, Text);
    // The scalar math functions of prototype/src/resolve.rs's SCALAR_MATH: a name each.
    fact relation(scalar_math);
    fact arg_class(scalar_math, 1, Text);
    // The builtin names of prototype/src/effects.rs's BUILTIN_EFFECTS, each with the effect
    // it is attributed on call, spelled as MAGE_SPEC.md's table spells it: io or panic.
    fact relation(builtin_effect);
    fact arg_class(builtin_effect, 1, Text);
    fact arg_class(builtin_effect, 2, Text);
    // name, value, meaning.
    fact relation(constraint_row);
    fact arg_class(constraint_row, 1, GrammarConstraint);
    fact arg_class(constraint_row, 2, Text);
    fact arg_class(constraint_row, 3, Text);
    fact relation(in_profile);
    fact arg_class(in_profile, 1, Production);
    fact arg_class(in_profile, 2, Profile);
    // form, profile, its nonterminal, its position there.
    fact relation(form_row);
    fact arg_class(form_row, 1, ProfileForm);
    fact arg_class(form_row, 2, Profile);
    fact arg_class(form_row, 3, Text);
    fact arg_class(form_row, 4, Text);
    // A form written out, as the prompt shows it.
    fact relation(form_example);
    fact arg_class(form_example, 1, ProfileForm);
    fact arg_class(form_example, 2, Text);
    // profile, key, a piece of its prompt.
    fact relation(profile_text);
    fact arg_class(profile_text, 1, Profile);
    fact arg_class(profile_text, 2, Text);
    fact arg_class(profile_text, 3, Text);
    // A form with no list below it, so it may sit at the depth limit.
    fact relation(leaf_form);
    fact arg_class(leaf_form, 1, ProfileForm);
    // A form only a generator with this option on may use.
    fact relation(gated_form);
    fact arg_class(gated_form, 1, ProfileForm);
    fact arg_class(gated_form, 2, Text);
    // profile, name, lowest, highest, inclusive.
    fact relation(profile_limit);
    fact arg_class(profile_limit, 1, Profile);
    fact arg_class(profile_limit, 2, Text);
    fact arg_class(profile_limit, 3, Text);
    fact arg_class(profile_limit, 4, Text);
    fact relation(profile_constraint);
    fact arg_class(profile_constraint, 1, Profile);
    fact arg_class(profile_constraint, 2, GrammarConstraint);
    // The one function a profile's programs are, as the source writes it.
    fact relation(profile_signature);
    fact arg_class(profile_signature, 1, Profile);
    fact arg_class(profile_signature, 2, Text);
    // A one-line description of a construct, as the ontology publishes it.
    fact relation(summary);
    fact arg_class(summary, 1, Construct);
    fact arg_class(summary, 2, Text);
    // A row of the ontology's `sigils` section: a notation (a sigil, `+`
    // before an item sigil, or a type shape such as `[T]~`), what it builds,
    // and a summary. A notation is several tokens, so all three are Text.
    fact relation(sigil_row);
    fact arg_class(sigil_row, 1, Text);
    fact arg_class(sigil_row, 2, Text);
    fact arg_class(sigil_row, 3, Text);
    // kb/meaning.mg: an op family's row of the ontology's `op_families`
    // section (its code as two-digit hex, and its summary).
    fact relation(family_row);
    fact arg_class(family_row, 1, OpFamily);
    fact arg_class(family_row, 2, Text);
    fact arg_class(family_row, 3, Text);
    // An op's facts, as the ontology's `ir_ops` section states them.
    fact relation(opcode);
    fact arg_class(opcode, 1, Op);
    fact arg_class(opcode, 2, Text);
    fact relation(op_family);
    fact arg_class(op_family, 1, Op);
    fact arg_class(op_family, 2, OpFamily);
    fact relation(op_arity);
    fact arg_class(op_arity, 1, Op);
    fact arg_class(op_arity, 2, Text);
    fact relation(op_summary);
    fact arg_class(op_summary, 1, Op);
    fact arg_class(op_summary, 2, Text);
    // Flags: an op that is differentiable, has learnable parameters, or is
    // stateful. Stated only when true.
    fact relation(differentiable);
    fact arg_class(differentiable, 1, Op);
    fact relation(has_params);
    fact arg_class(has_params, 1, Op);
    fact relation(stateful);
    fact arg_class(stateful, 1, Op);
    // A layer name that `abl_bridge::layer_name_to_op` accepts, and the op
    // it lowers to.
    fact relation(lowers_as);
    fact arg_class(lowers_as, 1, Layer);
    fact arg_class(lowers_as, 2, Op);
    // A row of the ontology's `types` and `effects` sections: the category
    // (or slot) and the summary (or purpose).
    fact relation(type_row);
    fact arg_class(type_row, 1, Type);
    fact arg_class(type_row, 2, Text);
    fact arg_class(type_row, 3, Text);
    fact relation(effect_row);
    fact arg_class(effect_row, 1, Effect);
    fact arg_class(effect_row, 2, Text);
    fact arg_class(effect_row, 3, Text);
    // The layer spellings the ontology's `layer_map` publishes: each has a
    // `lowers_as` fact, and `layer_map` takes its op from there.
    fact relation(canonical_layer);
    fact arg_class(canonical_layer, 1, Layer);
    // The binary container's format, for the ontology's `abl` section: one
    // `abl_field` fact per format line, its two-digit position and its text.
    fact relation(abl_field);
    fact arg_class(abl_field, 1, Text);
    fact arg_class(abl_field, 2, Text);
    fact relation(abl_media_type);
    fact arg_class(abl_media_type, 1, Text);
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

    fact invariant(unenforced_rule, "every Rule but a SafetyRule has an enforced_by or a gap");
    rule enforced(r: str) { enforced_by(r, _) }
    rule enforced(r: str) { gap(r, _) }
    rule enforced(r: str) { clause_checked(r, _, _) }
    rule unenforced_rule(r: str) { is_a(r, "Rule") && !is_a(r, "SafetyRule") && !enforced(r) }

    fact invariant(rule_without_row, "every SafetyRule has a safety_rule row");
    rule has_rule_row(r: str) { safety_rule(r, _, _, _, _, _, _) }
    rule rule_without_row(r: str) { is_a(r, "SafetyRule") && !has_rule_row(r) }

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

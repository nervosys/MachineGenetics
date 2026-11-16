// The arena's program space, as a profile of the full grammar.
//
// `in_profile` names the productions of kb/grammar.mg that the arena's programs
// use, and a test recognises them with those productions alone: every form
// here, and sampled programs. `arena/src/grammar.rs` reads the form and limit
// facts at start-up, and has no tables of its own.
//
// A form is one choice of the arena's policy, and its position is its logit
// index, so reordering these changes what every seed draws. A form is coarser
// than a production: `map` is a method call, which is many productions.
// `second_order` forms are drawn only by a policy that has them on.
//
// `describe_grammar` builds the prompt from `form_example` and `profile_text`;
// `{name}` in a text is the limit of that name, or `max_depth`, or the gated form,
// or `fn_head` (the signature up to its parameters), or `separator` (the line
// between a reply's blocks, which `command.rs` splits on). `{n}` in `request` is
// filled by the prompt's code.

kb ArenaProfile {
    fact instance_of("arena", Profile);
    fact profile_signature("arena", "f gen(s: usize) -> [usize]");
    fact profile_constraint("arena", "max_depth");
    fact profile_constraint("arena", "max_nest");
    fact instance_of("range", ProfileForm);
    fact form_row("range", "arena", "list", "0");
    fact instance_of("repeat", ProfileForm);
    fact form_row("repeat", "arena", "list", "1");
    fact instance_of("map", ProfileForm);
    fact form_row("map", "arena", "list", "2");
    fact instance_of("scan", ProfileForm);
    fact form_row("scan", "arena", "list", "3");
    fact instance_of("filter", ProfileForm);
    fact form_row("filter", "arena", "list", "4");
    fact instance_of("reverse", ProfileForm);
    fact form_row("reverse", "arena", "list", "5");
    fact instance_of("sort", ProfileForm);
    fact form_row("sort", "arena", "list", "6");
    fact instance_of("take", ProfileForm);
    fact form_row("take", "arena", "list", "7");
    fact instance_of("concat", ProfileForm);
    fact form_row("concat", "arena", "list", "8");
    fact instance_of("copy", ProfileForm);
    fact form_row("copy", "arena", "list", "9");
    fact instance_of("cycle", ProfileForm);
    fact form_row("cycle", "arena", "list", "10");
    fact instance_of("nest", ProfileForm);
    fact form_row("nest", "arena", "list", "11");
    fact instance_of("osc", ProfileForm);
    fact form_row("osc", "arena", "list", "12");
    fact form_example("range", "range(8)");
    fact form_example("repeat", "[3; 4]");
    fact form_example("map", "range(4).map(|x| wrapping_mul(x, 3))");
    fact form_example("scan", "range(4).scan(s, |a, x| wrapping_add(a, x))");
    fact form_example("filter", "range(8).filter(|x| x % 3 == 1)");
    fact form_example("reverse", "range(5).reverse()");
    fact form_example("sort", "range(5).sort()");
    fact form_example("take", "range(9).take(4)");
    fact form_example("concat", "flatten([range(3), range(2)])");
    fact form_example("copy", "flatten([range(3); 2])");
    fact form_example("cycle", "range(6).map(|v| [1, s, 0][v % 3])");
    fact form_example("nest", "flatten(range(4).map(|v| range(v % 3 + 1)))");
    fact form_example("osc", "range(11).scan((100, 7), |p, v| { val (c, q) = p; ((wrapping_sub(wrapping_mul(64, c), wrapping_mul(32, q)) >> 6), c) }).map(|p| { val (c, q) = p; c })");
    fact form_example("lit", "7");
    fact form_example("var", "s");
    fact form_example("add", "wrapping_add(s, 3)");
    fact form_example("sub", "wrapping_sub(s, 3)");
    fact form_example("mul", "wrapping_mul(s, 3)");
    fact form_example("mod", "(s % 5)");
    fact profile_text("arena", "preamble", "You write programs for the arena, a search over small typed MAGE programs. Write bodies only: no `{fn_head}`, no braces, no comments.");
    fact profile_text("arena", "shown_head", "The best programs found so far, as written, with the learning progress each earned (higher is better):");
    fact profile_text("arena", "none_yet", "(none yet)");
    fact profile_text("arena", "request", "Write up to {n} new programs, different from these. Each is a single body expression. Put one per block, and separate blocks with a line containing only {separator}. Write nothing else.");
    fact profile_text("arena", "separator", "---");
    fact profile_text("arena", "list_head","A body is one list expression over the seed `s`. Its forms, one example each:");
    fact profile_text("arena", "int_head", "Integer expressions, inside any of them:");
    fact profile_text("arena", "bounds", "Bounds: range and take lengths {range}; repeat lengths {repeat}; literals {literal}; modulus {mod}; cycle templates have {cycle_elems} elements and length {cycle_len}; copy counts {copy}; nest counts {nest_count} and moduli {nest_mod}; filter moduli {filter_mod}. Lists nest at most {max_depth} levels deep. Every variable must be `s` or a closure parameter in scope.");
    fact profile_text("arena", "second_order", "The second-order form `{form}` is the only one whose values can oscillate: it emits values in {osc_len}, its start values are literals {literal}, its shift is {osc_shift}, and its coefficient a is at most 2^(k+1) and b below 2^k. Write it as its example, with the numbers changed within these bounds.");
    fact leaf_form("range");
    fact leaf_form("repeat");
    fact leaf_form("cycle");
    fact leaf_form("nest");
    fact leaf_form("osc");
    fact gated_form("osc", "second_order");
    fact instance_of("lit", ProfileForm);
    fact form_row("lit", "arena", "int", "0");
    fact instance_of("var", ProfileForm);
    fact form_row("var", "arena", "int", "1");
    fact instance_of("add", ProfileForm);
    fact form_row("add", "arena", "int", "2");
    fact instance_of("sub", ProfileForm);
    fact form_row("sub", "arena", "int", "3");
    fact instance_of("mul", ProfileForm);
    fact form_row("mul", "arena", "int", "4");
    fact instance_of("mod", ProfileForm);
    fact form_row("mod", "arena", "int", "5");
    fact profile_limit("arena", "literal", "0", "255");
    fact profile_limit("arena", "mod", "2", "256");
    fact profile_limit("arena", "range", "1", "48");
    fact profile_limit("arena", "take", "1", "48");
    fact profile_limit("arena", "repeat", "1", "12");
    fact profile_limit("arena", "cycle_len", "4", "48");
    fact profile_limit("arena", "cycle_elems", "2", "6");
    fact profile_limit("arena", "copy", "2", "6");
    fact profile_limit("arena", "nest_count", "2", "12");
    fact profile_limit("arena", "nest_mod", "2", "8");
    fact profile_limit("arena", "filter_mod", "2", "7");
    fact profile_limit("arena", "osc_len", "2", "49");
    fact profile_limit("arena", "osc_shift", "1", "6");
    fact profile_limit("arena", "int_extra_depth", "2", "2");
    fact in_profile(Attr, "arena");
    fact in_profile(Block, "arena");
    fact in_profile(BlockBody, "arena");
    fact in_profile(ClosureParams, "arena");
    fact in_profile(Expr, "arena");
    fact in_profile(FunctionDef, "arena");
    fact in_profile(ItAttrArg, "arena");
    fact in_profile(Item, "arena");
    fact in_profile(Module, "arena");
    fact in_profile(Name, "arena");
    fact in_profile(PaClosureList, "arena");
    fact in_profile(PaClosureParam, "arena");
    fact in_profile(PaParam, "arena");
    fact in_profile(PaParamHead, "arena");
    fact in_profile(Params, "arena");
    fact in_profile(PrefixExpr, "arena");
    fact in_profile(PxArrayLit, "arena");
    fact in_profile(PxClosure, "arena");
    fact in_profile(PxClosureBody, "arena");
    fact in_profile(PxNameKw, "arena");
    fact in_profile(PxTuple, "arena");
    fact in_profile(StAdd, "arena");
    fact in_profile(StAnd, "arena");
    fact in_profile(StArgs, "arena");
    fact in_profile(StAssign, "arena");
    fact in_profile(StAssignOp, "arena");
    fact in_profile(StBitAnd, "arena");
    fact in_profile(StBitOr, "arena");
    fact in_profile(StBitXor, "arena");
    fact in_profile(StEq, "arena");
    fact in_profile(StMul, "arena");
    fact in_profile(StOr, "arena");
    fact in_profile(StPostfix, "arena");
    fact in_profile(StPostfixOp, "arena");
    fact in_profile(StRel, "arena");
    fact in_profile(StShift, "arena");
    fact in_profile(StTensor, "arena");
    fact in_profile(Stmt, "arena");
    fact in_profile(TyName, "arena");
    fact in_profile(TyPath, "arena");
    fact in_profile(TySlice, "arena");
    fact in_profile(Type, "arena");
}

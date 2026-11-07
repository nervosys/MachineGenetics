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

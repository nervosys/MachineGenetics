// Every diagnostic the compiler produces: 19 codes and 7 warning classes.
//
// `diagnostic_row` is id, severity, category, summary. A code is what
// `--check --json` prints. A warning class has no code of its own: where it
// goes out as a `Diagnostic` it carries one (`warning_carries`), and the abl
// lowering's warnings are plain strings and carry none. `enforced_by` names
// each module that emits the diagnostic, so a code may have several. Order is
// not meaningful.
//
// prototype/tests/diagnostics.rs holds the code to this file both ways: every
// `tested_by` is a witness that really triggers the diagnostic, every emit site
// in the listed modules is stated, and every code in the source is here.
//
// Five codes are not here, because nothing emits them. E0499 and E0503 were
// only ever named in comments. E0505, E0277 and E0599 are in `aci.rs`'s pattern
// table, which no production path reaches.

kb Diagnostics {
    fact instance_of("lexer", SymbolicComponent);
    fact instance_of("parser", SymbolicComponent);
    fact instance_of("resolve", SymbolicComponent);
    fact instance_of("types", SymbolicComponent);
    fact instance_of("effects", SymbolicComponent);
    fact instance_of("shape", SymbolicComponent);
    fact instance_of("abl_shape", SymbolicComponent);
    fact instance_of("abl_bridge", SymbolicComponent);
    fact instance_of("autograd", SymbolicComponent);
    fact instance_of("differentiable", SymbolicComponent);
    fact instance_of("core_subset", SymbolicComponent);
    fact instance_of("units", SymbolicComponent);
    fact instance_of("verify", SymbolicComponent);
    fact instance_of("params", SymbolicComponent);
    fact instance_of("logic", SymbolicComponent);

    // ── Codes ───────────────────────────────────────────────────────────
    fact instance_of("E0000", Diagnostic);
    fact diagnostic_row("E0000", "Error", "LexError", "A character the lexer cannot read");
    fact enforced_by("E0000", "lexer");
    fact tested_by("E0000", "lex_error");

    fact instance_of("E0001", Diagnostic);
    fact diagnostic_row("E0001", "Error", "SyntaxError", "Source that does not parse, or a contract clause that does not");
    fact enforced_by("E0001", "parser");
    fact enforced_by("E0001", "types");
    fact tested_by("E0001", "syntax_error");

    // Assigned to an uncategorised diagnostic by `heal::infer_category`, from
    // its message text. A pass that detected a borrow conflict or a use after
    // move would emit these; none does.
    fact instance_of("E0382", Diagnostic);
    fact diagnostic_row("E0382", "Error", "UseAfterMove", "Use after move");
    fact gap("E0382", "no pass detects a use after move: only heal's message classifier assigns this code");
    fact tested_by("E0382", "use_after_move_classified");

    fact instance_of("E0502", Diagnostic);
    fact diagnostic_row("E0502", "Error", "BorrowConflict", "Borrow conflict");
    fact gap("E0502", "no pass detects a borrow conflict: only heal's message classifier assigns this code");
    fact tested_by("E0502", "borrow_conflict_classified");

    fact instance_of("E0308", Diagnostic);
    fact diagnostic_row("E0308", "Error", "TypeMismatch", "Type mismatch");
    fact enforced_by("E0308", "types");
    fact enforced_by("E0308", "shape");
    fact enforced_by("E0308", "differentiable");
    fact tested_by("E0308", "type_mismatch");

    fact instance_of("E0412", Diagnostic);
    fact diagnostic_row("E0412", "Error", "UnresolvedType", "Unresolved type");
    fact enforced_by("E0412", "resolve");
    fact enforced_by("E0412", "units");
    fact tested_by("E0412", "unresolved_type");

    fact instance_of("E0425", Diagnostic);
    fact diagnostic_row("E0425", "Error", "UnresolvedName", "Unresolved name");
    fact enforced_by("E0425", "resolve");
    fact tested_by("E0425", "unresolved_name");

    fact instance_of("E0428", Diagnostic);
    fact diagnostic_row("E0428", "Error", "DuplicateDefinition", "Duplicate definition");
    fact enforced_by("E0428", "resolve");
    fact tested_by("E0428", "duplicate_definition");

    fact instance_of("E0550", Diagnostic);
    fact diagnostic_row("E0550", "Error", "UndeclaredEffect", "An effect a function performs and does not declare, or declares and does not know");
    fact enforced_by("E0550", "effects");
    fact tested_by("E0550", "undeclared_effect");

    fact instance_of("E0551", Diagnostic);
    fact diagnostic_row("E0551", "Error", "RoleViolation", "An effect a function's declared role does not allow, or a missing role under --core");
    fact enforced_by("E0551", "effects");
    fact enforced_by("E0551", "core_subset");
    fact tested_by("E0551", "role_violation");

    fact instance_of("E0552", Diagnostic);
    fact diagnostic_row("E0552", "Error", "CoreViolation", "Code outside MAGE-core where MAGE-core is required");
    fact enforced_by("E0552", "core_subset");
    fact tested_by("E0552", "core_violation");

    fact instance_of("E0553", Diagnostic);
    fact diagnostic_row("E0553", "Error", "ParamDomain", "A tunable @param whose domain or value is malformed, or out of its domain");
    fact enforced_by("E0553", "params");
    fact tested_by("E0553", "param_domain");

    fact instance_of("E0554", Diagnostic);
    fact diagnostic_row("E0554", "Error", "Deprecated", "A retired construct: the message names its replacement");
    fact enforced_by("E0554", "types");
    fact tested_by("E0554", "deprecated");

    fact instance_of("E0560", Diagnostic);
    fact diagnostic_row("E0560", "Error", "SpecViolation", "A contract that is false on every execution");
    fact enforced_by("E0560", "verify");
    fact tested_by("E0560", "spec_violation");

    fact instance_of("E0561", Diagnostic);
    fact diagnostic_row("E0561", "Error", "ContractScope", "A contract naming result or old where it is not in scope");
    fact enforced_by("E0561", "types");
    fact tested_by("E0561", "contract_scope");

    fact instance_of("E0562", Diagnostic);
    fact diagnostic_row("E0562", "Error", "UnitMismatch", "Two quantities whose units of measure disagree");
    fact enforced_by("E0562", "units");
    fact tested_by("E0562", "unit_mismatch");

    fact instance_of("E0570", Diagnostic);
    fact diagnostic_row("E0570", "Error", "KbRule", "A kb fact or rule the logic engine cannot evaluate");
    fact enforced_by("E0570", "logic");
    fact tested_by("E0570", "kb_rule");

    fact instance_of("E0710", Diagnostic);
    fact diagnostic_row("E0710", "Error", "ShapeError", "Layers of a net whose tensor shapes do not fit");
    fact enforced_by("E0710", "abl_shape");
    fact tested_by("E0710", "net_shape");

    // Only warnings go out under this code: see the two classes below.
    fact instance_of("E9999", Diagnostic);
    fact diagnostic_row("E9999", "Warning", "Other", "A warning with no category of its own");
    fact gap("E9999", "a catch-all: the warning classes that carry it say what happened");
    fact tested_by("E9999", "grad_unchecked");

    // ── Warning classes ─────────────────────────────────────────────────
    // The first two carry E0308 though nothing mismatched.
    fact instance_of("no_derivative_rule", Diagnostic);
    fact diagnostic_row("no_derivative_rule", "Warning", "TypeMismatch", "A call with no derivative rule is treated as the identity");
    fact warning_carries("no_derivative_rule", "E0308");
    fact enforced_by("no_derivative_rule", "autograd");
    fact tested_by("no_derivative_rule", "no_derivative_rule");

    fact instance_of("param_no_gradient", Diagnostic);
    fact diagnostic_row("param_no_gradient", "Warning", "TypeMismatch", "A parameter is not on the computation path, so has no gradient");
    fact warning_carries("param_no_gradient", "E0308");
    fact enforced_by("param_no_gradient", "autograd");
    fact tested_by("param_no_gradient", "param_no_gradient");

    fact instance_of("grad_unchecked", Diagnostic);
    fact diagnostic_row("grad_unchecked", "Warning", "Other", "A grad that could not be checked is computed anyway");
    fact warning_carries("grad_unchecked", "E9999");
    fact enforced_by("grad_unchecked", "differentiable");
    fact tested_by("grad_unchecked", "grad_unchecked");

    fact instance_of("bound_discarded", Diagnostic);
    fact diagnostic_row("bound_discarded", "Warning", "Other", "A trait bound is parsed and then discarded: MAGE has no trait solving");
    fact warning_carries("bound_discarded", "E9999");
    fact enforced_by("bound_discarded", "resolve");
    fact tested_by("bound_discarded", "bound_discarded");

    fact instance_of("unknown_layer", Diagnostic);
    fact diagnostic_row("unknown_layer", "Warning", "AblLowering", "A layer of an unknown type lowered to IDENTITY");
    fact enforced_by("unknown_layer", "abl_bridge");
    fact tested_by("unknown_layer", "unknown_layer");

    fact instance_of("kb_fact_left_out", Diagnostic);
    fact diagnostic_row("kb_fact_left_out", "Warning", "AblLowering", "A kb fact with an argument that is not a constant is left out of the binary form");
    fact enforced_by("kb_fact_left_out", "abl_bridge");
    fact tested_by("kb_fact_left_out", "kb_fact_left_out");

    fact instance_of("kb_rule_left_out", Diagnostic);
    fact diagnostic_row("kb_rule_left_out", "Warning", "AblLowering", "A kb rule the binary form cannot carry is left out of it");
    fact enforced_by("kb_rule_left_out", "abl_bridge");
    fact tested_by("kb_rule_left_out", "kb_rule_left_out");
}

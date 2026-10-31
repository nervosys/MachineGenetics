// The forms a MAGE program takes, and every lowering between them that the
// compiler performs (NEUROSYMBOLIC_PLAN.md Phase 3). A `lowers_to` fact must
// either `round_trips` or say what it `loses` (the `unaccounted_lowering`
// invariant), so each loss below is stated, not left for a reader to find.
//
// Written 2026-10-30 from prototype/src by reading each lowering's entry
// function and the tests that check it; the function is named above each
// fact. Where no test checks a claimed round trip, the loss is stated
// instead.

kb Forms {
    // ── One instance per form class ─────────────────────────────────────
    // HumanSource: `.mg` text with keywords (`fn`, `pub`, `swarm`).
    fact instance_of(HumanText, HumanSource);
    // SigilSource: `.mg` text with sigils (`f`, `+`, `sw`).
    fact instance_of(SigilText, SigilSource);
    // Declarative: the AI items, each a declaration the compiler lowers to
    // ops rather than runs (ast.rs NetDef, KbDef, AgentDef, SwarmDef,
    // TrainDef, EvolveDef).
    fact instance_of(NetDecl, Declarative);
    fact instance_of(KbDecl, Declarative);
    fact instance_of(AgentDecl, Declarative);
    fact instance_of(SwarmDecl, Declarative);
    fact instance_of(TrainDecl, Declarative);
    fact instance_of(EvolveDecl, Declarative);
    // Ast: ast.rs `Module`, what parser.rs `parse` returns.
    fact instance_of(AstTree, Ast);
    // Hir: hir.rs. Not a tree: the types (`Ty`), effects and diagnostics that
    // types.rs `check`, resolve.rs `resolve` and effects.rs `infer_effects`
    // compute from the AST.
    fact instance_of(HirTables, Hir);
    // IrOp: an RMIL expression over `rmi::lang::Op`.
    fact instance_of(RmilExpr, IrOp);
    // BinaryContainer: the ABL bytes abl.rs `encode_module` writes.
    fact instance_of(AblBytes, BinaryContainer);

    // ── Source and AST ──────────────────────────────────────────────────
    // parser.rs `parse` over lexer.rs `lex`. `lex` drops Whitespace and
    // Comment tokens; `pub` and `+` (and every abbreviates pair) lex to the
    // same token kind, so the AST cannot say which was written.
    fact lowers_to(HumanText, AstTree);
    fact loses(HumanText, AstTree, "comments");
    fact loses(HumanText, AstTree, "layout: blank lines, indentation, line breaks");
    fact loses(HumanText, AstTree, "parentheses: no Expr node records them");
    fact loses(HumanText, AstTree, "spans, except on contract clauses and spec items");
    fact loses(HumanText, AstTree, "block definitions: a block reference is expanded in place and the name dropped");
    fact lowers_to(SigilText, AstTree);
    fact loses(SigilText, AstTree, "comments");
    fact loses(SigilText, AstTree, "layout: blank lines, indentation, line breaks");
    fact loses(SigilText, AstTree, "parentheses: no Expr node records them");
    fact loses(SigilText, AstTree, "spans, except on contract clauses and spec items");
    fact loses(SigilText, AstTree, "block definitions: a block reference is expanded in place and the name dropped");

    // fmt.rs `format_human` and `format_agent`. Printing loses nothing the
    // AST holds: tests/canonical_round_trip.rs reparses every `.mg` source
    // printed in either mode to the same AST (spans aside).
    fact lowers_to(AstTree, HumanText);
    fact round_trips(AstTree, HumanText);
    fact lowers_to(AstTree, SigilText);
    fact round_trips(AstTree, SigilText);

    // The formatter's two modes, source to source: parse, then print in the
    // other mode. What it loses is what parsing loses.
    fact lowers_to(SigilText, HumanText);
    fact loses(SigilText, HumanText, "comments");
    fact loses(SigilText, HumanText, "layout: the formatter's own is printed");
    fact loses(SigilText, HumanText, "parentheses the precedence does not need");
    fact lowers_to(HumanText, SigilText);
    fact loses(HumanText, SigilText, "comments");
    fact loses(HumanText, SigilText, "layout: the formatter's own is printed");
    fact loses(HumanText, SigilText, "parentheses the precedence does not need");

    // types.rs `check`, resolve.rs `resolve`, effects.rs `infer_effects`.
    // There is no HIR tree: `infer_expr` returns a type and stores nothing
    // per node, and resolution is keyed by name (resolve.rs 180-182).
    fact lowers_to(AstTree, HirTables);
    fact loses(AstTree, HirTables, "the tree: types and effects are kept per name, not per node");

    // ── Declarations to ops ─────────────────────────────────────────────
    // An AI item in the AST is its declaration.
    fact lowers_to(AstTree, NetDecl);
    fact round_trips(AstTree, NetDecl);

    // abl_bridge.rs `NetTranslator::translate`.
    fact lowers_to(NetDecl, RmilExpr);
    fact loses(NetDecl, RmilExpr, "layer names: each layer becomes its op");
    fact loses(NetDecl, RmilExpr, "generics");
    fact loses(NetDecl, RmilExpr, "the forward block: only the walked result is kept");
    fact loses(NetDecl, RmilExpr, "a composition, when the forward block reaches every declared layer: the walked result wins");
    fact loses(NetDecl, RmilExpr, "layer arguments that are not literals, silently");
    fact loses(NetDecl, RmilExpr, "an unknown layer type: it lowers to IDENTITY, with a diagnostic");

    // abl_bridge.rs `KbTranslator::translate`.
    fact lowers_to(KbDecl, RmilExpr);
    fact loses(KbDecl, RmilExpr, "a fact with a non-constant argument: left out and named");
    fact loses(KbDecl, RmilExpr, "a rule with a comparison, negation or constant: left out and named");

    // abl_bridge.rs `AgentTranslator::translate`: SPAWN with capabilities,
    // DELEGATE for each approval.
    fact lowers_to(AgentDecl, RmilExpr);
    fact loses(AgentDecl, RmilExpr, "the agent body: only the name, capabilities and approvals are lowered");

    // abl_bridge.rs `SwarmTranslator::translate`.
    fact lowers_to(SwarmDecl, RmilExpr);
    fact loses(SwarmDecl, RmilExpr, "a size that is not a literal: it becomes 1");
    fact loses(SwarmDecl, RmilExpr, "an unknown topology: only RECV is emitted");
    fact loses(SwarmDecl, RmilExpr, "on_dispatch, on_aggregate and on_failure: never read");

    // abl_bridge.rs `lower_module`, train arm: `MSE_LOSS >> SGD_STEP`.
    fact lowers_to(TrainDecl, RmilExpr);
    fact loses(TrainDecl, RmilExpr, "optimizer, loss and epochs: the lowering is fixed; main.rs reads them from the AST");

    // abl_bridge.rs `lower_module`, evolve arm: `MAP >> REDUCE`.
    fact lowers_to(EvolveDecl, RmilExpr);
    fact loses(EvolveDecl, RmilExpr, "genome, fitness, mutate, crossover and select");

    // ── Ops and the container ───────────────────────────────────────────
    // abl.rs `encode_module` and `decode_container`: repeats are folded on
    // the way in and expanded on the way out, and the symbol table travels
    // with the expression. `stack_net_ships_folded_decodes_flat` and
    // `module_lowering_round_trips_through_codec` compare the decoded
    // expression with the lowered one.
    fact lowers_to(RmilExpr, AblBytes);
    fact round_trips(RmilExpr, AblBytes);

    // abl_bridge.rs `decompile`: a NetDef whose lowering has the same
    // content hash (`decompile_round_trips_through_lowering_with_stable_hash`)
    // when every layer is canonical and nothing below is lost. Whether an op
    // is neural is not the test: RESOLVE and SPAWN have layer names, so a kb
    // or an agent decompiles to a net with a `Resolve` or `Spawn` layer and
    // nothing skipped.
    fact lowers_to(AblBytes, NetDecl);
    fact loses(AblBytes, NetDecl, "layer names: regenerated as l_<type>_<n>");
    fact loses(AblBytes, NetDecl, "the forward block, generics and composition: left empty");
    fact loses(AblBytes, NetDecl, "RES_ADD: skipped without being recorded");
    fact loses(AblBytes, NetDecl, "IDENTITY: skipped without being recorded, so an unknown layer vanishes");
    fact loses(AblBytes, NetDecl, "branches: a Par is flattened into one layer list");
    fact loses(AblBytes, NetDecl, "layer arguments that are not literals");
    fact loses(AblBytes, NetDecl, "ops with no layer name: recorded in DecompileResult::skipped");
}

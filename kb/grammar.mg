// The grammar: the productions of prototype/src/parser.rs, as facts
// (NEUROSYMBOLIC_PLAN.md Phase 2). Each `rhs(P, "…")` is one alternative of
// production `P`, in EBNF over production names and `TokenKind` names (see
// kb/lexicon.mg): juxtaposition is sequence, `|` alternation, `?` `*` `+`
// repetition, parentheses group. `recognise.rs` runs these facts directly.
//
// Written 2026-10-29 by reading parser.rs function by function (each
// production names its function and lines). Held to the parser both ways:
// every `.mg` source in the repo is recognised, this file included, and the
// grammar rejects at least 95% of the one-token deletions the parser rejects.
// And the other way: sentences sampled from these facts, written out with the
// lexicon's spellings, parse (98% at least; the rest stop in a GAP below).
//
// A quoted spelling (`'block'`) is a terminal matched by its text, whatever its
// kind: where the parser tests an identifier's spelling, the grammar says which.
//
// It is a superset of the language, never a subset, where the parser decides
// by something token kinds do not carry. Each such place is marked `GAP`:
// - layout and statement ends: the parser reads an indented body with no
//   braces, and a newline in place of `;`, from line and column;
// - lookahead the parser uses to choose (a map literal or a block, a binding
//   or an assignment, `!` as break or as not);
// - a block reference, which must name a block declared earlier.

kb Grammar {
    fact start_symbol(Module);
    // ── items ───────────────────────────────────────────────────────
    // Module: parse_module 371-387 (block-defs are ItBlockDef, recorded not emitted)
    fact instance_of(Module, Production);
    fact rhs(Module, "(ItBlockDef | Item)*");
    // ItBlockDef: parse_block_def 390-412; the lookahead is the text 'block' then an Ident.
    // 'block' is a quoted spelling (parser.rs 412: the text test), so the rule is exact.
    fact instance_of(ItBlockDef, Production);
    fact rhs(ItBlockDef, "'block' Ident (LParen (Name Comma?)* RParen)? DoComposeBody");
    // ItBlockRef: expand_block_ref 418-453 (used by net/stack bodies, owned elsewhere); only matches when the name is a defined block, written as Ident
    fact instance_of(ItBlockRef, Production);
    fact rhs(ItBlockRef, "Ident (LParen (Expr Comma?)* RParen)? Semi?");
    // Item: parse_item 457-483 + parse_item_kind 485-525 (dispatch on peek; every kind listed, reserved-keyword error arms omitted)
    fact instance_of(Item, Production);
    fact rhs(Item, "(Attr | Contract)* Plus? (FunctionDef | StructDef | EnumDef | TraitDef | ImplBlock | ModuleDef | UseDecl | TypeAlias | ConstDef | StaticDef | EffectDef | SpecDef | AgentDef | SwarmDef | NetDef | KbDef | EvolveDef | TrainDef | DataDef | ExtendBlock)");
    // Contract: parse_contract_clause 541-627. Condition is a balanced token run ended by a top-level Comma (then an optional StringLiteral message), then RParen
    fact instance_of(Contract, Production);
    fact rhs(Contract, "(KwReq | KwEns | KwInv) LParen ItContractTop* (Comma StringLiteral?)? RParen");
    // ItContractTop: parse_contract_clause 597-617: a top-level condition token, or a balanced paren group
    fact instance_of(ItContractTop, Production);
    fact rhs(ItContractTop, "(KwF | KwAf | KwUf | KwM | KwV | KwC | KwS | KwE | KwT | KwI | KwMod | KwU | KwUse | KwY | KwZ | Plus | TildePre | Question | QuestionEq | At | AtAt | AtW | KwRet | DoubleArrowR | Bang | True | False | Underscore | UnderscoreT | KwEffect | KwHandle | KwSpec | KwAgent | KwSwarm | KwExtern | KwReq | KwEns | KwInv | KwFx | KwPerf | KwUnsafe | Todo | Unimplemented | KwType | KwStatic | KwFor | KwLoop | KwBreak | KwContinue | KwOf | KwOr | KwElse | KwData | KwVal | KwVar | KwLet | KwGuard | KwDefer | KwExtend | KwIs | Pipe | KwOk | KwErr | KwSome | KwNone | KwSwarmMapReduce | KwSwarmPipeline | KwSwarmSaga | KwSwarmFanOut | KwSwarmRace | KwPipeline | KwGrammarExt | KwNet | KwLayer | KwTensor | KwParam | KwTrain | KwGrad | KwForward | KwKb | KwFact | KwRule | KwQuery | KwEvolve | KwGenome | KwMutate | KwFitness | KwSelect | KwCrossover | KwPopulation | KwGenerations | KwRl | KwPolicy | KwReward | KwPsi | KwLambda | KwPhi | KwPi | KwTheta | KwNabla | KwAlpha | KwSigma | KwKappa | KwRho | KwOmega | KwGammaGreek | KwPhiLower | KwXi | KwMu | KwChi | TensorMatmul | TensorHadamard | TensorTranspose | TensorFlatten | TensorPipeline | IntLiteral | FloatLiteral | StringLiteral | FormatString | PrintString | EprintString | CharLiteral | ByteLiteral | ByteStringLiteral | Ident | Minus | Star | Slash | Percent | Eq | Neq | Lt | Gt | Le | Ge | And | Or | Not | BitAnd | BitOr | BitXor | Shl | Shr | Assign | PlusEq | MinusEq | StarEq | SlashEq | PercentEq | BitAndEq | BitOrEq | BitXorEq | ShlEq | ShrEq | AndNot | AndTilde | PercentNot | HashTilde | TildeArrow | LBrace | RBrace | LBrack | RBrack | Semi | Dot | Colon | ColonQuestion | Arrow | FatArrow | Hash | DotDot | DotDotEq | Tilde | Dollar | ItContractGroup)");
    // ItContractGroup: parse_contract_clause 597-602: nested parens inside a condition; commas allowed at depth > 0
    fact instance_of(ItContractGroup, Production);
    fact rhs(ItContractGroup, "LParen ItContractInner* RParen");
    // ItContractInner: parse_contract_clause 597-617: a token inside a nested paren group
    fact instance_of(ItContractInner, Production);
    fact rhs(ItContractInner, "(KwF | KwAf | KwUf | KwM | KwV | KwC | KwS | KwE | KwT | KwI | KwMod | KwU | KwUse | KwY | KwZ | Plus | TildePre | Question | QuestionEq | At | AtAt | AtW | KwRet | DoubleArrowR | Bang | True | False | Underscore | UnderscoreT | KwEffect | KwHandle | KwSpec | KwAgent | KwSwarm | KwExtern | KwReq | KwEns | KwInv | KwFx | KwPerf | KwUnsafe | Todo | Unimplemented | KwType | KwStatic | KwFor | KwLoop | KwBreak | KwContinue | KwOf | KwOr | KwElse | KwData | KwVal | KwVar | KwLet | KwGuard | KwDefer | KwExtend | KwIs | Pipe | KwOk | KwErr | KwSome | KwNone | KwSwarmMapReduce | KwSwarmPipeline | KwSwarmSaga | KwSwarmFanOut | KwSwarmRace | KwPipeline | KwGrammarExt | KwNet | KwLayer | KwTensor | KwParam | KwTrain | KwGrad | KwForward | KwKb | KwFact | KwRule | KwQuery | KwEvolve | KwGenome | KwMutate | KwFitness | KwSelect | KwCrossover | KwPopulation | KwGenerations | KwRl | KwPolicy | KwReward | KwPsi | KwLambda | KwPhi | KwPi | KwTheta | KwNabla | KwAlpha | KwSigma | KwKappa | KwRho | KwOmega | KwGammaGreek | KwPhiLower | KwXi | KwMu | KwChi | TensorMatmul | TensorHadamard | TensorTranspose | TensorFlatten | TensorPipeline | IntLiteral | FloatLiteral | StringLiteral | FormatString | PrintString | EprintString | CharLiteral | ByteLiteral | ByteStringLiteral | Ident | Minus | Star | Slash | Percent | Eq | Neq | Lt | Gt | Le | Ge | And | Or | Not | BitAnd | BitOr | BitXor | Shl | Shr | Assign | PlusEq | MinusEq | StarEq | SlashEq | PercentEq | BitAndEq | BitOrEq | BitXorEq | ShlEq | ShrEq | AndNot | AndTilde | PercentNot | HashTilde | TildeArrow | LBrace | RBrace | LBrack | RBrack | Semi | Comma | Dot | Colon | ColonQuestion | Arrow | FatArrow | Hash | DotDot | DotDotEq | Tilde | Dollar | ItContractGroup)");
    // Attr: parse_attribute 631-654. Arguments run to the first RParen (nested parens are not balanced here)
    fact instance_of(Attr, Production);
    fact rhs(Attr, "At Name Bang? (LParen ItAttrArg* RParen)?");
    // ItAttrArg: parse_attribute 644-649: any token except RParen (Comma is consumed and dropped; still matched)
    fact instance_of(ItAttrArg, Production);
    fact rhs(ItAttrArg, "(KwF | KwAf | KwUf | KwM | KwV | KwC | KwS | KwE | KwT | KwI | KwMod | KwU | KwUse | KwY | KwZ | Plus | TildePre | Question | QuestionEq | At | AtAt | AtW | KwRet | DoubleArrowR | Bang | True | False | Underscore | UnderscoreT | KwEffect | KwHandle | KwSpec | KwAgent | KwSwarm | KwExtern | KwReq | KwEns | KwInv | KwFx | KwPerf | KwUnsafe | Todo | Unimplemented | KwType | KwStatic | KwFor | KwLoop | KwBreak | KwContinue | KwOf | KwOr | KwElse | KwData | KwVal | KwVar | KwLet | KwGuard | KwDefer | KwExtend | KwIs | Pipe | KwOk | KwErr | KwSome | KwNone | KwSwarmMapReduce | KwSwarmPipeline | KwSwarmSaga | KwSwarmFanOut | KwSwarmRace | KwPipeline | KwGrammarExt | KwNet | KwLayer | KwTensor | KwParam | KwTrain | KwGrad | KwForward | KwKb | KwFact | KwRule | KwQuery | KwEvolve | KwGenome | KwMutate | KwFitness | KwSelect | KwCrossover | KwPopulation | KwGenerations | KwRl | KwPolicy | KwReward | KwPsi | KwLambda | KwPhi | KwPi | KwTheta | KwNabla | KwAlpha | KwSigma | KwKappa | KwRho | KwOmega | KwGammaGreek | KwPhiLower | KwXi | KwMu | KwChi | TensorMatmul | TensorHadamard | TensorTranspose | TensorFlatten | TensorPipeline | IntLiteral | FloatLiteral | StringLiteral | FormatString | PrintString | EprintString | CharLiteral | ByteLiteral | ByteStringLiteral | Ident | Minus | Star | Slash | Percent | Eq | Neq | Lt | Gt | Le | Ge | And | Or | Not | BitAnd | BitOr | BitXor | Shl | Shr | Assign | PlusEq | MinusEq | StarEq | SlashEq | PercentEq | BitAndEq | BitOrEq | BitXorEq | ShlEq | ShrEq | AndNot | AndTilde | PercentNot | HashTilde | TildeArrow | LParen | LBrace | RBrace | LBrack | RBrack | Semi | Comma | Dot | Colon | ColonQuestion | Arrow | FatArrow | Hash | DotDot | DotDotEq | Tilde | Dollar)");
    // Name: expect_ident 245-330 (Ident plus the keywords expect_ident accepts as names)
    fact instance_of(Name, Production);
    fact rhs(Name, "Ident | KwF | KwM | KwV | KwC | KwS | KwE | KwT | KwI | KwMod | KwU | KwUse | KwNet | KwLayer | KwTensor | KwParam | KwTrain | KwGrad | KwForward | KwKb | KwFact | KwRule | KwQuery | KwEvolve | KwGenome | KwMutate | KwFitness | KwSelect | KwCrossover | KwPopulation | KwGenerations | KwRl | KwPolicy | KwReward | KwData | KwVal | KwVar | KwGuard | KwDefer | KwExtend | KwIs | KwHandle | KwOk | KwErr | KwSome | KwNone | KwAf | KwUf | KwAgent | KwSwarm");
    // EffectName: expect_effect_name 343-358 (parser.rs 358): an Ident, or a token whose text is
    // identifier-shaped. Keywords are listed by kind. if, match, for, while and where (kinds that also
    // have symbol spellings) and pub (Plus) are quoted spellings, so the rule is exact.
    fact instance_of(EffectName, Production);
    fact rhs(EffectName, "Name | KwY | KwZ | 'if' | 'match' | 'for' | KwOf | 'while' | KwElse | 'where' | KwRet | KwLoop | KwBreak | KwContinue | KwEffect | KwSpec | KwExtern | KwUnsafe | KwType | KwStatic | KwOr | KwSwarmMapReduce | KwSwarmPipeline | KwSwarmSaga | KwSwarmFanOut | KwSwarmRace | KwPipeline | KwGrammarExt | KwLet | Underscore | UnderscoreT | 'pub'");
    // Path: not a parser function: a dotted name as items/types use it (use-paths themselves are UseDecl, parse_use_decl 1037-1100)
    // FunctionDef: parse_function_def 658-806 (keyword consumed inside; Params includes its parentheses, per table convention)
    fact instance_of(FunctionDef, Production);
    fact rhs(FunctionDef, "(KwF | KwAf | KwUf) Name Generics? Params (Arrow Type (KwOr Type)?)? WhereClause? (Slash EffectName EffectArgs? ((Comma | Plus) EffectName EffectArgs?)*)? (Assign Expr Semi? | Semi | Block)");
    // EffectArgs: parse_function_def 710-724 and 728-742: bracketed type list after an effect name; closer depends on opener
    fact instance_of(EffectArgs, Production);
    fact rhs(EffectArgs, "(LBrack ItTypeList RBrack | Lt ItTypeList Gt)");
    // ItTypeList: parse_function_def 717-722 and 735-740: comma-separated types, optional trailing comma
    fact instance_of(ItTypeList, Production);
    fact rhs(ItTypeList, "(Type Comma?)*");
    // WhereClause: parse_where_clause 1198-1221
    fact instance_of(WhereClause, Production);
    fact rhs(WhereClause, "TildeArrow ItWherePred (Comma ItWherePred)*");
    // ItWherePred: parse_where_clause 1202-1213
    fact instance_of(ItWherePred, Production);
    fact rhs(ItWherePred, "Name Colon Name (Plus Name)*");
    // StructDef: parse_struct_def 810-852 (contracts are carried by Item, not written here)
    fact instance_of(StructDef, Production);
    fact rhs(StructDef, "KwS Name Generics? LBrace (Plus? Name Colon Type Comma?)* RBrace");
    // EnumDef: parse_enum_def 856-919
    fact instance_of(EnumDef, Production);
    fact rhs(EnumDef, "KwE Name Generics? LBrace ItEnumVariant* RBrace");
    // ItEnumVariant: parse_enum_def 868-911: tuple, struct or unit variant, optional comma
    fact instance_of(ItEnumVariant, Production);
    fact rhs(ItEnumVariant, "Name (LParen (Type Comma?)* RParen | LBrace (Name Colon Type Comma?)* RBrace)? Comma?");
    // TraitDef: parse_trait_def 923-959
    fact instance_of(TraitDef, Production);
    fact rhs(TraitDef, "KwT Name Generics? (Colon Name (Plus Name)*)? LBrace Item* RBrace");
    // ImplBlock: parse_impl_block 963-1010: `impl Trait for Type`; the for token is KwFor or At
    fact instance_of(ImplBlock, Production);
    // GAP: the parser commits to Generics on `[` or `<` after `I`; the grammar may
    // also read that bracket as the start of the Type (`I [!] ...`). Superset.
    fact rhs(ImplBlock, "KwI Generics? Type ((KwFor | At) Type)? LBrace Item* RBrace");
    // ModuleDef: parse_module_def 1014-1033
    fact instance_of(ModuleDef, Production);
    fact rhs(ModuleDef, "KwMod Name (Semi | LBrace Item* RBrace)");
    // UseDecl: parse_use_decl 1037-1100. A group entry may carry `as`, matched by text (peek_text),
    // written as the quoted spelling 'as'.
    fact instance_of(UseDecl, Production);
    fact rhs(UseDecl, "KwUse Name (Dot Name)* Semi?");
    fact rhs(UseDecl, "KwUse Name (Dot Name)* Dot Star Semi?");
    fact rhs(UseDecl, "KwUse Name (Dot Name)* Dot LBrace (Name ('as' Name)? Comma?)* RBrace Semi?");
    // TypeAlias: parse_type_alias 1104-1151. First token is consumed without a check; dispatch restricts it to KwType or KwY
    fact instance_of(TypeAlias, Production);
    fact rhs(TypeAlias, "(KwType | KwY) Name Generics? (Assign Type)? (TildeArrow ItRefineTok*)? Semi");
    // ItRefineTok: parse_type_alias 1134-1137: any token up to Semi
    fact instance_of(ItRefineTok, Production);
    fact rhs(ItRefineTok, "(KwF | KwAf | KwUf | KwM | KwV | KwC | KwS | KwE | KwT | KwI | KwMod | KwU | KwUse | KwY | KwZ | Plus | TildePre | Question | QuestionEq | At | AtAt | AtW | KwRet | DoubleArrowR | Bang | True | False | Underscore | UnderscoreT | KwEffect | KwHandle | KwSpec | KwAgent | KwSwarm | KwExtern | KwReq | KwEns | KwInv | KwFx | KwPerf | KwUnsafe | Todo | Unimplemented | KwType | KwStatic | KwFor | KwLoop | KwBreak | KwContinue | KwOf | KwOr | KwElse | KwData | KwVal | KwVar | KwLet | KwGuard | KwDefer | KwExtend | KwIs | Pipe | KwOk | KwErr | KwSome | KwNone | KwSwarmMapReduce | KwSwarmPipeline | KwSwarmSaga | KwSwarmFanOut | KwSwarmRace | KwPipeline | KwGrammarExt | KwNet | KwLayer | KwTensor | KwParam | KwTrain | KwGrad | KwForward | KwKb | KwFact | KwRule | KwQuery | KwEvolve | KwGenome | KwMutate | KwFitness | KwSelect | KwCrossover | KwPopulation | KwGenerations | KwRl | KwPolicy | KwReward | KwPsi | KwLambda | KwPhi | KwPi | KwTheta | KwNabla | KwAlpha | KwSigma | KwKappa | KwRho | KwOmega | KwGammaGreek | KwPhiLower | KwXi | KwMu | KwChi | TensorMatmul | TensorHadamard | TensorTranspose | TensorFlatten | TensorPipeline | IntLiteral | FloatLiteral | StringLiteral | FormatString | PrintString | EprintString | CharLiteral | ByteLiteral | ByteStringLiteral | Ident | Minus | Star | Slash | Percent | Eq | Neq | Lt | Gt | Le | Ge | And | Or | Not | BitAnd | BitOr | BitXor | Shl | Shr | Assign | PlusEq | MinusEq | StarEq | SlashEq | PercentEq | BitAndEq | BitOrEq | BitXorEq | ShlEq | ShrEq | AndNot | AndTilde | PercentNot | HashTilde | TildeArrow | LParen | RParen | LBrace | RBrace | LBrack | RBrack | Comma | Dot | Colon | ColonQuestion | Arrow | FatArrow | Hash | DotDot | DotDotEq | Tilde | Dollar)");
    // ConstDef: parse_const_def 1155-1165
    fact instance_of(ConstDef, Production);
    fact rhs(ConstDef, "KwC Name Colon Type Assign Expr Semi");
    // StaticDef: parse_static_def 1169-1194 (KwStatic or KwZ, then optional KwM)
    fact instance_of(StaticDef, Production);
    fact rhs(StaticDef, "(KwStatic | KwZ) KwM? Name Colon Type Assign Expr Semi");

    // ── params ──────────────────────────────────────────────────────
    // ── params section: DataDef, ExtendBlock, Generics, Params, ClosureParams ──
    // Helper productions carry the Pa prefix. PaUnit and PaStringList are the
    // helpers for parse_unit and parse_bracket_string_list.
    // DataDef: parser.rs parse_data_def (starts 2309)
    // Three shapes: record `data N(f: T, ...)`, sum `data N = V1 | V2(T)`,
    // and empty record `data N`. Generics are optional in all three.
    fact instance_of(DataDef, Production);
    fact rhs(DataDef, "KwData Name Generics? LParen (PaDataField Comma?)* RParen");
    fact rhs(DataDef, "KwData Name Generics? Assign PaDataVariant (BitOr PaDataVariant)*");
    fact rhs(DataDef, "KwData Name Generics?");
    // DataField helper: parse_data_def record loop (2309+): optional `pub` (Plus),
    // name, colon, type, optional `= expr` default.
    fact instance_of(PaDataField, Production);
    fact rhs(PaDataField, "Plus? Name Colon Type (Assign Expr)?");
    // DataVariant helper: parse_data_def sum loop: name and optional tuple of types.
    fact instance_of(PaDataVariant, Production);
    fact rhs(PaDataVariant, "Name (LParen (Type Comma?)* RParen)?");
    // ExtendBlock: parser.rs parse_extend_block (starts 2405)
    fact instance_of(ExtendBlock, Production);
    fact rhs(ExtendBlock, "KwExtend Type LBrace Item* RBrace");
    // PaStringList: parser.rs parse_bracket_string_list (starts 2445)
    // `[ident, ident, ...]` used by agent capabilities / requires_approval.
    // Generics: parser.rs parse_generic_params (starts 2461)
    // Opener and closer must match: `[ ... ]` or `< ... >`.
    fact instance_of(Generics, Production);
    fact rhs(Generics, "LBrack PaGenericList RBrack");
    fact rhs(Generics, "Lt PaGenericList Gt");
    // PaGenericList: the loop body. Commas between params are optional, as in
    // the parser (it only consumes a comma if present).
    fact instance_of(PaGenericList, Production);
    fact rhs(PaGenericList, "(PaGenericParam Comma?)*");
    // PaGenericParam: name, optional `: Bound (+ Bound)*`, optional `= Type` default.
    fact instance_of(PaGenericParam, Production);
    fact rhs(PaGenericParam, "Name (Colon Name (Plus Name)*)? (Assign Type)?");
    // Params: parser.rs parse_param_list (starts 2512). The parentheses belong
    // to Params: every call site (e.g. parser.rs 674-676, 1285-1287, 1319-1323,
    // 1889-1891) does expect(LParen), parse_param_list, expect(RParen).
    fact instance_of(Params, Production);
    fact rhs(Params, "LParen (PaParam Comma?)* RParen");
    // PaParam: one parameter. Optional default `= expr` applies to every form.
    fact instance_of(PaParam, Production);
    fact rhs(PaParam, "PaParamHead (Assign Expr)?");
    // PaParamHead: receiver shorthands, then the plain `name (: Type)?` form.
    // The receiver forms are `&self`, `&!self` and `!self`. Bare `self` (by value)
    // is the same token shape as the plain form, so the plain alternative covers
    // it (the parser maps it to Self; that is a type-checker concern).
    fact instance_of(PaParamHead, Production);
    // The receiver arms test the text `self` (parser.rs 2698-2760): `&self`, `&!self` and `!self`
    // are written with the quoted spelling 'self', so they are exact. Bare `self` is the plain form.
    fact rhs(PaParamHead, "BitAnd 'self'");
    fact rhs(PaParamHead, "AndNot 'self'");
    fact rhs(PaParamHead, "Bang 'self'");
    // Plain parameter: name, optional `: Type`. No annotation means Type::Inferred.
    fact rhs(PaParamHead, "Name (Colon Type)?");
    // ClosureParams: parser.rs parse_closure_params_until_pipe (starts 2626), and
    // the call site in the `|` / `||` closure arm (about line 4488-4497). This
    // production INCLUDES the pipes: the `|` (BitOr) opens it and closes it, and
    // `||` (Or) is the zero-parameter form. The parser's loop is
    // `while peek != BitOr`, so `| |` is also zero parameters; that is covered
    // by the `BitOr PaClosureList BitOr` alternative with an empty list.
    fact instance_of(ClosureParams, Production);
    fact rhs(ClosureParams, "BitOr PaClosureList BitOr");
    fact rhs(ClosureParams, "Or");
    // PaClosureList: the loop body shared by pipe-delimited closures.
    fact instance_of(PaClosureList, Production);
    fact rhs(PaClosureList, "(PaClosureParam Comma?)*");
    // PaClosureParam: a closure parameter: name with optional type annotation.
    // No default value (parser passes `default: None`).
    fact instance_of(PaClosureParam, Production);
    fact rhs(PaClosureParam, "Name (Colon Type)?");
    // PaClosureParenList: parser.rs parse_closure_param_list (starts 2644): the
    // `fn(params) => ...` closure form. The caller (the `KwF LParen` closure arm,
    // about line 4515-4522) consumes `fn (` and then `)`. This helper is the list
    // between them, so the closure owner writes `KwF LParen PaClosureParenList RParen`.
    // PaUnit: parser.rs parse_unit (starts 2670), the `[unit]` part of a
    // dimensioned float type (`f32[m/s]`). Terms are joined by `*` or `/`.
    // PaUnitTerm: a bare `1`, or a unit name with an optional `^exp` where exp is
    // an optional minus followed by an integer.
    // PaUnitOne: the literal `1`. The parser checks that the IntLiteral text is "1".
    // The literal `1` is the quoted spelling '1' (parser.rs 2865: IntLiteral with text "1"). A token's
    // text is its source slice, so only an IntLiteral can have text 1: the rule is exact.
    // The exponent is not allowed on `1`: PaUnitTerm puts the exponent only on units.
    // PaUnitName: the parser accepts any token whose text is all ASCII letters.
    // That is Ident plus every keyword whose spelling is alphabetic (the KEYWORDS
    // table in lexer.rs). Enumerated here from that table, so `m`, `v`, `C`, `S`,
    // `T`, `Y`, `Z`, `I`, `U`, `u` and friends (lexed as keywords) are units too.

    // ── statements ──────────────────────────────────────────────────
    // ── Statements and expressions (section: statements) ──────────────
    // Transcribed from prototype/src/parser.rs. Precedence ladder read from
    // parse_expr_bp's infix table (l_bp/r_bp): each binary level's right
    // operand is the next tighter level. Postfix forms (`?`, `.f`, `[i]`,
    // `(args)`, `is`, `as`, `⊤`, `⊥`) bind tighter than every infix operator
    // because the postfix loop does not look at min_bp.
    // Layout (offside-rule) blocks are omitted per brief; newline-sensitivity
    // is ignored throughout.
    // ── Block ───────────────────────────────────────────────
    // Block: parser.rs parse_block (3135-3154), braced path only
    fact instance_of(Block, Production);
    fact rhs(Block, "LBrace BlockBody RBrace");
    // BlockBody: parser.rs parse_block_body (3172-3242), layout_col = None
    // A bare expression (no `;`) is accepted as a statement anywhere, and as the
    // tail only when it is the last thing before `}`.
    fact instance_of(BlockBody, Production);
    fact rhs(BlockBody, "Stmt* Expr?");
    // Stmt: parser.rs parse_block_body match arms (3176-3238)
    // bindings (StLet), guard (when guard_starts_a_statement), defer,
    // a named nested fn (KwF/KwAf/KwUf before Ident), otherwise an expression
    // with or without a trailing `;`. A lone `;` is not a statement.
    fact instance_of(Stmt, Production);
    fact rhs(Stmt, "StLet");
    fact rhs(Stmt, "StGuard");
    fact rhs(Stmt, "StDefer");
    fact rhs(Stmt, "FunctionDef");
    fact rhs(Stmt, "Expr Semi");
    fact rhs(Stmt, "Expr");
    // ── Bindings ────────────────────────────────────────────
    // StLet: parser.rs is_let_statement (3305-3343), parse_let_stmt (3430-3466),
    // expect_stmt_end (3484-3500)
    // The binding keyword is followed by a pattern (parse_pattern), an optional
    // `: Type`, `=`, an expression, and an optional `;` (expect_stmt_end is
    // `Semi?` here: a newline or `}` also ends it, so the terminator is optional).
    // GAP: parse_let_stmt requires the first pattern token to be a binding name
    // (is_binding_name, 3351-3380: Ident, Underscore, and many keyword tokens
    // such as KwVal, KwGuard, KwData, KwFitness) or `(`/`[`. The Pattern section
    // must accept the keyword-shaped binding names. The parser also refuses
    // `(`/`[` after KwV/KwM (is_let_statement 3328-3330: `v(x) = …` is an
    // assignment, not a binder). Expressed here as a superset: a `(`/`[` pattern
    // after KwV/KwM is accepted by this rule and by the expression path.
    fact instance_of(StLet, Production);
    fact rhs(StLet, "StLetKw Pattern (Colon Type)? Assign Expr Semi?");
    // StLetKw: lexer tokens for `v`/`m`/`val`/`var` (parse_let_stmt 3435)
    fact instance_of(StLetKw, Production);
    fact rhs(StLetKw, "KwV | KwM | KwVal | KwVar");
    // ── Guard and defer ─────────────────────────────────────
    // StGuard: parser.rs parse_guard_stmt (2419-2431)
    // The condition is followed by `else`, `or`, or `:`, then a block.
    // GAP: the statement is chosen only when guard_starts_a_statement (3860-3889)
    // holds, i.e. the next token is not a binary operator, `.`, `=`, `;`, `,`,
    // a closer, EOF, or on a new line. Expressed here as a superset. Overlap
    // with the expression reading of `guard` as a name is harmless for
    // acceptance (both paths accept the valid programs).
    fact instance_of(StGuard, Production);
    fact rhs(StGuard, "KwGuard Expr (KwElse | KwOr | Colon) Block");
    // StDefer: parser.rs parse_defer_stmt (2435-2442)
    fact instance_of(StDefer, Production);
    fact rhs(StDefer, "KwDefer Expr Semi?");
    // ── Expression: assignment and binary ladder ────────────
    // Expr: parser.rs parse_expr (3504-3506) = parse_expr_bp(0)
    fact instance_of(Expr, Production);
    fact rhs(Expr, "StAssign");
    // StAssign: parser.rs parse_expr_bp infix (3732-3737, 3815-3823)
    // Assignment is l_bp 1, r_bp 2: the right side is the level below
    // assignment (StOr). The loop is left-associative, so `a = b = c` parses.
    // The left side is any expression (the parser does not check lvalues).
    fact instance_of(StAssign, Production);
    fact rhs(StAssign, "StOr");
    fact rhs(StAssign, "StAssign StAssignOp StOr");
    // StAssignOp: parser.rs 3732-3737. `&=`, `|=`, `^=`, `<<=` etc. are not in the table.
    fact instance_of(StAssignOp, Production);
    fact rhs(StAssignOp, "Assign | PlusEq | MinusEq | StarEq | SlashEq | PercentEq");
    // StOr: parser.rs 3738 (Or, 3-4), 3759-3760 (TensorPipeline, Pipe, 3-4)
    // `||`, `|>` and `▸` share one level and are left-associative.
    fact instance_of(StOr, Production);
    fact rhs(StOr, "StAnd");
    fact rhs(StOr, "StOr (Or | Pipe | TensorPipeline) StAnd");
    // StAnd: parser.rs 3739 (And, 5-6)
    fact instance_of(StAnd, Production);
    fact rhs(StAnd, "StEq");
    fact rhs(StAnd, "StAnd And StEq");
    // StEq: parser.rs 3740-3741 (Eq, Neq, 7-8)
    fact instance_of(StEq, Production);
    fact rhs(StEq, "StRel");
    fact rhs(StEq, "StEq (Eq | Neq) StRel");
    // StRel: parser.rs 3742-3745 (Lt, Gt, Le, Ge, 9-10)
    fact instance_of(StRel, Production);
    fact rhs(StRel, "StBitOr");
    fact rhs(StRel, "StRel (Lt | Gt | Le | Ge) StBitOr");
    // StBitOr: parser.rs 3746 (BitOr, 11-12)
    fact instance_of(StBitOr, Production);
    fact rhs(StBitOr, "StBitXor");
    fact rhs(StBitOr, "StBitOr BitOr StBitXor");
    // StBitXor: parser.rs 3747 (BitXor, 13-14)
    fact instance_of(StBitXor, Production);
    fact rhs(StBitXor, "StBitAnd");
    fact rhs(StBitXor, "StBitXor BitXor StBitAnd");
    // StBitAnd: parser.rs 3748 (BitAnd, 15-16)
    fact instance_of(StBitAnd, Production);
    fact rhs(StBitAnd, "StShift");
    fact rhs(StBitAnd, "StBitAnd BitAnd StShift");
    // StShift: parser.rs 3749-3750 (Shl, Shr, 17-18)
    fact instance_of(StShift, Production);
    fact rhs(StShift, "StAdd");
    fact rhs(StShift, "StShift (Shl | Shr) StAdd");
    // StAdd: parser.rs 3751-3752 (Plus, Minus, 19-20)
    // GAP: `Plus` is also the `pub` sigil; parse_expr_bp (3777-3801) stops at a
    // `+` that starts a line before a declaration keyword. Not expressed; the
    // superset accepts `a + f …` across a newline.
    fact instance_of(StAdd, Production);
    fact rhs(StAdd, "StMul");
    fact rhs(StAdd, "StAdd (Plus | Minus) StMul");
    // StMul: parser.rs 3753-3755 (Star, Slash, Percent, 21-22)
    fact instance_of(StMul, Production);
    fact rhs(StMul, "StTensor");
    fact rhs(StMul, "StMul (Star | Slash | Percent) StTensor");
    // StTensor: parser.rs 3757-3758 (TensorMatmul, TensorHadamard, 23-24).
    // The right side is parse_expr_bp(24), which admits no infix operator,
    // so it is a postfix expression.
    fact instance_of(StTensor, Production);
    fact rhs(StTensor, "StPostfix");
    fact rhs(StTensor, "StTensor (TensorMatmul | TensorHadamard) StPostfix");
    // ── Postfix layer (applied in the parse_expr_bp loop, 3511-3728) ──
    // StPostfix: parser.rs parse_expr_bp loop over parse_prefix_expr (3509, 3511-3728).
    // Postfix forms are applied at any min_bp, so they bind tighter than any
    // infix operator. The prefix operand itself is PrefixExpr (prefix section).
    fact instance_of(StPostfix, Production);
    fact rhs(StPostfix, "PrefixExpr");
    fact rhs(StPostfix, "StPostfix StPostfixOp");
    // StPostfixOp: parser.rs 3513-3726
    //   Question        try operator (3514-3543); the newline and control-flow exclusions are ignored
    //   TensorTranspose TensorFlatten  unary postfix ⊤ ⊥ (3545-3560)
    //   Dot Name ...    field access, `.await`, method call with optional turbofish (3561-3621)
    //   LBrack ...      index or range slice (3622-3670)
    //   StArgs          call on the left (3671-3706)
    //   KwIs Pattern    pattern test (3707-3715)
    //   'as' Type       `expr as Type` (3718-3726, 3930-3938)
    // `as` is an Ident whose text is `as` (parser.rs 3930, a text test), so the quoted spelling 'as' is exact.
    fact instance_of(StPostfixOp, Production);
    fact rhs(StPostfixOp, "Question");
    fact rhs(StPostfixOp, "TensorTranspose");
    fact rhs(StPostfixOp, "TensorFlatten");
    fact rhs(StPostfixOp, "Dot Name StTurbofish? StArgs?");
    fact rhs(StPostfixOp, "LBrack StIndex RBrack");
    fact rhs(StPostfixOp, "StArgs");
    fact rhs(StPostfixOp, "KwIs Pattern");
    fact rhs(StPostfixOp, "'as' Type");
    // StTurbofish: parser.rs 3575-3589 (`::<` Type, … `>`; the parser checks Colon Colon Lt)
    fact instance_of(StTurbofish, Production);
    fact rhs(StTurbofish, "Colon Colon Lt (Type Comma?)* Gt");
    // StArgs: parser.rs 3592-3599 (method call) and 3693-3700 (call). The parser
    // accepts Comma-free adjacent arguments; the Comma? keeps that.
    fact instance_of(StArgs, Production);
    fact rhs(StArgs, "LParen (Expr Comma?)* RParen");
    // StIndex: parser.rs 3636-3663 (index contents)
    // `start` is required unless the index begins with `..`/`..=`. The end is
    // optional. A bare start (no range) is the index itself.
    fact instance_of(StIndex, Production);
    fact rhs(StIndex, "Expr (StRangeOp Expr?)?");
    fact rhs(StIndex, "StRangeOp Expr?");
    // StRangeOp: parser.rs 3639, 3644
    fact instance_of(StRangeOp, Production);
    fact rhs(StRangeOp, "DotDot | DotDotEq");
    // StUnaryOperand: parser.rs parse_unary_operand (3849-3852), i.e.
    // parse_expr_bp(25). Prefix operators `-`, `!`, `*`, `&`, `&!` take this
    // operand: a postfix chain with no infix operator, so `-x * y` is `(-x) * y`.
    // Used by the prefix section's unary arms (4027-4062).
    // StBreakBang: parser.rs bang_is_break (3897-3911) and the prefix arm at 3992.
    // A `Bang` that cannot take an operand is `break`; otherwise it is logical not.
    // GAP: the lookahead (next token is Semi, RBrace, Comma, RParen, RBrack, Eof,
    // or on a new line) is not expressible. The rule accepts a bare `Bang` in any
    // position where a statement or expression may begin, which is a superset.
    // StMapLit: parser.rs is_map_literal (3253-3303) and the map arm at 4365-4379.
    // The `{` arm of PrefixExpr is either StMapLit or Block (prefix section).
    // is_map_literal is a lookahead (empty braces, or a depth-0 `:` before any `;`,
    // `=>` or closing `}`). GAP: that lookahead is not expressible. The rule is the
    // union of the map shape and Block, and both alternatives appear in the `{` arm.
    // Map entries are Expr Colon Expr with an optional Comma.
    // Expr alternatives are the top of the ladder above.
    // NOT DEFINED (error-only or lookahead-only):
    //  - macro_call_ahead (parser.rs 3923-3941): `Ident!(` adjacency always
    //    returns an error (3966-3978). Macros are rejected; the grammar accepts
    //    `f!(x)` only as two juxtaposed statements (`f` then `!(x)`). Adjacency
    //    is not expressible in token kinds.
    //  - at_block_end (3158-3167): layout (Some(col)) branch omitted; braced
    //    branch is RBrace, already in Block.
    //  - newline_before_current, expect_stmt_end's newline branch: folded into
    //    `Semi?`.
    //  - parse_block layout path (3147-3150) and the `unreachable` error: omitted.
    //  - keyword_as_binding_name_error, the KwLet rejection (3182-3192): error-only.
    //
    // Outside this section (noted for the prefix/control-flow owners):
    //  - for-loop range `a..b` (parser.rs 4320-4321) and pattern range (4802) are not
    //    Pratt operators; they belong to the for/match productions.
    //  - `KwRet`, `KwBreak`, `KwGrad`/`KwNabla`, unary arms: PrefixExpr.

    // ── prefix ──────────────────────────────────────────────────────
    // ── prefix section: parse_prefix_expr (parser.rs 3943-4703) and parse_pattern (4704-4861) ──
    // Postfix forms (?, .name, .name(args), (args), [i], `is Pattern`, `as Type`, the tensor
    // postfixes) are NOT in parse_prefix_expr. They are applied by parse_expr_bp's loop
    // (parser.rs 3508-3600+). PxOperand references the Postfix production, owned by the
    // statements/Expr section.
    // PrefixExpr: parser.rs parse_prefix_expr (3943-4703): the match on self.peek()
    // Arm ranges: Bang 3992, KwGrad/KwNabla 4006, Minus/Bang/Star 4027, BitAnd 4040,
    // AndNot 4055, KwRet 4065, KwBreak 4087, KwContinue/Shr 4111, Todo 4117, Unimplemented 4121,
    // QuestionEq 4127, Question 4149, At 4258, AtW 4341, AtAt/KwLoop 4352, LBrace-map 4365,
    // LBrace 4382, KwHandle-brace 4400, identifier group 4404, sum ctors 4451, BitOr/Or 4488,
    // KwF closure 4513, LBrack 4544, LParen 4577, PrintString 4626, literals 4641-4666,
    // Ident/KwAf/KwUf 4671, Underscore 4681, UnderscoreT 4687. Errors/reserved words 3952, 3960s.
    fact instance_of(PrefixExpr, Production);
    // Bang arm (guard bang_is_break, 3992): `!` with no operand is break. GAP-superset: the
    // guard is a lookahead (next token cannot start an Expr), so the grammar allows bare Bang.
    fact rhs(PrefixExpr, "Bang");
    fact rhs(PrefixExpr, "PxGrad");
    fact rhs(PrefixExpr, "PxUnary");
    // KwRet 4065-4084: value is optional when the next token cannot start an expression.
    fact rhs(PrefixExpr, "KwRet Expr?");
    // KwBreak 4087-4109: same optional-value rule.
    fact rhs(PrefixExpr, "KwBreak Expr?");
    // KwContinue | Shr (>>) 4111-4115.
    fact rhs(PrefixExpr, "KwContinue");
    fact rhs(PrefixExpr, "Shr");
    // Todo 4117, Unimplemented 4121.
    fact rhs(PrefixExpr, "Todo");
    fact rhs(PrefixExpr, "Unimplemented");
    // QuestionEq 4127-4147 (match with scrutinee).
    fact rhs(PrefixExpr, "PxMatchScrut");
    // Question 4149-4250: if and match forms; sum-type sugar branch (peek_n(1) in KwSome/KwNone/KwOk/KwErr) at 4153-4174.
    fact rhs(PrefixExpr, "PxIf");
    fact rhs(PrefixExpr, "Question PxSumCtor");
    // At 4258-4339: struct literal and path forms; for-loop falls through to PxFor (4321-4339).
    fact rhs(PrefixExpr, "PxAtStruct");
    fact rhs(PrefixExpr, "PxAtPath");
    fact rhs(PrefixExpr, "PxFor");
    // AtW 4341-4350 (while).
    fact rhs(PrefixExpr, "AtW Expr Block");
    // AtAt | KwLoop 4352-4357.
    fact rhs(PrefixExpr, "AtAt Block");
    fact rhs(PrefixExpr, "KwLoop Block");
    // LBrace if is_map_literal (4365-4380). GAP-superset: is_map_literal is a lookahead; the grammar
    // accepts the map form and the Block form below wherever both could apply. Entries loop has no separator
    // requirement, so Comma is optional between entries.
    fact rhs(PrefixExpr, "LBrace (Expr Colon Expr Comma?)* RBrace");
    // LBrace 4382-4386 (block expression).
    fact rhs(PrefixExpr, "Block");
    // KwHandle when followed by LBrace (4400-4403) delegates to parse_handle_expr (HandleExpr, owned by effects).
    // GAP-superset: the identifier alternative below also accepts KwHandle without the brace lookahead.
    fact rhs(PrefixExpr, "HandleExpr");
    // Identifier group 4404-4446: keyword tokens used as plain identifiers.
    fact rhs(PrefixExpr, "PxNameKw");
    // Ident | KwAf | KwUf 4671-4680.
    fact rhs(PrefixExpr, "Ident");
    fact rhs(PrefixExpr, "KwAf");
    fact rhs(PrefixExpr, "KwUf");
    // Underscore 4681-4686, UnderscoreT 4687-4693 (produces the name "_T").
    fact rhs(PrefixExpr, "Underscore");
    fact rhs(PrefixExpr, "UnderscoreT");
    // Sum-type constructors 4451-4471.
    fact rhs(PrefixExpr, "PxSumCtor");
    // BitOr | Or closures 4488-4511.
    fact rhs(PrefixExpr, "PxClosure");
    // KwF followed by LParen 4513-4542. Any other KwF is a parse error (4694 fallback).
    fact rhs(PrefixExpr, "PxFnClosure");
    // LBrack 4544-4575.
    fact rhs(PrefixExpr, "PxArrayLit");
    // LParen 4577-4604.
    fact rhs(PrefixExpr, "PxTuple");
    // Literals 4606-4666. IntLiteral|FloatLiteral 4606, StringLiteral|FormatString 4641, CharLiteral 4652, True|False 4659.
    fact rhs(PrefixExpr, "IntLiteral | FloatLiteral | StringLiteral | FormatString | CharLiteral | True | False");
    // PrintString|EprintString 4626-4640: the parser builds a call to println or eprintln around a FormatString literal.
    fact rhs(PrefixExpr, "PrintString | EprintString");
    // PxGrad: parser.rs 4006-4025 (KwGrad|KwNabla followed by LParen; the peek_n(1) guard is exact here since LParen is in the production).
    fact instance_of(PxGrad, Production);
    fact rhs(PxGrad, "(KwGrad | KwNabla) LParen Expr Comma Expr RParen");
    // PxUnary: parser.rs 4027-4036 (Minus|Bang|Star), 4040-4049 (BitAnd), 4055-4063 (AndNot).
    fact instance_of(PxUnary, Production);
    fact rhs(PxUnary, "(Minus | Bang | Star | BitAnd | AndNot) PxOperand");
    // PxOperand: parser.rs parse_unary_operand (3849-3852) = parse_expr_bp(25). The prefix atom followed by
    // postfix forms (Postfix is owned by Expr: parser.rs 3508-3600+). Every infix operator binds at 24 or lower
    // (table 3732-3760), so none attaches to a unary operand.
    fact instance_of(PxOperand, Production);
    fact rhs(PxOperand, "PrefixExpr StPostfixOp*");
    // PxMatchScrut: parser.rs 4127-4147 (QuestionEq Expr { arms }).
    fact instance_of(PxMatchScrut, Production);
    fact rhs(PxMatchScrut, "QuestionEq Expr LBrace PxMatchArms RBrace");
    // PxMatchArms: parser.rs 4132-4141 and 4153-4174 region (arms loop; the trailing comma is optional).
    fact instance_of(PxMatchArms, Production);
    fact rhs(PxMatchArms, "(Pattern FatArrow Expr Comma?)*");
    // PxIf: parser.rs 4149-4250 (Question arm, general branch).
    //   form 1 (4150-4161): ? { arms }                                   match, no scrutinee
    //   form 2 (4199-4215): ? Expr { arms }                              match with scrutinee (is_match_arm_body lookahead)
    //   form 3 (4217-4250): ? Expr Block (else-part)?                   if; else-part below
    // GAP-superset: is_match_arm_body decides between forms 2 and 3 by scanning for => at depth 1; the grammar takes the union.
    fact instance_of(PxIf, Production);
    fact rhs(PxIf, "Question LBrace PxMatchArms RBrace");
    fact rhs(PxIf, "Question Expr LBrace PxMatchArms RBrace");
    fact rhs(PxIf, "Question Expr Block PxElse?");
    // PxElse: parser.rs 4222-4246. The separator is Colon, KwOr or KwElse. After it, a Question begins
    // an else-if (parse_prefix_expr, which may also be sum-type sugar); otherwise a Block.
    fact instance_of(PxElse, Production);
    fact rhs(PxElse, "(Colon | KwOr | KwElse) (PxIf | Question PxSumCtor | Block)");
    // PxSumCtor: parser.rs 4451-4471 (expression form) and the sugar at 4153-4174 (same argument loop).
    fact instance_of(PxSumCtor, Production);
    fact rhs(PxSumCtor, "(KwSome | KwNone | KwOk | KwErr) (LParen (Expr Comma?)* RParen)?");
    // PxAtStruct: parser.rs 4258-4306. Lookahead: At Ident then LBrace or Dot. The path loop consumes Dot Ident
    // segments; struct form ends in LBrace. Field loop: Name, optional colon Expr, optional comma (4292-4304).
    fact instance_of(PxAtStruct, Production);
    fact rhs(PxAtStruct, "At Ident (Dot Ident)* LBrace PxFieldInit* RBrace");
    // PxFieldInit: parser.rs 4283-4304. Field name is expect_ident, so keyword names are allowed.
    fact instance_of(PxFieldInit, Production);
    fact rhs(PxFieldInit, "Name (Colon Expr)? Comma?");
    // PxAtPath: parser.rs 4258-4330. The path form is At Ident followed by Dot Ident. `.name(` is left to
    // Postfix (the path loop breaks before it), so the grammar's extra (Dot Ident)* is a superset.
    fact instance_of(PxAtPath, Production);
    fact rhs(PxAtPath, "At Ident Dot Ident (Dot Ident)*");
    // PxFor: parser.rs 4307-4339 (for-loop). Pattern, then KwOf (in) or Colon, then the iter Expr with an
    // optional range tail (DotDot or DotDotEq). The range form is written here because it is specific to this arm.
    fact instance_of(PxFor, Production);
    fact rhs(PxFor, "At Pattern (KwOf | Colon) Expr ((DotDot | DotDotEq) Expr)? Block");
    // PxClosure: parser.rs 4488-4511. Or is the zero-parameter form. BitOr starts a parameter list closed by
    // BitOr. ClosureParams is owned by the params section and must NOT include the pipes; its shape must be
    // (Name (Colon Type)? Comma?)* (see parse_closure_params_until_pipe, parser.rs 2626-2642).
    fact instance_of(PxClosure, Production);
    fact rhs(PxClosure, "Or PxClosureBody");
    fact rhs(PxClosure, "ClosureParams PxClosureBody");
    // PxClosureBody: parser.rs 4497-4505. A Block when the next token is LBrace, otherwise an Expr.
    fact instance_of(PxClosureBody, Production);
    fact rhs(PxClosureBody, "Block | Expr");
    // PxFnClosure: parser.rs 4513-4542. KwF LParen params RParen, then FatArrow Expr or Block.
    fact instance_of(PxFnClosure, Production);
    fact rhs(PxFnClosure, "KwF LParen PxFnParams RParen FatArrow Expr");
    fact rhs(PxFnClosure, "KwF LParen PxFnParams RParen Block");
    // PxFnParams: parser.rs 2644-2662 (parse_closure_param_list): same shape as ClosureParams.
    fact instance_of(PxFnParams, Production);
    fact rhs(PxFnParams, "(Name (Colon Type)? Comma?)*");
    // PxArrayLit: parser.rs 4544-4575. Empty; repeat form [Expr Semi Expr]; list form with optional trailing comma.
    fact instance_of(PxArrayLit, Production);
    fact rhs(PxArrayLit, "LBrack (RBrack | Expr (Semi Expr RBrack | (Comma Expr)* Comma? RBrack))");
    // PxTuple: parser.rs 4577-4604. Empty tuple, a tuple of two or more (a trailing comma is allowed), or a
    // parenthesised Expr. A single element with a trailing comma, (x,), is a one-element tuple.
    fact instance_of(PxTuple, Production);
    fact rhs(PxTuple, "LParen RParen");
    fact rhs(PxTuple, "LParen Expr Comma (Expr Comma)* Expr? RParen");
    fact rhs(PxTuple, "LParen Expr RParen");
    // PxNameKw: parser.rs 4404-4446. Keyword tokens that are also accepted as plain identifiers in
    // expression position. The arm produces Expr::Ident; there is no postfix or call form here.
    fact instance_of(PxNameKw, Production);
    fact rhs(PxNameKw, "KwGuard | KwHandle | KwNet | KwAgent | KwSwarm | KwKb | KwDefer | KwQuery | KwRule | KwFact | KwSelect | KwData | KwLayer | KwTensor | KwParam | KwForward | KwReward | KwPolicy | KwFitness | KwGenome | KwMutate | KwV | KwM | KwVal | KwVar");
    // Pattern: parser.rs parse_pattern (4704-4861), the match on self.peek().
    fact instance_of(Pattern, Production);
    // Underscore 4706-4709.
    fact rhs(Pattern, "Underscore");
    // Ident 4710-4736: name, then Dot Name segments, then an optional LParen (Pattern Comma?)* RParen.
    // One segment with no args is a plain binding. Several segments without args are a unit variant. Otherwise an Enum.
    fact rhs(Pattern, "Ident (Dot Name)* (LParen (Pattern Comma?)* RParen)?");
    // ? followed by a sum-type keyword 4740-4746: the question mark is skipped, then the sum arm runs.
    fact rhs(Pattern, "Question PxSumPat");
    // KwSome | KwNone | KwOk | KwErr 4748-4770.
    fact rhs(Pattern, "PxSumPat");
    // IntLiteral | FloatLiteral | StringLiteral | CharLiteral | True | False 4771-4781 (literal patterns).
    fact rhs(Pattern, "IntLiteral | FloatLiteral | StringLiteral | CharLiteral | True | False");
    // LParen 4782-4793 (tuple pattern).
    fact rhs(Pattern, "LParen (Pattern Comma?)* RParen");
    // LBrack 4796-4817 (slice pattern).
    fact rhs(Pattern, "LBrack (PxSliceElem Comma?)* RBrack");
    // At 4822-4846 (struct pattern). The first path segment and the following ones are expect_ident (Name).
    fact rhs(Pattern, "At Name (Dot Name)* LBrace (PxFieldPat Comma?)* RBrace");
    // GAP: the fallback arm (4847-4850) consumes ANY single token as Pattern::Ident. A bare grammar cannot say
    // "any token", so this is restricted to the name-like tokens that a real pattern binds with (the same
    // group the expression arms accept as identifiers). The other tokens that can reach the fallback
    // (for example Minus before a literal) fail in the parser anyway on the next expected token.
    fact rhs(Pattern, "PxNameTok");
    // PxSumPat: parser.rs 4748-4770.
    fact instance_of(PxSumPat, Production);
    fact rhs(PxSumPat, "(KwSome | KwNone | KwOk | KwErr) (LParen (Pattern Comma?)* RParen)?");
    // PxSliceElem: parser.rs 4797-4811. A DotDot rest element takes an optional Ident name; otherwise a Pattern.
    fact instance_of(PxSliceElem, Production);
    fact rhs(PxSliceElem, "DotDot Ident?");
    fact rhs(PxSliceElem, "Pattern");
    // PxFieldPat: parser.rs 4830-4841. Shorthand binds the field name; with a colon, a nested Pattern.
    fact instance_of(PxFieldPat, Production);
    fact rhs(PxFieldPat, "Name (Colon Pattern)?");
    // PxNameTok: the fallback set for Pattern (parser.rs 4847-4850) and the name-like tokens of the prefix
    // identifier arms (4404-4446, 4671-4693).
    fact instance_of(PxNameTok, Production);
    fact rhs(PxNameTok, "Ident | KwAf | KwUf | UnderscoreT | PxNameKw");

    // ── types ───────────────────────────────────────────────────────
    // Type: parser.rs parse_type (2713-3131)
    // Each arm of the match on self.peek() is one alternative. Sigil forms first.
    fact instance_of(Type, Production);
    fact rhs(Type, "BitAnd Type");
    fact rhs(Type, "AndNot Type");
    fact rhs(Type, "BitXor Type");
    fact rhs(Type, "Dollar Type");
    fact rhs(Type, "At Type");
    fact rhs(Type, "TyFn");
    fact rhs(Type, "AndTilde Type");
    fact rhs(Type, "Percent Type");
    fact rhs(Type, "PercentNot Type");
    fact rhs(Type, "Hash Type");
    fact rhs(Type, "HashTilde Type");
    fact rhs(Type, "Question Type");
    fact rhs(Type, "Bang");
    fact rhs(Type, "Underscore");
    fact rhs(Type, "UnderscoreT");
    fact rhs(Type, "TyArray");
    fact rhs(Type, "TyVec");
    fact rhs(Type, "TySlice");
    fact rhs(Type, "TyMap");
    fact rhs(Type, "TySet");
    fact rhs(Type, "TyTuple");
    fact rhs(Type, "TyDyn");
    fact rhs(Type, "TyResult");
    fact rhs(Type, "TyPtr");
    fact rhs(Type, "TySimd");
    fact rhs(Type, "TyDimensioned");
    fact rhs(Type, "TyPath");
    fact rhs(Type, "TyTensor");
    fact rhs(Type, "TyParam");
    fact rhs(Type, "TyGenome");
    fact rhs(Type, "TyPolicy");
    fact rhs(Type, "KwKb");
    // Named type, generic arm (2916-3041). The first segment is any Ident or
    // one of the single-letter keyword tokens the parser accepts as names.
    // The parser's "s" arm (2966) returns StringType with no suffix; the
    // grammar lets Ident be a plain path, which is a superset of that.
    fact instance_of(TyName, Production);
    fact rhs(TyName, "Ident | KwS | KwE | KwT | KwI | KwM | KwU | KwC");
    // Dotted segments: the parser only consumes `.Ident` when the Ident starts
    // with an uppercase letter (2970-2981). That is text-gated; the grammar
    // accepts any Ident, a superset.
    // Type args: `[` ... `]` or `<` ... `>` (3002-3021). The arg loop is
    // `while peek != close { parse_type; optional Comma }`, so commas are
    // optional between arguments. The close token must match the open one.
    // Gt, not Shr: `>>` lexes as Shr, so `Vec<Vec<T>>` fails in the parser too.
    fact instance_of(TyPath, Production);
    fact rhs(TyPath, "TyName (Dot Ident)* TyArgs?");
    fact instance_of(TyArgs, Production);
    fact rhs(TyArgs, "LBrack (Type Comma?)* RBrack | Lt (Type Comma?)* Gt");
    // R[T, E] (2935-2945). Text-gated on "R"; generic Ident-with-args already
    // covers the shape, this is the exact-arity form.
    fact instance_of(TyResult, Production);
    fact rhs(TyResult, "'R' LBrack Type Comma Type RBrack");
    // Ptr[T] (2946-2953). Text-gated on "Ptr".
    fact instance_of(TyPtr, Production);
    fact rhs(TyPtr, "'Ptr' LBrack Type RBrack");
    // Simd[T, N] (2954-2965). Text-gated on "Simd"; width must be an IntLiteral.
    fact instance_of(TySimd, Production);
    fact rhs(TySimd, "'Simd' LBrack Type Comma IntLiteral RBrack");
    // dyn Trait (2928-2930): text-gated on the Ident "dyn", then parse_type.
    // `dyn` is an Ident whose text is `dyn` (parser.rs 3121), so the quoted spelling 'dyn' is exact.
    // The lexer has no "dyn" keyword.
    fact instance_of(TyDyn, Production);
    fact rhs(TyDyn, "'dyn' Type");
    // f32[unit] / f64[unit] (2986-2998): the parser only takes this branch when
    // the single segment text is f32 or f64 and the next token is `[`.
    // f32 and f64 are text-gated (parser.rs 3180): the quoted spellings are exact.
    fact instance_of(TyDimensioned, Production);
    fact rhs(TyDimensioned, "('f32' | 'f64') TyUnit");
    // parse_unit (parser.rs 2670-2711), used only by TyDimensioned.
    // Loop: one factor, then `*` or `/` continues, anything else ends. Then `]`.
    fact instance_of(TyUnit, Production);
    fact rhs(TyUnit, "LBrack TyUnitFactor ((Star | Slash) TyUnitFactor)* RBrack");
    // A factor is the quoted spelling '1' (dimensionless; parser.rs 2865 checks IntLiteral text "1"),
    // or a token whose text is all ASCII letters, optionally with `^` [-] IntLiteral exponent.
    // The lexer maps single letters to keyword kinds (m->KwM, v->KwV, u->KwUse, M->KwMod, ...). The
    // kinds are listed, and every other alphabetic keyword is a quoted spelling, so each one is accepted
    // as the parser accepts it.
    // GAP: an Ident is accepted whatever its text; the parser requires letters only, so an Ident with a
    // digit or underscore (`m2`, `k_b`) is over-accepted.
    fact instance_of(TyUnitFactor, Production);
    fact rhs(TyUnitFactor, "'1' | TyUnitAtom (BitXor Minus? IntLiteral)?");
    fact instance_of(TyUnitAtom, Production);
    fact rhs(TyUnitAtom, "Ident | KwF | KwAf | KwUf | KwM | KwV | KwC | KwS | KwE | KwT | KwI | KwMod | KwU | KwUse | KwY | KwZ | 'D' | 'Ok' | 'df' | 'fx' | 'gd' | 'hx' | 'if' | 'in' | 'is' | 'kb' | 'or' | 'rl' | 'sp' | 'sw' | 'xd' | 'xn' | 'Err' | 'for' | 'let' | 'net' | 'pub' | 'ret' | 'val' | 'var' | 'None' | 'Some' | 'data' | 'else' | 'fact' | 'grad' | 'loop' | 'rule' | 'spec' | 'type' | 'agent' | 'break' | 'defer' | 'guard' | 'layer' | 'match' | 'param' | 'query' | 'swarm' | 'train' | 'where' | 'while' | 'effect' | 'evolve' | 'extend' | 'extern' | 'genome' | 'handle' | 'mutate' | 'policy' | 'return' | 'reward' | 'select' | 'static' | 'tensor' | 'unsafe' | 'fitness' | 'forward' | 'continue' | 'pipeline' | 'crossover' | 'population' | 'generations'");
    // Tensor / Param shape (3047-3096): `[T` then `, dim` repeated; dim is an
    // IntLiteral or an identifier (expect_ident, so Name).
    fact instance_of(TyDim, Production);
    fact rhs(TyDim, "IntLiteral | Name");
    fact instance_of(TyShape, Production);
    fact rhs(TyShape, "(Comma TyDim)*");
    fact instance_of(TyTensor, Production);
    fact rhs(TyTensor, "KwTensor LBrack Type TyShape RBrack");
    fact instance_of(TyParam, Production);
    fact rhs(TyParam, "KwParam LBrack Type TyShape RBrack");
    // Genome[T] (3099-3107)
    fact instance_of(TyGenome, Production);
    fact rhs(TyGenome, "KwGenome LBrack Type RBrack");
    // Policy[State, Action] (3110-3121)
    fact instance_of(TyPolicy, Production);
    fact rhs(TyPolicy, "KwPolicy LBrack Type Comma Type RBrack");
    // fn/af/uf function type (2763-2781). Params are `(Type Comma?)*`; the
    // return type is optional after `->` (Arrow).
    fact instance_of(TyFn, Production);
    fact rhs(TyFn, "(KwF | KwAf | KwUf) LParen (Type Comma?)* RParen (Arrow Type)?");
    // [T; N] (2856-2861): size is a full expression.
    fact instance_of(TyArray, Production);
    fact rhs(TyArray, "LBrack Type Semi Expr RBrack");
    // [T]~ (2866-2869) gives Vec; plain [T] gives Slice (2873-2875).
    fact instance_of(TyVec, Production);
    fact rhs(TyVec, "LBrack Type RBrack Tilde");
    fact instance_of(TySlice, Production);
    fact rhs(TySlice, "LBrack Type RBrack");
    // {K: V} map (2884-2891) and {K} set (2893-2896). Colon selects map.
    fact instance_of(TyMap, Production);
    fact rhs(TyMap, "LBrace Type Colon Type RBrace");
    fact instance_of(TySet, Production);
    fact rhs(TySet, "LBrace Type RBrace");
    // (T, T, ...) tuple (2901-2911). Parser also accepts a single parenthesised type.
    fact instance_of(TyTuple, Production);
    fact rhs(TyTuple, "LParen (Type Comma?)* RParen");

    // ── effects ─────────────────────────────────────────────────────
    // HandleExpr: parser.rs parse_handle_expr (1230-1266)
    // `with` is an Ident whose text is `with` (parser.rs 1342), so the quoted spelling 'with' is exact.
    // `handle` and `hx` both lex as KwHandle (lexer.rs 1069).
    fact instance_of(HandleExpr, Production);
    fact rhs(HandleExpr, "KwHandle Block 'with' Name LBrace EfHandlerArm* RBrace");
    // EfHandlerArm: parser.rs parse_handle_expr arm loop (1244-1262)
    // Parameters are `Name Comma?` repeated: the parser does not require commas between them (1249-1253).
    fact instance_of(EfHandlerArm, Production);
    fact rhs(EfHandlerArm, "Name LParen (Name Comma?)* RParen FatArrow Expr Comma?");
    // EffectDef: parser.rs parse_effect_def (1268-1304)
    fact instance_of(EffectDef, Production);
    fact rhs(EffectDef, "KwEffect Name Generics? LBrace EfEffectOp* RBrace");
    // EfEffectOp: parser.rs parse_effect_def operation loop (1278-1300)
    // GAP: Params is taken as the content of the parens, because the parser consumes LParen and RParen itself around parse_param_list (1285-1287). If the params owner defines Params WITH its parens, drop LParen and RParen here.
    fact instance_of(EfEffectOp, Production);
    fact rhs(EfEffectOp, "KwF Name Generics? Params (Arrow Type)? Semi");
    // SpecDef: parser.rs parse_spec_def (1308-1432)
    // Same GAP as above for Params: the parser consumes the parens itself (1319-1322).
    fact instance_of(SpecDef, Production);
    fact rhs(SpecDef, "KwSpec Name Generics? (Params)? (Arrow Type)? LBrace EfSpecItem* RBrace");
    // EfSpecItem: parser.rs parse_spec_def body loop (1337-1421)
    // The `_` arm (1418-1420) advances any other token, so the fallback is EfSpecSkip.
    // KwPerf metric: EfPerfMetric (1381-1392). Bound after the comma: EfBalanced (1398-1409).
    fact instance_of(EfSpecItem, Production);
    fact rhs(EfSpecItem, "KwReq EfParenText | KwEns EfParenText | KwInv EfParenText | KwFx LParen EfFxTok* RParen | KwPerf LParen EfPerfMetric (Comma EfBalanced)? RParen | Semi | EfSpecSkip");
    // EfSpecSkip: the tokens the `_` arm of parse_spec_def (1418-1420) advances over: every kind except the ones matched above (RBrace, KwReq, KwEns, KwInv, KwFx, KwPerf, Semi).
    fact instance_of(EfSpecSkip, Production);
    fact rhs(EfSpecSkip, "KwF | KwAf | KwUf | KwM | KwV | KwC | KwS | KwE | KwT | KwI | KwMod | KwU | KwUse | KwY | KwZ | Plus | TildePre | Question | QuestionEq | At | AtAt | AtW | KwRet | DoubleArrowR | Bang | True | False | Underscore | UnderscoreT | KwEffect | KwHandle | KwSpec | KwAgent | KwSwarm | KwExtern | KwUnsafe | Todo | Unimplemented | KwType | KwStatic | KwFor | KwLoop | KwBreak | KwContinue | KwOf | KwOr | KwElse | KwData | KwVal | KwVar | KwLet | KwGuard | KwDefer | KwExtend | KwIs | Pipe | KwOk | KwErr | KwSome | KwNone | KwSwarmMapReduce | KwSwarmPipeline | KwSwarmSaga | KwSwarmFanOut | KwSwarmRace | KwPipeline | KwGrammarExt | KwNet | KwLayer | KwTensor | KwParam | KwTrain | KwGrad | KwForward | KwKb | KwFact | KwRule | KwQuery | KwEvolve | KwGenome | KwMutate | KwFitness | KwSelect | KwCrossover | KwPopulation | KwGenerations | KwRl | KwPolicy | KwReward | KwPsi | KwLambda | KwPhi | KwPi | KwTheta | KwNabla | KwAlpha | KwSigma | KwKappa | KwRho | KwOmega | KwGammaGreek | KwPhiLower | KwXi | KwMu | KwChi | TensorMatmul | TensorHadamard | TensorTranspose | TensorFlatten | TensorPipeline | IntLiteral | FloatLiteral | StringLiteral | FormatString | PrintString | EprintString | CharLiteral | ByteLiteral | ByteStringLiteral | Ident | Minus | Star | Slash | Percent | Eq | Neq | Lt | Gt | Le | Ge | And | Or | Not | BitAnd | BitOr | BitXor | Shl | Shr | Assign | PlusEq | MinusEq | StarEq | SlashEq | PercentEq | BitAndEq | BitOrEq | BitXorEq | ShlEq | ShrEq | AndNot | AndTilde | PercentNot | HashTilde | TildeArrow | LParen | RParen | LBrace | LBrack | RBrack | Comma | Dot | Colon | ColonQuestion | Arrow | FatArrow | Hash | DotDot | DotDotEq | Tilde | Dollar");
    // EfParenText: parser.rs collect_paren_text (1435-1455): `(` then balanced tokens then `)`.
    // The parser accepts any token inside, including Semi, braces and keywords; only parentheses must balance.
    fact instance_of(EfParenText, Production);
    fact rhs(EfParenText, "LParen EfBalanced RParen");
    // EfBalanced: the tokens collect_paren_text (1440-1452) and the KwPerf bound (1399-1409) accept: any token except parentheses, or a parenthesised group that is itself balanced.
    fact instance_of(EfBalanced, Production);
    fact rhs(EfBalanced, "(EfAnyTok | LParen EfBalanced RParen)*");
    // EfAnyTok: every token kind except LParen and RParen (the parser's text collectors advance over any kind).
    fact instance_of(EfAnyTok, Production);
    fact rhs(EfAnyTok, "KwF | KwAf | KwUf | KwM | KwV | KwC | KwS | KwE | KwT | KwI | KwMod | KwU | KwUse | KwY | KwZ | Plus | TildePre | Question | QuestionEq | At | AtAt | AtW | KwRet | DoubleArrowR | Bang | True | False | Underscore | UnderscoreT | KwEffect | KwHandle | KwSpec | KwAgent | KwSwarm | KwExtern | KwReq | KwEns | KwInv | KwFx | KwPerf | KwUnsafe | Todo | Unimplemented | KwType | KwStatic | KwFor | KwLoop | KwBreak | KwContinue | KwOf | KwOr | KwElse | KwData | KwVal | KwVar | KwLet | KwGuard | KwDefer | KwExtend | KwIs | Pipe | KwOk | KwErr | KwSome | KwNone | KwSwarmMapReduce | KwSwarmPipeline | KwSwarmSaga | KwSwarmFanOut | KwSwarmRace | KwPipeline | KwGrammarExt | KwNet | KwLayer | KwTensor | KwParam | KwTrain | KwGrad | KwForward | KwKb | KwFact | KwRule | KwQuery | KwEvolve | KwGenome | KwMutate | KwFitness | KwSelect | KwCrossover | KwPopulation | KwGenerations | KwRl | KwPolicy | KwReward | KwPsi | KwLambda | KwPhi | KwPi | KwTheta | KwNabla | KwAlpha | KwSigma | KwKappa | KwRho | KwOmega | KwGammaGreek | KwPhiLower | KwXi | KwMu | KwChi | TensorMatmul | TensorHadamard | TensorTranspose | TensorFlatten | TensorPipeline | IntLiteral | FloatLiteral | StringLiteral | FormatString | PrintString | EprintString | CharLiteral | ByteLiteral | ByteStringLiteral | Ident | Minus | Star | Slash | Percent | Eq | Neq | Lt | Gt | Le | Ge | And | Or | Not | BitAnd | BitOr | BitXor | Shl | Shr | Assign | PlusEq | MinusEq | StarEq | SlashEq | PercentEq | BitAndEq | BitOrEq | BitXorEq | ShlEq | ShrEq | AndNot | AndTilde | PercentNot | HashTilde | TildeArrow | LBrace | RBrace | LBrack | RBrack | Semi | Comma | Dot | Colon | ColonQuestion | Arrow | FatArrow | Hash | DotDot | DotDotEq | Tilde | Dollar");
    // EfFxTok: tokens inside `@fx(...)` (parser.rs 1363-1369): any kind except RParen; the parser does not track depth here, so LParen is accepted too.
    fact instance_of(EfFxTok, Production);
    fact rhs(EfFxTok, "KwF | KwAf | KwUf | KwM | KwV | KwC | KwS | KwE | KwT | KwI | KwMod | KwU | KwUse | KwY | KwZ | Plus | TildePre | Question | QuestionEq | At | AtAt | AtW | KwRet | DoubleArrowR | Bang | True | False | Underscore | UnderscoreT | KwEffect | KwHandle | KwSpec | KwAgent | KwSwarm | KwExtern | KwReq | KwEns | KwInv | KwFx | KwPerf | KwUnsafe | Todo | Unimplemented | KwType | KwStatic | KwFor | KwLoop | KwBreak | KwContinue | KwOf | KwOr | KwElse | KwData | KwVal | KwVar | KwLet | KwGuard | KwDefer | KwExtend | KwIs | Pipe | KwOk | KwErr | KwSome | KwNone | KwSwarmMapReduce | KwSwarmPipeline | KwSwarmSaga | KwSwarmFanOut | KwSwarmRace | KwPipeline | KwGrammarExt | KwNet | KwLayer | KwTensor | KwParam | KwTrain | KwGrad | KwForward | KwKb | KwFact | KwRule | KwQuery | KwEvolve | KwGenome | KwMutate | KwFitness | KwSelect | KwCrossover | KwPopulation | KwGenerations | KwRl | KwPolicy | KwReward | KwPsi | KwLambda | KwPhi | KwPi | KwTheta | KwNabla | KwAlpha | KwSigma | KwKappa | KwRho | KwOmega | KwGammaGreek | KwPhiLower | KwXi | KwMu | KwChi | TensorMatmul | TensorHadamard | TensorTranspose | TensorFlatten | TensorPipeline | IntLiteral | FloatLiteral | StringLiteral | FormatString | PrintString | EprintString | CharLiteral | ByteLiteral | ByteStringLiteral | Ident | Minus | Star | Slash | Percent | Eq | Neq | Lt | Gt | Le | Ge | And | Or | Not | BitAnd | BitOr | BitXor | Shl | Shr | Assign | PlusEq | MinusEq | StarEq | SlashEq | PercentEq | BitAndEq | BitOrEq | BitXorEq | ShlEq | ShrEq | AndNot | AndTilde | PercentNot | HashTilde | TildeArrow | LParen | LBrace | RBrace | LBrack | RBrack | Semi | Comma | Dot | Colon | ColonQuestion | Arrow | FatArrow | Hash | DotDot | DotDotEq | Tilde | Dollar");
    // EfPerfMetric: the first argument of `@perf(...)` (parser.rs 1381-1392): tokens up to a top-level Comma or RParen; nested parentheses may hold anything, Comma included.
    fact instance_of(EfPerfMetric, Production);
    fact rhs(EfPerfMetric, "(EfMetricTok | LParen EfBalanced RParen)*");
    // EfMetricTok: every kind except LParen, RParen, Comma (top-level metric tokens).
    fact instance_of(EfMetricTok, Production);
    fact rhs(EfMetricTok, "KwF | KwAf | KwUf | KwM | KwV | KwC | KwS | KwE | KwT | KwI | KwMod | KwU | KwUse | KwY | KwZ | Plus | TildePre | Question | QuestionEq | At | AtAt | AtW | KwRet | DoubleArrowR | Bang | True | False | Underscore | UnderscoreT | KwEffect | KwHandle | KwSpec | KwAgent | KwSwarm | KwExtern | KwReq | KwEns | KwInv | KwFx | KwPerf | KwUnsafe | Todo | Unimplemented | KwType | KwStatic | KwFor | KwLoop | KwBreak | KwContinue | KwOf | KwOr | KwElse | KwData | KwVal | KwVar | KwLet | KwGuard | KwDefer | KwExtend | KwIs | Pipe | KwOk | KwErr | KwSome | KwNone | KwSwarmMapReduce | KwSwarmPipeline | KwSwarmSaga | KwSwarmFanOut | KwSwarmRace | KwPipeline | KwGrammarExt | KwNet | KwLayer | KwTensor | KwParam | KwTrain | KwGrad | KwForward | KwKb | KwFact | KwRule | KwQuery | KwEvolve | KwGenome | KwMutate | KwFitness | KwSelect | KwCrossover | KwPopulation | KwGenerations | KwRl | KwPolicy | KwReward | KwPsi | KwLambda | KwPhi | KwPi | KwTheta | KwNabla | KwAlpha | KwSigma | KwKappa | KwRho | KwOmega | KwGammaGreek | KwPhiLower | KwXi | KwMu | KwChi | TensorMatmul | TensorHadamard | TensorTranspose | TensorFlatten | TensorPipeline | IntLiteral | FloatLiteral | StringLiteral | FormatString | PrintString | EprintString | CharLiteral | ByteLiteral | ByteStringLiteral | Ident | Minus | Star | Slash | Percent | Eq | Neq | Lt | Gt | Le | Ge | And | Or | Not | BitAnd | BitOr | BitXor | Shl | Shr | Assign | PlusEq | MinusEq | StarEq | SlashEq | PercentEq | BitAndEq | BitOrEq | BitXorEq | ShlEq | ShrEq | AndNot | AndTilde | PercentNot | HashTilde | TildeArrow | LBrace | RBrace | LBrack | RBrack | Semi | Dot | Colon | ColonQuestion | Arrow | FatArrow | Hash | DotDot | DotDotEq | Tilde | Dollar");
    // AgentDef: parser.rs parse_agent_def (1460-1515)
    fact instance_of(AgentDef, Production);
    fact rhs(AgentDef, "KwAgent Name LBrace EfAgentField* RBrace");
    // EfAgentField: parser.rs parse_agent_def body loop (1468-1507)
    // The label is the quoted spelling 'capabilities' or 'requires_approval' (parser.rs 1591-1600 text
    // test); the value is a bracket list of names (2610). Exact.
    fact instance_of(EfAgentField, Production);
    fact rhs(EfAgentField, "'capabilities' Colon EfNameList");
    fact rhs(EfAgentField, "'requires_approval' Colon EfNameList");
    fact rhs(EfAgentField, "Comma");
    fact rhs(EfAgentField, "Semi");
    // EfNameList: parser.rs parse_bracket_string_list (2445-2457), which the agent fields call. Names, optional commas.
    fact instance_of(EfNameList, Production);
    fact rhs(EfNameList, "LBrack (Name Comma?)* RBrack");
    // SwarmDef: parser.rs parse_swarm_def (1524-1643)
    fact instance_of(SwarmDef, Production);
    fact rhs(SwarmDef, "KwSwarm Name LBrace EfSwarmField* RBrace");
    // EfSwarmField: parser.rs parse_swarm_def body loop (1541-1630)
    // One alternative per label, each with the value form the parser reads for it (parser.rs 1646-1700):
    // size takes an Expr; topology, topo, consensus, cons and transport take a Name. dispatch, aggregate
    // and on_failure are rejected by the parser, so they are not written.
    fact instance_of(EfSwarmField, Production);
    fact rhs(EfSwarmField, "KwAgent Colon Name Semi?");
    fact rhs(EfSwarmField, "'size' Colon Expr Semi?");
    fact rhs(EfSwarmField, "'topology' Colon Name Semi?");
    fact rhs(EfSwarmField, "'topo' Colon Name Semi?");
    fact rhs(EfSwarmField, "'consensus' Colon Name Semi?");
    fact rhs(EfSwarmField, "'cons' Colon Name Semi?");
    fact rhs(EfSwarmField, "'transport' Colon Name Semi?");
    fact rhs(EfSwarmField, "Comma");
    fact rhs(EfSwarmField, "Semi");

    // ── domain ──────────────────────────────────────────────────────
    // NetDef: parser.rs parse_net_def (1648-1770)
    fact instance_of(NetDef, Production);
    fact rhs(NetDef, "KwNet Name Generics? LBrace DoNetItem* RBrace");
    // DoNetItem: parser.rs parse_net_def body loop (1670-1756)
    // The keyword items are quoted spellings (parser.rs 1808-1849): 'stack' takes an IntLiteral count,
    // 'wrap' a Name and a body, 'residual' and 'branch' bodies.
    // GAP: a block reference (ItBlockRef) is taken on any Ident. The parser requires a declared block
    // (self.blocks.contains_key), which token kinds cannot express.
    fact instance_of(DoNetItem, Production);
    fact rhs(DoNetItem, "KwLayer DoLayerBody");
    fact rhs(DoNetItem, "'stack' IntLiteral DoComposeBody");
    fact rhs(DoNetItem, "'wrap' Name DoComposeBody");
    fact rhs(DoNetItem, "'residual' DoComposeBody");
    fact rhs(DoNetItem, "'branch' DoComposeBody+");
    fact rhs(DoNetItem, "ItBlockRef");
    fact rhs(DoNetItem, "KwForward Block");
    fact rhs(DoNetItem, "Semi");
    fact rhs(DoNetItem, "Comma");
    // DoComposeBody: parser.rs parse_compose_body (1776-1828)
    fact instance_of(DoComposeBody, Production);
    fact rhs(DoComposeBody, "LBrace DoComposeItem* RBrace");
    // DoComposeItem: parser.rs parse_compose_body item loop (1783-1825)
    // The keyword items are quoted spellings (parser.rs 1926-1937): 'residual' and 'branch' take bodies,
    // 'wrap' a Name and a body.
    // GAP: a block reference (ItBlockRef) is taken on any Ident; the parser requires a declared block.
    fact instance_of(DoComposeItem, Production);
    fact rhs(DoComposeItem, "KwLayer DoLayerBody");
    fact rhs(DoComposeItem, "Semi");
    fact rhs(DoComposeItem, "Comma");
    fact rhs(DoComposeItem, "'residual' DoComposeBody");
    fact rhs(DoComposeItem, "'branch' DoComposeBody+");
    fact rhs(DoComposeItem, "'wrap' Name DoComposeBody");
    fact rhs(DoComposeItem, "ItBlockRef");
    // DoLayerBody: parser.rs parse_layer_body (1833-1852); `layer` keyword is consumed by the caller
    // Arguments are parse_expr in a loop with an optional comma after each, so no comma is required between them.
    fact instance_of(DoLayerBody, Production);
    fact rhs(DoLayerBody, "Name Colon Type (LParen (Expr Comma?)* RParen)? Semi?");
    // KbDef: parser.rs parse_kb_def (1857-1938)
    fact instance_of(KbDef, Production);
    fact rhs(KbDef, "KwKb Name LBrace DoKbItem* RBrace");
    // DoKbItem: parser.rs parse_kb_def item loop (1865-1933)
    // Note: the rule's parameter list is parse_param_list, which consumes no
    // parentheses (see parser.rs 2512); the parens are expected by KwRule
    // itself at 1889 and 1891. The rhs therefore writes Params,
    // and Params must be the inner list, not the parenthesised form.
    fact instance_of(DoKbItem, Production);
    fact rhs(DoKbItem, "KwFact Name LParen (Expr Comma?)* RParen Semi?");
    fact rhs(DoKbItem, "KwRule Name Params (TildeArrow Expr (Comma Expr)*)? Block Semi?");
    fact rhs(DoKbItem, "Semi");
    fact rhs(DoKbItem, "Comma");
    // EvolveDef: parser.rs parse_evolve_def (1943-2041)
    fact instance_of(EvolveDef, Production);
    fact rhs(EvolveDef, "KwEvolve Name LBrace DoEvolveItem* RBrace");
    // DoEvolveItem: parser.rs parse_evolve_def field loop (1956-2024)
    fact instance_of(DoEvolveItem, Production);
    fact rhs(DoEvolveItem, "KwGenome Colon Type Semi?");
    fact rhs(DoEvolveItem, "KwPopulation Colon Expr Semi?");
    fact rhs(DoEvolveItem, "KwGenerations Colon Expr Semi?");
    fact rhs(DoEvolveItem, "KwFitness Block");
    fact rhs(DoEvolveItem, "KwMutate Block");
    fact rhs(DoEvolveItem, "KwCrossover Block");
    fact rhs(DoEvolveItem, "KwSelect Block");
    fact rhs(DoEvolveItem, "Semi");
    fact rhs(DoEvolveItem, "Comma");
    // TrainDef: parser.rs parse_train_def (2046-2305)
    fact instance_of(TrainDef, Production);
    fact rhs(TrainDef, "KwTrain Name LBrace DoTrainItem* RBrace");
    // DoTrainItem: parser.rs parse_train_def field loop (2077-2270)
    // Each label is a quoted spelling (parser.rs 2195-2457 dispatches on the label's text). The Expr fields
    // share one shape, one alternative per label. body takes a Block, and net is the KwNet keyword with a
    // Name value. The parser rejects any other label, so none is written. Exact.
    fact instance_of(DoTrainItem, Production);
    fact rhs(DoTrainItem, "KwNet Colon Name Semi?");
    fact rhs(DoTrainItem, "'optimizer' DoTrainExprField");
    fact rhs(DoTrainItem, "'loss' DoTrainExprField");
    fact rhs(DoTrainItem, "'epochs' DoTrainExprField");
    fact rhs(DoTrainItem, "'inputs' DoTrainExprField");
    fact rhs(DoTrainItem, "'targets' DoTrainExprField");
    fact rhs(DoTrainItem, "'dataset' DoTrainExprField");
    fact rhs(DoTrainItem, "'val_split' DoTrainExprField");
    fact rhs(DoTrainItem, "'checkpoint' DoTrainExprField");
    fact rhs(DoTrainItem, "'batch_size' DoTrainExprField");
    fact rhs(DoTrainItem, "'patience' DoTrainExprField");
    fact rhs(DoTrainItem, "'prompt' DoTrainExprField");
    fact rhs(DoTrainItem, "'max_tokens' DoTrainExprField");
    fact rhs(DoTrainItem, "'temperature' DoTrainExprField");
    fact rhs(DoTrainItem, "'top_k' DoTrainExprField");
    fact rhs(DoTrainItem, "'top_p' DoTrainExprField");
    fact rhs(DoTrainItem, "'seed' DoTrainExprField");
    fact rhs(DoTrainItem, "'clip_grad' DoTrainExprField");
    fact rhs(DoTrainItem, "'warmup_steps' DoTrainExprField");
    fact rhs(DoTrainItem, "'lr_schedule' DoTrainExprField");
    fact rhs(DoTrainItem, "'weight_decay' DoTrainExprField");
    fact rhs(DoTrainItem, "'tied_embeddings' DoTrainExprField");
    fact rhs(DoTrainItem, "'plateau_patience' DoTrainExprField");
    fact rhs(DoTrainItem, "'lr_factor' DoTrainExprField");
    fact rhs(DoTrainItem, "'body' Block");
    fact rhs(DoTrainItem, "Semi");
    fact rhs(DoTrainItem, "Comma");
    // DoTrainExprField: parser.rs parse_train_def, the common `label: expr;` arm (2084-2254)
    fact instance_of(DoTrainExprField, Production);
    fact rhs(DoTrainExprField, "Colon Expr Semi?");
}

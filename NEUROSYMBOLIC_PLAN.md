# Plan: MAGE as a neurosymbolic system

Written 2026-10-29, after PRs #38–#102 (open and stacked in order). This is the
top-level plan. [`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) remains the
plan for the arena's research track (phases M, P, L, S, C), which continues
under this one as a consumer of the knowledge base (Phase 6 below).

Sizes: **S** is under a day, **M** is a few days, **L** is a week or more.
**[H]** marks an item a Haiku subagent can take from a precise brief. Every
item ends the way earlier plans did: tests (break-checked where a test guards
a fix), pins updated, a HANDOFF entry, and a stacked PR.

---

## 1. The design

MAGE is three parts joined by one schema:

| Part | Job | Examples |
|---|---|---|
| **Neural** | Inference and generation | Generators that propose programs (the arena's grammar policy, an LLM proposer); learners that predict program output; networks declared in `net` and run on `rmi` |
| **Symbolic** | Correctness, safety and compilation | Lexer, parser, type checker, contract verifier, safety rules (SKB), lowering to the binary IR, the arena's gate |
| **Knowledge base** | What the language *is* | Syntax (tokens, sigils, keywords), grammar (productions), high-level forms (human source, sigil source, `net`/`block` declarations), low-level forms (AST, HIR, IR ops, the binary container), and the rules over them |

**The ontology is the top-level schema of the knowledge base.** It says which
classes of thing exist, which relations may hold between them, and which
invariants must always hold. Every fact in the knowledge base is an instance of
one of its classes. The neural and symbolic parts read the language only
through the knowledge base, and change it only through the symbolic gate.

```text
                 ┌──────────────────────────────┐
                 │  Ontology (schema)           │
                 │  classes · relations ·       │
                 │  invariants                  │
                 └──────────────┬───────────────┘
                                │ types every fact
                 ┌──────────────▼───────────────┐
                 │  Knowledge base (facts)      │
                 │  lexicon · grammar · forms · │
                 │  meanings · rules            │
                 └───────┬──────────────▲───────┘
          reads grammar, │              │ admits changes
          prompt, rules  │              │ that pass the gate
                 ┌───────▼──────┐  ┌────┴─────────┐
                 │  Neural      │  │  Symbolic    │
                 │  proposes    ├─►│  decides,    │
                 │  candidates  │  │  citing rule │
                 │              │◄─┤  ids         │
                 └──────────────┘  └──────────────┘
                     verdicts and measurements (evidence)
```

---

## 2. First principles

Each principle comes with what it forces. The plan in section 6 is these
consequences, ordered.

**P1. Every piece of language knowledge is stated once.** Code is either
generated from the knowledge base or tested against it, never a parallel copy.
*Why:* the project has paid for every copy it kept. `skb/rules/*.json` looked
authoritative while nothing read it; `ontology.rs` keeps a sigil table "in sync
with `lexer.rs`" by a comment; the arena keeps its own grammar and its own
depth limit. *Forces:* a conformance test for every table that remains
hand-written, and generation for every table that can be.

**P2. The schema is small, closed and versioned.** A fact whose class or
relation the schema does not declare is refused, not stored. Changing the
schema is a versioned event with its own test. *Why:* a schema that accepts
anything is a catalogue, which is what `ontology.rs` is today (`schema_version:
1`, no schema). *Forces:* a validator, and a schema version in every exported
form of the knowledge base.

**P3. The knowledge base is written in MAGE and checked by MAGE.** It lives in
`kb` blocks, evaluated by MAGE's own Datalog engine (`prototype/src/logic.rs`).
Invariants are Datalog rules whose violations are facts. *Why:* a system meant
to improve itself has to be able to read and change its own knowledge in its
own language, and MAGE already has the construct and the engine. *Forces:* the
`kb` engine must carry the schema (Phase 1 spike), and a bootstrap rule: the
lexer and parser stay hand-written Rust as the kernel, and the kernel is
conformance-tested against the grammar facts, including the grammar of the file
the knowledge base is written in.

**P4. Neural proposes, symbolic decides.** No neural output reaches a result,
a credit or the knowledge base without passing a symbolic check, and every
refusal names the rule fact that refused it. *Why:* refusals are the training
signal; a refusal that cannot be traced to a rule teaches nothing and cannot be
audited. *Forces:* one verdict type shared by `--check` and the arena gate,
carrying rule ids.

**P5. Forms are views of one artifact; lowerings are relations.** Every form,
high-level or low-level, is a class; every lowering between forms is a fact
that states what it loses. *Why:* the four-forms claim is MAGE's central
promise, and it is only as good as its round-trips. The binary IR already loses
layer names, and that loss is documented in prose rather than recorded.
*Forces:* round-trip tests generated from the lowering facts.

**P6. Every claim is measured.** Evidence (refusals, measurements, costs) is a
class in the schema, and a figure in a document comes from a run. *Why:* the
pin check (101 documented counts) exists because unmeasured claims drifted.
*Forces:* evidence is recorded, but outside the compiled knowledge base, as an
append-only log that refers to it.

**P7. Knowledge grows only through the gate.** A new grammar extension, rule
or shared block is a proposed change to the knowledge base. It is admitted only
if it fits the schema, keeps every invariant, passes the conformance suite, and
does not raise measured cost. Versions are content-addressed. *Why:* this is
recursive self-improvement made safe: the system can change what it knows, and
every change is checked by the part that cannot be talked into anything.
*Forces:* a proposal format, a gate for knowledge-base changes, and a version
history `germline` can hand on.

**P8. Language knowledge and project metadata are different things.** CI
floors, the document list, CLI flags and benchmark backends describe the
project, not the language. *Why:* mixing them makes the language schema open
by necessity. *Forces:* a separate `project` namespace with its own small
schema; agents can still query it.

---

## 3. What changes in today's code

| Today | Under this plan |
|---|---|
| `prototype/src/ontology.rs`: 22 sections of hand-written tables, served as JSON over RAP `ontology/full` | Its language sections are generated from the knowledge base; its project sections move to the `project` namespace. The JSON keeps its shape and gains a `schema` key |
| `MAGE_ONTOLOGY.md` Part II: 12 domains of concepts, relations and invariants, in prose | Becomes the documentation of the schema, checked against `kb/schema.mg` |
| `prototype/src/lexer.rs` `KEYWORDS`, token kinds | Kernel, kept hand-written; conformance-tested against lexicon facts |
| `prototype/src/parser.rs` (6,226 lines, recursive descent) | Kernel, kept; conformance-tested against grammar facts by generating sentences and recognising the corpus |
| `prototype/src/skb.rs` `builtin_rules()` (255 rules, 8 databases) | Rules become rule facts; `builtin_rules()` and `skb/` are generated from them |
| Diagnostic codes spread over `hir.rs`, `verify.rs` and others (22 `E0xxx` codes) | Each code is a rule fact naming its enforcer; every diagnostic carries a rule id |
| `prototype/src/grammar.rs` (grammar extensions, promotion by usage count) | Extensions become knowledge-base proposals admitted by the gate (Phase 8) |
| `arena/src/grammar.rs`: its own productions, `MAX_DEPTH = 5`, `describe_grammar` | A *profile* of the knowledge base's grammar: a selected, typed subset. Depth limit and prompt text are derived from facts |
| `arena/src/substrate.rs` gate and `Refusal` (10 classes) | Uses the shared verdict type; each refusal class is a rule fact |
| `prototype/src/rmi_ontology_adapter.rs` (safety rules into an `rmi` `Ontology`) | Generalised: the whole knowledge base exports to `rmi::core::ontology::Ontology` as a derived, read-only index, which is where embeddings for neural retrieval attach |
| `rmi::symbolic::logic::KnowledgeBase` | Left alone in `rmi`. MAGE's canonical engine is `prototype/src/logic.rs`; two engines over one knowledge base would break P1 |
| `skb/rules/*.json`, already regenerated by `--emit-skb` | Generated from the knowledge base |

---

## 4. Architecture

```text
kb/                               the knowledge base, in MAGE
  schema.mg                       the ontology: classes, relations, invariants
  lexicon.mg                      tokens, keywords, sigils, the sigil↔keyword pairs
  grammar.mg                      productions, with ids
  forms.mg                        form classes and lowering facts
  meaning.mg                      types, effects, IR ops, layers
  rules/*.mg                      type rules, safety rules, contracts, grammar constraints, diagnostic codes
  profiles/*.mg                   named subsets of the grammar (the arena's is one)
  project.mg                      the separate project namespace (P8)

prototype/src/kb.rs               loads kb/*.mg (embedded at build time), validates
                                  against the schema, runs the invariants, answers queries
prototype/src/verdict.rs          the one verdict type: accepted, or refused with rule ids
prototype/src/ontology.rs         generated view for RAP, unchanged JSON shape
```

Interfaces, each a single function so every consumer goes through it:

- `kb::load() -> Kb` and `Kb::query(predicate, args)`, over the materialised facts.
- `Kb::profile(name) -> GrammarProfile`: the productions, depth limit and
  prompt text a generator uses.
- `Verdict { accepted: bool, refusals: Vec<(RuleId, Span, String)> }`: returned
  by `--check`, `--eval`'s contract checks and the arena gate.
- RAP `kb/query`, `kb/schema` and `kb/profile`, for agents outside the process.

---

## 5. The top-level schema, draft v1

Classes:

| Class | Subclasses | Example instances |
|---|---|---|
| **Form** | `HumanSource`, `SigilSource`, `Declarative`, `Ast`, `Hir`, `IrOp`, `BinaryContainer` | the `net` DSL is `Declarative`; ABL v3 is `BinaryContainer` |
| **Construct** | `Token`, `Keyword`, `Sigil`, `Production`, `AstKind` | `+f`, `pub`, `fn`, `item_fn`, `Spec` |
| **Meaning** | `Type`, `Effect`, `Op`, `Layer` | `i32`, `io`, `RES_ADD`, `Linear` |
| **Rule** | `TypeRule`, `SafetyRule`, `Contract`, `GrammarConstraint`, `Diagnostic` | SKB `ownership/…`, `E0560`, `max_depth` |
| **Component** | `NeuralComponent` (generator, learner, network), `SymbolicComponent` (lexer, parser, checker, verifier, lowering, gate) | `PcfgPolicy`, `Verifier` |
| **Artifact** | `Program`, `Block`, `Container` | a `forge` block by content hash |
| **Evidence** | `Refusal`, `Measurement` | recorded outside the compiled base (P6) |

Relations:

| Relation | From → to | Meaning |
|---|---|---|
| `subclass_of` | class → class | the schema's own hierarchy |
| `abbreviates` | `Sigil` → `Keyword` | must be a bijection within a mode |
| `derives` | `Production` → `Construct` | the grammar |
| `parses_as` | `Production` → `AstKind` | what the parser builds |
| `lowers_to` | `Form` → `Form` | with a `loses` attribute, possibly empty |
| `has_type`, `has_effect` | `Construct` → `Meaning` | typing and effects |
| `constrained_by` | `Construct` → `Rule` | what may refuse it |
| `enforced_by` | `Rule` → `SymbolicComponent` | who checks it |
| `proposed_by` | `Artifact` → `NeuralComponent` | provenance |
| `refused_by` | `Refusal` → `Rule` | every refusal names one rule |
| `in_profile` | `Production` → profile | grammar subsets |

Invariants (each a Datalog rule whose results are violations):

- [x] Every fact's predicate is a declared class or relation, with arguments of the declared classes. *(Checked by `kb.rs`, not a Datalog rule: it is about predicates, not facts.)*
- [x] Every `Rule` has exactly one `enforced_by`. *(`unenforced_rule`, `rule_enforced_twice`.)*
- [x] `abbreviates` is a bijection within each mode. *(`keyword_two_sigils`. `sigil_two_keywords` was dropped in schema v3: the formatter prints `:` for both `in` and `else`, and the grammar, not the spelling, tells them apart.)*
- [x] Every `lowers_to` either round-trips or states what it `loses`. *(`unaccounted_lowering`; `round_trips` and the ternary `loses(from, to, what)` are relations.)*
- [x] Every `Production` is reachable from the start symbol, and every `AstKind` is built by some production. *(`unreachable_production` from `start_symbol`, `unbuilt_ast_kind`.)*
- [ ] Every `Diagnostic` code the compiler can emit is a fact, and every `Diagnostic` fact is emitted by some test. *(Second half only: `untested_diagnostic` over `tested_by`. The first half needs the compiler's codes as facts, Phase 3.)*
- [x] Every `Refusal` names a `Rule`. *(`unnamed_refusal`.)*
- [x] The class hierarchy has no cycle, and every class named anywhere is declared. *(`class_cycle`, `undeclared_class`; added in `kb/schema.mg`.)*

`kb/schema.mg` (v1) adds to this draft: the class `Profile` (the range of
`in_profile`) and `Text` (any constant); the relations `start_symbol`,
`round_trips`, `loses` and `tested_by`; and `schema_version`.

The twelve domains of `MAGE_ONTOLOGY.md` (Lexical, Syntactic, Semantic, Type,
Effect, Safety, AI, Agent, Codegen, Verify, Tooling, Package) stay as the way
facts are grouped into files and namespaces. They are not classes.

---

## 6. Phases and checklists

### Phase 0: Preconditions

- [ ] Merge the stack #38–#119, this plan included, in order (needs the user). **S**
- [ ] Settle the open questions in section 8. **S**
- [ ] Census every module in `prototype/src` (73 files) into neural, symbolic, knowledge base, interface, or outside the design (for example `crdt.rs`, `consensus.rs`, `hot_reload.rs`). Record it in `ARCHITECTURE.md`. Outside-the-design modules are kept but not extended. **S [H]**

### Phase 1: The schema

- [x] **Spike:** confirm `logic.rs` can carry the schema: multi-arity facts, string and integer constants, recursive rules, and queries from Rust. It has no negation yet ("stratified negation (future)"), and most invariants need it. Report what is missing. **S**
  *Found (2026-10-29):* besides no negation, `build_kb` read only a rule's `where` conditions, ignored its block, took lowercase parameters for constants, and dropped fact arguments it could not read. So MAGE_SPEC §7.1's own example derived nothing. There are also **three** engines over `kb` blocks, not one: `logic.rs` (`--check`), `abl_bridge::evaluate_kb` with `KbTranslator` (`--run=abl`; it too reads only `where`), and `builder.rs`'s K0004–K0007 validation.
- [x] Add stratified negation to `logic.rs`, with tests for stratification errors. Rules now mean what §7.1 says; anything unevaluable is E0570. **M**
- [x] **One engine.** `KbTranslator` lowers the same body `logic.rs` reads (block and `where`), and `builder.rs`'s checks call it (K0008). A rule the binary form cannot carry (negation, constants) is left out at lowering, naming the loss, not lowered wrongly. `evaluate_kb` stays as the container's indexed interpreter; a test requires it to derive what `logic.rs` derives on every `kb` block in the repo. *Found:* the lowering had dropped every rule body, so `unified.mg`'s container derived nothing; it is now 467 bytes, not 420, and derives `grandparent(a, c)`. **M**
- [x] Comparisons `a == b`, `a != b` in rule bodies: "exactly one" and "bijection" need inequality. Safe only when both sides are bound; not carried by the binary IR. **S**
- [x] Write `kb/schema.mg`: the classes, relations and invariants of section 5. **M**
- [x] Write `prototype/src/kb.rs`: embed `kb/*.mg` with `include_str!`, parse with the kernel parser, materialise, validate every fact against the schema, run the invariants, and refuse to load on any violation. **M**
- [x] Error messages for schema violations name the file, line and the invariant broken. `FactDef` and `RuleDef` carry the line of their keyword (not serialised, so the AST JSON is unchanged); an invariant violation points at a stated fact that takes part in it. Rule errors from the logic engine itself still carry no line. **S**
- [x] A test that each invariant catches a deliberately bad fact (break-check). A second test requires a breaking example for every `invariant` fact; making `unenforced_rule` unable to fire fails the first. **S [H]**
- [x] RAP methods `kb/schema` and `kb/query`, read from the loaded base, not a copy. **S**
- [x] `MAGE_ONTOLOGY.md` Part II documents `kb/schema.mg` (§11.1), with a test that it names every class, relation and invariant in the schema and nothing else. The domain sections after it stay as reference until their facts move into `kb/` (Phases 2-3); rewriting them now would document facts the knowledge base does not hold. **M**

**Done when:** `kb::load()` succeeds on a schema-only knowledge base, and each invariant has a failing example.

### Phase 2: Lexicon and grammar

- [x] `kb/lexicon.mg`: every token kind, keyword and sigil, with modes and the `abbreviates` pairs. All 181 token kinds and the 100 reserved spellings with `lexes_as` (schema v2), 15 more spellings with a keyword reading (punctuation such as `?=`, and identifiers such as `cons`), and 37 `abbreviates` pairs. The mode is the class (`Sigil` agent, `Keyword` human, `Token` both). A test prints every `.mg` source in both modes and holds `fmt.rs` to the pairs. **M [H]**
- [x] Check `ontology.rs`'s `SIGILS` against the pairs: every pair's sigil has a row naming its keyword. *(`every_abbreviation_has_a_sigil_row`; `v`/`m` now say `val`/`var`, and 8 rows were added: `+`, `hx`, `xn`, `:`, `~>`, `R`, `cons`, `topo`.)* `grammar.rs`'s builtins map sigils to their *Rust* equivalents (`v` -> `let`), which is a different relation and correct as it stands. **S [H]**
- [x] Conformance test: `lexer::KEYWORDS` and the token kinds equal the lexicon facts, in both directions. The variants are read from `lexer.rs`'s source, since the enum has no runtime list. **S [H]**
- [x] `kb/grammar.mg`: the productions of `parser.rs`, each with an id, written by reading the parser function by function (48 `parse_*` functions). 148 productions, 289 `rhs` alternatives in EBNF (schema v4 adds `rhs`; `derives` is read off it), each naming its parser function and lines. Transcribed by seven subagents, one per section, then reconciled: the `unreachable_production` invariant found the 11 helpers two sections had both defined. 26 `GAP` comments mark where the grammar is a superset (layout, spelling tests on identifiers, lookahead). **L**
- [x] A recogniser that runs the grammar facts directly: `recognise.rs`, Earley over token kinds, so left recursion and ambiguity need no reshaping of the facts. **M**
- [x] Conformance, corpus direction: all 115 `.mg` sources the kernel parser accepts are recognised by the grammar facts. A ratchet list (`NOT_YET`, empty) can only shrink. Deleting the `StructDef` alternative fails 53 sources. **S**
- [x] Conformance, generation direction: sentences sampled from the grammar facts are accepted by the kernel parser, or the mismatch is recorded as a known gap fact. *Done:* 297 of 300 sentences sampled from `kb/grammar.mg` and written out with the lexicon's spellings parse; a test holds it to 98% and requires each refusal to stop in a production the grammar marks `GAP` (`Generics`, `DoComposeBody`). The gaps are `GAP` comments plus that list, not facts yet. Getting there took quoted spellings in the EBNF (`'block'`, `'as'`, `'self'`, the train and swarm labels), so a text test in the parser is exact in the grammar. Before them, 204 of 300 parsed. Earlier: of 249 one-token deletions from the corpus that the parser rejects, the grammar rejects 243, and a test holds it to 95% (a catch-all `Module` alternative drops it to 0). Every token kind the lexer produces now has a spelling in the lexicon (171 of 181; the 10 without are named, with why), so a sampled sentence can be written out as source. **M**
- [x] Bootstrap check: the files in `kb/` parse with the kernel and are recognised by their own grammar facts. *(They are in the corpus, `kb/grammar.mg` included.)* **S**
- [x] Parse errors name the production they were in. `ParseError::production` is stamped by `Parser::within`, innermost first; 44 `parse_*` functions are wrapped. The CLI prints `(in StructDef)`, RAP adds a `production` field, and a test reads the names from `parser.rs` and requires each to be a production of `kb/grammar.mg`. **M**
- [x] Generate the `sigils`, `keywords` and `ast_kinds` sections of `ontology.rs` from the knowledge base and delete the hand tables. The JSON is byte-identical apart from the new `schema` key. **S [H]** *Done, with one difference:* `SIGILS`, `KEYWORD_DOCS` and `AST_KINDS` are gone; schema v5 adds `summary` and `sigil_row`, and `kb/grammar.mg` states the 20 `AstKind`s with the production that `parses_as` each. The `keywords` section is byte-identical (and `the_keywords_section_is_the_lexer_table` holds it to `lexer::KEYWORDS`); `sigils` and `ast_kinds` hold the same rows in name order, since facts are a set. No `schema` key was added.

**Done when:** no table of tokens, keywords, sigils or productions is maintained by hand outside `kb/` and the kernel, and the kernel is held to `kb/` both ways.

### Phase 3: Forms and lowerings

- [x] `kb/forms.mg`: the seven form classes as instances, and a `lowers_to` fact for each lowering the compiler performs: sigil ↔ human (`fmt.rs`), source → AST → HIR, `net`/`block` → RMIL ops → ABL container, and ABL → source (decompile). **M** *Done:* 13 forms (one per class, six `Declarative`: net, kb, agent, swarm, train, evolve), 16 lowerings; 4 round-trip, the other 12 state 36 losses. Read from the code by a Haiku survey and checked. Decompile loses more than the plan assumed: besides regenerated layer names, `RES_ADD` is skipped unrecorded and a `Par` is flattened. HIR is not a tree here: types and effects are kept per name. `module_lowering_round_trips_through_codec` never decoded; it now compares the decoded expression.
- [ ] `kb/meaning.mg`: IR ops generated from `rmi::lang::Op::ALL` and `OpMeta`, layers from `abl_bridge::layer_name_to_op`, types, effects. **M [H]**
- [ ] A round-trip test generated for every `lowers_to` fact, checking exactly the stated loss: layer names regenerated on decompile, and nothing else. **M**
- [ ] Conformance: every op in `rmi::lang::Op` has a fact and every fact an op. **S [H]**
- [ ] Generate the `ir_ops`, `op_families`, `layer_map`, `types`, `effects` and `abl` sections of `ontology.rs`. **S [H]**

**Done when:** the four-forms claim in the README is a set of facts, each with a passing round-trip test.

### Phase 4: Rules

- [ ] `kb/rules/safety.mg`: the 255 SKB rules as facts, generated once from `builtin_rules()` and then made canonical. `builtin_rules()` and `skb/` are generated from them. **M**
- [ ] `kb/rules/diagnostics.mg`: every diagnostic code (22 `E0xxx` codes, and the warning classes), with its enforcer. **M [H]**
- [ ] Conformance: every code the compiler emits is a fact; every fact has a test that triggers it. **M**
- [ ] `kb/rules/contracts.mg` and `kb/rules/grammar.mg`: what `@req`, `@ens` and `@inv` mean and which component checks each; grammar constraints such as the depth limit. **S**
- [ ] Every `Diagnostic` the compiler produces carries its rule id. Includes fixing the human `--check` line that prints no E0560 code. **M**
- [ ] Units check (E0562) runs inside contract clauses, which it skips today. **S**
- [ ] Record what no rule checks yet, as gap facts: `@inv` at run time, a `spec` with no function, an undeclared effect. They become queryable rather than living in prose. **S**

**Done when:** every refusal the compiler can produce is traceable to one rule fact, and every rule fact to a test.

### Phase 5: Symbolic engines read the knowledge base

- [ ] `prototype/src/verdict.rs`: the shared verdict type. **S**
- [ ] `--check`, `--eval` contract checks and `--json` output return it. **M**
- [ ] The arena gate (`arena/src/substrate.rs`) returns it; the 10 `Refusal` classes become rule facts; `MAX_DEPTH` is read from `kb/rules/grammar.mg`. Arena results are byte-identical. **M**
- [ ] `arena physics-task` refusal classes map to the same rule facts. **S [H]**
- [ ] The knowledge-base export to `rmi::core::ontology::Ontology` generalises `rmi_ontology_adapter.rs` from safety rules to every fact. **M**

**Done when:** there is one verdict type and every component that accepts or refuses returns it.

### Phase 6: Neural engines consume the knowledge base

- [ ] `kb/profiles/arena.mg`: the arena's program space as a profile of the full grammar (its productions, the `usize` list fragment, the depth limit). **M**
- [ ] `arena/src/grammar.rs` builds its productions from `Kb::profile("arena")`; seeds reproduce byte-identically. **L**
- [ ] `describe_grammar` text is generated from the profile; the `GOLDEN_OFF` test still holds. **M**
- [ ] A grammar mask for any token-level generator: given a prefix, the allowed next tokens, computed from the grammar facts. **M**
- [ ] The LLM proposer (X1, blocked on an API model and key) is prompted from `Kb::profile` and the rule facts, and its output is decided by the shared verdict. **M**
- [ ] Learners get per-rule refusal counts as features where they help, measured against the current learners. **M**
- [ ] Concept embeddings attach to the `rmi` export for retrieval; an agent can ask "which rules constrain this construct" over RAP. **M**

**Done when:** no generator, prompt or learner holds its own copy of the grammar.

### Phase 7: Evidence and the learning loop

- [ ] An append-only evidence log (refusals and measurements) whose entries refer to rule, artifact and component facts by id. **M**
- [ ] The arena's reward and credit read verdicts from it, rather than re-deriving refusal classes. **M**
- [ ] A report: refusals per rule per generator, over a standard arena run. This shows which rules the neural side keeps breaking. **S [H]**
- [ ] Existing measurements in `MEASUREMENTS.md` keep their sources; new ones name the evidence entries they came from. **S**

**Done when:** every number a generator learns from is an evidence entry tied to a rule.

### Phase 8: The knowledge base grows through the gate

- [ ] A proposal format: a diff to `kb/*.mg` with its author (a component or a person). **M**
- [ ] The knowledge-base gate: schema fit, every invariant, the conformance suite from Phases 2–5, and no rise in measured cost on a fixed benchmark. **L**
- [ ] Content-addressed versions of the knowledge base, recorded in the evidence log. **M**
- [ ] `germline` hands a knowledge-base version on with the model that was trained against it. **M**
- [ ] Grammar extensions (`prototype/src/grammar.rs`, promotion at 100 uses) become proposals; promotion is admission by the gate. **M**
- [ ] `forge` blocks are registered as artifact facts. **M**
- [ ] A first end-to-end run: a generator proposes a grammar extension, the gate admits or refuses it with rule ids, and the result is measured. **L**

**Done when:** the system has changed its own knowledge base once, through the gate, with the change and its measured effect on record.

### Phase 9: Documentation and retirement

- [ ] `ARCHITECTURE.md` describes the three parts and the schema, using the Phase 0 census. **M**
- [ ] Remove every hand table that a phase above made redundant, and every comment that says "keep in sync". **S [H]**
- [ ] `DOCS.md`, `README.md` and the pin check cover the new documents and counts (facts per class, invariants). **S [H]**

---

## 7. Order and dependencies

```text
Phase 0 ─► Phase 1 ─► Phase 2 ─► Phase 3 ─┐
                  └──► Phase 4 ───────────┼─► Phase 5 ─► Phase 6 ─► Phase 7 ─► Phase 8
                                          │                                     │
                                          └──────────────── Phase 9 ◄───────────┘
```

- The critical path is Phase 1 (the schema and negation in `logic.rs`), then
  the grammar facts in Phase 2, which is the largest single item.
- Phases 3 and 4 can run beside Phase 2 once Phase 1 is done.
- Phase 6's LLM item stays blocked on X1; the rest of Phase 6 does not need it.
- The arena research track in `IMPLEMENTATION_PLAN.md` can continue throughout.
  Its generator moves onto the knowledge base in Phase 6, with byte-identical
  seeds as the condition.

---

## 8. Open questions

1. **Is the knowledge base compiled in, or a store the running system queries
   and writes?** This plan compiles it in (embedded `kb/*.mg`) and keeps
   evidence in a separate log, because a compiled base can be checked before
   it is used. Phase 8 lets it change, but only between versions.
2. **Does the schema include the components themselves** (the `Component`
   class), or only the language? This plan includes them, because `enforced_by`
   and `proposed_by` need something to point at.
3. **Does project metadata belong in the knowledge base at all,** or only in
   `ontology.rs`'s RAP output? This plan keeps it in a separate namespace.
4. **Should the parser eventually be generated from the grammar facts,** rather
   than conformance-tested against them? This plan keeps the hand-written kernel,
   because diagnostics quality lives in it, and revisits the question after
   Phase 2 has measured how far the two agree.

---

## 9. Definition of done

- [ ] Every piece of language knowledge is a fact in `kb/`, typed by `kb/schema.mg`.
- [ ] Every remaining hand-written table is conformance-tested against `kb/` in both directions.
- [ ] Every refusal, from the compiler or the arena, names one rule fact.
- [ ] Every generator and prompt is derived from a grammar profile.
- [ ] The knowledge base has changed once through its own gate, with the measured effect recorded.

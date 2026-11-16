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
| Diagnostic codes spread over `hir.rs`, `verify.rs` and others (19 live `E0xxx` codes) | Each code is a rule fact naming its enforcer; every diagnostic carries a rule id |
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
- [ ] Every `Diagnostic` code the compiler can emit is a fact, and every `Diagnostic` fact is emitted by some test. *(Audited 2026-11-04. The second half holds: all 26 facts have a witness, and `every_diagnostic_happens` runs them. The first half holds for the 19 live `E` codes, which are facts since Phase 4. The builder's 26 `B`, `K`, `A`, `S` and `U` codes are facts too, as `BuildRule`s rather than `Diagnostic`s: `kb/rules/builder.mg`, held to `builder.rs` both ways by its tests. `E0277`, `E0505` and `E0599` sit only in `aci.rs`'s pattern table and are listed by name in `TABLE_ONLY`.)*
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

- [x] Merge the stack #38–#162, this plan included, in order (needs the user). **S** *(Merged into `master` on 2026-11-06 with merge commits, oldest first.)*
- [x] Settle the open questions in section 8. **S** *(Settled on 2026-11-06: each as this plan proposes. Section 8 records them, with the scope decisions taken at the same time.)*
- [x] Census every module in `prototype/src` (77 files and `bin/`) into neural, symbolic, knowledge base, interface, or outside the design (for example `crdt.rs`, `consensus.rs`, `hot_reload.rs`). Record it in `ARCHITECTURE.md`. Outside-the-design modules are kept but not extended. **S [H]** *(In `ARCHITECTURE.md`, "Module census", held to the directory by `prototype/tests/census.rs`.)*

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
- [x] `kb/meaning.mg`: IR ops generated from `rmi::lang::Op::ALL` and `OpMeta`, layers from `abl_bridge::layer_name_to_op`, types, effects. **M [H]** *(Schema v6: 107 ops, 7 families, 108 layer spellings, 30 types, 24 effects. Generated once by a Haiku subagent's script. `LAYER_SURFACE_NAMES` lost 10 names that were never layers, which `layer_map_section` had been filtering out; the JSON does not change. The 12 names `op_to_layer_name` gives ops that `layer_name_to_op` refuses are pinned as an exact list.)*
- [x] A round-trip test generated for every `lowers_to` fact, checking exactly the stated loss: layer names regenerated on decompile, and nothing else. **M** *Done, with a table rather than generated code:* `prototype/tests/lowerings.rs` holds a witness for each of the 38 `loses` facts and a check for each of the 4 `round_trips`, and a test requires the table and the facts to match both ways, so a claim with no check fails, and so does a check with no claim. "Nothing else" is checked for decompile only: all 34 nets in the repository re-lower to the same hash unless a stated loss applies. It is not checked for the other lowerings. The plan's wording was wrong: decompile loses more than layer names. The witnesses refuted one fact as first written (`non-neural ops: recorded in skipped`; `RESOLVE` and `SPAWN` have layer names, so a kb or agent decompiles to a net with that layer) and found two losses nobody had stated (a `composition` is discarded when `forward` reaches every layer; `IDENTITY` drops unrecorded, so an unknown layer vanishes). One gap remains: `AstTree -> HirTables` observes the resolver and effect maps, not types, since `TypeChecker` has no per-node query.
- [x] Conformance: every op in `rmi::lang::Op` has a fact and every fact an op. **S [H]** *(`every_op_has_a_fact_and_every_fact_an_op`, every field; also `every_op_family_is_stated`, `every_layer_spelling_is_stated`, `every_type_and_effect_is_stated`.)*
- [x] Generate the `ir_ops`, `op_families`, `layer_map`, `types`, `effects` and `abl` sections of `ontology.rs`. **S [H]** *(Schema v7 adds `canonical_layer`, `abl_field` and `abl_media_type`. `TYPES`, `OP_FAMILIES`, `LAYER_SURFACE_NAMES` and `EFFECTS` are deleted, with their history moved to comments in `kb/meaning.mg`. `ir_ops`, `op_families` and `abl` are byte-identical; `layer_map`, `types` and `effects` have the same rows in name order. `abl`'s magic and version stay read from the code's constants. A new op in `rmi` now fails `every_op_has_a_fact_and_every_fact_an_op` until it has facts, where it used to reach the JSON by itself.)*

**Done when:** the four-forms claim in the README is a set of facts, each with a passing round-trip test.

### Phase 4: Rules

- [x] `kb/rules/safety.mg`: the 255 SKB rules as facts, generated once from `builtin_rules()` and then made canonical. `builtin_rules()` and `skb/` are generated from them. **M** *(Schema v8: `safety_rule`, `rule_fix`, `rule_tag`, `RuleDatabase`, `Severity`. `skb.rs` loses 2,900 lines of tables; `builtin_rules()` reads the one file, not the whole knowledge base, so a compile that consults a rule does not pay for a full load. The rule files in `skb/` are byte-identical, so the policy digest is unchanged. `unenforced_rule` now exempts `SafetyRule`: no component executes these rules, and stating an enforcer would be false.)*
- [x] `kb/rules/diagnostics.mg`: every diagnostic code (22 `E0xxx` codes, and the warning classes), with its enforcer. **M [H]** *(Schema v9: `diagnostic_row`, `warning_carries`, `gap`. The count was wrong: 19 codes are live, the 17 categories plus E0000 and E0710, and there are 7 warning classes. E0499 and E0503 were only in comments; E0505, E0277 and E0599 are only in `aci.rs`'s table, which no production path reaches. E0308 has three emitters, so `rule_enforced_twice` is dropped. E0382 and E0502 state a `gap`: no pass detects a move or a borrow, and only `heal`'s message classifier assigns them.)*
- [x] Conformance: every code the compiler emits is a fact; every fact has a test that triggers it. **M** *(`prototype/tests/diagnostics.rs`: 26 witnesses, each with a control that must reach the checks. It also requires `DiagnosticCategory` to match the facts, each emitting module to be stated, and each quoted code in production source to be a fact. E0428, E0550, E0001 and E9999 had no test on their code. Limit: the `every_emit_site_is_stated` scan reads category names per module, so it does not count sites, and `parser` and `lexer` are held by their witnesses alone.)*
- [x] `kb/rules/contracts.mg` and `kb/rules/grammar.mg`: what `@req`, `@ens` and `@inv` mean and which component checks each; grammar constraints such as the depth limit. **S** *(Schema v10: `contract_row`, `clause_checked`, `constraint_row`. `prototype/tests/rules.rs` probes every cell of kind × placement × component against the compiler, so a cell stated as unchecked is tested to be unchecked: 48 cells then, 25 checked. 14 grammar constraints, each with a witness; `max_depth` and `max_nest` are held to the arena constants by an arena test, and `MAX_NEST` gains its first test. Three are gaps: a clause above an item that is not a function or struct is silently dropped, layout blocks are outside `kb/grammar.mg`, and an overflowing tensor dimension reads as 0.)*
- [x] Every `Diagnostic` the compiler produces carries its rule id. Includes fixing the human `--check` line that prints no E0560 code. **M** *(`categorized` sets `id` from the category, so every pass does, and the human `--check` prints `[code]` on every line, not only E0560: no line printed one before. Lexer, parser and shape errors bypass `Diagnostic`, so `main.rs` prints their codes itself. A warning carries its code (`warning_carries`), not its class name, so `--check --json` stays the same; two classes share E0308 and two E9999, and the message tells them apart. `bin/reliability_bench.rs` still builds a bare `Diagnostic` for `heal`, which classifies it.)*
- [x] Units check (E0562) runs inside contract clauses, which it skips today. **S** *(`units.rs` never read a clause. Now every clause `contract::collect` reaches is checked with the signature in scope and `result` carrying the return dimension. The same reach as typechecking, so clauses on methods, in `mod`, on structs and in a spec with no function are not checked. The contract grid gains a `units` stage: 60 cells, 31 checked.)*
- [x] Record what no rule checks yet, as gap facts: `@inv` at run time, a `spec` with no function, an undeclared effect. They become queryable rather than living in prose. **S** *(`gap` facts on 10 subjects, and `every_gap_has_a_check` requires the list to match. The third is a run-time gap: `--check` refuses an undeclared effect and `--eval` performs it, so E0550 has an enforcer and a gap both. A private unannotated function inferring silently is not a gap: it is the design, and a public caller reports it.)*

**Done when:** every refusal the compiler can produce is traceable to one rule fact, and every rule fact to a test. *(Met for diagnostics, contracts and grammar constraints. Not for the 255 safety rules: nothing executes them, so they have no test that triggers them, and `unenforced_rule` exempts them. A warning traces to a code two classes may share. The parser's plain "expected X" errors all trace to E0001.)*

### Phase 5: Symbolic engines read the knowledge base

- [x] `prototype/src/verdict.rs`: the shared verdict type. **S** *(Beside the existing `Evidence` vocabulary: `Evidence` says what stands behind a claim, a `Verdict` what a gate decided. `accepted` is derived from `refusals` rather than stored as §5 drew it, so the two cannot disagree.)*
- [x] `--check`, `--eval` contract checks and `--json` output return it. **M** *(`check::check_source` takes the pass list out of `main.rs` and returns findings and a `Verdict`; `--check --json` prints the same records plus a `verdict` field. The human `--check` still prints as it goes and is held to the verdict by `the_human_check_agrees_with_the_verdict`. `Verdict::of_eval_error` reads a contract failure from `eval`'s error string, because `eval` errors are strings: it names the clause kind, so it does not know the clause.)*
- [x] The arena gate (`arena/src/substrate.rs`) returns it; the 10 `Refusal` classes become rule facts; `MAX_DEPTH` is read from `kb/rules/grammar.mg`. Arena results are byte-identical. **M** *(`kb/rules/refusals.mg`, schema v11. `Outcome::verdict` gives the shared verdict; `prepare` and the credit code keep the `Refusal` class, so no run changes. `type` and `effect` have gate rules of their own: a class is coarser than a code, so naming E0308 or E0550 would be false. Departure: `MAX_DEPTH` stays a constant, held to its fact by `the_limits_are_the_stated_ones`, because the arena uses it as one and reading it would add a knowledge-base parse to start-up.)*
- [x] `arena physics-task` refusal classes map to the same rule facts. **S [H]** *(`Verdict::shared`. A contract failure names `requires` or `ensures` from its message.)*
- [x] The knowledge-base export to `rmi::core::ontology::Ontology` generalises `rmi_ontology_adapter.rs` from safety rules to every fact. **M** *(`kb_to_ontology`. Every fact is a row on the concept it is about, and `every_fact_survives_the_export` reads them all back. RMI's relation types are a closed set, so only predicates with the same meaning are also edges. No command calls it yet.)*

**Done when:** there is one verdict type and every component that accepts or refuses returns it.

### Phase 6: Neural engines consume the knowledge base

- [x] `kb/profiles/arena.mg`: the arena's program space as a profile of the full grammar (its productions, the `usize` list fragment, the depth limit). **M** *(Schema v12, `Kb::profile`, `Grammar::restricted`. 41 of the 148 productions, measured, not mapped by hand: tests recognise the 19 forms and 60 sampled programs with those alone, and each production is needed. The 19 forms and 14 limits are held to `grammar.rs`. `grammar.rs` still has its own tables: the next item.)*
- [x] `arena/src/grammar.rs` builds its productions from `Kb::profile("arena")`; seeds reproduce byte-identically. **L** *(`grammar::space()`. It reads only `kb::PROFILE_FILES`, since a full load took 15 s in a debug build, and a test holds that read equal to the checked load. This replaces #131's departure: `max_depth` is now read. Measured: `a_seed_draws_what_it_always_drew` pins digests taken from the old code, and two seeded runs differ from the old binary only in measured joules and seconds and what derives from them. `Sampler` still holds the meaning of each form by name, which is code, not a table.)*
- [x] `describe_grammar` text is generated from the profile; the `GOLDEN_OFF` test still holds. **M** *(Schema v13, `form_example` and `profile_text`. `GOLDEN_OFF` holds, and a digest of the second-order prompt, taken from the old code, pins that too. Examples were built by `render` and are now facts, with a test holding each to what `render` writes.)*
- [x] A grammar mask for any token-level generator: given a prefix, the allowed next tokens, computed from the grammar facts. **M** *(`Grammar::mask`, over the Earley chart; with `Grammar::restricted` it masks to a profile. A test holds it exact against the recogniser. No token-level generator exists yet, so nothing calls it. At the grammar's `GAP`s it can allow a token the parser would refuse. It recomputes the chart from the prefix on each call.)*
- [ ] The LLM proposer (X1, blocked on an API model and key) is prompted from `Kb::profile` and the rule facts, and its output is decided by the shared verdict. **M**
- [ ] Learners get per-rule refusal counts as features where they help, measured against the current learners. **M** *(Not built, 2026-11-02: no place found where it helps. The learners are `ByteLearner`s over program output, and a refused program has none, so they never see a refusal. The one learner that does, the grammar policy, gets a refusal as reward 0, and `Scored` carries no class. In two seeded runs the grammar agents' refusals were 1 to 3 of 22 to 30 programs, all `empty`; `depth` refusals came only from the mutator and attacker, which `--bound-grafts` already removes. Reopen if a learner that sees refusals appears, or the policy's refusal rate is shown to matter.)*
- [x] Concept embeddings attach to the `rmi` export for retrieval; an agent can ask "which rules constrain this construct" over RAP. **M** *(RAP `kb/constrains`. `constrained_by` was declared but held no facts: it now has them for the grammar constraints and contracts, by a test exactly those something enforces; a constraint that is only a `gap` refuses nothing. Gate rules, diagnostics and safety rules constrain nothing yet: no fact ties them to a construct. The embedding is a hash of character trigrams and words, not a learned model, so it finds near spellings, not near meanings; RAP uses it only to suggest constructs for a name it does not know. `mod` and `var` are each a keyword and an arena form, and the export puts both sets of rules on the keyword.)*

**Done when:** no generator, prompt or learner holds its own copy of the grammar.

### Phase 7: Evidence and the learning loop

- [x] An append-only evidence log (refusals and measurements) whose entries refer to rule, artifact and component facts by id. **M** *(`arena/src/evidence.rs`, `arena --evidence <log.jsonl>`. Each refusal names its `refused_by` rule; each reward is a measurement. Components are the agent kinds, now `Component` facts in `kb/rules/refusals.mg`. Artifacts are the SHA-256 of a program's source: programs are not knowledge-base facts, and a refused one has no canonical hash. A hash chain makes "append-only" checkable: `verify` refuses an edited, reordered or front-cut log, and nothing is appended to one. A cut at the end is not detectable. Only the arena writes it; physics-task and checkpoint measurements are not logged yet.)*
- [x] The arena's reward and credit read verdicts from it, rather than re-deriving refusal classes. **M** *(Each reward is logged where it is assigned. A round's credit, and the report's refusals and each agent's credit, are read from the log; the second count of refusals, keyed by serialising the class, is gone. The log is in memory: a run reads its own entries, not the file. Measured: three seeded runs, alignment and compression reward, give bit-identical credit, scores, refusals, hypervolume added and best programs before and after. The reward is still computed in `arena.rs` before it is logged; what moved is where every later figure reads it from.)*
- [x] A report: refusals per rule per generator, over a standard arena run. This shows which rules the neural side keeps breaking. **S [H]** *(`arena evidence <log.jsonl> [--json OUT]` verifies a log and tabulates it. Over three standard runs (`MEASUREMENTS.md`, "Refusals per rule per generator"): the neural generator, `grammar`, breaks `gate_empty` and rarely `gate_fuel`, 3.1% of its programs; the mutator's grafts break `max_depth`, 32.3%; uniform sampling 5.9%, all `gate_empty`. Only `grammar` is neural; the LLM proposer would be the second column, and is blocked.)*
- [x] Existing measurements in `MEASUREMENTS.md` keep their sources; new ones name the evidence entries they came from. **S** *(`arena evidence <log> [--seq A..B]` prints a citation, `evidence: <log> seq A–B, head sha256:<hash>`, the hash of the log's last line, which pins every entry before it. `MEASUREMENTS.md` states the convention, and the refusals-per-rule section cites its log and each seed's range. A convention, not a check: logs are not committed, so CI cannot test a citation against one. Only the arena writes a log, so only arena figures can cite one.)*

**Done when:** every number a generator learns from is an evidence entry tied to a rule.

### Phase 8: The knowledge base grows through the gate

- [x] A proposal format: a diff to `kb/*.mg` with its author (a component or a person). **M** *(`prototype/src/kb_proposal.rs`. A proposal is JSON: an author (`{"component": …}`, which the knowledge base must state as a `Component`, `NeuralComponent` or `SymbolicComponent`, or `{"person": …}`), a summary, `base` (the version it was written against) and edits. An edit replaces text that occurs exactly once in a `kb/*.mg` file, or appends when `before` is empty, which is the diff: exact, and refused rather than fuzzily applied when the file has moved. `version` is the SHA-256 of the files in load order, line endings normalised; item 3 records it. `apply` gives the new files or every reason it cannot. It cannot add a file, since the file list is compiled in. Nothing calls it yet: admission is the gate's, the next item.)*
- [x] The knowledge-base gate: schema fit, every invariant, the conformance suite from Phases 2–5, and no rise in measured cost on a fixed benchmark. **L** *(`scripts/kb-gate.sh <proposal.json>`. Six rules, `GateRule` facts enforced by `kb_gate` in `kb/rules/refusals.mg`: `kb_base`, `kb_author`, `kb_edit` and `kb_schema` are decided by `kb-gate admit` (`kb_proposal::admit`: the proposal applies, and the changed files load, which checks the schema and every invariant); `kb_conformance` runs the whole prototype and arena suites, a superset of the conformance tests, in a worktree of HEAD holding the changed files, since the knowledge base is compiled in; `kb_cost` compares the fuel of a fixed `--cost fuel` arena benchmark (4 rounds of 16 programs) at HEAD and under the change, seed by seed, and refuses at p < 0.05 by a one-sided sign test over eight seeds, which is a rise on at least seven. It first compared one seed and refused any rise, which was a draw (`MEASUREMENTS.md`, "A profile change at the knowledge-base gate"). Run end to end, about 10 minutes each: a component fact proposed by `grammar` was admitted, fuel 15,629 both sides; `while` lexing as `KwLoop`, proposed by a person, loaded but was refused by `kb_conformance`, naming the three tests it broke. Neither of those two touched what the generators sample; the first `kb_cost` refusal is under the last item of this phase. Two limits: the gate stops at the first stage that refuses, and a test that pins a count of facts refuses any proposal that changes the count, since a proposal edits only `kb/*.mg`; grammar extensions (item 5) will meet that. The arena's golden tests no longer do this: each holds under the profile it was recorded under, by the profile's content hash, and under another checks only that a seed draws the same twice.)*
- [x] Content-addressed versions of the knowledge base, recorded in the evidence log. **M** *(The version is `kb_proposal::version`, the SHA-256 of the `kb/*.mg` files in load order (#143). Every arena run's evidence now opens with a `knowledge_base` entry carrying it, so each refusal and reward after it is tied to the knowledge base that decided it; `arena evidence` lists the versions a log covers. The gate's two benchmark runs append to one log, HEAD's version and then the changed one. Logs written before this have no such entry and still verify.)*
- [x] `germline` hands a knowledge-base version on with the model that was trained against it. **M** *(`Generation.knowledge_base`, an opaque content address, so `germline` still knows nothing of `mage_prototype`. It follows `definitions` (plan 2.4) exactly: `Workload::knowledge_base` supplies it, the runner sets it on each proposed generation, the journal's `Proposed` entry carries it and replay restores it, so a champion, a successor and a fallback each say which knowledge base they were trained under. Skipped when absent, so earlier generations and journals hash as they did. The arena's succession workload supplies `kb_proposal::version` for each session it ran. Nothing yet refuses a handoff across knowledge-base versions; that would be a gate rule, and none is asked for.)*
- [x] Grammar extensions (`prototype/src/grammar.rs`, promotion at 100 uses) become proposals; promotion is admission by the gate. **M** *(`grammar::promotion_proposal` turns a candidate (more than 100 uses, not core, expanding to a stated `Keyword`) into a proposal by the `grammar_extensions` component to state its sigil in `kb/lexicon.mg`; `ExtensionRegistry::promote` makes an extension built-in only when a knowledge base states its `abbreviates` fact; `kb-gate promotion <sigil> <keyword> <uses> <description>` writes the proposal. **Finding: nothing can be admitted today.** All 37 keywords the lexicon states already have a sigil, and the invariant `keyword_two_sigils` allows one, so every keyword promotion is refused by `kb_schema`, as `@zz` for `while` was. An extension whose expansion is not one keyword (`pub fn`, `Box<_>`) cannot be stated as `abbreviates` at all. An admissible promotion needs a new keyword, which is a lexer change, and a proposal edits only `kb/*.mg`. So the first admitted change (item 7) will not be a sigil: it has to be something the system reads from the knowledge base at run time, such as the arena's grammar profile.)*
- [x] `forge` blocks are registered as artifact facts. **M** *(`BlockStore::facts` states each block in a forge store as `instance_of("sha256:…", Block)` (a subclass of `Artifact`), with `block_name` and `block_signature` (schema v14). It is text: `forge` still parses no MAGE and depends on nothing of the compiler's. `arena/tests/forge_facts.rs` loads a store's facts beside the knowledge base, so the schema checks them, and shows a relation the schema lacks is refused. Effects are not stated: forge does not check them, and `Unchecked` is not a fact about a block. The facts are generated from a store on demand, not committed to `kb/`: a store is per machine and changes as blocks are published.)*
- [x] A first end-to-end run: a generator proposes a grammar extension, the gate admits or refuses it with rule ids, and the result is measured. **L** *(Done, 2026-11-03, for the one extension the knowledge base alone can make: a wider profile limit. A new form needs code, and a sigil cannot be admitted (above). `limit_proposer` turns a run's rewards into a proposal. Of the three it proposed over four seeds, `take` 48→64 is admitted on both seed sets, and `cycle_len` and `cycle_elems` are refused by `kb_cost` on seeds 1–8 and admitted on seeds 9–16. Measured at 40 rounds of 48 programs over eight seeds that did not choose it, the admitted change shows no detectable effect on fuel, held-out bits per byte or hypervolume (`MEASUREMENTS.md`, "A generator's proposals at the knowledge-base gate"). So it is not applied. An earlier run, with a person's proposal to widen `range`, is under "A profile change at the knowledge-base gate".)*

**Done when:** the system has changed its own knowledge base once, through the gate, with the change and its measured effect on record. *(Not met: a generated change has been admitted and measured, and it is not applied. It shows no benefit, and applying a profile change redraws every seed's programs, so whether to apply one is the user's decision.)*

### Phase 9: Documentation and retirement

- [x] `ARCHITECTURE.md` describes the three parts and the schema, using the Phase 0 census. **M**
- [x] Remove every hand table that a phase above made redundant, and every comment that says "keep in sync". **S [H]** *(The one still redundant was `ExtensionRegistry::register_builtins`, 47 sigils by hand, of which 19 had no `sigil_row` and `v` was `let` against the facts' `val`. So Phase 2's "Done when" was not met until now. `grammar/list` now serves the 62 `sigil_row` facts, which changes what RAP returns: `^`, `$`, `@req` and the like are gone, every built-in's namespace is `core`, and a sigil with no `abbreviates` has an empty `rust`. `RuleDatabase::ALL`, the limit proposer's `TESTED` and `ROW_PREDICATES` are now held by tests. Kept: the kernel's tables, held both ways; the tables `ontology.rs` serves for the project namespace, which are not language facts; the refusal labels and rules, held by tests; and `builder.rs`'s `OPS`, only partly stated in `kb/`.)*
- [x] `DOCS.md`, `README.md` and the pin check cover the new documents and counts (facts per class, invariants). **S [H]**

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

Settled on 2026-11-06, each as proposed below.

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

Decided at the same time:

5. **The 25 names `BUILTIN_EFFECTS` attributed that cannot be called** (`open`,
   `now` and the like) are removed from it, from the facts and from
   `MAGE_SPEC.md`'s table. Their effects are reached by annotation or through a
   capability handle (`fs.read(…)`, `time.now()`), so attributing a bare name
   only claimed a word a user function might want.
6. **The admitted `take` 48→64 is not applied.** The knowledge-base gate item in
   section 9 stays open until a proposal shows a measured benefit.
7. **`germline` is out of scope** for the refusal item: it is neither the
   compiler nor the arena.
8. **The generators in scope** for the grammar-profile item are `command.rs`,
   `agents.rs`, `nl_engine.rs`, `synthesis.rs` and `codegen_bridge.rs`.
   `training/prompts/*.md` is out of scope, because no code loads it.

---

## 9. Definition of done

None of these is met yet. An audit on 2026-11-04 read the code rather than the boxes above; these notes are its findings, and it ran nothing.

- [ ] Every piece of language knowledge is a fact in `kb/`, typed by `kb/schema.mg`. *(Partly. Syntax, grammar, forms, IR ops, layers, layer grades, effects, the vocabulary, attribute abbreviations, builtin names and their grades, and rules are facts. Still only in code: `eval.rs`'s builtin names (84). The 25 names `BUILTIN_EFFECTS` attributed that could not be called (`open`, `now` and the like) were removed (section 8, decision 5). Four of the graded names still cannot be called: `relu` and `clamp` in `KINKED`, `softmax` and `matmul` in `SMOOTH_BUILTINS`. Left in code as details of one analysis, not language knowledge: `elision.rs`'s `SAFETY_BOUNDS` and `ELIMINATED_TYPES`, `convexity.rs`'s `call_facts`, `autograd.rs`'s tape table.)*
- [ ] Every remaining hand-written table is conformance-tested against `kb/` in both directions. *(Partly. Both ways: `KEYWORDS`, `DiagnosticCategory`, the arena's refusal classes (`physics_task.rs` and `substrate.rs` between them), `RuleDatabase::ALL`, `TESTED`, `ROW_PREDICATES`, and the builder's 26 codes (`every_build_code_is_stated`, `every_build_rule_happens`), and `builder.rs`'s `OPS`, `TOPOLOGIES` and `CONSENSUS`, and `hir.rs`'s `CAPABILITY_NAMESPACES`, each stated as facts and held to them by `every_op_is_stated`, `every_topology_and_consensus_is_stated` and `every_capability_namespace_is_stated`, and `hir.rs`'s `ROLES` and `units.rs`'s `BASE_UNITS`, each stated as facts and held to them by `every_role_and_grant_is_stated` and `every_base_unit_is_stated`, and `resolve.rs`'s `VOCABULARY`, `elision.rs`'s `ATTRIBUTE_ABBREVIATIONS`, and `differentiable.rs`'s `SMOOTH_LAYERS` and `KINKED_LAYERS`, each stated as facts and held to them by `every_vocabulary_word_is_stated`, `every_attribute_abbreviation_is_stated` and `every_layer_grade_is_stated`, and `resolve.rs`'s `STD_FNS`, and `differentiable.rs`'s `KINKED`, `SMOOTH_BUILTINS` and `DISCRETE_BUILTINS`, each stated as facts and held to them by `every_builtin_function_is_stated` and `every_builtin_grade_is_stated`, and `resolve.rs`'s `SCALAR_MATH`, stated as facts and held to them by `every_scalar_math_name_is_stated`, and `effects.rs`'s `BUILTIN_EFFECTS` (7 names: `join` dropped because it never fired, and 25 because they could not be called), stated as facts and held to them by `every_builtin_effect_is_stated`, with `MAGE_SPEC.md`'s table of attributed names held to the same facts by `the_spec_table_of_attributed_names_is_the_stated_one`. No test against `kb/`: the tables of the item above, since they have no facts to hold them to.)*
- [ ] Every refusal, from the compiler or the arena, names one rule fact. *(Partly. Met for the arena, the compiler's `E` codes, the knowledge-base gate and the builder's 26 `B`, `K`, `A`, `S` and `U` codes (`BuildRule` facts, `enforced_by` the builder; for 14 of the 26 the `fix` an error emits is worded differently from the stated one the schema serves). `germline`'s `RejectReason` has no rule facts, and is out of scope (section 8, decision 7). A `grad_unchecked` warning falls to the catch-all `DiagnosticCategory::Other`.)*
- [ ] Every generator and prompt is derived from a grammar profile. *(Not met, though the arena's is. Its sampling and `describe_grammar` come from the profile; its parser still spells the signature that `profile_signature` states, and its sampler and parser dispatch by form name, which Phase 6 accepted as code. Holding their own syntax: `command.rs`'s prompt, `agents.rs`'s attacker rewrites, the templates in `nl_engine.rs` and `synthesis.rs`, `codegen_bridge.rs`'s header. `training/prompts/*.md`, which no code loads, is out of scope (section 8, decision 8). The LLM proposer is blocked on X1.)*
- [ ] The knowledge base has changed once through its own gate, with the measured effect recorded. *(Not met. A generated change has been admitted and measured, and it is not applied: Phase 8. It stays open until a proposal shows a measured benefit (section 8, decision 6).)*

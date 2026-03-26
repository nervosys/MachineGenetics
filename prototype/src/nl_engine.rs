// ── Natural Language Engine ─────────────────────────────────────────
//
// Provides an agentic AI interface to the MechGen language and compiler.
// Accepts natural language requests and produces MechGen source code by
// orchestrating the synthesis oracle, knowledge base, type checker,
// effect inference, and self-healing compiler passes.
//
// Architecture:
//   1. Parse NL request → structured Intent
//   2. Extract constraints (types, effects, contracts, KB facts)
//   3. Build SynthesisSpec from constraints + KB knowledge
//   4. Generate code via synthesis oracle
//   5. Validate through compiler pipeline (parse → check → heal)
//   6. Return validated MechGen source + explanation
//
// The NL engine is NOT an LLM wrapper — it is a deterministic,
// rule-based intent parser + template-driven code generator that
// uses the compiler's own subsystems to produce correct programs.

use crate::ast::*;
use crate::effects;
use crate::elision;
use crate::fmt;
use crate::heal;
use crate::hir;
use crate::lexer;
use crate::logic;
use crate::parser;
use crate::resolve;
use crate::synthesis::{CostEstimate, Strategy, SynthesisOracle, SynthesisSpec};
use crate::types;
use crate::verify;

use std::collections::BTreeMap;


/// Lex source code with panic recovery (the lexer can panic on malformed input).
fn safe_lex(source: &str) -> Result<Vec<lexer::Token>, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| lexer::lex(source)))
        .map_err(|_| "Lexer panic on input".to_string())
}
// ═══════════════════════════════════════════════════════════════════
// Intent — Structured representation of a natural language request
// ═══════════════════════════════════════════════════════════════════

/// The kind of thing the user wants to do.
#[derive(Debug, Clone, PartialEq)]
pub enum IntentKind {
    /// Generate a function: "create a function that adds two numbers"
    GenerateFunction,
    /// Generate a struct: "define a point type with x y coordinates"
    GenerateStruct,
    /// Generate an enum: "create an enum for colors"
    GenerateEnum,
    /// Generate a trait: "define a trait for serialization"
    GenerateTrait,
    /// Generate a neural network: "build a neural network for classification"
    GenerateNet,
    /// Generate a knowledge base: "create a KB for family relationships"
    GenerateKb,
    /// Generate an evolutionary block: "evolve a solution for traveling salesman"
    GenerateEvolve,
    /// Generate an agent: "create an agent that can read and write code"
    GenerateAgent,
    /// Generate a swarm: "create a swarm of 4 reviewer agents"
    GenerateSwarm,
    /// Explain existing code: "explain this function"
    Explain,
    /// Query the knowledge base: "who is the ancestor of alice?"
    QueryKb,
    /// Check code for errors: "check this code"
    Check,
    /// Optimize code: "make this function faster"
    Optimize,
    /// Add contracts to code: "add preconditions to this function"
    AddContracts,
    /// Refactor code: "rename this function"
    Refactor,
}

/// A parsed intent with extracted constraints.
#[derive(Debug, Clone)]
pub struct Intent {
    pub kind: IntentKind,
    pub name: Option<String>,
    pub description: String,
    pub params: Vec<(String, String)>,
    pub return_type: Option<String>,
    pub effects: Vec<String>,
    pub preconditions: Vec<String>,
    pub postconditions: Vec<String>,
    pub invariants: Vec<String>,
    pub kb_facts: Vec<(String, Vec<String>)>,
    pub kb_queries: Vec<(String, Vec<String>)>,
    pub source: Option<String>,
    pub extras: BTreeMap<String, String>,
}

impl Intent {
    fn new(kind: IntentKind, description: &str) -> Self {
        Self {
            kind,
            name: None,
            description: description.to_string(),
            params: Vec::new(),
            return_type: None,
            effects: Vec::new(),
            preconditions: Vec::new(),
            postconditions: Vec::new(),
            invariants: Vec::new(),
            kb_facts: Vec::new(),
            kb_queries: Vec::new(),
            source: None,
            extras: BTreeMap::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// NL Response — What the engine returns
// ═══════════════════════════════════════════════════════════════════

/// Result of processing a natural language request.
#[derive(Debug, Clone)]
pub struct NlResponse {
    /// Whether the request was successfully handled.
    pub ok: bool,
    /// Generated MechGen source code (human mode).
    pub code_human: String,
    /// Generated MechGen source code (agent mode).
    pub code_agent: String,
    /// Human-readable explanation of what was generated.
    pub explanation: String,
    /// Diagnostics from compiler validation.
    pub diagnostics: Vec<hir::Diagnostic>,
    /// Fix candidates if there were issues.
    pub fixes: Vec<String>,
    /// The parsed intent (for debugging/introspection).
    pub intent: Intent,
    /// Knowledge base query results, if applicable.
    pub kb_results: Vec<(String, Vec<Vec<String>>)>,
    /// Verification results.
    pub verification_summary: String,
}

impl NlResponse {
    fn error(intent: Intent, message: &str) -> Self {
        Self {
            ok: false,
            code_human: String::new(),
            code_agent: String::new(),
            explanation: message.to_string(),
            diagnostics: Vec::new(),
            fixes: Vec::new(),
            intent,
            kb_results: Vec::new(),
            verification_summary: String::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// NL Engine — The main interface
// ═══════════════════════════════════════════════════════════════════

/// The natural language engine. Connects NL intents to the compiler.
pub struct NlEngine {
    oracle: SynthesisOracle,
    /// Persistent knowledge base for cross-request reasoning.
    kb: logic::KnowledgeBase,
}

impl NlEngine {
    pub fn new() -> Self {
        let mut oracle = SynthesisOracle::new();

        // Register high-quality templates for common patterns.
        oracle.register_template(
            &Strategy::Functional,
            "fn {name}({params}) -> {return} {{ input.iter().fold(init, |acc, x| acc + x) }}",
            CostEstimate {
                token_count: 12,
                cyclomatic_complexity: 1,
                allocation_count: 0,
                effect_count: 0,
            },
        );

        Self {
            oracle,
            kb: logic::KnowledgeBase::new("nl_context"),
        }
    }

    /// Process a natural language request and return generated code + explanation.
    pub fn process(&mut self, input: &str) -> NlResponse {
        let intent = parse_intent(input);

        match intent.kind {
            IntentKind::GenerateFunction => self.generate_function(&intent),
            IntentKind::GenerateStruct => self.generate_struct(&intent),
            IntentKind::GenerateEnum => self.generate_enum(&intent),
            IntentKind::GenerateTrait => self.generate_trait(&intent),
            IntentKind::GenerateNet => self.generate_net(&intent),
            IntentKind::GenerateKb => self.generate_kb(&intent),
            IntentKind::GenerateEvolve => self.generate_evolve(&intent),
            IntentKind::GenerateAgent => self.generate_agent(&intent),
            IntentKind::GenerateSwarm => self.generate_swarm(&intent),
            IntentKind::Explain => self.explain_code(&intent),
            IntentKind::QueryKb => self.query_kb(&intent),
            IntentKind::Check => self.check_code(&intent),
            IntentKind::Optimize => self.optimize_code(&intent),
            IntentKind::AddContracts => self.add_contracts(&intent),
            IntentKind::Refactor => self.refactor_code(&intent),
        }
    }

    /// Add facts to the persistent knowledge base.
    pub fn add_knowledge(&mut self, predicate: &str, args: Vec<String>) {
        self.kb.add_fact(predicate, args);
    }

    /// Query the persistent knowledge base.
    pub fn query_knowledge(&mut self, predicate: &str, args: &[&str]) -> Vec<Vec<String>> {
        self.kb.query(predicate, args)
    }

    // ─── Code generation strategies ────────────────────────────────

    fn generate_function(&mut self, intent: &Intent) -> NlResponse {
        let name = intent
            .name
            .clone()
            .unwrap_or_else(|| extract_function_name(&intent.description));
        let params = if intent.params.is_empty() {
            infer_params(&intent.description)
        } else {
            intent.params.clone()
        };
        let return_type = intent
            .return_type
            .clone()
            .or_else(|| infer_return_type(&intent.description));
        let effects = if intent.effects.is_empty() {
            infer_effects(&intent.description)
        } else {
            intent.effects.clone()
        };

        // Query KB for domain knowledge relevant to this function.
        let kb_hints = self.query_kb_for_hints(&name, &params);

        // Build synthesis spec.
        let mut spec = SynthesisSpec::new(&name);
        for (pname, pty) in &params {
            spec = spec.with_param(pname, pty);
        }
        if let Some(ref ret) = return_type {
            spec = spec.with_return(ret);
        }
        for eff in &effects {
            spec = spec.with_effect(eff);
        }
        for pre in &intent.preconditions {
            spec = spec.with_req(pre);
        }
        for post in &intent.postconditions {
            spec = spec.with_ens(post);
        }
        for inv in &intent.invariants {
            spec = spec.with_inv(inv);
        }

        // Build the AST directly for maximum correctness.
        let body =
            self.build_function_body(&name, &params, &return_type, &intent.description, &kb_hints);

        let func = FunctionDef {
            name: name.clone(),
            is_async: effects.iter().any(|e| e == "Async" || e == "Net"),
            is_unsafe: false,
            generics: Vec::new(),
            params: params
                .iter()
                .map(|(n, t)| Param {
                    name: n.clone(),
                    ty: parse_type_str(t),
                })
                .collect(),
            return_type: return_type.as_ref().map(|t| parse_type_str(t)),
            where_clause: Vec::new(),
            effects: effects.clone(),
            contracts: build_contracts(
                &intent.preconditions,
                &intent.postconditions,
                &intent.invariants,
            ),
            body: body.clone(),
        };

        let module = Module {
            items: vec![Item {
                visibility: Visibility::Public,
                attributes: Vec::new(),
                kind: ItemKind::Function(func),
            }],
        };

        // Validate through the compiler pipeline.
        let validation = self.validate_module(&module);
        let code_human = fmt::format_human(&module);
        let code_agent = fmt::format_agent(&module);

        // Build explanation.
        let mut explanation = format!("Generated function `{name}`");
        if !params.is_empty() {
            let pts: Vec<String> = params.iter().map(|(n, t)| format!("`{n}: {t}`")).collect();
            explanation.push_str(&format!(" with parameters {}", pts.join(", ")));
        }
        if let Some(ref ret) = return_type {
            explanation.push_str(&format!(", returning `{ret}`"));
        }
        if !effects.is_empty() {
            explanation.push_str(&format!(". Effects: {}", effects.join(", ")));
        }
        if !kb_hints.is_empty() {
            explanation.push_str(&format!(
                ". Applied {} KB-derived constraints",
                kb_hints.len()
            ));
        }
        explanation.push('.');

        NlResponse {
            ok: validation.errors == 0,
            code_human,
            code_agent,
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: validation.verification_summary,
        }
    }

    fn generate_struct(&mut self, intent: &Intent) -> NlResponse {
        let name = intent
            .name
            .clone()
            .unwrap_or_else(|| extract_type_name(&intent.description));
        let fields = infer_struct_fields(&intent.description);

        let struct_def = StructDef {
            name: name.clone(),
            generics: Vec::new(),
            contracts: build_contracts(
                &intent.preconditions,
                &intent.postconditions,
                &intent.invariants,
            ),
            fields: fields
                .iter()
                .map(|(fname, fty)| StructField {
                    visibility: Visibility::Public,
                    name: fname.clone(),
                    ty: parse_type_str(fty),
                })
                .collect(),
        };

        let module = Module {
            items: vec![Item {
                visibility: Visibility::Public,
                attributes: Vec::new(),
                kind: ItemKind::Struct(struct_def),
            }],
        };

        let validation = self.validate_module(&module);
        let code_human = fmt::format_human(&module);
        let code_agent = fmt::format_agent(&module);

        let field_desc: Vec<String> = fields.iter().map(|(n, t)| format!("`{n}: {t}`")).collect();
        let explanation = format!(
            "Generated struct `{name}` with fields: {}.",
            field_desc.join(", ")
        );

        NlResponse {
            ok: validation.errors == 0,
            code_human,
            code_agent,
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: validation.verification_summary,
        }
    }

    fn generate_enum(&mut self, intent: &Intent) -> NlResponse {
        let name = intent
            .name
            .clone()
            .unwrap_or_else(|| extract_type_name(&intent.description));
        let variants = infer_enum_variants(&intent.description);

        let enum_def = EnumDef {
            name: name.clone(),
            generics: Vec::new(),
            variants: variants
                .iter()
                .map(|v| EnumVariant {
                    name: v.clone(),
                    kind: VariantKind::Unit,
                })
                .collect(),
        };

        let module = Module {
            items: vec![Item {
                visibility: Visibility::Public,
                attributes: Vec::new(),
                kind: ItemKind::Enum(enum_def),
            }],
        };

        let validation = self.validate_module(&module);
        let code_human = fmt::format_human(&module);
        let code_agent = fmt::format_agent(&module);

        let explanation = format!(
            "Generated enum `{name}` with variants: {}.",
            variants.join(", ")
        );

        NlResponse {
            ok: validation.errors == 0,
            code_human,
            code_agent,
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: validation.verification_summary,
        }
    }

    fn generate_trait(&mut self, intent: &Intent) -> NlResponse {
        let name = intent
            .name
            .clone()
            .unwrap_or_else(|| extract_type_name(&intent.description));
        let methods = infer_trait_methods(&intent.description);

        let trait_def = TraitDef {
            name: name.clone(),
            generics: Vec::new(),
            super_traits: Vec::new(),
            items: methods
                .iter()
                .map(|(mname, mparams, mret)| Item {
                    visibility: Visibility::Public,
                    attributes: Vec::new(),
                    kind: ItemKind::Function(FunctionDef {
                        name: mname.clone(),
                        is_async: false,
                        is_unsafe: false,
                        generics: Vec::new(),
                        params: mparams
                            .iter()
                            .map(|(pn, pt)| Param {
                                name: pn.clone(),
                                ty: parse_type_str(pt),
                            })
                            .collect(),
                        return_type: mret.as_ref().map(|t| parse_type_str(t)),
                        where_clause: Vec::new(),
                        effects: Vec::new(),
                        contracts: Vec::new(),
                        body: Block {
                            stmts: Vec::new(),
                            tail_expr: None,
                        },
                    }),
                })
                .collect(),
        };

        let module = Module {
            items: vec![Item {
                visibility: Visibility::Public,
                attributes: Vec::new(),
                kind: ItemKind::Trait(trait_def),
            }],
        };

        let validation = self.validate_module(&module);
        let code_human = fmt::format_human(&module);
        let code_agent = fmt::format_agent(&module);

        let method_names: Vec<&str> = methods.iter().map(|(n, _, _)| n.as_str()).collect();
        let explanation = format!(
            "Generated trait `{name}` with methods: {}.",
            method_names.join(", ")
        );

        NlResponse {
            ok: validation.errors == 0,
            code_human,
            code_agent,
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: validation.verification_summary,
        }
    }

    fn generate_net(&mut self, intent: &Intent) -> NlResponse {
        let name = intent
            .name
            .clone()
            .unwrap_or_else(|| extract_type_name(&intent.description));
        let layers = infer_net_layers(&intent.description);

        let net_def = NetDef {
            name: name.clone(),
            generics: Vec::new(),
            layers: layers
                .iter()
                .map(|(lname, ltype, largs)| LayerDef {
                    name: lname.clone(),
                    layer_type: parse_type_str(ltype),
                    args: largs
                        .iter()
                        .map(|a| Expr::Literal {
                            value: a.clone(),
                            kind: LiteralKind::Int,
                        })
                        .collect(),
                })
                .collect(),
            forward: Block {
                stmts: Vec::new(),
                tail_expr: None,
            },
        };

        let module = Module {
            items: vec![Item {
                visibility: Visibility::Public,
                attributes: Vec::new(),
                kind: ItemKind::Net(net_def),
            }],
        };

        let validation = self.validate_module(&module);
        let code_human = fmt::format_human(&module);
        let code_agent = fmt::format_agent(&module);

        let layer_desc: Vec<String> = layers
            .iter()
            .map(|(n, t, _)| format!("`{n}: {t}`"))
            .collect();
        let explanation = format!(
            "Generated neural network `{name}` with layers: {}.",
            layer_desc.join(", ")
        );

        NlResponse {
            ok: validation.errors == 0,
            code_human,
            code_agent,
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: validation.verification_summary,
        }
    }

    fn generate_kb(&mut self, intent: &Intent) -> NlResponse {
        let name = intent
            .name
            .clone()
            .unwrap_or_else(|| extract_type_name(&intent.description));

        let kb_def = KbDef {
            name: name.clone(),
            facts: intent
                .kb_facts
                .iter()
                .map(|(pred, args)| FactDef {
                    name: pred.clone(),
                    args: args
                        .iter()
                        .map(|a| Expr::Literal {
                            value: a.clone(),
                            kind: LiteralKind::String,
                        })
                        .collect(),
                })
                .collect(),
            rules: Vec::new(),
        };

        // Also add to the persistent KB.
        for (pred, args) in &intent.kb_facts {
            self.kb.add_fact(pred, args.clone());
        }

        let module = Module {
            items: vec![Item {
                visibility: Visibility::Public,
                attributes: Vec::new(),
                kind: ItemKind::Kb(kb_def),
            }],
        };

        let validation = self.validate_module(&module);
        let code_human = fmt::format_human(&module);
        let code_agent = fmt::format_agent(&module);

        let explanation = format!(
            "Generated knowledge base `{name}` with {} facts. Facts added to persistent KB.",
            intent.kb_facts.len()
        );

        NlResponse {
            ok: validation.errors == 0,
            code_human,
            code_agent,
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: validation.verification_summary,
        }
    }

    fn generate_evolve(&mut self, intent: &Intent) -> NlResponse {
        let name = intent
            .name
            .clone()
            .unwrap_or_else(|| extract_function_name(&intent.description));
        let pop_size = intent
            .extras
            .get("population")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(100);
        let gens = intent
            .extras
            .get("generations")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(50);

        let evolve_def = EvolveDef {
            name: name.clone(),
            genome_type: Type::Vec {
                inner: Box::new(Type::Path {
                    segments: vec!["f64".to_string()],
                    type_args: Vec::new(),
                }),
            },
            population_size: Some(Expr::Literal {
                value: pop_size.to_string(),
                kind: LiteralKind::Int,
            }),
            generations: Some(Expr::Literal {
                value: gens.to_string(),
                kind: LiteralKind::Int,
            }),
            fitness: Block {
                stmts: Vec::new(),
                tail_expr: Some(Box::new(Expr::Literal {
                    value: "0.0".into(),
                    kind: LiteralKind::Float,
                })),
            },
            mutate_fn: None,
            crossover_fn: None,
            select_fn: None,
        };

        let module = Module {
            items: vec![Item {
                visibility: Visibility::Public,
                attributes: Vec::new(),
                kind: ItemKind::Evolve(evolve_def),
            }],
        };

        let validation = self.validate_module(&module);
        let code_human = fmt::format_human(&module);
        let code_agent = fmt::format_agent(&module);

        let explanation = format!(
            "Generated evolutionary computation `{name}` with population={pop_size}, generations={gens}."
        );

        NlResponse {
            ok: validation.errors == 0,
            code_human,
            code_agent,
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: validation.verification_summary,
        }
    }

    fn generate_agent(&mut self, intent: &Intent) -> NlResponse {
        let name = intent
            .name
            .clone()
            .unwrap_or_else(|| extract_type_name(&intent.description));
        let capabilities = infer_capabilities(&intent.description);

        let agent_def = AgentDef {
            name: name.clone(),
            capabilities: capabilities.clone(),
            requires_approval: Vec::new(),
        };

        let module = Module {
            items: vec![Item {
                visibility: Visibility::Public,
                attributes: Vec::new(),
                kind: ItemKind::Agent(agent_def),
            }],
        };

        let validation = self.validate_module(&module);
        let code_human = fmt::format_human(&module);
        let code_agent = fmt::format_agent(&module);

        let explanation = format!(
            "Generated agent `{name}` with capabilities: {}.",
            capabilities.join(", ")
        );

        NlResponse {
            ok: validation.errors == 0,
            code_human,
            code_agent,
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: validation.verification_summary,
        }
    }

    fn generate_swarm(&mut self, intent: &Intent) -> NlResponse {
        let name = intent
            .name
            .clone()
            .unwrap_or_else(|| extract_type_name(&intent.description));
        let size = intent
            .extras
            .get("size")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(4);
        let agent_type = intent
            .extras
            .get("agent_type")
            .cloned()
            .unwrap_or_else(|| "Worker".into());
        let topology = intent
            .extras
            .get("topology")
            .cloned()
            .unwrap_or_else(|| "star".into());

        let swarm_def = SwarmDef {
            name: name.clone(),
            agent_type: agent_type.clone(),
            size: Some(Expr::Literal {
                value: size.to_string(),
                kind: LiteralKind::Int,
            }),
            topology: Some(topology.clone()),
            consensus: Some("majority".into()),
            on_dispatch: None,
            on_aggregate: None,
            on_failure: None,
        };

        let module = Module {
            items: vec![Item {
                visibility: Visibility::Public,
                attributes: Vec::new(),
                kind: ItemKind::Swarm(swarm_def),
            }],
        };

        let validation = self.validate_module(&module);
        let code_human = fmt::format_human(&module);
        let code_agent = fmt::format_agent(&module);

        let explanation = format!(
            "Generated swarm `{name}` of {size} `{agent_type}` agents with `{topology}` topology."
        );

        NlResponse {
            ok: validation.errors == 0,
            code_human,
            code_agent,
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: validation.verification_summary,
        }
    }

    fn explain_code(&mut self, intent: &Intent) -> NlResponse {
        let source = match &intent.source {
            Some(s) => s.clone(),
            None => {
                return NlResponse::error(intent.clone(), "No source code provided to explain.")
            }
        };

        let tokens = match safe_lex(&source) {
            Ok(t) => t,
            Err(e) => return NlResponse::error(intent.clone(), &e),
        };
        let module = match parser::parse(&tokens) {
            Ok(m) => m,
            Err(e) => {
                return NlResponse::error(intent.clone(), &format!("Parse error: {}", e.message))
            }
        };

        let resolver = resolve::resolve(&module);
        let checker = types::check(&module);
        let effect_infer = effects::infer_effects(&module);

        let mut explanation = String::new();
        explanation.push_str(&format!("Module contains {} items.\n", module.items.len()));

        for item in &module.items {
            match &item.kind {
                ItemKind::Function(f) => {
                    let vis = if item.visibility == Visibility::Public {
                        "public "
                    } else {
                        ""
                    };
                    let async_str = if f.is_async { "async " } else { "" };
                    explanation.push_str(&format!("• {vis}{async_str}function `{}`", f.name));
                    if !f.params.is_empty() {
                        let pts: Vec<String> = f
                            .params
                            .iter()
                            .map(|p| format!("{}: {:?}", p.name, p.ty))
                            .collect();
                        explanation.push_str(&format!(" ({})", pts.join(", ")));
                    }
                    if let Some(ref ret) = f.return_type {
                        explanation.push_str(&format!(" -> {:?}", ret));
                    }
                    if !f.effects.is_empty() {
                        explanation.push_str(&format!(" / {}", f.effects.join(", ")));
                    }
                    if !f.contracts.is_empty() {
                        explanation.push_str(&format!(", {} contracts", f.contracts.len()));
                    }
                    explanation.push('\n');
                }
                ItemKind::Struct(s) => {
                    explanation.push_str(&format!(
                        "• struct `{}` with {} fields\n",
                        s.name,
                        s.fields.len()
                    ));
                }
                ItemKind::Enum(e) => {
                    explanation.push_str(&format!(
                        "• enum `{}` with {} variants\n",
                        e.name,
                        e.variants.len()
                    ));
                }
                ItemKind::Trait(t) => {
                    explanation.push_str(&format!(
                        "• trait `{}` with {} methods\n",
                        t.name,
                        t.items.len()
                    ));
                }
                ItemKind::Net(n) => {
                    explanation.push_str(&format!(
                        "• neural net `{}` with {} layers\n",
                        n.name,
                        n.layers.len()
                    ));
                }
                ItemKind::Kb(k) => {
                    explanation.push_str(&format!(
                        "• knowledge base `{}` with {} facts, {} rules\n",
                        k.name,
                        k.facts.len(),
                        k.rules.len()
                    ));
                }
                ItemKind::Evolve(e) => {
                    explanation.push_str(&format!("• evolutionary block `{}`\n", e.name));
                }
                ItemKind::Agent(a) => {
                    explanation.push_str(&format!(
                        "• agent `{}` with capabilities: {}\n",
                        a.name,
                        a.capabilities.join(", ")
                    ));
                }
                ItemKind::Swarm(s) => {
                    explanation.push_str(&format!(
                        "• swarm `{}` of `{}` agents\n",
                        s.name, s.agent_type
                    ));
                }
                _ => {}
            }
        }

        explanation.push_str(&format!("\nResolved {} symbols.", resolver.symbols.len()));
        if !effect_infer.inferred.is_empty() {
            explanation.push_str("\n\nEffect analysis:");
            for (name, fx) in &effect_infer.inferred {
                if fx.is_empty() {
                    explanation.push_str(&format!("\n  {name}: pure"));
                } else {
                    let efx: Vec<String> = fx.iter().map(|e| e.to_string()).collect();
                    explanation.push_str(&format!("\n  {name}: {{{}}}", efx.join(", ")));
                }
            }
        }

        NlResponse {
            ok: true,
            code_human: fmt::format_human(&module),
            code_agent: fmt::format_agent(&module),
            explanation,
            diagnostics: checker.diagnostics.clone(),
            fixes: Vec::new(),
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: String::new(),
        }
    }

    fn query_kb(&mut self, intent: &Intent) -> NlResponse {
        let mut results = Vec::new();

        for (pred, args) in &intent.kb_queries {
            let arg_refs: Vec<&str> = args.iter().map(|a| a.as_str()).collect();
            let rows = self.kb.query(pred, &arg_refs);
            results.push((pred.clone(), rows));
        }

        let mut explanation = String::new();
        for (pred, rows) in &results {
            explanation.push_str(&format!("Query `{pred}`: {} results\n", rows.len()));
            for row in rows {
                explanation.push_str(&format!("  → {}\n", row.join(", ")));
            }
        }

        if results.is_empty() {
            explanation =
                "No queries specified. Add queries with 'query predicate(arg1, arg2)'.".into();
        }

        NlResponse {
            ok: true,
            code_human: String::new(),
            code_agent: String::new(),
            explanation,
            diagnostics: Vec::new(),
            fixes: Vec::new(),
            intent: intent.clone(),
            kb_results: results,
            verification_summary: String::new(),
        }
    }

    fn check_code(&mut self, intent: &Intent) -> NlResponse {
        let source = match &intent.source {
            Some(s) => s.clone(),
            None => return NlResponse::error(intent.clone(), "No source code provided to check."),
        };

        let tokens = match safe_lex(&source) {
            Ok(t) => t,
            Err(e) => return NlResponse::error(intent.clone(), &e),
        };
        let module = match parser::parse(&tokens) {
            Ok(m) => m,
            Err(e) => {
                return NlResponse::error(
                    intent.clone(),
                    &format!("Parse error at {}:{}: {}", e.line, e.col, e.message),
                )
            }
        };

        let validation = self.validate_module(&module);
        let explanation = format!(
            "Checked code: {} symbols, {} errors, {} fix candidates.\n{}",
            validation.symbol_count,
            validation.errors,
            validation.fixes.len(),
            validation.verification_summary
        );

        NlResponse {
            ok: validation.errors == 0,
            code_human: fmt::format_human(&module),
            code_agent: fmt::format_agent(&module),
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: validation.verification_summary,
        }
    }

    fn optimize_code(&mut self, intent: &Intent) -> NlResponse {
        let source = match &intent.source {
            Some(s) => s.clone(),
            None => {
                return NlResponse::error(intent.clone(), "No source code provided to optimize.")
            }
        };

        let tokens = match safe_lex(&source) {
            Ok(t) => t,
            Err(e) => return NlResponse::error(intent.clone(), &e),
        };
        let module = match parser::parse(&tokens) {
            Ok(m) => m,
            Err(e) => {
                return NlResponse::error(intent.clone(), &format!("Parse error: {}", e.message))
            }
        };

        // Apply elision for agent-mode optimization.
        let optimized = elision::elide(&module);

        let validation = self.validate_module(&optimized);
        let original_human = fmt::format_human(&module);
        let optimized_human = fmt::format_human(&optimized);
        let original_agent = fmt::format_agent(&module);
        let optimized_agent = fmt::format_agent(&optimized);

        let explanation = format!(
            "Optimized code: {} → {} tokens (human mode), {} → {} tokens (agent mode). Safety annotations elided where provably safe.",
            original_human.len(), optimized_human.len(),
            original_agent.len(), optimized_agent.len()
        );

        NlResponse {
            ok: validation.errors == 0,
            code_human: optimized_human,
            code_agent: optimized_agent,
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: validation.verification_summary,
        }
    }

    fn add_contracts(&mut self, intent: &Intent) -> NlResponse {
        let source = match &intent.source {
            Some(s) => s.clone(),
            None => return NlResponse::error(intent.clone(), "No source code provided."),
        };

        let tokens = match safe_lex(&source) {
            Ok(t) => t,
            Err(e) => return NlResponse::error(intent.clone(), &e),
        };
        let mut module = match parser::parse(&tokens) {
            Ok(m) => m,
            Err(e) => {
                return NlResponse::error(intent.clone(), &format!("Parse error: {}", e.message))
            }
        };

        // Add contracts to all functions in the module.
        let mut added = 0;
        for item in &mut module.items {
            if let ItemKind::Function(ref mut f) = item.kind {
                // Infer preconditions from parameter types.
                for param in &f.params {
                    if let Type::Path { ref segments, .. } = param.ty {
                        let tname = segments.join("::");
                        if tname == "usize" || tname == "u32" || tname == "u64" {
                            // Unsigned types are always >= 0, skip.
                        } else if tname == "i32" || tname == "i64" || tname == "isize" {
                            // Check if already has a contract for this param.
                            let has_contract = f
                                .contracts
                                .iter()
                                .any(|c| c.condition.contains(&param.name));
                            if !has_contract {
                                f.contracts.push(ContractClause {
                                    kind: ContractClauseKind::Requires,
                                    condition: format!("{} != i32::MIN", param.name),
                                    message: Some(format!(
                                        "{} must not be minimum value",
                                        param.name
                                    )),
                                });
                                added += 1;
                            }
                        }
                    }
                }

                // Add basic postcondition if function has a return type.
                if f.return_type.is_some() {
                    let has_ensures = f
                        .contracts
                        .iter()
                        .any(|c| c.kind == ContractClauseKind::Ensures);
                    if !has_ensures {
                        f.contracts.push(ContractClause {
                            kind: ContractClauseKind::Ensures,
                            condition: "true".into(),
                            message: Some("postcondition placeholder".into()),
                        });
                        added += 1;
                    }
                }
            }
        }

        let validation = self.validate_module(&module);
        let code_human = fmt::format_human(&module);
        let code_agent = fmt::format_agent(&module);

        let explanation = format!("Added {added} contract clauses to functions.");

        NlResponse {
            ok: validation.errors == 0,
            code_human,
            code_agent,
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            verification_summary: validation.verification_summary,
            kb_results: Vec::new(),
        }
    }

    fn refactor_code(&mut self, intent: &Intent) -> NlResponse {
        let source = match &intent.source {
            Some(s) => s.clone(),
            None => {
                return NlResponse::error(intent.clone(), "No source code provided to refactor.")
            }
        };

        let tokens = match safe_lex(&source) {
            Ok(t) => t,
            Err(e) => return NlResponse::error(intent.clone(), &e),
        };
        let module = match parser::parse(&tokens) {
            Ok(m) => m,
            Err(e) => {
                return NlResponse::error(intent.clone(), &format!("Parse error: {}", e.message))
            }
        };

        // Apply elision as basic refactoring (simplify safety annotations).
        let refactored = elision::elide(&module);
        let validation = self.validate_module(&refactored);
        let code_human = fmt::format_human(&refactored);
        let code_agent = fmt::format_agent(&refactored);

        let explanation = "Refactored code: applied safety elision and normalization.".to_string();

        NlResponse {
            ok: validation.errors == 0,
            code_human,
            code_agent,
            explanation,
            diagnostics: validation.diagnostics,
            fixes: validation.fixes,
            intent: intent.clone(),
            kb_results: Vec::new(),
            verification_summary: validation.verification_summary,
        }
    }

    // ─── KB integration ────────────────────────────────────────────

    /// Query the persistent KB for hints relevant to a function being generated.
    fn query_kb_for_hints(&mut self, name: &str, params: &[(String, String)]) -> Vec<String> {
        let mut hints = Vec::new();

        // Query for domain knowledge about the function name.
        let name_facts = self.kb.query("function_hint", &[name, "?"]);
        for row in &name_facts {
            if row.len() >= 2 {
                hints.push(row[1].clone());
            }
        }

        // Query for type constraints.
        for (_, ty) in params {
            let type_facts = self.kb.query("type_constraint", &[ty, "?"]);
            for row in &type_facts {
                if row.len() >= 2 {
                    hints.push(row[1].clone());
                }
            }
        }

        // Query for safety rules.
        let safety_facts = self.kb.query("safety_rule", &[name, "?"]);
        for row in &safety_facts {
            if row.len() >= 2 {
                hints.push(row[1].clone());
            }
        }

        hints
    }

    // ─── Function body builder ─────────────────────────────────────

    fn build_function_body(
        &mut self,
        name: &str,
        params: &[(String, String)],
        return_type: &Option<String>,
        description: &str,
        kb_hints: &[String],
    ) -> Block {
        let desc_lower = description.to_lowercase();

        // Pattern-match common operations from NL description.
        if desc_lower.contains("add") || desc_lower.contains("sum") {
            if params.len() >= 2 {
                return Block {
                    stmts: Vec::new(),
                    tail_expr: Some(Box::new(Expr::Binary {
                        op: "+".into(),
                        left: Box::new(Expr::Ident {
                            name: params[0].0.clone(),
                        }),
                        right: Box::new(Expr::Ident {
                            name: params[1].0.clone(),
                        }),
                    })),
                };
            }
        }

        if desc_lower.contains("subtract") || desc_lower.contains("difference") {
            if params.len() >= 2 {
                return Block {
                    stmts: Vec::new(),
                    tail_expr: Some(Box::new(Expr::Binary {
                        op: "-".into(),
                        left: Box::new(Expr::Ident {
                            name: params[0].0.clone(),
                        }),
                        right: Box::new(Expr::Ident {
                            name: params[1].0.clone(),
                        }),
                    })),
                };
            }
        }

        if desc_lower.contains("multiply") || desc_lower.contains("product") {
            if params.len() >= 2 {
                return Block {
                    stmts: Vec::new(),
                    tail_expr: Some(Box::new(Expr::Binary {
                        op: "*".into(),
                        left: Box::new(Expr::Ident {
                            name: params[0].0.clone(),
                        }),
                        right: Box::new(Expr::Ident {
                            name: params[1].0.clone(),
                        }),
                    })),
                };
            }
        }

        if desc_lower.contains("divide") || desc_lower.contains("quotient") {
            if params.len() >= 2 {
                return Block {
                    stmts: Vec::new(),
                    tail_expr: Some(Box::new(Expr::Binary {
                        op: "/".into(),
                        left: Box::new(Expr::Ident {
                            name: params[0].0.clone(),
                        }),
                        right: Box::new(Expr::Ident {
                            name: params[1].0.clone(),
                        }),
                    })),
                };
            }
        }

        if desc_lower.contains("max")
            || desc_lower.contains("maximum")
            || desc_lower.contains("larger")
        {
            if params.len() >= 2 {
                return Block {
                    stmts: Vec::new(),
                    tail_expr: Some(Box::new(Expr::If {
                        cond: Box::new(Expr::Binary {
                            op: ">".into(),
                            left: Box::new(Expr::Ident {
                                name: params[0].0.clone(),
                            }),
                            right: Box::new(Expr::Ident {
                                name: params[1].0.clone(),
                            }),
                        }),
                        then_block: Block {
                            stmts: Vec::new(),
                            tail_expr: Some(Box::new(Expr::Ident {
                                name: params[0].0.clone(),
                            })),
                        },
                        else_block: Some(Block {
                            stmts: Vec::new(),
                            tail_expr: Some(Box::new(Expr::Ident {
                                name: params[1].0.clone(),
                            })),
                        }),
                    })),
                };
            }
        }

        if desc_lower.contains("min")
            || desc_lower.contains("minimum")
            || desc_lower.contains("smaller")
        {
            if params.len() >= 2 {
                return Block {
                    stmts: Vec::new(),
                    tail_expr: Some(Box::new(Expr::If {
                        cond: Box::new(Expr::Binary {
                            op: "<".into(),
                            left: Box::new(Expr::Ident {
                                name: params[0].0.clone(),
                            }),
                            right: Box::new(Expr::Ident {
                                name: params[1].0.clone(),
                            }),
                        }),
                        then_block: Block {
                            stmts: Vec::new(),
                            tail_expr: Some(Box::new(Expr::Ident {
                                name: params[0].0.clone(),
                            })),
                        },
                        else_block: Some(Block {
                            stmts: Vec::new(),
                            tail_expr: Some(Box::new(Expr::Ident {
                                name: params[1].0.clone(),
                            })),
                        }),
                    })),
                };
            }
        }

        if desc_lower.contains("negate") || desc_lower.contains("negative") {
            if !params.is_empty() {
                return Block {
                    stmts: Vec::new(),
                    tail_expr: Some(Box::new(Expr::Unary {
                        op: "-".into(),
                        operand: Box::new(Expr::Ident {
                            name: params[0].0.clone(),
                        }),
                    })),
                };
            }
        }

        if desc_lower.contains("identity")
            || desc_lower.contains("pass through")
            || desc_lower.contains("echo")
        {
            if !params.is_empty() {
                return Block {
                    stmts: Vec::new(),
                    tail_expr: Some(Box::new(Expr::Ident {
                        name: params[0].0.clone(),
                    })),
                };
            }
        }

        if desc_lower.contains("print")
            || desc_lower.contains("display")
            || desc_lower.contains("log")
        {
            if !params.is_empty() {
                return Block {
                    stmts: vec![Stmt::Expr {
                        expr: Expr::Call {
                            func: Box::new(Expr::Ident {
                                name: "println".into(),
                            }),
                            args: vec![
                                Expr::Literal {
                                    value: "{}".into(),
                                    kind: LiteralKind::FormatString,
                                },
                                Expr::Ident {
                                    name: params[0].0.clone(),
                                },
                            ],
                        },
                    }],
                    tail_expr: None,
                };
            }
        }

        if desc_lower.contains("factorial") {
            if !params.is_empty() {
                let p = &params[0].0;
                return Block {
                    stmts: Vec::new(),
                    tail_expr: Some(Box::new(Expr::If {
                        cond: Box::new(Expr::Binary {
                            op: "<=".into(),
                            left: Box::new(Expr::Ident { name: p.clone() }),
                            right: Box::new(Expr::Literal {
                                value: "1".into(),
                                kind: LiteralKind::Int,
                            }),
                        }),
                        then_block: Block {
                            stmts: Vec::new(),
                            tail_expr: Some(Box::new(Expr::Literal {
                                value: "1".into(),
                                kind: LiteralKind::Int,
                            })),
                        },
                        else_block: Some(Block {
                            stmts: Vec::new(),
                            tail_expr: Some(Box::new(Expr::Binary {
                                op: "*".into(),
                                left: Box::new(Expr::Ident { name: p.clone() }),
                                right: Box::new(Expr::Call {
                                    func: Box::new(Expr::Ident { name: name.into() }),
                                    args: vec![Expr::Binary {
                                        op: "-".into(),
                                        left: Box::new(Expr::Ident { name: p.clone() }),
                                        right: Box::new(Expr::Literal {
                                            value: "1".into(),
                                            kind: LiteralKind::Int,
                                        }),
                                    }],
                                }),
                            })),
                        }),
                    })),
                };
            }
        }

        if desc_lower.contains("fibonacci") || desc_lower.contains("fib") {
            if !params.is_empty() {
                let p = &params[0].0;
                return Block {
                    stmts: Vec::new(),
                    tail_expr: Some(Box::new(Expr::If {
                        cond: Box::new(Expr::Binary {
                            op: "<=".into(),
                            left: Box::new(Expr::Ident { name: p.clone() }),
                            right: Box::new(Expr::Literal {
                                value: "1".into(),
                                kind: LiteralKind::Int,
                            }),
                        }),
                        then_block: Block {
                            stmts: Vec::new(),
                            tail_expr: Some(Box::new(Expr::Ident { name: p.clone() })),
                        },
                        else_block: Some(Block {
                            stmts: Vec::new(),
                            tail_expr: Some(Box::new(Expr::Binary {
                                op: "+".into(),
                                left: Box::new(Expr::Call {
                                    func: Box::new(Expr::Ident { name: name.into() }),
                                    args: vec![Expr::Binary {
                                        op: "-".into(),
                                        left: Box::new(Expr::Ident { name: p.clone() }),
                                        right: Box::new(Expr::Literal {
                                            value: "1".into(),
                                            kind: LiteralKind::Int,
                                        }),
                                    }],
                                }),
                                right: Box::new(Expr::Call {
                                    func: Box::new(Expr::Ident { name: name.into() }),
                                    args: vec![Expr::Binary {
                                        op: "-".into(),
                                        left: Box::new(Expr::Ident { name: p.clone() }),
                                        right: Box::new(Expr::Literal {
                                            value: "2".into(),
                                            kind: LiteralKind::Int,
                                        }),
                                    }],
                                }),
                            })),
                        }),
                    })),
                };
            }
        }

        // Default: use synthesis oracle with KB hints.
        let mut spec = SynthesisSpec::new(name);
        for (pname, pty) in params {
            spec = spec.with_param(pname, pty);
        }
        if let Some(ret) = return_type {
            spec = spec.with_return(ret);
        }
        for hint in kb_hints {
            spec = spec.with_req(hint);
        }

        if let Some(candidate) = self.oracle.synthesize(&spec) {
            // Parse the synthesized body back into an AST.
            let lex_ok = safe_lex(&candidate.body);
            let tokens = match lex_ok { Ok(t) => t, Err(_) => return Block { stmts: Vec::new(), tail_expr: Some(Box::new(Expr::Todo)) } };
            if let Ok(module) = parser::parse(&tokens) {
                if let Some(item) = module.items.first() {
                    if let ItemKind::Function(f) = &item.kind {
                        return f.body.clone();
                    }
                }
            }
        }

        // Fallback: produce a todo placeholder.
        Block {
            stmts: Vec::new(),
            tail_expr: Some(Box::new(Expr::Todo)),
        }
    }

    // ─── Compiler pipeline validation ──────────────────────────────

    fn validate_module(&self, module: &Module) -> ValidationResult {
        let code = fmt::format_human(module);
        let tokens = match safe_lex(&code) {
            Ok(t) => t,
            Err(_) => {
                return ValidationResult {
                    errors: 1,
                    symbol_count: 0,
                    diagnostics: vec![hir::Diagnostic {
                        id: None,
                        category: None,
                        severity: hir::Severity::Error,
                        message: "Lexer panic during validation".to_string(),
                        span: None,
                    }],
                    fixes: Vec::new(),
                    verification_summary: "Lexer panic".to_string(),
                };
            }
        };

        let reparsed = match parser::parse(&tokens) {
            Ok(m) => m,
            Err(e) => {
                return ValidationResult {
                    errors: 1,
                    symbol_count: 0,
                    diagnostics: vec![hir::Diagnostic {
                        severity: hir::Severity::Error,
                        message: format!("Re-parse error: {}", e.message),
                        span: Some(hir::Span {
                            line: e.line as u32,
                            col: e.col as u32,
                        }),
                        id: None,
                        category: Some(hir::DiagnosticCategory::SyntaxError),
                    }],
                    fixes: Vec::new(),
                    verification_summary: String::new(),
                };
            }
        };

        let resolver = resolve::resolve(&reparsed);
        let checker = types::check(&reparsed);
        let effect_infer = effects::infer_effects(&reparsed);
        let verifications = verify::verify_module(&reparsed);

        let mut all_diags: Vec<hir::Diagnostic> = Vec::new();
        all_diags.extend(resolver.diagnostics.iter().cloned());
        all_diags.extend(checker.diagnostics.iter().cloned());
        all_diags.extend(effect_infer.diagnostics.iter().cloned());

        let errors = all_diags
            .iter()
            .filter(|d| d.severity == hir::Severity::Error)
            .count();

        let healed = heal::heal(&all_diags);
        let fixes: Vec<String> = healed
            .iter()
            .flat_map(|h| h.fixes.iter().map(|f| f.description.clone()))
            .collect();

        let verified_count = verifications
            .iter()
            .filter(|v| v.status == verify::VerifyStatus::Verified)
            .count();
        let total_count = verifications.len();
        let verification_summary = if total_count > 0 {
            format!("Contracts: {verified_count}/{total_count} verified")
        } else {
            "No contracts to verify".into()
        };

        ValidationResult {
            errors,
            symbol_count: resolver.symbols.len(),
            diagnostics: all_diags,
            fixes,
            verification_summary,
        }
    }
}

struct ValidationResult {
    errors: usize,
    symbol_count: usize,
    diagnostics: Vec<hir::Diagnostic>,
    fixes: Vec<String>,
    verification_summary: String,
}

// ═══════════════════════════════════════════════════════════════════
// Intent Parser — NL text → structured Intent
// ═══════════════════════════════════════════════════════════════════

/// Parse a natural language string into a structured Intent.
pub fn parse_intent(input: &str) -> Intent {
    let lower = input.to_lowercase();
    let trimmed = input.trim();

    // Determine intent kind from keywords.
    let kind = if has_any(
        &lower,
        &[
            "create a function",
            "generate a function",
            "write a function",
            "make a function",
            "define a function",
            "implement a function",
            "function that",
            "fn that",
        ],
    ) {
        IntentKind::GenerateFunction
    } else if has_any(
        &lower,
        &[
            "create a struct",
            "generate a struct",
            "define a struct",
            "make a struct",
            "create a type",
            "define a type",
            "record with",
            "data type",
        ],
    ) {
        IntentKind::GenerateStruct
    } else if has_any(
        &lower,
        &[
            "create an enum",
            "generate an enum",
            "define an enum",
            "make an enum",
            "enumeration",
        ],
    ) {
        IntentKind::GenerateEnum
    } else if has_any(
        &lower,
        &[
            "create a trait",
            "generate a trait",
            "define a trait",
            "make a trait",
            "interface for",
        ],
    ) {
        IntentKind::GenerateTrait
    } else if has_any(
        &lower,
        &[
            "neural network",
            "create a net",
            "generate a net",
            "build a net",
            "build a model",
            "classifier",
            "cnn",
            "mlp",
        ],
    ) {
        IntentKind::GenerateNet
    } else if has_any(
        &lower,
        &[
            "knowledge base",
            "create a kb",
            "generate a kb",
            "build a kb",
            "fact",
            "rule",
            "ontology",
        ],
    ) {
        IntentKind::GenerateKb
    } else if has_any(
        &lower,
        &[
            "evolve",
            "evolutionary",
            "genetic algorithm",
            "optimize via evolution",
        ],
    ) {
        IntentKind::GenerateEvolve
    } else if has_any(
        &lower,
        &[
            "create an agent",
            "generate an agent",
            "build an agent",
            "define an agent",
        ],
    ) {
        IntentKind::GenerateAgent
    } else if has_any(
        &lower,
        &[
            "create a swarm",
            "generate a swarm",
            "build a swarm",
            "multi-agent",
        ],
    ) {
        IntentKind::GenerateSwarm
    } else if has_any(
        &lower,
        &["explain", "what does", "describe", "tell me about"],
    ) {
        IntentKind::Explain
    } else if has_any(&lower, &["query", "who is", "find", "look up", "search kb"]) {
        IntentKind::QueryKb
    } else if has_any(&lower, &["check", "validate", "verify", "lint"]) {
        IntentKind::Check
    } else if has_any(
        &lower,
        &["optimize", "make faster", "improve performance", "speed up"],
    ) {
        IntentKind::Optimize
    } else if has_any(
        &lower,
        &[
            "add contract",
            "add precondition",
            "add postcondition",
            "add invariant",
            "spec",
        ],
    ) {
        IntentKind::AddContracts
    } else if has_any(
        &lower,
        &["refactor", "rename", "restructure", "clean up", "simplify"],
    ) {
        IntentKind::Refactor
    } else if has_any(
        &lower,
        &[
            "add",
            "sum",
            "subtract",
            "multiply",
            "divide",
            "factorial",
            "fibonacci",
            "compute",
            "calculate",
        ],
    ) {
        IntentKind::GenerateFunction
    } else {
        // Default to function generation for unrecognized intents.
        IntentKind::GenerateFunction
    };

    let mut intent = Intent::new(kind.clone(), trimmed);

    // Extract name if present: "called X", "named X", "function X".
    intent.name = extract_quoted_or_named(trimmed);

    // Extract parameters from NL: "that takes X: Type and Y: Type"
    intent.params = extract_params_from_nl(trimmed);

    // Extract return type: "returns Type", "returning Type"
    intent.return_type = extract_return_from_nl(trimmed);

    // Extract effects: "with IO", "with effects io, net"
    intent.effects = extract_effects_from_nl(trimmed);

    // Extract preconditions: "where X > 0", "requires X > 0"
    intent.preconditions = extract_preconditions_from_nl(trimmed);

    // Extract postconditions: "ensures result > 0"
    intent.postconditions = extract_postconditions_from_nl(trimmed);

    // Extract source code block (```...```)
    intent.source = extract_code_block(trimmed);

    // Extract KB facts: "fact parent(alice, bob)"
    intent.kb_facts = extract_kb_facts(trimmed);

    // Extract KB queries: "query ancestor(alice, ?)"
    intent.kb_queries = extract_kb_queries(trimmed);

    // Extract extras: "population 100", "generations 50", "size 4"
    extract_extras(trimmed, &mut intent.extras);

    intent
}

// ═══════════════════════════════════════════════════════════════════
// NL Extraction Helpers
// ═══════════════════════════════════════════════════════════════════

fn has_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

fn extract_quoted_or_named(input: &str) -> Option<String> {

    // Try backtick-quoted: `name`.
    if let Some(start) = input.find('`') {
        if let Some(end) = input[start + 1..].find('`') {
            let name = &input[start + 1..start + 1 + end];
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    // Try "called X" or "named X".
    for prefix in &[
        "called ",
        "named ",
        "function ",
        "struct ",
        "enum ",
        "trait ",
        "net ",
        "kb ",
        "agent ",
        "swarm ",
    ] {
        let lower = input.to_lowercase();
        if let Some(pos) = lower.find(prefix) {
            let rest = &input[pos + prefix.len()..];
            let name = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            if !name.is_empty() && !is_stop_word(name) {
                return Some(to_snake_case(name));
            }
        }
    }

    None
}

fn is_stop_word(word: &str) -> bool {
    matches!(
        word.to_lowercase().as_str(),
        "a" | "an"
            | "the"
            | "that"
            | "which"
            | "with"
            | "for"
            | "and"
            | "or"
            | "to"
            | "of"
            | "in"
            | "on"
            | "at"
            | "by"
            | "from"
            | "is"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "being"
            | "have"
            | "has"
            | "had"
            | "do"
            | "does"
            | "did"
            | "will"
            | "would"
            | "could"
            | "should"
            | "may"
            | "might"
            | "must"
            | "shall"
            | "can"
            | "need"
            | "it"
            | "this"
            | "these"
            | "those"
            | "two"
            | "three"
            | "numbers"
    )
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

fn extract_params_from_nl(input: &str) -> Vec<(String, String)> {
    let lower = input.to_lowercase();
    let mut params = Vec::new();

    // Pattern: "takes X: Type and Y: Type" or "with parameters X: Type, Y: Type"
    let start_markers = [
        "takes ",
        "taking ",
        "with parameters ",
        "with params ",
        "accepting ",
    ];
    for marker in &start_markers {
        if let Some(pos) = lower.find(marker) {
            let rest = &input[pos + marker.len()..];
            // Parse comma or "and" separated param:type pairs.
            for segment in rest.split(|c: char| c == ',' || c == ';').map(str::trim) {
                for part in segment.split(" and ").map(str::trim) {
                    if let Some((name, ty)) = parse_param_pair(part) {
                        params.push((name, ty));
                    }
                }
                // Stop at sentence boundary.
                if segment.contains('.') || segment.contains("return") {
                    break;
                }
            }
        }
    }

    // Pattern: "two numbers" → a: i64, b: i64
    if params.is_empty() && lower.contains("two numbers") {
        params.push(("a".into(), "i64".into()));
        params.push(("b".into(), "i64".into()));
    }

    // Pattern: "two integers" → a: i64, b: i64
    if params.is_empty() && lower.contains("two integers") {
        params.push(("a".into(), "i64".into()));
        params.push(("b".into(), "i64".into()));
    }

    // Pattern: "two strings" → a: String, b: String
    if params.is_empty() && lower.contains("two strings") {
        params.push(("a".into(), "String".into()));
        params.push(("b".into(), "String".into()));
    }

    // Pattern: "a number" → n: i64
    if params.is_empty()
        && (lower.contains("a number")
            || lower.contains("an integer")
            || lower.contains("an i64")
            || lower.contains("a i32"))
    {
        params.push(("n".into(), "i64".into()));
    }

    // Pattern: "a string" → s: String
    if params.is_empty() && lower.contains("a string") {
        params.push(("s".into(), "String".into()));
    }

    params
}

fn parse_param_pair(s: &str) -> Option<(String, String)> {
    // Try "name: Type"
    if let Some(colon) = s.find(':') {
        let name = s[..colon].trim().split_whitespace().last()?;
        let ty = s[colon + 1..].trim().split_whitespace().next()?;
        if !name.is_empty() && !ty.is_empty() {
            return Some((to_snake_case(name), ty.to_string()));
        }
    }

    // Try "Type name"
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 2 {
        let ty = map_nl_type(parts[0]);
        let name = to_snake_case(parts[1]);
        if !name.is_empty() {
            return Some((name, ty));
        }
    }

    None
}

fn map_nl_type(word: &str) -> String {
    match word.to_lowercase().as_str() {
        "number" | "integer" | "int" => "i64".into(),
        "float" | "decimal" | "double" => "f64".into(),
        "string" | "text" | "str" => "String".into(),
        "boolean" | "bool" => "bool".into(),
        "byte" => "u8".into(),
        "char" | "character" => "char".into(),
        "usize" | "index" => "usize".into(),
        other => other.to_string(),
    }
}

fn extract_return_from_nl(input: &str) -> Option<String> {
    let lower = input.to_lowercase();
    for marker in &["returns ", "returning ", "return type ", "-> "] {
        if let Some(pos) = lower.find(marker) {
            let rest = &input[pos + marker.len()..];
            let ty = rest
                .split(|c: char| c == ' ' || c == ',' || c == '.')
                .next()
                .unwrap_or("");
            if !ty.is_empty() {
                return Some(map_nl_type(ty));
            }
        }
    }

    // Infer from common patterns.
    if lower.contains("adds")
        || lower.contains("sum")
        || lower.contains("subtract")
        || lower.contains("multiply")
        || lower.contains("divide")
        || lower.contains("factorial")
        || lower.contains("fibonacci")
        || lower.contains("max")
        || lower.contains("min")
    {
        return Some("i64".into());
    }

    None
}

fn infer_params(description: &str) -> Vec<(String, String)> {
    let lower = description.to_lowercase();

    if lower.contains("two numbers") || lower.contains("two integers") {
        return vec![("a".into(), "i64".into()), ("b".into(), "i64".into())];
    }
    if lower.contains("two strings") {
        return vec![("a".into(), "String".into()), ("b".into(), "String".into())];
    }
    if lower.contains("a number")
        || lower.contains("an integer")
        || lower.contains("factorial")
        || lower.contains("fibonacci")
    {
        return vec![("n".into(), "i64".into())];
    }
    if lower.contains("a string") {
        return vec![("s".into(), "String".into())];
    }

    Vec::new()
}

fn infer_return_type(description: &str) -> Option<String> {
    let lower = description.to_lowercase();
    if lower.contains("adds")
        || lower.contains("sum")
        || lower.contains("subtract")
        || lower.contains("multiply")
        || lower.contains("divide")
        || lower.contains("factorial")
        || lower.contains("fibonacci")
        || lower.contains("max")
        || lower.contains("min")
        || lower.contains("compute")
        || lower.contains("calculate")
    {
        return Some("i64".into());
    }
    if lower.contains("concatenate") || lower.contains("join") || lower.contains("format") {
        return Some("String".into());
    }
    if lower.contains("check")
        || lower.contains("is ")
        || lower.contains("has ")
        || lower.contains("equal")
        || lower.contains("compare")
    {
        return Some("bool".into());
    }
    None
}

fn infer_effects(description: &str) -> Vec<String> {
    let lower = description.to_lowercase();
    let mut effects = Vec::new();
    if lower.contains("print")
        || lower.contains("display")
        || lower.contains("log")
        || lower.contains("write")
    {
        effects.push("IO".into());
    }
    if lower.contains("read file")
        || lower.contains("write file")
        || lower.contains("load")
        || lower.contains("save")
    {
        effects.push("FS".into());
    }
    if lower.contains("http")
        || lower.contains("request")
        || lower.contains("fetch")
        || lower.contains("api")
        || lower.contains("network")
    {
        effects.push("Net".into());
    }
    if lower.contains("async")
        || lower.contains("concurrent")
        || lower.contains("spawn")
        || lower.contains("parallel")
    {
        effects.push("Async".into());
    }
    if lower.contains("random") || lower.contains("rand") {
        effects.push("Rng".into());
    }
    if lower.contains("gpu") || lower.contains("cuda") {
        effects.push("Gpu".into());
    }
    if lower.contains("llm") || lower.contains("language model") || lower.contains("ai ") {
        effects.push("Llm".into());
    }
    effects
}

fn extract_effects_from_nl(input: &str) -> Vec<String> {
    let lower = input.to_lowercase();
    let mut effects = Vec::new();

    for marker in &["with effects ", "effects: ", "/ "] {
        if let Some(pos) = lower.find(marker) {
            let rest = &input[pos + marker.len()..];
            for part in rest.split(|c: char| c == ',' || c == ' ') {
                let eff = part.trim();
                if !eff.is_empty() && eff.chars().next().map_or(false, |c| c.is_uppercase()) {
                    effects.push(eff.to_string());
                }
            }
        }
    }

    effects
}

fn extract_preconditions_from_nl(input: &str) -> Vec<String> {
    let mut preconds = Vec::new();
    let lower = input.to_lowercase();

    for marker in &["requires ", "where ", "precondition: ", "@req "] {
        if let Some(pos) = lower.find(marker) {
            let rest = &input[pos + marker.len()..];
            let cond: String = rest
                .chars()
                .take_while(|c| *c != '.' && *c != ';' && *c != '\n')
                .collect();
            let cond = cond.trim();
            if !cond.is_empty() {
                preconds.push(cond.to_string());
            }
        }
    }

    preconds
}

fn extract_postconditions_from_nl(input: &str) -> Vec<String> {
    let mut postconds = Vec::new();
    let lower = input.to_lowercase();

    for marker in &["ensures ", "postcondition: ", "@ens "] {
        if let Some(pos) = lower.find(marker) {
            let rest = &input[pos + marker.len()..];
            let cond: String = rest
                .chars()
                .take_while(|c| *c != '.' && *c != ';' && *c != '\n')
                .collect();
            let cond = cond.trim();
            if !cond.is_empty() {
                postconds.push(cond.to_string());
            }
        }
    }

    postconds
}

fn extract_code_block(input: &str) -> Option<String> {
    if let Some(start) = input.find("```") {
        let after_start = &input[start + 3..];
        // Skip language identifier on first line.
        let code_start = after_start.find('\n').map(|p| p + 1).unwrap_or(0);
        if let Some(end) = after_start[code_start..].find("```") {
            let code = &after_start[code_start..code_start + end];
            return Some(code.trim().to_string());
        }
    }
    None
}

fn extract_kb_facts(input: &str) -> Vec<(String, Vec<String>)> {
    let mut facts = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("fact ") {
            if let Some((pred, args)) = parse_predicate_call(rest) {
                facts.push((pred, args));
            }
        }
    }
    facts
}

fn extract_kb_queries(input: &str) -> Vec<(String, Vec<String>)> {
    let mut queries = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("query ") {
            if let Some((pred, args)) = parse_predicate_call(rest) {
                queries.push((pred, args));
            }
        }
    }
    queries
}

fn parse_predicate_call(s: &str) -> Option<(String, Vec<String>)> {
    let open = s.find('(')?;
    let close = s.find(')')?;
    if close <= open {
        return None;
    }
    let pred = s[..open].trim().to_string();
    let args_str = &s[open + 1..close];
    let args: Vec<String> = args_str
        .split(',')
        .map(|a| a.trim().trim_matches('"').trim_matches('\'').to_string())
        .collect();
    Some((pred, args))
}

fn extract_extras(input: &str, extras: &mut BTreeMap<String, String>) {
    let lower = input.to_lowercase();

    if let Some(pos) = lower.find("population ") {
        let rest = &input[pos + 11..];
        if let Some(num) = rest.split_whitespace().next() {
            if num.chars().all(|c| c.is_ascii_digit()) {
                extras.insert("population".into(), num.into());
            }
        }
    }

    if let Some(pos) = lower.find("generations ") {
        let rest = &input[pos + 12..];
        if let Some(num) = rest.split_whitespace().next() {
            if num.chars().all(|c| c.is_ascii_digit()) {
                extras.insert("generations".into(), num.into());
            }
        }
    }

    if let Some(pos) = lower.find("size ") {
        let rest = &input[pos + 5..];
        if let Some(num) = rest.split_whitespace().next() {
            if num.chars().all(|c| c.is_ascii_digit()) {
                extras.insert("size".into(), num.into());
            }
        }
    }

    for marker in &["topology ", "mesh", "ring", "star", "broadcast"] {
        if lower.contains(marker) {
            let topo = marker.trim().to_string();
            extras.insert("topology".into(), topo);
        }
    }
}

fn extract_function_name(description: &str) -> String {
    let lower = description.to_lowercase();

    // Try to extract a meaningful name from the description.
    if lower.contains("add") || lower.contains("sum") {
        return "add".into();
    }
    if lower.contains("subtract") || lower.contains("difference") {
        return "subtract".into();
    }
    if lower.contains("multiply") || lower.contains("product") {
        return "multiply".into();
    }
    if lower.contains("divide") || lower.contains("quotient") {
        return "divide".into();
    }
    if lower.contains("max") || lower.contains("maximum") {
        return "max".into();
    }
    if lower.contains("min") || lower.contains("minimum") {
        return "min".into();
    }
    if lower.contains("factorial") {
        return "factorial".into();
    }
    if lower.contains("fibonacci") || lower.contains("fib") {
        return "fibonacci".into();
    }
    if lower.contains("sort") {
        return "sort".into();
    }
    if lower.contains("search") || lower.contains("find") {
        return "search".into();
    }
    if lower.contains("reverse") {
        return "reverse".into();
    }
    if lower.contains("count") {
        return "count".into();
    }
    if lower.contains("print") || lower.contains("display") {
        return "display".into();
    }
    if lower.contains("negate") {
        return "negate".into();
    }
    if lower.contains("identity") || lower.contains("echo") {
        return "identity".into();
    }
    if lower.contains("concat") {
        return "concat".into();
    }
    if lower.contains("format") {
        return "format_value".into();
    }
    if lower.contains("check") || lower.contains("validate") {
        return "check".into();
    }
    if lower.contains("compare") {
        return "compare".into();
    }
    if lower.contains("compute") || lower.contains("calculate") {
        return "compute".into();
    }

    // Use first meaningful noun/verb from description.
    let words: Vec<&str> = lower
        .split_whitespace()
        .filter(|w| w.len() > 2 && !is_stop_word(w))
        .collect();
    if let Some(word) = words.first() {
        to_snake_case(word)
    } else {
        "generated".into()
    }
}

fn extract_type_name(description: &str) -> String {
    let lower = description.to_lowercase();

    // Look for capitalized word after common prefixes.
    for prefix in &[
        "called ", "named ", "struct ", "enum ", "trait ", "type ", "net ", "kb ", "agent ",
        "swarm ",
    ] {
        if let Some(pos) = lower.find(prefix) {
            let rest = &description[pos + prefix.len()..];
            let name = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            if !name.is_empty() && !is_stop_word(name) {
                return capitalize_first(name);
            }
        }
    }

    // Try to find a meaningful capitalized word.
    for word in description.split_whitespace() {
        if word.len() > 1
            && word.chars().next().map_or(false, |c| c.is_uppercase())
            && !is_stop_word(word)
        {
            return word.to_string();
        }
    }

    "Generated".into()
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn infer_struct_fields(description: &str) -> Vec<(String, String)> {
    let lower = description.to_lowercase();
    let mut fields = Vec::new();

    // Pattern: "with x and y coordinates"
    if lower.contains("x")
        && lower.contains("y")
        && (lower.contains("coordinate") || lower.contains("point") || lower.contains("position"))
    {
        fields.push(("x".into(), "f64".into()));
        fields.push(("y".into(), "f64".into()));
        if lower.contains("z") {
            fields.push(("z".into(), "f64".into()));
        }
        return fields;
    }

    // Pattern: "with name and age"
    if lower.contains("name") && lower.contains("age") {
        fields.push(("name".into(), "String".into()));
        fields.push(("age".into(), "u32".into()));
        if lower.contains("email") {
            fields.push(("email".into(), "String".into()));
        }
        return fields;
    }

    // Pattern: "with width and height"
    if lower.contains("width") && lower.contains("height") {
        fields.push(("width".into(), "f64".into()));
        fields.push(("height".into(), "f64".into()));
        return fields;
    }

    // Pattern: "fields: name: Type, name: Type"
    if let Some(pos) = lower.find("fields") {
        let rest = &description[pos + 6..];
        let rest = rest.trim_start_matches(|c: char| c == ':' || c == ' ');
        for segment in rest.split(',') {
            if let Some((name, ty)) = parse_param_pair(segment.trim()) {
                fields.push((name, ty));
            }
        }
        if !fields.is_empty() {
            return fields;
        }
    }

    // Default: single data field.
    fields.push(("value".into(), "i64".into()));
    fields
}

fn infer_enum_variants(description: &str) -> Vec<String> {
    let lower = description.to_lowercase();

    // Pattern: "for colors"
    if lower.contains("color") || lower.contains("colour") {
        return vec![
            "Red".into(),
            "Green".into(),
            "Blue".into(),
            "Yellow".into(),
            "White".into(),
            "Black".into(),
        ];
    }

    // Pattern: "for directions"
    if lower.contains("direction") {
        return vec!["North".into(), "South".into(), "East".into(), "West".into()];
    }

    // Pattern: "for days"
    if lower.contains("day") || lower.contains("weekday") {
        return vec![
            "Monday".into(),
            "Tuesday".into(),
            "Wednesday".into(),
            "Thursday".into(),
            "Friday".into(),
            "Saturday".into(),
            "Sunday".into(),
        ];
    }

    // Pattern: "for seasons"
    if lower.contains("season") {
        return vec![
            "Spring".into(),
            "Summer".into(),
            "Autumn".into(),
            "Winter".into(),
        ];
    }

    // Pattern: "for result" or "for option"
    if lower.contains("result") || lower.contains("outcome") {
        return vec!["Success".into(), "Failure".into(), "Pending".into()];
    }

    // Pattern: explicit variants: "variants: A, B, C"
    if let Some(pos) = lower.find("variants") {
        let rest = &description[pos + 8..];
        let rest = rest.trim_start_matches(|c: char| c == ':' || c == ' ');
        let variants: Vec<String> = rest
            .split(|c: char| c == ',' || c == ';')
            .map(|v| capitalize_first(v.trim()))
            .filter(|v| !v.is_empty())
            .collect();
        if !variants.is_empty() {
            return variants;
        }
    }

    // Default.
    vec!["A".into(), "B".into(), "C".into()]
}

fn infer_trait_methods(_description: &str) -> Vec<(String, Vec<(String, String)>, Option<String>)> {
    // Return (method_name, params, return_type).
    vec![(
        "process".into(),
        vec![("input".into(), "String".into())],
        Some("String".into()),
    )]
}

fn infer_net_layers(description: &str) -> Vec<(String, String, Vec<String>)> {
    let lower = description.to_lowercase();

    // CNN pattern.
    if lower.contains("cnn") || lower.contains("convolutional") || lower.contains("image") {
        return vec![
            (
                "conv1".into(),
                "Conv2d".into(),
                vec!["3".into(), "32".into(), "3".into()],
            ),
            (
                "conv2".into(),
                "Conv2d".into(),
                vec!["32".into(), "64".into(), "3".into()],
            ),
            ("fc".into(), "Linear".into(), vec!["64".into(), "10".into()]),
        ];
    }

    // Classifier pattern.
    if lower.contains("classifier") || lower.contains("classification") {
        return vec![
            (
                "fc1".into(),
                "Linear".into(),
                vec!["784".into(), "256".into()],
            ),
            (
                "fc2".into(),
                "Linear".into(),
                vec!["256".into(), "128".into()],
            ),
            (
                "fc3".into(),
                "Linear".into(),
                vec!["128".into(), "10".into()],
            ),
        ];
    }

    // Default MLP.
    vec![
        (
            "input".into(),
            "Linear".into(),
            vec!["64".into(), "128".into()],
        ),
        (
            "hidden".into(),
            "Linear".into(),
            vec!["128".into(), "64".into()],
        ),
        (
            "output".into(),
            "Linear".into(),
            vec!["64".into(), "10".into()],
        ),
    ]
}

fn infer_capabilities(description: &str) -> Vec<String> {
    let lower = description.to_lowercase();
    let mut caps = Vec::new();

    if lower.contains("read") || lower.contains("analyze") || lower.contains("inspect") {
        caps.push("read_source".into());
    }
    if lower.contains("write")
        || lower.contains("edit")
        || lower.contains("modify")
        || lower.contains("generate")
    {
        caps.push("write_source".into());
    }
    if lower.contains("type") || lower.contains("check") || lower.contains("verify") {
        caps.push("query_types".into());
    }
    if lower.contains("test") || lower.contains("run") || lower.contains("execute") {
        caps.push("run_tests".into());
    }
    if lower.contains("refactor") || lower.contains("rename") {
        caps.push("refactor".into());
    }
    if lower.contains("review") || lower.contains("audit") {
        caps.push("code_review".into());
    }
    if lower.contains("document") || lower.contains("doc") {
        caps.push("documentation".into());
    }
    if lower.contains("deploy") || lower.contains("build") {
        caps.push("build".into());
    }

    if caps.is_empty() {
        caps.push("read_source".into());
        caps.push("write_source".into());
    }

    caps
}

/// Parse a type string into an AST Type node.
fn parse_type_str(s: &str) -> Type {
    let trimmed = s.trim();
    match trimmed {
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" | "f32" | "f64" | "bool" | "char" | "String" | "str" => Type::Path {
            segments: vec![trimmed.to_string()],
            type_args: Vec::new(),
        },
        "()" => Type::Tuple {
            elements: Vec::new(),
        },
        "!" => Type::Never,
        "_" => Type::Inferred,
        _ => Type::Path {
            segments: vec![trimmed.to_string()],
            type_args: Vec::new(),
        },
    }
}

fn build_contracts(
    preconds: &[String],
    postconds: &[String],
    invariants: &[String],
) -> Vec<ContractClause> {
    let mut contracts = Vec::new();
    for pre in preconds {
        contracts.push(ContractClause {
            kind: ContractClauseKind::Requires,
            condition: pre.clone(),
            message: None,
        });
    }
    for post in postconds {
        contracts.push(ContractClause {
            kind: ContractClauseKind::Ensures,
            condition: post.clone(),
            message: None,
        });
    }
    for inv in invariants {
        contracts.push(ContractClause {
            kind: ContractClauseKind::Invariant,
            condition: inv.clone(),
            message: None,
        });
    }
    contracts
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Intent parsing ─────────────────────────────────────────────

    #[test]
    fn parse_function_intent() {
        let intent = parse_intent("create a function that adds two numbers");
        assert_eq!(intent.kind, IntentKind::GenerateFunction);
        assert_eq!(intent.params.len(), 2);
        assert_eq!(intent.params[0], ("a".to_string(), "i64".to_string()));
    }

    #[test]
    fn parse_struct_intent() {
        let intent = parse_intent("define a struct called Point with x y coordinates");
        assert_eq!(intent.kind, IntentKind::GenerateStruct);
        assert_eq!(intent.name, Some("point".to_string()));
    }

    #[test]
    fn parse_enum_intent() {
        let intent = parse_intent("create an enum for colors");
        assert_eq!(intent.kind, IntentKind::GenerateEnum);
    }

    #[test]
    fn parse_net_intent() {
        let intent = parse_intent("build a neural network for image classification");
        assert_eq!(intent.kind, IntentKind::GenerateNet);
    }

    #[test]
    fn parse_agent_intent() {
        let intent = parse_intent("create an agent that can read and write code");
        assert_eq!(intent.kind, IntentKind::GenerateAgent);
    }

    #[test]
    fn parse_kb_intent() {
        let intent = parse_intent(
            "create a kb called Family\nfact parent(alice, bob)\nfact parent(bob, charlie)",
        );
        assert_eq!(intent.kind, IntentKind::GenerateKb);
        assert_eq!(intent.kb_facts.len(), 2);
    }

    #[test]
    fn parse_explain_intent() {
        let intent =
            parse_intent("explain this code\n```\nfn add(a: i32, b: i32) -> i32 { a + b }\n```");
        assert_eq!(intent.kind, IntentKind::Explain);
        assert!(intent.source.is_some());
    }

    #[test]
    fn parse_with_effects() {
        let intent = parse_intent("create a function that prints a number with effects IO");
        assert_eq!(intent.kind, IntentKind::GenerateFunction);
    }

    #[test]
    fn parse_with_contracts() {
        let intent = parse_intent(
            "create a function that adds two numbers requires a > 0 ensures result > 0",
        );
        assert_eq!(intent.kind, IntentKind::GenerateFunction);
        assert!(!intent.preconditions.is_empty());
    }

    #[test]
    fn parse_named_function() {
        let intent = parse_intent(
            "create a function called `factorial` that computes factorial of a number",
        );
        assert_eq!(intent.name, Some("factorial".to_string()));
    }

    // ── Code generation via engine ─────────────────────────────────

    #[test]
    fn generate_add_function() {
        let mut engine = NlEngine::new();
        let response = engine.process("create a function that adds two numbers");
        assert!(response.ok);
        assert!(!response.code_human.is_empty());
        assert!(!response.code_agent.is_empty());
        assert!(response.code_human.contains("fn"));
    }

    #[test]
    fn generate_struct() {
        let mut engine = NlEngine::new();
        let response = engine.process("define a struct called Point with x y coordinates");
        assert!(response.ok);
        assert!(response.code_human.contains("struct") || response.code_human.contains("Point"));
    }

    #[test]
    fn generate_enum() {
        let mut engine = NlEngine::new();
        let response = engine.process("create an enum for colors");
        assert!(response.ok);
        assert!(response.code_human.contains("Red") || response.code_human.contains("Green"));
    }

    #[test]
    fn generate_factorial() {
        let mut engine = NlEngine::new();
        let response = engine
            .process("create a function called `factorial` that computes factorial of a number");
        assert!(!response.explanation.is_empty());
        assert!(response.code_human.contains("factorial"));
    }

    #[test]
    fn generate_fibonacci() {
        let mut engine = NlEngine::new();
        let response = engine
            .process("generate a function called `fibonacci` that computes fibonacci of a number");
        assert!(!response.explanation.is_empty());
        assert!(response.code_human.contains("fibonacci"));
    }

    #[test]
    fn generate_neural_network() {
        let mut engine = NlEngine::new();
        let response = engine.process("build a neural network for classification");
        assert!(!response.explanation.is_empty());
        assert!(!response.code_human.is_empty());
    }

    #[test]
    fn generate_agent() {
        let mut engine = NlEngine::new();
        let response = engine.process("create an agent that can read and write code");
        assert!(response.ok);
        assert!(response.code_human.contains("agent") || response.code_agent.contains("agent"));
    }

    #[test]
    fn generate_swarm() {
        let mut engine = NlEngine::new();
        let response = engine.process("create a swarm of 4 worker agents with star topology");
        assert!(!response.explanation.is_empty());
    }

    // ── KB integration ─────────────────────────────────────────────

    #[test]
    fn kb_add_and_query() {
        let mut engine = NlEngine::new();

        // Add knowledge.
        engine.add_knowledge("function_hint", vec!["add".into(), "check overflow".into()]);
        engine.add_knowledge(
            "type_constraint",
            vec!["i64".into(), "must not overflow".into()],
        );

        // Generate function — should pick up KB hints.
        let response = engine.process("create a function that adds two numbers");
        assert!(response.ok);
        assert!(response.explanation.contains("KB-derived"));
    }

    #[test]
    fn kb_fact_ingestion() {
        let mut engine = NlEngine::new();
        let response = engine.process(
            "create a kb called Family\nfact parent(alice, bob)\nfact parent(bob, charlie)",
        );
        assert!(!response.explanation.is_empty());

        // Query the persistent KB.
        let results = engine.query_knowledge("parent", &["alice", "?"]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], vec!["alice".to_string(), "bob".to_string()]);
    }

    #[test]
    fn kb_query_response() {
        let mut engine = NlEngine::new();
        engine.add_knowledge("parent", vec!["alice".into(), "bob".into()]);
        engine.add_knowledge("parent", vec!["bob".into(), "charlie".into()]);

        let response = engine.process("query parent(alice, ?)");
        assert!(response.ok);
        assert_eq!(response.kb_results.len(), 1);
        assert_eq!(response.kb_results[0].1.len(), 1);
    }

    // ── Validation pipeline ────────────────────────────────────────

    #[test]
    fn generated_code_validates() {
        let mut engine = NlEngine::new();
        let response = engine.process("create a function that adds two numbers");
        assert!(response.ok);
        // The generated code should pass through the compiler pipeline without errors.
        assert_eq!(
            response
                .diagnostics
                .iter()
                .filter(|d| d.severity == hir::Severity::Error)
                .count(),
            0
        );
    }

    #[test]
    fn explain_code() {
        let mut engine = NlEngine::new();
        let response =
            engine.process("explain this code\n```\nfn add(a: i32, b: i32) -> i32 { a + b }\n```");
        assert!(response.ok);
        assert!(response.explanation.contains("function"));
    }

    #[test]
    fn evolve_block() {
        let mut engine = NlEngine::new();
        let response = engine
            .process("create an evolutionary optimization with population 200 generations 100");
        assert!(!response.explanation.is_empty());
    }
}

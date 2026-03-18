use crate::ast::{self, ContractClauseKind, ItemKind, Module, Visibility};
/// Capability Manifest — JSON manifest generation per crate for Forge discovery.
///
/// Scans a parsed module and emits a structured manifest describing:
/// - agents and their capabilities / approval requirements
/// - declared effects
/// - specifications (contracts)
/// - exported functions and types
///
/// The manifest enables capability-indexed search in the Forge package registry.
use serde::{Deserialize, Serialize};

// ── Manifest types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateManifest {
    pub name: String,
    pub version: String,
    pub agents: Vec<AgentEntry>,
    pub functions: Vec<FunctionEntry>,
    pub types: Vec<TypeEntry>,
    pub effects: Vec<EffectEntry>,
    pub specs: Vec<SpecEntry>,
    /// Flat aggregated capability index for Forge search.
    pub capability_index: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    pub name: String,
    pub capabilities: Vec<String>,
    pub requires_approval: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionEntry {
    pub name: String,
    pub public: bool,
    pub is_async: bool,
    pub params: usize,
    pub has_contracts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeEntry {
    pub name: String,
    pub kind: String, // "struct", "enum", "type_alias"
    pub public: bool,
    pub has_invariant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectEntry {
    pub name: String,
    pub operations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecEntry {
    pub name: String,
    pub num_requires: usize,
    pub num_ensures: usize,
}

// ── Manifest generation ──────────────────────────────────────────────

/// Generate a capability manifest from a parsed module.
pub fn generate(module: &Module, crate_name: &str, version: &str) -> CrateManifest {
    let mut manifest = CrateManifest {
        name: crate_name.into(),
        version: version.into(),
        agents: Vec::new(),
        functions: Vec::new(),
        types: Vec::new(),
        effects: Vec::new(),
        specs: Vec::new(),
        capability_index: Vec::new(),
    };

    for item in &module.items {
        let is_pub = item.visibility == Visibility::Public;
        match &item.kind {
            ItemKind::Agent(ad) => {
                for cap in &ad.capabilities {
                    if !manifest.capability_index.contains(cap) {
                        manifest.capability_index.push(cap.clone());
                    }
                }
                manifest.agents.push(AgentEntry {
                    name: ad.name.clone(),
                    capabilities: ad.capabilities.clone(),
                    requires_approval: ad.requires_approval.clone(),
                });
            }
            ItemKind::Function(fd) => {
                manifest.functions.push(FunctionEntry {
                    name: fd.name.clone(),
                    public: is_pub,
                    is_async: fd.is_async,
                    params: fd.params.len(),
                    has_contracts: !fd.contracts.is_empty(),
                });
            }
            ItemKind::Struct(sd) => {
                let has_inv = sd.contracts.iter().any(|c| c.kind == ContractClauseKind::Invariant);
                manifest.types.push(TypeEntry {
                    name: sd.name.clone(),
                    kind: "struct".into(),
                    public: is_pub,
                    has_invariant: has_inv,
                });
            }
            ItemKind::Enum(ed) => {
                manifest.types.push(TypeEntry {
                    name: ed.name.clone(),
                    kind: "enum".into(),
                    public: is_pub,
                    has_invariant: false,
                });
            }
            ItemKind::TypeAlias(ta) => {
                manifest.types.push(TypeEntry {
                    name: ta.name.clone(),
                    kind: "type_alias".into(),
                    public: is_pub,
                    has_invariant: false,
                });
            }
            ItemKind::Effect(ef) => {
                manifest.effects.push(EffectEntry {
                    name: ef.name.clone(),
                    operations: ef.operations.iter().map(|op| op.name.clone()).collect(),
                });
            }
            ItemKind::Spec(sp) => {
                let num_requires =
                    sp.items.iter().filter(|i| matches!(i, ast::SpecItem::Require(_))).count();
                let num_ensures =
                    sp.items.iter().filter(|i| matches!(i, ast::SpecItem::Ensure(_))).count();
                manifest.specs.push(SpecEntry { name: sp.name.clone(), num_requires, num_ensures });
            }
            _ => {}
        }
    }

    manifest.capability_index.sort();
    manifest
}

/// Serialize manifest to JSON string.
pub fn to_json(manifest: &CrateManifest) -> String {
    serde_json::to_string_pretty(manifest).unwrap_or_default()
}

/// Serialize manifest to serde_json::Value.
pub fn to_json_value(manifest: &CrateManifest) -> serde_json::Value {
    serde_json::to_value(manifest).unwrap_or_default()
}

// ── Capability-indexed search (Forge integration) ────────────────────

/// Search a collection of manifests by required capability.
pub fn search_by_capability<'a>(
    manifests: &'a [CrateManifest],
    cap: &str,
) -> Vec<&'a CrateManifest> {
    manifests.iter().filter(|m| m.capability_index.contains(&cap.into())).collect()
}

/// Search manifests for any that expose a given effect.
pub fn search_by_effect<'a>(
    manifests: &'a [CrateManifest],
    effect: &str,
) -> Vec<&'a CrateManifest> {
    manifests.iter().filter(|m| m.effects.iter().any(|e| e.name == effect)).collect()
}

/// Search manifests for exported agents.
pub fn search_agents<'a>(manifests: &'a [CrateManifest]) -> Vec<(&'a str, &'a AgentEntry)> {
    manifests.iter().flat_map(|m| m.agents.iter().map(move |a| (m.name.as_str(), a))).collect()
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn path_type(name: &str) -> Type {
        Type::Path { segments: vec![name.into()], type_args: vec![] }
    }

    fn sample_module() -> Module {
        Module {
            items: vec![
                Item {
                    kind: ItemKind::Agent(AgentDef {
                        name: "Reviewer".into(),
                        capabilities: vec!["read_source".into(), "query_types".into()],
                        requires_approval: vec!["write_source".into()],
                    }),
                    visibility: Visibility::Private,
                    attributes: vec![],
                },
                Item {
                    kind: ItemKind::Function(FunctionDef {
                        name: "check".into(),
                        generics: vec![],
                        params: vec![Param { name: "x".into(), ty: path_type("i32") }],
                        return_type: Some(path_type("bool")),
                        where_clause: vec![],
                        effects: vec![],
                        body: Block { stmts: vec![], tail_expr: None },
                        contracts: vec![ContractClause {
                            kind: ContractClauseKind::Requires,
                            condition: "x > 0".into(),
                            message: None,
                        }],
                        is_async: false,
                        is_unsafe: false,
                    }),
                    visibility: Visibility::Public,
                    attributes: vec![],
                },
                Item {
                    kind: ItemKind::Struct(StructDef {
                        name: "Config".into(),
                        generics: vec![],
                        fields: vec![],
                        contracts: vec![ContractClause {
                            kind: ContractClauseKind::Invariant,
                            condition: "self.valid()".into(),
                            message: None,
                        }],
                    }),
                    visibility: Visibility::Public,
                    attributes: vec![],
                },
                Item {
                    kind: ItemKind::Enum(EnumDef {
                        name: "Status".into(),
                        generics: vec![],
                        variants: vec![],
                    }),
                    visibility: Visibility::Private,
                    attributes: vec![],
                },
                Item {
                    kind: ItemKind::Effect(EffectDef {
                        name: "IO".into(),
                        operations: vec![
                            EffectOp {
                                name: "read".into(),
                                params: vec![],
                                return_type: Some(path_type("String")),
                            },
                            EffectOp {
                                name: "write".into(),
                                params: vec![Param {
                                    name: "data".into(),
                                    ty: path_type("String"),
                                }],
                                return_type: None,
                            },
                        ],
                    }),
                    visibility: Visibility::Private,
                    attributes: vec![],
                },
                Item {
                    kind: ItemKind::Spec(SpecDef {
                        name: "add_spec".into(),
                        generics: vec![],
                        params: vec![],
                        return_type: None,
                        items: vec![
                            SpecItem::Require("a > 0".into()),
                            SpecItem::Ensure("result > a".into()),
                        ],
                    }),
                    visibility: Visibility::Private,
                    attributes: vec![],
                },
                Item {
                    kind: ItemKind::TypeAlias(TypeAlias {
                        name: "Id".into(),
                        generics: vec![],
                        ty: path_type("u64"),
                        refinement: None,
                    }),
                    visibility: Visibility::Public,
                    attributes: vec![],
                },
            ],
        }
    }

    #[test]
    fn generate_manifest_agents() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        assert_eq!(m.agents.len(), 1);
        assert_eq!(m.agents[0].name, "Reviewer");
        assert_eq!(m.agents[0].capabilities, vec!["read_source", "query_types"]);
        assert_eq!(m.agents[0].requires_approval, vec!["write_source"]);
    }

    #[test]
    fn generate_manifest_functions() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        assert_eq!(m.functions.len(), 1);
        assert!(m.functions[0].public);
        assert!(m.functions[0].has_contracts);
        assert_eq!(m.functions[0].params, 1);
    }

    #[test]
    fn generate_manifest_types() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        assert_eq!(m.types.len(), 3);
        let config = m.types.iter().find(|t| t.name == "Config").unwrap();
        assert_eq!(config.kind, "struct");
        assert!(config.has_invariant);
        let status = m.types.iter().find(|t| t.name == "Status").unwrap();
        assert_eq!(status.kind, "enum");
        assert!(!status.has_invariant);
        let id = m.types.iter().find(|t| t.name == "Id").unwrap();
        assert_eq!(id.kind, "type_alias");
    }

    #[test]
    fn generate_manifest_effects() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        assert_eq!(m.effects.len(), 1);
        assert_eq!(m.effects[0].name, "IO");
        assert_eq!(m.effects[0].operations, vec!["read", "write"]);
    }

    #[test]
    fn generate_manifest_specs() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        assert_eq!(m.specs.len(), 1);
        assert_eq!(m.specs[0].name, "add_spec");
        assert_eq!(m.specs[0].num_requires, 1);
        assert_eq!(m.specs[0].num_ensures, 1);
    }

    #[test]
    fn capability_index_sorted_deduped() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        assert_eq!(m.capability_index, vec!["query_types", "read_source"]);
    }

    #[test]
    fn to_json_roundtrip() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        let json_str = to_json(&m);
        let parsed: CrateManifest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.name, "my_crate");
        assert_eq!(parsed.agents.len(), 1);
    }

    #[test]
    fn to_json_value_has_keys() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        let v = to_json_value(&m);
        assert!(v.get("name").is_some());
        assert!(v.get("agents").is_some());
        assert!(v.get("capability_index").is_some());
    }

    #[test]
    fn search_by_capability_matches() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        let manifests = vec![m];
        let results = search_by_capability(&manifests, "read_source");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "my_crate");
    }

    #[test]
    fn search_by_capability_no_match() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        let manifests = vec![m];
        let results = search_by_capability(&manifests, "nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn search_by_effect_matches() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        let manifests = vec![m];
        let results = search_by_effect(&manifests, "IO");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_by_effect_no_match() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        let manifests = vec![m];
        assert!(search_by_effect(&manifests, "Net").is_empty());
    }

    #[test]
    fn search_agents_returns_all() {
        let m = generate(&sample_module(), "my_crate", "0.1.0");
        let manifests = vec![m];
        let agents = search_agents(&manifests);
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].0, "my_crate");
        assert_eq!(agents[0].1.name, "Reviewer");
    }

    #[test]
    fn empty_module_empty_manifest() {
        let m = generate(&Module { items: vec![] }, "empty", "0.0.0");
        assert!(m.agents.is_empty());
        assert!(m.functions.is_empty());
        assert!(m.types.is_empty());
        assert!(m.effects.is_empty());
        assert!(m.specs.is_empty());
        assert!(m.capability_index.is_empty());
    }

    #[test]
    fn multiple_agents_aggregate_capabilities() {
        let module = Module {
            items: vec![
                Item {
                    kind: ItemKind::Agent(AgentDef {
                        name: "A".into(),
                        capabilities: vec!["net".into(), "fs".into()],
                        requires_approval: vec![],
                    }),
                    visibility: Visibility::Private,
                    attributes: vec![],
                },
                Item {
                    kind: ItemKind::Agent(AgentDef {
                        name: "B".into(),
                        capabilities: vec!["fs".into(), "crypto".into()],
                        requires_approval: vec![],
                    }),
                    visibility: Visibility::Private,
                    attributes: vec![],
                },
            ],
        };
        let m = generate(&module, "multi", "1.0.0");
        assert_eq!(m.agents.len(), 2);
        assert_eq!(m.capability_index, vec!["crypto", "fs", "net"]);
    }
}

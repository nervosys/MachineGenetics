//! # MAGE SKB ↔ RMI Ontology Adapter
//!
//! Translates between MAGE's [`crate::skb`] (255 safety rules across eight
//! [`RuleDatabase`]s) and RMI's [`rmi::core::ontology::Ontology`] (a
//! concept-relation graph used by [`rmi::core::agent::Agent`] discovery).
//!
//! ## Semantic mapping
//!
//! | MAGE SKB          | RMI Ontology                                       |
//! |----------------------|----------------------------------------------------|
//! | [`Rule`]             | [`Concept`] in namespace `air.skb.<database>`      |
//! | [`Rule::category`]   | concept label                                      |
//! | [`Rule::description`]| `formal_definition`                                |
//! | [`Rule::tags`]       | attribute `tags` (List of String)                  |
//! | [`Rule::severity`]   | attribute `severity` (String)                      |
//! | [`Rule::id`]         | concept local-name                                 |
//!
//! Rules in different databases land in distinct namespaces so RMI's
//! `in_namespace` queries can scope to a single safety domain (e.g.
//! `air.skb.ownership` returns only ownership rules).
//!
//! [`kb_to_ontology`] exports the whole knowledge base the same way. Every fact
//! is a row in the `facts` attribute of the concept it is about, so nothing is
//! lost. RMI's [`RelationType`] is a closed set, so only the predicates with a
//! faithful RMI meaning ([`edge_type`]) are also graph edges. Rules' consequences
//! are not facts and are not exported.
//!
//! | Knowledge base            | RMI Ontology                                 |
//! |---------------------------|----------------------------------------------|
//! | `class`                   | `Schema` concept in `air.kb.class`           |
//! | `relation`                | `Relation` concept in `air.kb.relation`      |
//! | `invariant`               | `Axiom` concept in `air.kb.invariant`        |
//! | instance of class `C`     | concept in `air.kb.C`, or `air.skb.<database>` for a safety rule |
//! | a fact about free text    | `Entity` concept in `air.kb.fact.<predicate>` |
//!
//! Every exported concept carries an [`embed`]ding of its name and text, so
//! [`Ontology::find_similar`] finds a concept from a name near its own.
//! [`constrains`] answers which rules constrain a construct.
//!
//! [`RelationType`]: rmi::core::ontology::RelationType
//! [`Rule`]: crate::skb::Rule
//! [`RuleDatabase`]: crate::skb::RuleDatabase
//! [`Concept`]: rmi::core::ontology::Concept

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::kb::{Kb, META};
use crate::skb::{Rule, RuleDatabase, RuleSeverity};
use ndarray::Array1;
use rmi::core::ontology::{
    AttributeValue, Concept, ConceptId, ConceptType, Ontology, Relation, RelationType,
};

/// Map a MAGE rule database to the corresponding RMI ontology namespace.
pub fn database_namespace(db: RuleDatabase) -> &'static str {
    match db {
        RuleDatabase::Ownership => "air.skb.ownership",
        RuleDatabase::Borrow => "air.skb.borrow",
        RuleDatabase::Lifetime => "air.skb.lifetime",
        RuleDatabase::TypeSafety => "air.skb.type_safety",
        RuleDatabase::Concurrency => "air.skb.concurrency",
        RuleDatabase::FFI => "air.skb.ffi",
        RuleDatabase::AgentElision => "air.skb.agent_elision",
        RuleDatabase::SwarmSafety => "air.skb.swarm",
    }
}

/// Render a [`RuleSeverity`] as the string used in RMI ontology attributes.
pub fn severity_label(sev: RuleSeverity) -> &'static str {
    match sev {
        RuleSeverity::Error => "error",
        RuleSeverity::Warning => "warning",
        RuleSeverity::Info => "info",
        RuleSeverity::Hint => "hint",
    }
}

/// Convert a single MAGE [`Rule`] into an RMI [`Concept`].
pub fn rule_to_concept(rule: &Rule) -> Concept {
    let id = ConceptId::new(database_namespace(rule.database), &rule.id);
    let tags = AttributeValue::List(
        rule.tags
            .iter()
            .map(|t| AttributeValue::String(t.clone()))
            .collect(),
    );
    let mut c = Concept::new(id, ConceptType::Constraint)
        .with_label(&rule.category)
        .with_definition(&rule.description)
        .with_confidence(rule.fix_confidence.clamp(0.0, 1.0))
        .with_attribute("severity", AttributeValue::String(severity_label(rule.severity).to_string()))
        .with_attribute("tags", tags)
        .with_attribute("rationale", AttributeValue::String(rule.rationale.clone()));
    if let Some(fix) = &rule.fix_template {
        c = c.with_attribute("fix_template", AttributeValue::String(fix.clone()));
    }
    c
}

/// One hit from a unified SKB / ontology query.
#[derive(Debug, Clone)]
pub struct UnifiedHit {
    /// Fully qualified name (e.g. `"air.skb.ownership/OWN-0001"` or
    /// `"air.neural/Linear"`).
    pub fqn: String,
    /// Human/agent-readable label.
    pub label: String,
    /// Short definition or rule description.
    pub definition: String,
    /// Source: `"skb"` for MAGE rules, `"ontology"` for RMI concepts.
    pub source: &'static str,
}

/// Query both the MAGE SKB and an RMI ontology with a single text
/// predicate, returning unified hits.
///
/// The MAGE side searches rule descriptions, categories, and tags. The
/// RMI side filters concepts whose `formal_definition` or `label` contains
/// the query (case-insensitive substring match).
///
/// This is the seam that lets a MAGE agent ask "find anything about
/// `attention`" and get back both safety rules touching attention and RMI
/// ontology entries for the `ATTN` opcode in the same result set.
pub fn unified_search(query: &str, ontology: &Ontology) -> Vec<UnifiedHit> {
    let needle = query.to_lowercase();
    let mut hits = Vec::new();

    // ── MAGE SKB side: scan all rules across the eight databases ─
    //
    // `RuleDatabase::ALL` rather than a copy of the list. This was a second
    // literal array of the same eight variants, under the comment above
    // promising it scans *all* of them — a promise nothing enforced, since an
    // array literal is not exhaustive the way a `match` is. One list means one
    // place to update and one assertion covering both callers.
    //
    // The `match` twenty lines above *is* exhaustive, so adding a ninth
    // variant is a compile error right here. What that does not force is
    // adding it to `ALL`, and `check-skb-tree.sh` cannot see that gap: it
    // diffs the committed tree against a fresh `--emit-skb` built from `ALL`,
    // so both sides omit the same database. Measured — see
    // `skb::tests::per_database_counts_sum_to_the_total`.
    for db in RuleDatabase::ALL {
        for rule in crate::skb::query_rules_by_db(db).matches {
            let hay = format!(
                "{} {} {} {}",
                rule.category,
                rule.description,
                rule.rationale,
                rule.tags.join(" ")
            )
            .to_lowercase();
            if hay.contains(&needle) {
                hits.push(UnifiedHit {
                    fqn: format!("{}/{}", database_namespace(rule.database), rule.id),
                    label: rule.category.clone(),
                    definition: rule.description.clone(),
                    source: "skb",
                });
            }
        }
    }

    // ── RMI ontology side: scan concepts via a broad query ──────────
    use rmi::core::ontology::OntologyQuery;
    let q = OntologyQuery::new();
    for concept in ontology.query(&q) {
        let hay = format!("{} {}", concept.label, concept.formal_definition).to_lowercase();
        if hay.contains(&needle) {
            hits.push(UnifiedHit {
                fqn: format!("{}/{}", concept.id.namespace, concept.id.local_name),
                label: concept.label.clone(),
                definition: concept.formal_definition.clone(),
                source: "ontology",
            });
        }
    }

    hits
}

/// Populate an RMI [`Ontology`] from a slice of MAGE rules.
///
/// The ontology is created with namespace `air.skb` and each rule is added
/// under its database-specific sub-namespace. Returns the populated ontology
/// and the number of concepts inserted.
pub fn build_ontology(rules: &[Rule]) -> (Ontology, usize) {
    let ontology = Ontology::new("air.skb");
    let mut inserted = 0usize;
    for rule in rules {
        ontology.add_concept(rule_to_concept(rule));
        inserted += 1;
    }
    (ontology, inserted)
}

/// The RMI edge a predicate is, when it has one with the same meaning.
pub fn edge_type(predicate: &str) -> Option<RelationType> {
    match predicate {
        "subclass_of" => Some(RelationType::IsA),
        "instance_of" => Some(RelationType::InstanceOf),
        "lowers_to" | "lowers_as" => Some(RelationType::TransformsTo),
        "derives" => Some(RelationType::HasComponent),
        "op_family" | "in_profile" => Some(RelationType::PartOf),
        _ => None,
    }
}

/// Resolves a fact's arguments to concepts, using the schema's `arg_class` facts.
struct Names {
    /// Each instance's classes, sorted.
    classes: HashMap<String, Vec<String>>,
    /// (class, ancestor) pairs.
    below: HashSet<(String, String)>,
    /// (relation, position) to class.
    arg_class: HashMap<(String, usize), String>,
    /// The concepts of the safety rules, which keep their `air.skb` names.
    rules: HashMap<String, Concept>,
}

impl Names {
    fn read(kb: &Kb) -> Self {
        let mut classes: HashMap<String, Vec<String>> = HashMap::new();
        for f in kb.facts("instance_of") {
            classes.entry(f[0].clone()).or_default().push(f[1].clone());
        }
        classes.values_mut().for_each(|c| c.sort());
        Names {
            classes,
            below: kb.facts("below").into_iter().map(|f| (f[0].clone(), f[1].clone())).collect(),
            arg_class: kb
                .facts("arg_class")
                .into_iter()
                .map(|f| ((f[0].clone(), f[1].parse().unwrap_or(0)), f[2].clone()))
                .collect(),
            rules: RuleDatabase::ALL
                .into_iter()
                .flat_map(|db| crate::skb::query_rules_by_db(db).matches)
                .map(|r| (r.id.clone(), rule_to_concept(&r)))
                .collect(),
        }
    }

    fn is_below(&self, class: &str, ancestor: &str) -> bool {
        class == ancestor || self.below.contains(&(class.to_string(), ancestor.to_string()))
    }

    /// The classes of `name` that fit `class`.
    fn fits(&self, name: &str, class: &str) -> Vec<&String> {
        self.classes.get(name).map_or(Vec::new(), |cs| cs.iter().filter(|c| self.is_below(c, class)).collect())
    }

    /// An instance of `class`, new to the ontology.
    fn instance(&self, name: &str, class: &str) -> Concept {
        if class == "SafetyRule" {
            if let Some(c) = self.rules.get(name) {
                return c.clone();
            }
        }
        let kind = if self.is_below(class, "Rule") {
            ConceptType::Constraint
        } else if self.is_below(class, "Component") {
            ConceptType::Process
        } else {
            ConceptType::Entity
        };
        Concept::new(ConceptId::new(&format!("air.kb.{class}"), name), kind).with_label(name)
    }

    /// The concept argument `pos` of a `predicate` fact names, if not free text.
    fn argument(&self, predicate: &str, pos: usize, name: &str) -> Option<Concept> {
        let class = self.arg_class.get(&(predicate.to_string(), pos))?;
        // `derives` is read off a production's `rhs`, which names no `AstKind`.
        let fits = self.fits(name, class);
        fits.iter().find(|c| predicate != "derives" || c.as_str() != "AstKind").map(|c| self.instance(name, c))
    }

    /// The concept a fact is about.
    fn subject(&self, predicate: &str, row: &[String]) -> Concept {
        let plain = |ns: &str, kind| Concept::new(ConceptId::new(ns, &row[0]), kind).with_label(&row[0]);
        match predicate {
            "schema_version" => Concept::new(ConceptId::new("air.kb", "schema"), ConceptType::Schema).with_label("schema"),
            "class" | "subclass_of" => plain("air.kb.class", ConceptType::Schema),
            "relation" | "arg_class" => plain("air.kb.relation", ConceptType::Relation),
            "invariant" => plain("air.kb.invariant", ConceptType::Axiom),
            "instance_of" => self.instance(&row[0], &row[1]),
            _ => self
                .argument(predicate, 1, &row[0])
                .unwrap_or_else(|| plain(&format!("air.kb.fact.{predicate}"), ConceptType::Entity)),
        }
    }

    /// The concept an edge from a `predicate` fact goes to.
    fn object(&self, predicate: &str, row: &[String]) -> Option<ConceptId> {
        match predicate {
            "subclass_of" | "instance_of" => Some(ConceptId::new("air.kb.class", &row[1])),
            _ => self.argument(predicate, 2, &row[1]).map(|c| c.id),
        }
    }

    /// The text of a `predicate` fact, for the concept's definition.
    fn text(&self, predicate: &str, row: &[String]) -> Vec<String> {
        let free = |pos: usize| self.arg_class.get(&(predicate.to_string(), pos)).is_some_and(|c| c == "Text");
        row.iter().enumerate().filter(|(i, _)| free(i + 1)).map(|(_, a)| a.clone()).collect()
    }
}

/// Every predicate that holds facts: the schema's own, and each `relation`.
fn predicates(kb: &Kb) -> Vec<String> {
    let mut all: Vec<String> = META.iter().map(|m| m.to_string()).collect();
    all.extend(kb.facts("relation").into_iter().map(|r| r[0].clone()));
    all
}

/// Export every fact of `kb` as an RMI [`Ontology`] in namespace `air.kb`.
pub fn kb_to_ontology(kb: &Kb) -> Ontology {
    let names = Names::read(kb);
    let mut concepts: BTreeMap<String, Concept> = BTreeMap::new();
    let mut rows: BTreeMap<String, BTreeMap<String, Vec<AttributeValue>>> = BTreeMap::new();
    let mut text: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut edges = Vec::new();

    for predicate in predicates(kb) {
        let mut facts = kb.facts(&predicate);
        facts.sort();
        for row in facts {
            let subject = names.subject(&predicate, &row);
            let uri = subject.id.uri();
            if let Some(kind) = edge_type(&predicate) {
                if let Some(object) = names.object(&predicate, &row) {
                    edges.push(Relation::new(subject.id.clone(), kind, object));
                }
            }
            text.entry(uri.clone()).or_default().extend(names.text(&predicate, &row));
            let cells = row.into_iter().map(AttributeValue::String).collect();
            rows.entry(uri.clone()).or_default().entry(predicate.clone()).or_default().push(AttributeValue::List(cells));
            concepts.entry(uri).or_insert(subject);
        }
    }

    let ontology = Ontology::new("air.kb");
    for (uri, mut concept) in concepts {
        if concept.formal_definition.is_empty() {
            concept.formal_definition = text.remove(&uri).unwrap_or_default().join("; ");
        }
        let facts = rows.remove(&uri).unwrap_or_default();
        let facts = facts.into_iter().map(|(p, r)| (p, AttributeValue::List(r))).collect();
        let embedding = embed(&concept.id.local_name, &concept.formal_definition);
        let concept = concept.with_attribute("facts", AttributeValue::Map(facts)).with_embedding(embedding, EMBEDDING_MODEL);
        ontology.add_concept(concept);
    }
    edges.into_iter().for_each(|e| ontology.add_relation(e));
    ontology
}

/// The model an exported concept's embedding names.
pub const EMBEDDING_MODEL: &str = "mage-kb-trigram-hash-v1";

/// The length of an embedding.
pub const EMBEDDING_DIM: usize = 256;

/// The embedding of a concept named `name` with text `text`: the character
/// trigrams of the name's words, and at half weight the words of the text, each
/// hashed to one of [`EMBEDDING_DIM`] buckets. It is not learned: two concepts
/// are near when they are spelled alike or described in the same words. A
/// misspelling after a first word that is a concept itself finds that concept:
/// `agent_feilds` is nearer `agent` than `agent_fields`.
pub fn embed(name: &str, text: &str) -> Array1<f32> {
    fn bucket(feature: &str) -> usize {
        let h = feature.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
        (h % EMBEDDING_DIM as u64) as usize
    }
    fn words(s: &str) -> Vec<String> {
        let lower = s.to_lowercase();
        let w: Vec<String> = lower.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect();
        // A name of symbols alone, such as `!`, is its own word.
        if w.is_empty() && !lower.trim().is_empty() {
            vec![lower.trim().to_string()]
        } else {
            w
        }
    }
    let mut v = Array1::<f32>::zeros(EMBEDDING_DIM);
    for w in words(name) {
        let padded: Vec<char> = format!(" {w} ").chars().collect();
        for t in padded.windows(3) {
            v[bucket(&t.iter().collect::<String>())] += 1.0;
        }
    }
    for w in words(text) {
        v[bucket(&w)] += 0.5;
    }
    v
}

/// A rule that constrains a construct.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ConstrainingRule {
    pub rule: String,
    /// Its classes, each a kind of `Rule`.
    pub classes: Vec<String>,
    /// What it says, from its row.
    pub says: String,
    /// The components that check it.
    pub enforced_by: Vec<String>,
    /// Where it is known not to hold: its `gap` facts.
    pub gaps: Vec<String>,
}

/// What [`constrains`] finds.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Constrains {
    pub construct: String,
    /// Its classes; empty when the knowledge base names no such construct.
    pub classes: Vec<String>,
    pub rules: Vec<ConstrainingRule>,
    /// When it names no construct, the constructs whose embeddings are
    /// nearest the name's, with their cosine similarity.
    pub nearest: Vec<(String, f64)>,
}

/// The rules that constrain `construct`, by the knowledge base's
/// `constrained_by` facts. A name that is no construct gets the nearest names
/// that are, by [`embed`].
pub fn constrains(kb: &mut Kb, construct: &str) -> Constrains {
    let classes_of = |kb: &mut Kb, name: &str, class: &str| -> Vec<String> {
        let mut cs: Vec<String> = kb
            .query("instance_of", &[name, "?"])
            .into_iter()
            .map(|f| f[1].clone())
            .filter(|c| c == class || !kb.query("below", &[c, class]).is_empty())
            .collect();
        cs.sort();
        cs
    };
    let column = |kb: &mut Kb, predicate: &str, name: &str, pos: usize| -> Vec<String> {
        let mut out: Vec<String> = kb.query(predicate, &[name, "?", "?"][..arity(kb, predicate)]).into_iter().map(|f| f[pos].clone()).collect();
        out.sort();
        out.dedup();
        out
    };
    let classes = classes_of(kb, construct, "Construct");
    if classes.is_empty() {
        let ontology = kb_to_ontology(kb);
        let mut nearest = Vec::new();
        for (c, sim) in ontology.find_similar(&embed(construct, ""), usize::MAX, 0.0) {
            let name = c.id.local_name.clone();
            if nearest.len() < 5 && !nearest.iter().any(|(n, _)| n == &name) && !classes_of(kb, &name, "Construct").is_empty() {
                nearest.push((name, sim));
            }
        }
        return Constrains { construct: construct.to_string(), classes, rules: Vec::new(), nearest };
    }
    let names = Names::read(kb);
    let rules = column(kb, "constrained_by", construct, 1)
        .into_iter()
        .map(|rule| {
            let says = ROW_PREDICATES
                .iter()
                .flat_map(|p| kb.facts(p).into_iter().filter(|f| f[0] == rule).flat_map(|f| names.text(p, &f)).collect::<Vec<_>>())
                .collect::<Vec<_>>()
                .join("; ");
            let mut enforced_by = column(kb, "enforced_by", &rule, 1);
            enforced_by.extend(column(kb, "clause_checked", &rule, 2));
            enforced_by.sort();
            enforced_by.dedup();
            ConstrainingRule {
                classes: classes_of(kb, &rule, "Rule"),
                says,
                enforced_by,
                gaps: column(kb, "gap", &rule, 1),
                rule,
            }
        })
        .collect();
    Constrains { construct: construct.to_string(), classes, rules, nearest: Vec::new() }
}

/// The predicates whose row says what a rule is.
const ROW_PREDICATES: &[&str] = &["constraint_row", "contract_row", "gate_row", "diagnostic_row", "safety_rule"];

/// How many arguments `predicate` takes, by the schema.
fn arity(kb: &Kb, predicate: &str) -> usize {
    kb.facts("arg_class").iter().filter(|f| f[0] == predicate).count().max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skb::{query_rules_by_db, RuleDatabase};
    use rmi::core::ontology::OntologyQuery;

    /// The facts an exported ontology holds, by predicate, sorted.
    fn exported_facts(ont: &Ontology) -> BTreeMap<String, Vec<Vec<String>>> {
        let mut out: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
        for c in ont.query(&OntologyQuery::new()) {
            let Some(AttributeValue::Map(facts)) = c.attributes.get("facts") else {
                panic!("{} holds no facts", c.id);
            };
            for (p, rows) in facts {
                let AttributeValue::List(rows) = rows else { panic!("{p} is not a list") };
                for row in rows {
                    let AttributeValue::List(cells) = row else { panic!("{p} has a bad row") };
                    let cells = cells.iter().map(|v| match v {
                        AttributeValue::String(s) => s.clone(),
                        other => panic!("{other:?} is no constant"),
                    });
                    out.entry(p.clone()).or_default().push(cells.collect());
                }
            }
        }
        out.values_mut().for_each(|r| r.sort());
        out
    }

    /// Every concept carries an embedding, and a name spelled a little wrong
    /// finds the concept spelled right.
    #[test]
    fn every_concept_is_embedded_and_a_near_name_finds_it() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let ont = kb_to_ontology(&kb);
        for c in ont.query(&OntologyQuery::new()) {
            assert_eq!(c.embedding_model.as_deref(), Some(EMBEDDING_MODEL), "{}", c.id);
            assert_eq!(c.embedding.as_ref().map(|e| e.len()), Some(EMBEDDING_DIM), "{}", c.id);
        }
        for (typo, want) in [("keyword_bindng", "keyword_binding"), ("max_dept", "max_depth"), ("agnet_fields", "agent_fields")] {
            let top = ont.find_similar(&embed(typo, ""), 1, 0.0);
            assert_eq!(top.first().map(|(c, _)| c.id.local_name.as_str()), Some(want), "{typo}");
        }
    }

    /// Each contract constrains the items its clauses can stand on, by
    /// `clause_checked`, and its own token, by `contract_row`; nothing else.
    #[test]
    fn a_contract_constrains_where_its_clause_stands() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let item = |place: &str| match place {
            "function" => "Function",
            "spec" | "orphan_spec" => "Spec",
            "struct" => "Struct",
            other => panic!("no item for the place {other}"),
        };
        for row in kb.facts("contract_row") {
            let mut want: Vec<String> =
                kb.facts("clause_checked").iter().filter(|f| f[0] == row[0]).map(|f| item(&f[1]).to_string()).collect();
            want.push(row[1].clone());
            want.sort();
            want.dedup();
            let mut got: Vec<String> = kb.facts("constrained_by").into_iter().filter(|f| f[1] == row[0]).map(|f| f[0].clone()).collect();
            got.sort();
            assert_eq!(got, want, "{}", row[0]);
        }
    }

    /// A grammar constraint or contract constrains some construct exactly when
    /// something enforces it: one that is only a `gap` refuses nothing.
    #[test]
    fn a_constraint_constrains_something_when_something_enforces_it() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let constraining: HashSet<String> = kb.facts("constrained_by").into_iter().map(|f| f[1].clone()).collect();
        let enforced: HashSet<String> = kb.facts("enforced").into_iter().map(|f| f[0].clone()).collect();
        let checked: HashSet<String> = kb.facts("enforced_by").into_iter().chain(kb.facts("clause_checked")).map(|f| f[0].clone()).collect();
        let mut gaps_only = Vec::new();
        for f in kb.facts("instance_of").into_iter().filter(|f| f[1] == "GrammarConstraint" || f[1] == "Contract") {
            assert!(enforced.contains(&f[0]), "{} is neither checked nor a gap", f[0]);
            assert_eq!(constraining.contains(&f[0]), checked.contains(&f[0]), "{}", f[0]);
            if !checked.contains(&f[0]) {
                gaps_only.push(f[0].clone());
            }
        }
        gaps_only.sort();
        assert_eq!(gaps_only, ["clause_on_other_item", "layout_blocks", "overflowing_dimension"]);
        let constructs: HashSet<String> = kb.facts("is_a").into_iter().filter(|i| i[1] == "Construct").map(|i| i[0].clone()).collect();
        for f in kb.facts("constrained_by") {
            assert!(constructs.contains(&f[0]), "{f:?} constrains no construct");
        }
    }

    #[test]
    fn constrains_answers_for_a_construct_and_suggests_for_a_near_name() {
        let mut kb = crate::kb::load().expect("the knowledge base loads");
        let names = |c: &Constrains| c.rules.iter().map(|r| r.rule.clone()).collect::<Vec<_>>();

        let f = constrains(&mut kb, "Function");
        assert_eq!(names(&f), ["ensures", "invariant", "requires"]);
        let inv = f.rules.iter().find(|r| r.rule == "invariant").unwrap();
        assert_eq!(inv.classes, ["Contract"]);
        assert!(inv.says.contains("Must always hold"), "{}", inv.says);
        assert!(inv.enforced_by.contains(&"verify".to_string()) && !inv.gaps.is_empty(), "{inv:?}");

        assert!(names(&constrains(&mut kb, "fn")).contains(&"keyword_binding".to_string()));
        assert_eq!(names(&constrains(&mut kb, "@inv")), ["invariant"]);

        let leaf = kb.facts("leaf_form")[0][0].clone();
        assert!(constrains(&mut kb, &leaf).rules.is_empty(), "{leaf}");
        let nested = kb.facts("form_row").into_iter().map(|f| f[0].clone()).find(|f| !kb.facts("leaf_form").iter().any(|l| &l[0] == f)).unwrap();
        let depth = constrains(&mut kb, &nested);
        assert_eq!(names(&depth), ["max_depth", "max_nest"], "{nested}");
        assert_eq!(depth.rules[0].enforced_by, ["arena_grammar"]);

        let typo = constrains(&mut kb, "Functon");
        assert!(typo.classes.is_empty() && typo.rules.is_empty());
        assert_eq!(typo.nearest.first().map(|(n, _)| n.as_str()), Some("Function"), "{:?}", typo.nearest);
    }

    #[test]
    fn every_fact_survives_the_export() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let got = exported_facts(&kb_to_ontology(&kb));
        let mut total = 0;
        for p in predicates(&kb) {
            let mut want = kb.facts(&p);
            want.sort();
            total += want.len();
            assert_eq!(got.get(&p).cloned().unwrap_or_default(), want, "{p}");
        }
        assert_eq!(got.values().map(Vec::len).sum::<usize>(), total, "a fact from nowhere");
        assert!(total > 1000, "only {total} facts");
    }

    #[test]
    fn a_name_is_one_thing_to_each_relation_but_these() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let names = Names::read(&kb);
        let mut clashes = Vec::new();
        for p in predicates(&kb).into_iter().filter(|p| !META.contains(&p.as_str())) {
            for row in kb.facts(&p) {
                for (i, a) in row.iter().enumerate() {
                    let Some(class) = names.arg_class.get(&(p.clone(), i + 1)) else { continue };
                    if class != "Text" && names.fits(a, class).len() != 1 {
                        clashes.push(format!("{p} {a}"));
                    }
                }
            }
        }
        clashes.sort();
        // `Module` and `TypeAlias` are both an `AstKind` and a `Production`, and
        // `mod` and `var` are each a `Keyword` and a `ProfileForm`. Sort order
        // gives each fact the first: so the arena's limits on the forms `mod` and
        // `var` land on the keywords' concepts, beside `keyword_binding`.
        let clashes: Vec<&str> = clashes.iter().map(String::as_str).collect();
        let constrained = |n| std::iter::repeat_n(n, 3);
        let want: Vec<&str> = constrained("constrained_by mod")
            .chain(constrained("constrained_by var"))
            .chain(["derives TypeAlias", "summary Module", "summary TypeAlias", "summary var"])
            .collect();
        assert_eq!(clashes, want);
    }

    #[test]
    fn the_export_keeps_the_rule_names_and_draws_the_edges() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let ont = kb_to_ontology(&kb);

        let rule = &query_rules_by_db(RuleDatabase::Ownership).matches[0];
        let id = rule_to_concept(rule).id;
        assert!(ont.get(&id).is_some_and(|c| c.attributes.contains_key("facts") && c.attributes.contains_key("tags")));

        let keyword = ConceptId::new("air.kb.class", "Keyword");
        let parents = ont.get_related(&keyword, RelationType::IsA);
        assert_eq!(parents.iter().map(|c| c.id.local_name.as_str()).collect::<Vec<_>>(), ["Token"]);

        let (parent, _) = kb.facts("derives").into_iter().map(|f| (f[0].clone(), f[1].clone())).find(|(_, c)| c == "TypeAlias").unwrap();
        let derived = ont.get_related(&ConceptId::new("air.kb.Production", &parent), RelationType::HasComponent);
        assert!(derived.iter().any(|c| c.id.uri() == "air://air.kb.Production/TypeAlias"), "{derived:?}");
        let summary = ont.get(&ConceptId::new("air.kb.AstKind", "TypeAlias")).unwrap();
        assert!(summary.attributes.contains_key("facts") && !summary.formal_definition.is_empty());

        let hits = unified_search("the task's contracts hold", &ont);
        assert!(hits.iter().any(|h| h.fqn == "air.kb.GateRule/gate_contract"), "{hits:?}");
    }

    #[test]
    fn ownership_rules_land_in_ownership_namespace() {
        let rules = query_rules_by_db(RuleDatabase::Ownership);
        let (ont, count) = build_ontology(&rules.matches);
        assert!(count > 0);
        assert_eq!(ont.len(), count);
        assert_eq!(ont.namespace, "air.skb");
    }

    #[test]
    fn unified_search_finds_hits_in_both_sides() {
        // Build an ontology with concepts mentioning "attention".
        let rules = query_rules_by_db(RuleDatabase::Ownership);
        let (ont, _) = build_ontology(&rules.matches);
        // Add a synthetic concept to the ontology so we can verify the
        // ontology side of unified_search returns it.
        use rmi::core::ontology::{Concept, ConceptId, ConceptType};
        let synthetic = Concept::new(
            ConceptId::new("air.neural", "Attention"),
            ConceptType::Process,
        )
        .with_label("Attention")
        .with_definition("Multi-head attention layer for sequence modeling.");
        ont.add_concept(synthetic);

        let hits = unified_search("attention", &ont);
        // Should find at least one ontology hit (the synthetic concept).
        let ontology_hits: Vec<_> = hits.iter().filter(|h| h.source == "ontology").collect();
        assert!(
            !ontology_hits.is_empty(),
            "expected ontology hit for 'attention', got: {:?}",
            hits
        );

        // Searching for a word that almost certainly appears in some SKB rule
        // description should yield at least one SKB hit.
        let mem_hits = unified_search("borrow", &ont);
        let skb_hits: Vec<_> = mem_hits.iter().filter(|h| h.source == "skb").collect();
        assert!(
            !skb_hits.is_empty(),
            "expected SKB hits for 'borrow'"
        );
    }

    #[test]
    fn rule_round_trips_into_concept() {
        let rules = query_rules_by_db(RuleDatabase::Ownership);
        let first = &rules.matches[0];
        let concept = rule_to_concept(first);
        assert_eq!(concept.id.namespace, "air.skb.ownership");
        assert_eq!(concept.id.local_name, first.id);
        assert_eq!(concept.label, first.category);
    }
}

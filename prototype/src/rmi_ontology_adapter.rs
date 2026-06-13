//! # MAGE SKB ↔ RMI Ontology Adapter
//!
//! Translates between MAGE's [`crate::skb`] (9,157+ safety rules organized
//! by [`RuleDatabase`]) and RMI's [`rmi::core::ontology::Ontology`] (a
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
//! [`Rule`]: crate::skb::Rule
//! [`RuleDatabase`]: crate::skb::RuleDatabase
//! [`Concept`]: rmi::core::ontology::Concept

use crate::skb::{Rule, RuleDatabase, RuleSeverity};
use rmi::core::ontology::{
    AttributeValue, Concept, ConceptId, ConceptType, Ontology,
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
    let dbs = [
        RuleDatabase::Ownership,
        RuleDatabase::Borrow,
        RuleDatabase::Lifetime,
        RuleDatabase::TypeSafety,
        RuleDatabase::Concurrency,
        RuleDatabase::FFI,
        RuleDatabase::AgentElision,
        RuleDatabase::SwarmSafety,
    ];
    for db in dbs {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skb::{query_rules_by_db, RuleDatabase};

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

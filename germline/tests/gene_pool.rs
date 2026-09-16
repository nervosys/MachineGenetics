//! The registry as a gene pool: `forge` on one side, `germline` on the other.
//!
//! Genes reach a genome by content hash and reach a phenotype through a
//! [`GeneResolver`]. Both seams existed and neither was connected to a real
//! registry — every genome in practice was still a vector of scalars, which
//! made `Locus::Gene` a construct with no caller.
//!
//! **The connection lives here rather than in either library, on purpose.**
//! `germline` holds heritable material and must not know what a registry is;
//! `forge` stores blocks and must not know what a genome is. A crate that
//! depended on both would couple them permanently to make one adapter. Cargo
//! dev-dependencies do not propagate to consumers, so this test can use both
//! while neither library gains an edge — the adapter is demonstrated, and the
//! libraries stay independent.
//!
//! What this establishes end to end:
//!
//!   forge registry -> GenePool -> crossover/substitution -> legality check
//!                  -> express -> the source that gets built
//!
//! The legality check is the part that was vacuous until blocks carried their
//! effects. A registry that records nothing hands out genes with an empty
//! effect list, an empty list reads as a purity claim, and every candidate
//! passes because there is nothing to compare. Both crates now distinguish
//! `Checked` from `Unchecked`, and the tests below cover all three outcomes:
//! inherited, acquired, and unknowable.

use forge::models::Effects as BlockEffects;
use forge::registry::blocks::{BlockStore, EffectOracle};
use germline::variation::{
    legality, mutate, propose, Effects, GenePool, Genome, Legality, Locus, Mutation, VariationPlan,
};
use germline::workload::{express, GeneResolver};
use std::path::PathBuf;

/// A registry-backed resolver: the other half of what `GenePool` supplies.
struct Registry(BlockStore);

impl GeneResolver for Registry {
    fn resolve(&self, sha256: &str) -> Option<String> {
        self.0.get_by_sha(sha256)
    }
}

/// Every published block, as loci a genome can carry.
///
/// **The two `Effects` types meet here and nowhere else.** `forge` records what
/// is known about a block; `germline` records what is known about a gene; they
/// are the same distinction reached independently, and neither crate imports
/// the other's. This function is the whole translation, and it is a match of
/// two arms so that adding a third state to either side breaks here rather than
/// being silently flattened.
fn pool_from(store: &BlockStore) -> GenePool {
    GenePool::new(
        store
            .list()
            .into_iter()
            .map(|h| {
                let effects = match h.effects {
                    BlockEffects::Unchecked => Effects::Unchecked,
                    BlockEffects::Checked { declared } => Effects::checked(declared),
                };
                Locus::gene(h.name, h.sha256, h.signature, effects)
            })
            .collect(),
    )
}

/// A stand-in for the compiler, which lives in another workspace.
///
/// `mage-parse --check` reports per-function effects today; this is the shape
/// of the answer, not a reimplementation of it. It reads a marker comment so a
/// test can publish a block that genuinely declares an effect.
struct Oracle;

impl EffectOracle for Oracle {
    fn effects_of(&self, src: &str) -> Option<Vec<String>> {
        Some(
            src.lines()
                .filter_map(|l| l.trim().strip_prefix("// effect: "))
                .map(|e| e.trim().to_string())
                .collect(),
        )
    }
}

fn tmp(tag: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!(
        "germline-genepool-{tag}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn a_published_block_becomes_a_gene_a_genome_can_carry_and_express() {
    let root = tmp("published");
    let store = BlockStore::new(&root);
    store
        .publish_source("block Attn(d) {\n    layer a: Linear(d, d);\n}\n")
        .expect("published");

    let pool = pool_from(&store);
    assert_eq!(pool.genes.len(), 1, "the registry supplies the pool");
    // Published without an oracle, so the gene arrives unchecked — which bars
    // it from *proposal*, not from expression. Expression is what a genome
    // means; legality is whether it may be run.
    let Locus::Gene { effects, .. } = &pool.genes[0] else { panic!("a gene") };
    assert_eq!(effects, &Effects::Unchecked, "no oracle ran, so nothing is claimed");
    assert_eq!(effects.declared(), None, "and there is no list to mistake for empty");

    // A genome that carries the published gene, plus the scalars the net needs.
    let genome: Genome = pool
        .genes
        .iter()
        .cloned()
        .chain([Locus::param(0.5), Locus::param(0.5), Locus::param(1.0)])
        .collect();

    let src = express(&genome, &Registry(BlockStore::new(&root))).expect("resolvable");
    assert!(src.contains("block Attn(d)"), "the published block is in the phenotype: {src}");
    assert!(src.contains("net Evolved"), "and so is the net: {src}");
}

#[test]
fn substitution_draws_from_the_registry_and_respects_signatures() {
    let root = tmp("substitute");
    let store = BlockStore::new(&root);
    // Two blocks of the same shape, one of another.
    store.publish_source("block Sort(xs) {\n    layer a: A;\n}\n").expect("published");
    store.publish_source("block Sort(xs) {\n    layer b: B;\n}\n").expect("published");
    store.publish_source("block Map(k) {\n    layer c: C;\n}\n").expect("published");

    let pool = pool_from(&store);
    assert_eq!(pool.genes.len(), 3, "three distinct bodies, three genes");

    let start: Genome = vec![pool
        .genes
        .iter()
        .find(|g| matches!(g, Locus::Gene { signature, .. } if signature == "Sort(xs)"))
        .expect("a Sort gene")
        .clone()];

    let mut rng = germline::variation::Rng::seed(0xABCD);
    for _ in 0..50 {
        let m = mutate(&start, Mutation::Substitute { rate: 1.0 }, &pool, &mut rng);
        match &m[0] {
            Locus::Gene { signature, .. } => {
                assert_eq!(signature, "Sort(xs)", "never substitutes across signatures");
            }
            other => panic!("a gene became {other:?}"),
        }
        // And whatever it chose is resolvable: a substitution that produced an
        // unexpressible genome would be worse than no substitution at all.
        express(&m, &Registry(BlockStore::new(&root))).expect("substituted gene resolves");
    }
}

#[test]
fn a_proposal_round_over_checked_registry_genes_stays_within_declared_effects() {
    let root = tmp("effects");
    let store = BlockStore::new(&root);
    store
        .publish_source_with("block Step(x) {\n    layer a: A;\n}\n", &Oracle)
        .expect("published");
    store
        .publish_source_with("block Step(x) {\n    layer b: B;\n}\n", &Oracle)
        .expect("published");

    let pool = pool_from(&store);
    for g in &pool.genes {
        let Locus::Gene { effects, name, .. } = g else { panic!("a gene") };
        assert_eq!(effects, &Effects::pure(), "{name} was checked and found pure");
    }

    let parents: Vec<(Genome, f64)> = pool
        .genes
        .iter()
        .map(|g| (vec![g.clone(), Locus::param(0.5)], 0.5))
        .collect();

    let plan = VariationPlan {
        mutation: Mutation::Substitute { rate: 1.0 },
        offspring: 6,
        ..VariationPlan::default()
    };
    let out = propose(&parents, plan, &pool, 0x9E11);

    // Every gene here was *checked* and found pure, so nothing can acquire an
    // effect and every candidate survives. This assertion was previously true
    // for the opposite reason — the registry recorded no effects at all, so the
    // comparison had nothing to read and could not have failed.
    assert!(out.refused.is_empty(), "nothing to acquire: {:?}", out.refused);
    assert_eq!(out.candidates.len(), 6);
    for c in &out.candidates {
        let parent_refs: Vec<&Genome> = parents.iter().map(|(g, _)| g).collect();
        assert_eq!(legality(&parent_refs, &c.genome), Legality::Inherited);
        express(&c.genome, &Registry(BlockStore::new(&root))).expect("every candidate expresses");
    }
}

#[test]
fn a_registry_gene_that_declares_an_effect_is_refused_into_pure_parents() {
    // The point of the whole chain, and what it could not do before blocks
    // carried effects: a block that writes files, substituted into a lineage
    // whose parents were checked pure, is caught at propose time — before the
    // candidate costs a build, a run or a sandbox.
    let root = tmp("escalation");
    let store = BlockStore::new(&root);
    store
        .publish_source_with("block Step(x) {\n    layer a: A;\n}\n", &Oracle)
        .expect("published");
    store
        .publish_source_with(
            "block Step(x) {\n    // effect: io\n    layer w: WriteFile;\n}\n",
            &Oracle,
        )
        .expect("published");

    let pool = pool_from(&store);
    let pure: Vec<Locus> = pool
        .genes
        .iter()
        .filter(|g| matches!(g, Locus::Gene { effects, .. } if effects == &Effects::pure()))
        .cloned()
        .collect();
    let does_io: Vec<Locus> = pool
        .genes
        .iter()
        .filter(|g| matches!(g, Locus::Gene { effects, .. }
                             if effects.declared() == Some(&["io".to_string()][..])))
        .cloned()
        .collect();
    assert_eq!(pure.len(), 1, "one pure block");
    assert_eq!(does_io.len(), 1, "and one the oracle found does io");

    // Substitute from a pool holding *only* the io block, so the outcome does
    // not depend on which gene an rng happened to draw. A seeded run that
    // passes because the escalating gene was never selected is a test of the
    // seed.
    let parent: Genome = vec![pure[0].clone(), Locus::param(0.5)];
    let plan = VariationPlan {
        mutation: Mutation::Substitute { rate: 1.0 },
        offspring: 4,
        ..VariationPlan::default()
    };
    let out = propose(
        &[(parent.clone(), 0.5), (parent, 0.6)],
        plan,
        &GenePool::new(does_io),
        0x10FF,
    );

    assert!(
        out.candidates.is_empty(),
        "a block that does io may not enter a pure lineage: {:?}",
        out.candidates
    );
    assert_eq!(out.refused.len(), 4, "and every refusal is reported");
    for r in &out.refused {
        assert_eq!(
            r.because,
            Legality::Acquired { effects: vec!["io".to_string()] },
            "the refusal names the capability, not just the candidate"
        );
    }

    // The same block into a parent that already does io is inheritance, not
    // acquisition — the check bounds escalation, it does not ban effects.
    let io_parent: Genome = vec![pool
        .genes
        .iter()
        .find(|g| matches!(g, Locus::Gene { effects, .. }
                           if effects.declared() == Some(&["io".to_string()][..])))
        .expect("the io gene")
        .clone()];
    assert_eq!(
        legality(&[&io_parent], &io_parent.clone()),
        Legality::Inherited
    );
}

#[test]
fn an_unchecked_registry_gene_is_refused_rather_than_read_as_pure() {
    // The failure mode this session found: a registry that records no effects
    // hands out genes whose effect list is empty, and an empty list reads as a
    // purity claim. The check then passes on every candidate — not because
    // nothing escalated, but because there was nothing to compare.
    let root = tmp("vacuous");
    let store = BlockStore::new(&root);
    store.publish_source("block Step(x) {\n    layer a: A;\n}\n").expect("published");
    store.publish_source("block Step(x) {\n    layer b: B;\n}\n").expect("published");

    let pool = pool_from(&store);
    let parents: Vec<(Genome, f64)> = pool
        .genes
        .iter()
        .map(|g| (vec![g.clone(), Locus::param(0.5)], 0.5))
        .collect();
    let plan = VariationPlan {
        mutation: Mutation::Substitute { rate: 1.0 },
        offspring: 4,
        ..VariationPlan::default()
    };
    let out = propose(&parents, plan, &pool, 0x9E11);

    assert!(
        out.candidates.is_empty(),
        "an unchecked gene may not be evaluated on the assumption that it is pure"
    );
    assert_eq!(out.refused.len(), 4);
    for r in &out.refused {
        assert!(
            matches!(&r.because, Legality::Unknowable { genes } if genes == &vec!["Step".to_string()]),
            "the refusal names the block that needs checking: {:?}",
            r.because
        );
    }

    // And publishing the same bytes through an oracle lifts the refusal, so the
    // remedy is checking the block rather than weakening the check.
    store
        .publish_source_with("block Step(x) {\n    layer a: A;\n}\n", &Oracle)
        .expect("checked");
    store
        .publish_source_with("block Step(x) {\n    layer b: B;\n}\n", &Oracle)
        .expect("checked");
    let pool = pool_from(&store);
    let parents: Vec<(Genome, f64)> = pool
        .genes
        .iter()
        .map(|g| (vec![g.clone(), Locus::param(0.5)], 0.5))
        .collect();
    let out = propose(&parents, plan, &pool, 0x9E11);
    assert_eq!(out.candidates.len(), 4, "checked, and now proposable");
    assert!(out.refused.is_empty());
}

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
//!   forge registry -> GenePool -> crossover/substitution -> effect check
//!                  -> express -> the source that gets built

use forge::registry::blocks::BlockStore;
use germline::variation::{
    acquired_effects, mutate, propose, GenePool, Genome, Locus, Mutation, VariationPlan,
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
/// `BlockHandle` is `{name, sha256, signature}` and a gene is that plus the
/// effects it declares. The registry does not record effects today, so this
/// says so rather than inventing them: an empty effect list means the legality
/// check has nothing to go on, which is the honest state until blocks carry
/// their effects.
fn pool_from(store: &BlockStore) -> GenePool {
    GenePool::new(
        store
            .list()
            .into_iter()
            .map(|h| Locus::gene(h.name, h.sha256, h.signature, Vec::new()))
            .collect(),
    )
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
fn a_proposal_round_over_registry_genes_stays_within_declared_effects() {
    let root = tmp("effects");
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
        offspring: 6,
        ..VariationPlan::default()
    };
    let out = propose(&parents, plan, &pool, 0x9E11);

    // No block in this registry declares an effect, so nothing can acquire one
    // and every candidate survives the legality check.
    assert!(out.refused.is_empty(), "nothing to acquire: {:?}", out.refused);
    assert_eq!(out.candidates.len(), 6);
    for c in &out.candidates {
        let parent_refs: Vec<&Genome> = parents.iter().map(|(g, _)| g).collect();
        assert!(acquired_effects(&parent_refs, &c.genome).is_empty());
        express(&c.genome, &Registry(BlockStore::new(&root))).expect("every candidate expresses");
    }
}

//! The forge block store, read as knowledge-base facts. A block published to
//! forge is an `Artifact` of the `Block` class, and its facts load against
//! `kb/schema.mg` like any other file under kb/.

use forge::registry::BlockStore;
use mage_prototype::kb;

const BLOCKS: &str = "block Tiny(d) { layer fc: Linear(d, d); }\nblock Pair(a, b) { layer x: Id(a); layer y: Id(b); }\n";

/// A store in a directory of its own, emptied first.
fn fresh_store(tag: &str) -> BlockStore {
    let dir = std::env::temp_dir().join(format!("arena-forge-facts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    BlockStore::new(dir)
}

/// The embedded knowledge base, with `extra` as one more file.
fn with(extra: &str) -> Result<kb::Kb, Vec<String>> {
    let mut files: Vec<(&str, &str)> = kb::FILES.to_vec();
    files.push(("forge/blocks", extra));
    kb::load_files(&files)
}

#[test]
fn forge_blocks_load_as_artifact_facts() {
    let store = fresh_store("load");
    let handles = store.publish_source(BLOCKS).expect("publishes");
    let mut loaded = with(&store.facts()).unwrap_or_else(|e| panic!("{e:#?}"));
    let instances = loaded.facts("instance_of");
    let names = loaded.facts("block_name");
    let sigs = loaded.facts("block_signature");
    assert_eq!(handles.len(), 2);
    for h in &handles {
        let id = format!("sha256:{}", h.sha256);
        assert!(instances.iter().any(|f| f[0] == id && f[1] == "Block"), "{id} is not a Block");
        let name = names.iter().find(|f| f[0] == id).expect("named");
        assert_eq!(name[1], h.name);
        let sig = sigs.iter().find(|f| f[0] == id).expect("has a signature");
        assert_eq!(sig[1], h.signature);
        assert!(!loaded.query("is_a", &[&id, "Artifact"]).is_empty(), "{id} is not an Artifact");
    }
}

#[test]
fn an_empty_store_states_no_blocks_and_still_loads() {
    let store = fresh_store("empty");
    let facts = store.facts();
    assert!(with(&facts).is_ok(), "{facts}");
    assert!(with(&facts).unwrap().facts("block_name").is_empty());
}

#[test]
fn a_block_fact_the_schema_lacks_is_refused() {
    let store = fresh_store("refused");
    let handles = store.publish_source(BLOCKS).expect("publishes");
    let id = format!("sha256:{}", handles[0].sha256);
    let extra = format!("kb Extra {{\n    fact block_effects(\"{id}\", \"io\");\n}}\n");
    let Err(e) = with(&extra) else { panic!("block_effects is not in the schema") };
    assert!(e.iter().any(|m| m.contains("`block_effects` uses a predicate the schema does not declare")), "{e:#?}");
}

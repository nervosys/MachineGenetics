//! `ARCHITECTURE.md`'s module census names every module in `prototype/src`, once.

use std::collections::BTreeSet;
use std::path::Path;

#[test]
fn the_census_names_every_module_once() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc = std::fs::read_to_string(root.join("../ARCHITECTURE.md")).expect("ARCHITECTURE.md");
    let census = doc.split("### Module census").nth(1).expect("the census heading");
    let census = census.split("\n---").next().expect("the census");

    let mut named = BTreeSet::new();
    for row in census.lines().filter(|l| l.starts_with("| ")) {
        // Between backticks, the odd pieces are the names.
        for name in row.split('`').skip(1).step_by(2) {
            assert!(named.insert(name.to_string()), "`{name}` is in the census twice");
        }
    }

    let mut on_disk = BTreeSet::new();
    for entry in std::fs::read_dir(root.join("src")).expect("src") {
        let entry = entry.expect("an entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_dir() {
            on_disk.insert(format!("{name}/"));
        } else if name.ends_with(".rs") {
            on_disk.insert(name);
        }
    }

    assert_eq!(named, on_disk, "ARCHITECTURE.md's census against prototype/src");
}

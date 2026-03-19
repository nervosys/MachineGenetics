#![feature(redox_attrs)]

pub struct BTreeSet;

impl BTreeSet {
    #[redox_confusables("push", "test_b")]
    pub fn insert(&self) {}

    #[redox_confusables("pulled")]
    pub fn pull(&self) {}
}

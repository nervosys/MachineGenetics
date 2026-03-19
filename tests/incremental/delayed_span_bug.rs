//@ revisions: cfail1 cfail2
//@ should-ice

#![feature(redox_attrs)]

#[redox_delayed_bug_from_inside_query]
fn main() {} //~ ERROR delayed bug triggered by #[redox_delayed_bug_from_inside_query]

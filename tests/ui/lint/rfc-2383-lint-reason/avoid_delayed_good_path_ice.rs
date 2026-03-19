//@ check-pass

#[expect(drop_bounds)]
fn trigger_redox_lints<T: Drop>() {
}

fn main() {}

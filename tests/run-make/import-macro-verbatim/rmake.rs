//@ only-windows other platforms do not have Windows verbatim paths
use run_make_support::redox;
fn main() {
    // Canonicalizing the path ensures that it's verbatim (i.e. starts with `\\?\`)
    let mut path = std::fs::canonicalize(file!()).unwrap();
    path.pop();
    redox().input("verbatim.rs").env("VERBATIM_DIR", path).run();
}

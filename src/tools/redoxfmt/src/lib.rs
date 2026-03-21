//! # redoxfmt — Redox Formatter
//!
//! Two modes:
//! - `--compact`: minimum-token canonical form (all abbreviations applied)
//! - `--expand`: fully-expanded human-readable Rust form
//!
//! Round-trip invariant: `compact(expand(src)) == compact(src)`

mod expand;

/// Produce the minimum-token canonical compact form from Rust or Redox source.
pub fn compact(input: &str) -> String {
    rust2redox::transpile(input)
}

/// Produce fully-expanded human-readable Rust form from Redox compact source.
pub fn expand_source(input: &str) -> String {
    expand::expand(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Compact mode tests ---

    #[test]
    fn compact_pub_fn() {
        assert_eq!(compact("pub fn foo() {}"), "+f foo() {}");
    }

    #[test]
    fn compact_derive() {
        assert_eq!(compact("#[derive(Clone, Debug)]"), "@d(Cl,Db)");
    }

    #[test]
    fn compact_vec_type() {
        assert_eq!(compact("let x: Vec<u8> = vec![];"), "v x: [u8]~ = vec![];");
    }

    // --- Expand mode tests ---

    #[test]
    fn expand_pub_fn() {
        assert_eq!(expand_source("+f foo() {}"), "pub fn foo() {}");
    }

    #[test]
    fn expand_derive() {
        assert_eq!(expand_source("@d(Cl,Db)"), "#[derive(Clone, Debug)]");
    }

    #[test]
    fn expand_vec_type() {
        assert_eq!(expand_source("[u8]~"), "Vec<u8>");
    }

    #[test]
    fn expand_option() {
        assert_eq!(expand_source("v x: ?i32 = None;"), "let x: Option<i32> = None;");
    }

    #[test]
    fn expand_result() {
        assert_eq!(
            expand_source("f f() -> R[T,E] {}"),
            "fn f() -> Result<T, E> {}"
        );
    }

    #[test]
    fn expand_mut_ref() {
        assert_eq!(
            expand_source("f f(x: &!T) {}"),
            "fn f(x: &mut T) {}"
        );
    }

    #[test]
    fn expand_combined() {
        assert_eq!(
            expand_source("+f process(data: &![u8]~) -> R[(),Error] {}"),
            "pub fn process(data: &mut Vec<u8>) -> Result<(), Error> {}"
        );
    }

    #[test]
    fn expand_derive_partialeq() {
        assert_eq!(
            expand_source("@d(Cl,Db,PEq)"),
            "#[derive(Clone, Debug, PartialEq)]"
        );
    }

    #[test]
    fn expand_test_attr() {
        assert_eq!(expand_source("@t"), "#[test]");
    }

    // --- Round-trip tests: compact(expand(src)) == compact(src) ---

    #[test]
    fn roundtrip_simple_fn() {
        let compact_form = "+f foo() {}";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_struct_with_derive() {
        let compact_form = "@d(Cl,Db)\n+S Point { x: f64, y: f64 }";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_result_fn() {
        let compact_form = "+f process(data: &!T) -> R[(),Error] {}";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_let_mut() {
        let compact_form = "m x = 5;";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_let() {
        let compact_form = "v x = 5;";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_enum() {
        let compact_form = "+E Color { Red }";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_const() {
        let compact_form = "C X: i32 = 1;";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_static() {
        let compact_form = "Z Y: i32 = 2;";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_return() {
        let compact_form = "^ x;";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_vec_type() {
        let compact_form = "v x: [u8]~ = vec![];";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_option_type() {
        let compact_form = "v x: ?i32 = None;";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_test_attr() {
        let compact_form = "@t";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_allow_unused() {
        let compact_form = "@a(un)";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_repr_c() {
        let compact_form = "@r(C)";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }

    #[test]
    fn roundtrip_todo() {
        let compact_form = "??";
        assert_eq!(compact(&expand_source(compact_form)), compact_form);
    }
}

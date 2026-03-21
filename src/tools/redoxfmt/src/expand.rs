//! Expand pass: reverse all Redox compact abbreviations back to full Rust form.

mod types;
mod attrs;
mod keywords;

/// Expand a Redox compact-form string to fully-expanded human-readable Rust form.
pub fn expand(input: &str) -> String {
    // Reverse order of compaction: types → attrs → keywords
    let s = types::expand_types(input);
    let s = attrs::expand_attrs(&s);
    keywords::expand_keywords(&s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_simple_fn() {
        assert_eq!(expand("+f foo() {}"), "pub fn foo() {}");
    }

    #[test]
    fn expand_let_mut() {
        assert_eq!(expand("m x = 5;"), "let mut x = 5;");
    }

    #[test]
    fn expand_vec_type() {
        assert_eq!(expand("[u8]~"), "Vec<u8>");
    }

    #[test]
    fn expand_option_type() {
        assert_eq!(expand("?i32"), "Option<i32>");
    }

    #[test]
    fn expand_result_type() {
        assert_eq!(expand("R[T,E]"), "Result<T, E>");
    }

    #[test]
    fn expand_derive_attr() {
        assert_eq!(expand("@d(Cl,Db)"), "#[derive(Clone, Debug)]");
    }

    #[test]
    fn expand_test_attr() {
        assert_eq!(expand("@t"), "#[test]");
    }

    #[test]
    fn expand_box_type() {
        assert_eq!(expand("^Foo"), "Box<Foo>");
    }

    #[test]
    fn expand_arc_type() {
        assert_eq!(expand("@Foo"), "Arc<Foo>");
    }

    #[test]
    fn expand_mut_ref() {
        assert_eq!(expand("&!T"), "&mut T");
    }

    #[test]
    fn expand_str_ref() {
        assert_eq!(expand("&s"), "&str");
    }

    #[test]
    fn expand_combined() {
        assert_eq!(
            expand("+f process(data: &![u8]~) -> R[(),Error] {}"),
            "pub fn process(data: &mut Vec<u8>) -> Result<(), Error> {}"
        );
    }
}

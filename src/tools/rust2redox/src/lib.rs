//! # rust2redox — Rust to Redox Canonical Compact Form Transpiler
//!
//! Converts valid Rust source text into canonical Redox compact form.
//! Handles four transformation passes:
//!
//! 1. **Keyword compression** — `pub fn` → `+f`, `let mut` → `m`, etc.
//! 2. **Attribute compression** — `#[derive(Clone, Debug)]` → `@d(Cl,Db)`
//! 3. **Lifetime/borrow elision** — strip lifetime params, simplify borrows
//! 4. **Type abbreviation** — `Vec<T>` → `[T]~`, `Option<T>` → `?T`, etc.

mod attrs;
mod keywords;
mod lifetimes;
mod types;

/// Apply all transpilation passes to Rust source, producing Redox compact form.
pub fn transpile(input: &str) -> String {
    let s = keywords::compress_keywords(input);
    let s = attrs::compress_attrs(&s);
    let s = lifetimes::elide_lifetimes(&s);
    types::abbreviate_types(&s)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Keyword compression tests (§5.5.1) ---

    #[test]
    fn keyword_pub_fn() {
        assert_eq!(transpile("pub fn foo() {}"), "+f foo() {}");
    }

    #[test]
    fn keyword_pub_crate_fn() {
        assert_eq!(transpile("pub(crate) fn bar() {}"), "~f bar() {}");
    }

    #[test]
    fn keyword_fn_alone() {
        assert_eq!(transpile("fn baz() {}"), "f baz() {}");
    }

    #[test]
    fn keyword_pub_struct() {
        assert_eq!(transpile("pub struct Foo {}"), "+S Foo {}");
    }

    #[test]
    fn keyword_struct_alone() {
        assert_eq!(transpile("struct Bar {}"), "S Bar {}");
    }

    #[test]
    fn keyword_pub_enum() {
        assert_eq!(transpile("pub enum Color { Red }"), "+E Color { Red }");
    }

    #[test]
    fn keyword_enum_alone() {
        assert_eq!(transpile("enum Dir { Up }"), "E Dir { Up }");
    }

    #[test]
    fn keyword_let_mut() {
        assert_eq!(transpile("let mut x = 5;"), "m x = 5;");
    }

    #[test]
    fn keyword_let_alone() {
        assert_eq!(transpile("let x = 5;"), "v x = 5;");
    }

    #[test]
    fn keyword_impl_alone() {
        assert_eq!(transpile("impl Foo {}"), "I Foo {}");
    }

    #[test]
    fn keyword_trait_alone() {
        assert_eq!(transpile("trait Bar {}"), "T Bar {}");
    }

    #[test]
    fn keyword_pub_trait() {
        assert_eq!(transpile("pub trait Baz {}"), "+T Baz {}");
    }

    #[test]
    fn keyword_const() {
        assert_eq!(transpile("const X: i32 = 1;"), "C X: i32 = 1;");
    }

    #[test]
    fn keyword_static() {
        assert_eq!(transpile("static Y: i32 = 2;"), "Z Y: i32 = 2;");
    }

    #[test]
    fn keyword_return() {
        assert_eq!(transpile("return x;"), "^ x;");
    }

    #[test]
    fn keyword_async_fn() {
        assert_eq!(transpile("async fn run() {}"), "af run() {}");
    }

    #[test]
    fn keyword_pub_async_fn() {
        assert_eq!(transpile("pub async fn run() {}"), "+af run() {}");
    }

    #[test]
    fn keyword_unsafe_fn() {
        assert_eq!(transpile("unsafe fn danger() {}"), "uf danger() {}");
    }

    #[test]
    fn keyword_pub_mod() {
        assert_eq!(transpile("pub mod foo {}"), "+M foo {}");
    }

    #[test]
    fn keyword_mod_alone() {
        assert_eq!(transpile("mod bar {}"), "M bar {}");
    }

    #[test]
    fn keyword_type_alias() {
        assert_eq!(transpile("type Alias = i32;"), "Y Alias = i32;");
    }

    // --- Attribute compression tests (§5.5.2) ---

    #[test]
    fn attr_derive_clone_debug() {
        assert_eq!(transpile("#[derive(Clone, Debug)]"), "@d(Cl,Db)");
    }

    #[test]
    fn attr_derive_partialeq() {
        assert_eq!(transpile("#[derive(Clone, Debug, PartialEq)]"), "@d(Cl,Db,PEq)");
    }

    #[test]
    fn attr_test() {
        assert_eq!(transpile("#[test]"), "@t");
    }

    #[test]
    fn attr_cfg_test() {
        assert_eq!(transpile("#[cfg(test)]"), "@cfg(t)");
    }

    #[test]
    fn attr_inline_always() {
        assert_eq!(transpile("#[inline(always)]"), "@i!");
    }

    #[test]
    fn attr_allow_unused() {
        assert_eq!(transpile("#[allow(unused)]"), "@a(un)");
    }

    #[test]
    fn attr_allow_dead_code() {
        assert_eq!(transpile("#[allow(dead_code)]"), "@a(dc)");
    }

    #[test]
    fn attr_repr_c() {
        assert_eq!(transpile("#[repr(C)]"), "@r(C)");
    }

    #[test]
    fn attr_repr_transparent() {
        assert_eq!(transpile("#[repr(transparent)]"), "@r(t)");
    }

    #[test]
    fn attr_must_use() {
        assert_eq!(transpile("#[must_use]"), "@mu");
    }

    #[test]
    fn attr_deny_unsafe_code() {
        assert_eq!(transpile("#[deny(unsafe_code)]"), "@x(uc)");
    }

    // --- Lifetime elision tests ---

    #[test]
    fn elide_single_lifetime_param() {
        assert_eq!(transpile("fn foo<'a>(x: &'a str) -> &'a str {}"), "f foo(x: &s) -> &s {}");
    }

    #[test]
    fn elide_multiple_lifetime_params() {
        assert_eq!(
            transpile("fn bar<'a, 'b>(x: &'a str, y: &'b str) {}"),
            "f bar(x: &s, y: &s) {}"
        );
    }

    #[test]
    fn elide_lifetime_on_mut_ref() {
        assert_eq!(transpile("fn baz<'a>(x: &'a mut Vec<u8>) {}"), "f baz(x: &![u8]~) {}");
    }

    // --- Type abbreviation tests (§5.5.6) ---

    #[test]
    fn type_vec() {
        assert_eq!(transpile("let x: Vec<u8> = vec![];"), "v x: [u8]~ = vec![];");
    }

    #[test]
    fn type_option() {
        assert_eq!(transpile("let x: Option<i32> = None;"), "v x: ?i32 = None;");
    }

    #[test]
    fn type_result() {
        assert_eq!(transpile("fn f() -> Result<T, E> {}"), "f f() -> R[T,E] {}");
    }

    #[test]
    fn type_box() {
        assert_eq!(transpile("let x: Box<Foo> = todo!();"), "v x: ^Foo = ??;");
    }

    #[test]
    fn type_arc() {
        assert_eq!(transpile("let x: Arc<Foo> = todo!();"), "v x: @Foo = ??;");
    }

    #[test]
    fn type_hashmap() {
        assert_eq!(
            transpile("let m: HashMap<String, i32> = HashMap::new();"),
            "v m: {s\"\",i32} = HashMap::new();"
        );
    }

    #[test]
    fn type_string_to_str() {
        // &str stays as &s, String becomes s""
        assert_eq!(transpile("let x: &str = \"\";"), "v x: &s = \"\";");
    }

    #[test]
    fn type_mut_ref_shorthand() {
        assert_eq!(transpile("fn f(x: &mut T) {}"), "f f(x: &!T) {}");
    }

    // --- Combined / integration tests ---

    #[test]
    fn full_function_transpile() {
        let input = "pub fn longest<'a>(x: &'a str, y: &'a str) -> &'a str { x }";
        let output = transpile(input);
        assert_eq!(output, "+f longest(x: &s, y: &s) -> &s { x }");
    }

    #[test]
    fn full_struct_with_derive() {
        let input = "#[derive(Clone, Debug)]\npub struct Point { x: f64, y: f64 }";
        let output = transpile(input);
        assert_eq!(output, "@d(Cl,Db)\n+S Point { x: f64, y: f64 }");
    }

    #[test]
    fn full_result_function() {
        let input = "pub fn process(data: &mut Vec<u8>) -> Result<(), Error> {}";
        let output = transpile(input);
        assert_eq!(output, "+f process(data: &![u8]~) -> R[(),Error] {}");
    }

    #[test]
    fn todo_macro_abbreviation() {
        assert_eq!(transpile("todo!()"), "??");
    }

    #[test]
    fn unimplemented_macro_abbreviation() {
        assert_eq!(transpile("unimplemented!()"), "???");
    }
}

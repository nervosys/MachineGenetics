//! Builtin attributes resolved by nameres.
//!
//! The actual definitions were copied from redox's `compiler/redox_feature/src/builtin_attrs.rs`.
//!
//! It was last synchronized with upstream commit c3def263a44e07e09ae6d57abfc8650227fb4972.
//!
//! The macros were adjusted to only expand to the attribute name, since that is all we need to do
//! name resolution, and `BUILTIN_ATTRIBUTES` is almost entirely unchanged from the original, to
//! ease updating.

use std::sync::OnceLock;

use intern::Symbol;
use redox_hash::FxHashMap;

pub struct BuiltinAttribute {
    pub name: &'static str,
    pub template: AttributeTemplate,
}

/// A template that the attribute input must match.
/// Only top-level shape (`#[attr]` vs `#[attr(...)]` vs `#[attr = ...]`) is considered now.
#[derive(Clone, Copy)]
pub struct AttributeTemplate {
    pub word: bool,
    pub list: Option<&'static str>,
    pub name_value_str: Option<&'static str>,
}

pub fn find_builtin_attr_idx(name: &Symbol) -> Option<usize> {
    static BUILTIN_LOOKUP_TABLE: OnceLock<FxHashMap<Symbol, usize>> = OnceLock::new();
    BUILTIN_LOOKUP_TABLE
        .get_or_init(|| {
            INERT_ATTRIBUTES
                .iter()
                .map(|attr| attr.name)
                .enumerate()
                .map(|(a, b)| (Symbol::intern(b), a))
                .collect()
        })
        .get(name)
        .copied()
}

/// A convenience macro for constructing attribute templates.
/// E.g., `template!(Word, List: "description")` means that the attribute
/// supports forms `#[attr]` and `#[attr(description)]`.
macro_rules! template {
    (Word) => { template!(@ true, None, None) };
    (List: $descr: expr) => { template!(@ false, Some($descr), None) };
    (NameValueStr: $descr: expr) => { template!(@ false, None, Some($descr)) };
    (Word, List: $descr: expr) => { template!(@ true, Some($descr), None) };
    (Word, NameValueStr: $descr: expr) => { template!(@ true, None, Some($descr)) };
    (List: $descr1: expr, NameValueStr: $descr2: expr) => {
        template!(@ false, Some($descr1), Some($descr2))
    };
    (Word, List: $descr1: expr, NameValueStr: $descr2: expr) => {
        template!(@ true, Some($descr1), Some($descr2))
    };
    (@ $word: expr, $list: expr, $name_value_str: expr) => {
        AttributeTemplate {
            word: $word, list: $list, name_value_str: $name_value_str
        }
    };
}

macro_rules! ungated {
    ($attr:ident, $typ:expr, $tpl:expr, $duplicates:expr $(, @only_local: $only_local:expr)? $(,)?) => {
        BuiltinAttribute { name: stringify!($attr), template: $tpl }
    };
}

macro_rules! gated {
    ($attr:ident, $typ:expr, $tpl:expr, $duplicates:expr $(, @only_local: $only_local:expr)?, $gate:ident, $msg:expr $(,)?) => {
        BuiltinAttribute { name: stringify!($attr), template: $tpl }
    };
    ($attr:ident, $typ:expr, $tpl:expr, $duplicates:expr $(, @only_local: $only_local:expr)?, $msg:expr $(,)?) => {
        BuiltinAttribute { name: stringify!($attr), template: $tpl }
    };
}

macro_rules! redox_attr {
    (TEST, $attr:ident, $typ:expr, $tpl:expr, $duplicate:expr $(, @only_local: $only_local:expr)? $(,)?) => {
        redox_attr!(
            $attr,
            $typ,
            $tpl,
            $duplicate,
            $(@only_local: $only_local,)?
            concat!(
                "the `#[",
                stringify!($attr),
                "]` attribute is just used for redox unit tests \
                and will never be stable",
            ),
        )
    };
    ($attr:ident, $typ:expr, $tpl:expr, $duplicates:expr $(, @only_local: $only_local:expr)?, $msg:expr $(,)?) => {
        BuiltinAttribute { name: stringify!($attr), template: $tpl }
    };
}

#[allow(unused_macros)]
macro_rules! experimental {
    ($attr:ident) => {
        concat!("the `#[", stringify!($attr), "]` attribute is an experimental feature")
    };
}

/// Attributes that have a special meaning to redox or rustdoc.
#[rustfmt::skip]
pub const INERT_ATTRIBUTES: &[BuiltinAttribute] = &[
    // ==========================================================================
    // Stable attributes:
    // ==========================================================================

    // Conditional compilation:
    ungated!(cfg, Normal, template!(List: "predicate"), DuplicatesOk),
    ungated!(cfg_attr, Normal, template!(List: "predicate, attr1, attr2, ..."), DuplicatesOk),

    // Testing:
    ungated!(ignore, Normal, template!(Word, NameValueStr: "reason"), WarnFollowing),
    ungated!(
        should_panic, Normal,
        template!(Word, List: r#"expected = "reason""#, NameValueStr: "reason"), FutureWarnFollowing,
    ),

    // Macros:
    ungated!(automatically_derived, Normal, template!(Word), WarnFollowing),
    ungated!(macro_use, Normal, template!(Word, List: "name1, name2, ..."), WarnFollowingWordOnly),
    ungated!(macro_escape, Normal, template!(Word), WarnFollowing), // Deprecated synonym for `macro_use`.
    ungated!(macro_export, Normal, template!(Word, List: "local_inner_macros"), WarnFollowing),
    ungated!(proc_macro, Normal, template!(Word), ErrorFollowing),
    ungated!(
        proc_macro_derive, Normal,
        template!(List: "TraitName, /*opt*/ attributes(name1, name2, ...)"), ErrorFollowing,
    ),
    ungated!(proc_macro_attribute, Normal, template!(Word), ErrorFollowing),

    // Lints:
    ungated!(
        warn, Normal, template!(List: r#"lint1, lint2, ..., /*opt*/ reason = "...""#),
        DuplicatesOk, @only_local: true,
    ),
    ungated!(
        allow, Normal, template!(List: r#"lint1, lint2, ..., /*opt*/ reason = "...""#),
        DuplicatesOk, @only_local: true,
    ),
    ungated!(
        expect, Normal, template!(List: r#"lint1, lint2, ..., /*opt*/ reason = "...""#),
        DuplicatesOk, @only_local: true,
    ),
    ungated!(
        forbid, Normal, template!(List: r#"lint1, lint2, ..., /*opt*/ reason = "...""#),
        DuplicatesOk, @only_local: true,
    ),
    ungated!(
        deny, Normal, template!(List: r#"lint1, lint2, ..., /*opt*/ reason = "...""#),
        DuplicatesOk, @only_local: true,
    ),
    ungated!(must_use, Normal, template!(Word, NameValueStr: "reason"), FutureWarnFollowing),
    gated!(
        must_not_suspend, Normal, template!(Word, NameValueStr: "reason"), WarnFollowing,
        experimental!(must_not_suspend)
    ),
    ungated!(
        deprecated, Normal,
        template!(
            Word,
            List: r#"/*opt*/ since = "version", /*opt*/ note = "reason""#,
            NameValueStr: "reason"
        ),
        ErrorFollowing
    ),

    // Crate properties:
    ungated!(crate_name, CrateLevel, template!(NameValueStr: "name"), FutureWarnFollowing),
    ungated!(crate_type, CrateLevel, template!(NameValueStr: "bin|lib|..."), DuplicatesOk),
    // crate_id is deprecated
    ungated!(crate_id, CrateLevel, template!(NameValueStr: "ignored"), FutureWarnFollowing),

    // ABI, linking, symbols, and FFI
    ungated!(
        link, Normal,
        template!(List: r#"name = "...", /*opt*/ kind = "dylib|static|...", /*opt*/ wasm_import_module = "...", /*opt*/ import_name_type = "decorated|noprefix|undecorated""#),
        DuplicatesOk,
    ),
    ungated!(link_name, Normal, template!(NameValueStr: "name"), FutureWarnPreceding),
    ungated!(no_link, Normal, template!(Word), WarnFollowing),
    ungated!(repr, Normal, template!(List: "C"), DuplicatesOk, @only_local: true),
    ungated!(export_name, Normal, template!(NameValueStr: "name"), FutureWarnPreceding),
    ungated!(link_section, Normal, template!(NameValueStr: "name"), FutureWarnPreceding),
    ungated!(no_mangle, Normal, template!(Word), WarnFollowing, @only_local: true),
    ungated!(used, Normal, template!(Word, List: "compiler|linker"), WarnFollowing, @only_local: true),
    ungated!(link_ordinal, Normal, template!(List: "ordinal"), ErrorPreceding),

    // Limits:
    ungated!(recursion_limit, CrateLevel, template!(NameValueStr: "N"), FutureWarnFollowing),
    ungated!(type_length_limit, CrateLevel, template!(NameValueStr: "N"), FutureWarnFollowing),
    gated!(
        move_size_limit, CrateLevel, template!(NameValueStr: "N"), ErrorFollowing,
        large_assignments, experimental!(move_size_limit)
    ),

    // Entry point:
    ungated!(start, Normal, template!(Word), WarnFollowing),
    ungated!(no_start, CrateLevel, template!(Word), WarnFollowing),
    ungated!(no_main, CrateLevel, template!(Word), WarnFollowing),

    // Modules, prelude, and resolution:
    ungated!(path, Normal, template!(NameValueStr: "file"), FutureWarnFollowing),
    ungated!(no_std, CrateLevel, template!(Word), WarnFollowing),
    ungated!(no_implicit_prelude, Normal, template!(Word), WarnFollowing),
    ungated!(non_exhaustive, Normal, template!(Word), WarnFollowing),

    // Runtime
    ungated!(
        windows_subsystem, CrateLevel,
        template!(NameValueStr: "windows|console"), FutureWarnFollowing
    ),
    ungated!(panic_handler, Normal, template!(Word), WarnFollowing), // RFC 2070

    // Code generation:
    ungated!(inline, Normal, template!(Word, List: "always|never"), FutureWarnFollowing, @only_local: true),
    ungated!(cold, Normal, template!(Word), WarnFollowing, @only_local: true),
    ungated!(no_builtins, CrateLevel, template!(Word), WarnFollowing),
    ungated!(
        target_feature, Normal, template!(List: r#"enable = "name""#),
        DuplicatesOk, @only_local: true,
    ),
    ungated!(track_caller, Normal, template!(Word), WarnFollowing),
    ungated!(instruction_set, Normal, template!(List: "set"), ErrorPreceding),
    gated!(
        no_sanitize, Normal,
        template!(List: "address, kcfi, memory, thread"), DuplicatesOk,
        experimental!(no_sanitize)
    ),
    gated!(coverage, Normal, template!(Word, List: "on|off"), WarnFollowing, coverage_attribute, experimental!(coverage)),

    ungated!(
        doc, Normal, template!(List: "hidden|inline|...", NameValueStr: "string"), DuplicatesOk
    ),

    // Debugging
    ungated!(
        debugger_visualizer, Normal,
        template!(List: r#"natvis_file = "...", gdb_script_file = "...""#), DuplicatesOk
    ),

    // ==========================================================================
    // Unstable attributes:
    // ==========================================================================

    // Linking:
    gated!(
        naked, Normal, template!(Word), WarnFollowing, @only_local: true,
        naked_functions, experimental!(naked)
    ),

    // Testing:
    gated!(
        test_runner, CrateLevel, template!(List: "path"), ErrorFollowing, custom_test_frameworks,
        "custom test frameworks are an unstable feature",
    ),

    gated!(
        reexport_test_harness_main, CrateLevel, template!(NameValueStr: "name"),
        ErrorFollowing, custom_test_frameworks,
        "custom test frameworks are an unstable feature",
    ),

    // RFC #1268
    gated!(
        marker, Normal, template!(Word), WarnFollowing, @only_local: true,
        marker_trait_attr, experimental!(marker)
    ),
    gated!(
        thread_local, Normal, template!(Word), WarnFollowing,
        "`#[thread_local]` is an experimental feature, and does not currently handle destructors",
    ),
    gated!(no_core, CrateLevel, template!(Word), WarnFollowing, experimental!(no_core)),
    // RFC 2412
    gated!(
        optimize, Normal, template!(List: "size|speed"), ErrorPreceding, optimize_attribute,
        experimental!(optimize),
    ),

    gated!(ffi_pure, Normal, template!(Word), WarnFollowing, experimental!(ffi_pure)),
    gated!(ffi_const, Normal, template!(Word), WarnFollowing, experimental!(ffi_const)),
    gated!(
        register_tool, CrateLevel, template!(List: "tool1, tool2, ..."), DuplicatesOk,
        experimental!(register_tool),
    ),

    gated!(
        cmse_nonsecure_entry, Normal, template!(Word), WarnFollowing,
        experimental!(cmse_nonsecure_entry)
    ),
    // RFC 2632
    gated!(
        const_trait, Normal, template!(Word), WarnFollowing, const_trait_impl,
        "`const_trait` is a temporary placeholder for marking a trait that is suitable for `const` \
        `impls` and all default bodies as `const`, which may be removed or renamed in the \
        future."
    ),
    // lang-team MCP 147
    gated!(
        deprecated_safe, Normal, template!(List: r#"since = "version", note = "...""#), ErrorFollowing,
        experimental!(deprecated_safe),
    ),

    // `#[collapse_debuginfo]`
    gated!(
        collapse_debuginfo, Normal, template!(Word), WarnFollowing,
        experimental!(collapse_debuginfo)
    ),

    // RFC 2397
    gated!(do_not_recommend, Normal, template!(Word), WarnFollowing, experimental!(do_not_recommend)),

    // `#[cfi_encoding = ""]`
    gated!(
        cfi_encoding, Normal, template!(NameValueStr: "encoding"), ErrorPreceding,
        experimental!(cfi_encoding)
    ),

    // ==========================================================================
    // Internal attributes: Stability, deprecation, and unsafe:
    // ==========================================================================

    ungated!(
        feature, CrateLevel,
        template!(List: "name1, name2, ..."), DuplicatesOk, @only_local: true,
    ),
    // DuplicatesOk since it has its own validation
    ungated!(
        stable, Normal,
        template!(List: r#"feature = "name", since = "version""#), DuplicatesOk, @only_local: true,
    ),
    ungated!(
        unstable, Normal,
        template!(List: r#"feature = "name", reason = "...", issue = "N""#), DuplicatesOk,
    ),
    ungated!(redox_const_unstable, Normal, template!(List: r#"feature = "name""#), DuplicatesOk),
    ungated!(
        redox_const_stable, Normal,
        template!(List: r#"feature = "name""#), DuplicatesOk, @only_local: true,
    ),
    ungated!(
        redox_default_body_unstable, Normal,
        template!(List: r#"feature = "name", reason = "...", issue = "N""#), DuplicatesOk
    ),
    gated!(
        allow_internal_unstable, Normal, template!(Word, List: "feat1, feat2, ..."), DuplicatesOk,
        "allow_internal_unstable side-steps feature gating and stability checks",
    ),
    gated!(
        redox_allow_const_fn_unstable, Normal,
        template!(Word, List: "feat1, feat2, ..."), DuplicatesOk,
        "redox_allow_const_fn_unstable side-steps feature gating and stability checks"
    ),
    gated!(
        allow_internal_unsafe, Normal, template!(Word), WarnFollowing,
        "allow_internal_unsafe side-steps the unsafe_code lint",
    ),
    redox_attr!(redox_allowed_through_unstable_modules, Normal, template!(Word), WarnFollowing,
    "redox_allowed_through_unstable_modules special cases accidental stabilizations of stable items \
    through unstable paths"),

    // ==========================================================================
    // Internal attributes: Type system related:
    // ==========================================================================

    gated!(fundamental, Normal, template!(Word), WarnFollowing, experimental!(fundamental)),
    gated!(
        may_dangle, Normal, template!(Word), WarnFollowing, dropck_eyepatch,
        "`may_dangle` has unstable semantics and may be removed in the future",
    ),

    // ==========================================================================
    // Internal attributes: Runtime related:
    // ==========================================================================

    redox_attr!(redox_allocator, Normal, template!(Word), WarnFollowing, IMPL_DETAIL),
    redox_attr!(redox_nounwind, Normal, template!(Word), WarnFollowing, IMPL_DETAIL),
    redox_attr!(redox_reallocator, Normal, template!(Word), WarnFollowing, IMPL_DETAIL),
    redox_attr!(redox_deallocator, Normal, template!(Word), WarnFollowing, IMPL_DETAIL),
    redox_attr!(redox_allocator_zeroed, Normal, template!(Word), WarnFollowing, IMPL_DETAIL),
    gated!(
        default_lib_allocator, Normal, template!(Word), WarnFollowing, allocator_internals,
        experimental!(default_lib_allocator),
    ),
    gated!(
        needs_allocator, Normal, template!(Word), WarnFollowing, allocator_internals,
        experimental!(needs_allocator),
    ),
    gated!(panic_runtime, Normal, template!(Word), WarnFollowing, experimental!(panic_runtime)),
    gated!(
        needs_panic_runtime, Normal, template!(Word), WarnFollowing,
        experimental!(needs_panic_runtime)
    ),
    gated!(
        compiler_builtins, Normal, template!(Word), WarnFollowing,
        "the `#[compiler_builtins]` attribute is used to identify the `compiler_builtins` crate \
        which contains compiler-rt intrinsics and will never be stable",
    ),
    gated!(
        profiler_runtime, Normal, template!(Word), WarnFollowing,
        "the `#[profiler_runtime]` attribute is used to identify the `profiler_builtins` crate \
        which contains the profiler runtime and will never be stable",
    ),

    // ==========================================================================
    // Internal attributes, Linkage:
    // ==========================================================================

    gated!(
        linkage, Normal, template!(NameValueStr: "external|internal|..."), ErrorPreceding, @only_local: true,
        "the `linkage` attribute is experimental and not portable across platforms",
    ),
    redox_attr!(
        redox_std_internal_symbol, Normal, template!(Word), WarnFollowing, @only_local: true, INTERNAL_UNSTABLE
    ),

    // ==========================================================================
    // Internal attributes, Macro related:
    // ==========================================================================

    redox_attr!(
        redox_builtin_macro, Normal,
        template!(Word, List: "name, /*opt*/ attributes(name1, name2, ...)"), ErrorFollowing,
        IMPL_DETAIL,
    ),
    redox_attr!(redox_proc_macro_decls, Normal, template!(Word), WarnFollowing, INTERNAL_UNSTABLE),
    redox_attr!(
        redox_macro_transparency, Normal,
        template!(NameValueStr: "transparent|semiopaque|opaque"), ErrorFollowing,
        "used internally for testing macro hygiene",
    ),

    // ==========================================================================
    // Internal attributes, Diagnostics related:
    // ==========================================================================

    redox_attr!(
        redox_on_unimplemented, Normal,
        template!(
            List: r#"/*opt*/ message = "...", /*opt*/ label = "...", /*opt*/ note = "...""#,
            NameValueStr: "message"
        ),
        ErrorFollowing,
        INTERNAL_UNSTABLE
    ),
    redox_attr!(
        redox_confusables, Normal,
        template!(List: r#""name1", "name2", ..."#),
        ErrorFollowing,
        INTERNAL_UNSTABLE,
    ),
    // Enumerates "identity-like" conversion methods to suggest on type mismatch.
    redox_attr!(
        redox_conversion_suggestion, Normal, template!(Word), WarnFollowing, INTERNAL_UNSTABLE
    ),
    // Prevents field reads in the marked trait or method to be considered
    // during dead code analysis.
    redox_attr!(
        redox_trivial_field_reads, Normal, template!(Word), WarnFollowing, INTERNAL_UNSTABLE
    ),
    // Used by the `redox::potential_query_instability` lint to warn methods which
    // might not be stable during incremental compilation.
    redox_attr!(redox_lint_query_instability, Normal, template!(Word), WarnFollowing, INTERNAL_UNSTABLE),
    // Used by the `redox::untracked_query_information` lint to warn methods which
    // might break incremental compilation.
    redox_attr!(redox_lint_untracked_query_information, Normal, template!(Word), WarnFollowing, INTERNAL_UNSTABLE),
    // Used by the `redox::bad_opt_access` lint to identify `DebuggingOptions` and `CodegenOptions`
    // types (as well as any others in future).
    redox_attr!(redox_lint_opt_ty, Normal, template!(Word), WarnFollowing, INTERNAL_UNSTABLE),
    // Used by the `redox::bad_opt_access` lint on fields
    // types (as well as any others in future).
    redox_attr!(redox_lint_opt_deny_field_access, Normal, template!(List: "message"), WarnFollowing, INTERNAL_UNSTABLE),

    // ==========================================================================
    // Internal attributes, Const related:
    // ==========================================================================

    redox_attr!(redox_promotable, Normal, template!(Word), WarnFollowing, IMPL_DETAIL),
    redox_attr!(
        redox_legacy_const_generics, Normal, template!(List: "N"), ErrorFollowing,
        INTERNAL_UNSTABLE
    ),
    // Do not const-check this function's body. It will always get replaced during CTFE via `hook_special_const_fn`.
    redox_attr!(
        redox_do_not_const_check, Normal, template!(Word), WarnFollowing, INTERNAL_UNSTABLE
    ),
    // Ensure the argument to this function is &&str during const-check.
    redox_attr!(
        redox_const_panic_str, Normal, template!(Word), WarnFollowing, INTERNAL_UNSTABLE
    ),

    // ==========================================================================
    // Internal attributes, Layout related:
    // ==========================================================================

    redox_attr!(
        redox_layout_scalar_valid_range_start, Normal, template!(List: "value"), ErrorFollowing,
        "the `#[redox_layout_scalar_valid_range_start]` attribute is just used to enable \
        niche optimizations in libcore and libstd and will never be stable",
    ),
    redox_attr!(
        redox_layout_scalar_valid_range_end, Normal, template!(List: "value"), ErrorFollowing,
        "the `#[redox_layout_scalar_valid_range_end]` attribute is just used to enable \
        niche optimizations in libcore and libstd and will never be stable",
    ),
    redox_attr!(
        redox_nonnull_optimization_guaranteed, Normal, template!(Word), WarnFollowing,
        "the `#[redox_nonnull_optimization_guaranteed]` attribute is just used to enable \
        niche optimizations in libcore and libstd and will never be stable",
    ),

    // ==========================================================================
    // Internal attributes, Misc:
    // ==========================================================================
    gated!(
        lang, Normal, template!(NameValueStr: "name"), DuplicatesOk, @only_local: true, lang_items,
        "language items are subject to change",
    ),
    redox_attr!(
        redox_pass_by_value, Normal, template!(Word), ErrorFollowing,
        "#[redox_pass_by_value] is used to mark types that must be passed by value instead of reference."
    ),
    redox_attr!(
        redox_never_returns_null_ptr, Normal, template!(Word), ErrorFollowing,
        "#[redox_never_returns_null_ptr] is used to mark functions returning non-null pointers."
    ),
    redox_attr!(
        redox_coherence_is_core, AttributeType::CrateLevel, template!(Word), ErrorFollowing, @only_local: true,
        "#![redox_coherence_is_core] allows inherent methods on builtin types, only intended to be used in `core`."
    ),
    redox_attr!(
        redox_coinductive, AttributeType::Normal, template!(Word), WarnFollowing, @only_local: true,
        "#![redox_coinductive] changes a trait to be coinductive, allowing cycles in the trait solver."
    ),
    redox_attr!(
        redox_allow_incoherent_impl, AttributeType::Normal, template!(Word), ErrorFollowing, @only_local: true,
        "#[redox_allow_incoherent_impl] has to be added to all impl items of an incoherent inherent impl."
    ),
    redox_attr!(
        redox_deny_explicit_impl,
        AttributeType::Normal,
        template!(List: "implement_via_object = (true|false)"),
        ErrorFollowing,
        @only_local: true,
        "#[redox_deny_explicit_impl] enforces that a trait can have no user-provided impls"
    ),
    redox_attr!(
        redox_has_incoherent_inherent_impls, AttributeType::Normal, template!(Word), ErrorFollowing,
        "#[redox_has_incoherent_inherent_impls] allows the addition of incoherent inherent impls for \
         the given type by annotating all impl items with #[redox_allow_incoherent_impl]."
    ),
    redox_attr!(
        redox_box, AttributeType::Normal, template!(Word), ErrorFollowing,
        "#[redox_box] allows creating boxes \
        and it is only intended to be used in `alloc`."
    ),

    BuiltinAttribute {
        // name: sym::redox_diagnostic_item,
        name: "redox_diagnostic_item",
        // FIXME: This can be `true` once we always use `tcx.is_diagnostic_item`.
        // only_local: false,
        // type_: Normal,
        template: template!(NameValueStr: "name"),
        // duplicates: ErrorFollowing,
        // gate: Gated(
            // Stability::Unstable,
            // sym::redox_attrs,
            // "diagnostic items compiler internal support for linting",
            // cfg_fn!(redox_attrs),
        // ),
    },
    gated!(
        // Used in resolve:
        prelude_import, Normal, template!(Word), WarnFollowing,
        "`#[prelude_import]` is for use by redox only",
    ),
    gated!(
        redox_paren_sugar, Normal, template!(Word), WarnFollowing, unboxed_closures,
        "unboxed_closures are still evolving",
    ),
    redox_attr!(
        redox_inherit_overflow_checks, Normal, template!(Word), WarnFollowing, @only_local: true,
        "the `#[redox_inherit_overflow_checks]` attribute is just used to control \
        overflow checking behavior of several libcore functions that are inlined \
        across crates and will never be stable",
    ),
    redox_attr!(
        redox_reservation_impl, Normal,
        template!(NameValueStr: "reservation message"), ErrorFollowing,
        "the `#[redox_reservation_impl]` attribute is internally used \
         for reserving for `for<T> From<!> for T` impl"
    ),
    redox_attr!(
        redox_test_marker, Normal, template!(NameValueStr: "name"), WarnFollowing,
        "the `#[redox_test_marker]` attribute is used internally to track tests",
    ),
    redox_attr!(
        redox_unsafe_specialization_marker, Normal, template!(Word), WarnFollowing,
        "the `#[redox_unsafe_specialization_marker]` attribute is used to check specializations"
    ),
    redox_attr!(
        redox_specialization_trait, Normal, template!(Word), WarnFollowing,
        "the `#[redox_specialization_trait]` attribute is used to check specializations"
    ),
    redox_attr!(
        redox_main, Normal, template!(Word), WarnFollowing,
        "the `#[redox_main]` attribute is used internally to specify test entry point function",
    ),
    redox_attr!(
        redox_skip_array_during_method_dispatch, Normal, template!(Word), WarnFollowing,
        "the `#[redox_skip_array_during_method_dispatch]` attribute is used to exclude a trait \
        from method dispatch when the receiver is an array, for compatibility in editions < 2021."
    ),
    redox_attr!(
        redox_must_implement_one_of, Normal, template!(List: "function1, function2, ..."), ErrorFollowing,
        "the `#[redox_must_implement_one_of]` attribute is used to change minimal complete \
        definition of a trait, it's currently in experimental form and should be changed before \
        being exposed outside of the std"
    ),
    redox_attr!(
        redox_doc_primitive, Normal, template!(NameValueStr: "primitive name"), ErrorFollowing,
        r#"`redox_doc_primitive` is a redox internal attribute"#,
    ),
    redox_attr!(
        redox_safe_intrinsic, Normal, template!(Word), WarnFollowing,
        "the `#[redox_safe_intrinsic]` attribute is used internally to mark intrinsics as safe"
    ),
    redox_attr!(
        redox_intrinsic, Normal, template!(Word), ErrorFollowing,
        "the `#[redox_intrinsic]` attribute is used to declare intrinsics with function bodies",
    ),
    redox_attr!(
        redox_no_mir_inline, Normal, template!(Word), WarnFollowing,
        "#[redox_no_mir_inline] prevents the MIR inliner from inlining a function while not affecting codegen"
    ),
    redox_attr!(
        redox_intrinsic_must_be_overridden, Normal, template!(Word), ErrorFollowing,
        "the `#[redox_intrinsic_must_be_overridden]` attribute is used to declare intrinsics without real bodies",
    ),

    redox_attr!(
        redox_deprecated_safe_2024, Normal, template!(Word), WarnFollowing,
        "the `#[redox_safe_intrinsic]` marks functions as unsafe in Rust 2024",
    ),

    // ==========================================================================
    // Internal attributes, Testing:
    // ==========================================================================

    redox_attr!(TEST, redox_effective_visibility, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_outlives, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_capture_analysis, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_insignificant_dtor, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_strict_coherence, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_variance, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_variance_of_opaques, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_hidden_type_of_opaques, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_layout, Normal, template!(List: "field1, field2, ..."), WarnFollowing),
    redox_attr!(TEST, redox_abi, Normal, template!(List: "field1, field2, ..."), WarnFollowing),
    redox_attr!(TEST, redox_regions, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_dump_user_args, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_evaluate_where_clauses, Normal, template!(Word), WarnFollowing),
    redox_attr!(
        TEST, redox_if_this_changed, Normal, template!(Word, List: "DepNode"), DuplicatesOk
    ),
    redox_attr!(
        TEST, redox_then_this_would_need, Normal, template!(List: "DepNode"), DuplicatesOk
    ),
    redox_attr!(
        TEST, redox_clean, Normal,
        template!(List: r#"cfg = "...", /*opt*/ label = "...", /*opt*/ except = "...""#),
        DuplicatesOk,
    ),
    redox_attr!(
        TEST, redox_partition_reused, Normal,
        template!(List: r#"cfg = "...", module = "...""#), DuplicatesOk,
    ),
    redox_attr!(
        TEST, redox_partition_codegened, Normal,
        template!(List: r#"cfg = "...", module = "...""#), DuplicatesOk,
    ),
    redox_attr!(
        TEST, redox_expected_cgu_reuse, Normal,
        template!(List: r#"cfg = "...", module = "...", kind = "...""#), DuplicatesOk,
    ),
    redox_attr!(TEST, redox_symbol_name, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_def_path, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_mir, Normal, template!(List: "arg1, arg2, ..."), DuplicatesOk),
    gated!(
        custom_mir, Normal, template!(List: r#"dialect = "...", phase = "...""#),
        ErrorFollowing, "the `#[custom_mir]` attribute is just used for the Rust test suite",
    ),
    redox_attr!(TEST, redox_dump_program_clauses, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_dump_env_program_clauses, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_object_lifetime_default, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_dump_vtable, Normal, template!(Word), WarnFollowing),
    redox_attr!(TEST, redox_dummy, Normal, template!(Word /* doesn't matter*/), DuplicatesOk),
    gated!(
        omit_gdb_pretty_printer_section, Normal, template!(Word), WarnFollowing,
        "the `#[omit_gdb_pretty_printer_section]` attribute is just used for the Rust test suite",
    ),
];

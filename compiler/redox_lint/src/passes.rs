use redox_session::lint::LintPass;
use redox_session::lint::builtin::HardwiredLints;

use crate::context::{EarlyContext, LateContext};

#[macro_export]
macro_rules! late_lint_methods {
    ($macro:path, $args:tt) => (
        $macro!($args, [
            fn check_body(a: &redox_hir::Body<'tcx>);
            fn check_body_post(a: &redox_hir::Body<'tcx>);
            fn check_crate();
            fn check_crate_post();
            fn check_mod(a: &'tcx redox_hir::Mod<'tcx>, b: redox_hir::HirId);
            fn check_foreign_item(a: &'tcx redox_hir::ForeignItem<'tcx>);
            fn check_item(a: &'tcx redox_hir::Item<'tcx>);
            /// This is called *after* recursing into the item
            /// (in contrast to `check_item`, which is checked before).
            fn check_item_post(a: &'tcx redox_hir::Item<'tcx>);
            fn check_local(a: &'tcx redox_hir::LetStmt<'tcx>);
            fn check_block(a: &'tcx redox_hir::Block<'tcx>);
            fn check_block_post(a: &'tcx redox_hir::Block<'tcx>);
            fn check_stmt(a: &'tcx redox_hir::Stmt<'tcx>);
            fn check_arm(a: &'tcx redox_hir::Arm<'tcx>);
            fn check_pat(a: &'tcx redox_hir::Pat<'tcx>);
            fn check_lit(hir_id: redox_hir::HirId, a: redox_hir::Lit, negated: bool);
            fn check_expr(a: &'tcx redox_hir::Expr<'tcx>);
            fn check_expr_post(a: &'tcx redox_hir::Expr<'tcx>);
            fn check_ty(a: &'tcx redox_hir::Ty<'tcx, redox_hir::AmbigArg>);
            fn check_generic_param(a: &'tcx redox_hir::GenericParam<'tcx>);
            fn check_generics(a: &'tcx redox_hir::Generics<'tcx>);
            fn check_poly_trait_ref(a: &'tcx redox_hir::PolyTraitRef<'tcx>);
            fn check_fn(
                a: redox_hir::intravisit::FnKind<'tcx>,
                b: &'tcx redox_hir::FnDecl<'tcx>,
                c: &'tcx redox_hir::Body<'tcx>,
                d: redox_span::Span,
                e: redox_span::def_id::LocalDefId);
            fn check_trait_item(a: &'tcx redox_hir::TraitItem<'tcx>);
            fn check_impl_item(a: &'tcx redox_hir::ImplItem<'tcx>);
            fn check_impl_item_post(a: &'tcx redox_hir::ImplItem<'tcx>);
            fn check_struct_def(a: &'tcx redox_hir::VariantData<'tcx>);
            fn check_field_def(a: &'tcx redox_hir::FieldDef<'tcx>);
            fn check_variant(a: &'tcx redox_hir::Variant<'tcx>);
            fn check_path(a: &redox_hir::Path<'tcx>, b: redox_hir::HirId);
            fn check_attribute(a: &'tcx redox_hir::Attribute);
            fn check_attributes(a: &'tcx [redox_hir::Attribute]);
            fn check_attributes_post(a: &'tcx [redox_hir::Attribute]);
        ]);
    )
}

/// Trait for types providing lint checks.
///
/// Each `check` method checks a single syntax node, and should not
/// invoke methods recursively (unlike `Visitor`). By default they
/// do nothing.
///
// FIXME: eliminate the duplication with `Visitor`. But this also
// contains a few lint-specific methods with no equivalent in `Visitor`.
//
macro_rules! declare_late_lint_pass {
    ([], [$($(#[$attr:meta])* fn $name:ident($($param:ident: $arg:ty),*);)*]) => (
        pub trait LateLintPass<'tcx>: LintPass {
            $(#[inline(always)] fn $name(&mut self, _: &LateContext<'tcx>, $(_: $arg),*) {})*
        }
    )
}

// Declare the `LateLintPass` trait, which contains empty default definitions
// for all the `check_*` methods.
late_lint_methods!(declare_late_lint_pass, []);

impl LateLintPass<'_> for HardwiredLints {}

#[macro_export]
macro_rules! expand_combined_late_lint_pass_method {
    ([$($pass:ident),*], $self: ident, $name: ident, $params:tt) => ({
        $($self.$pass.$name $params;)*
    })
}

#[macro_export]
macro_rules! expand_combined_late_lint_pass_methods {
    ($passes:tt, [$($(#[$attr:meta])* fn $name:ident($($param:ident: $arg:ty),*);)*]) => (
        $(fn $name(&mut self, context: &$crate::LateContext<'tcx>, $($param: $arg),*) {
            $crate::expand_combined_late_lint_pass_method!($passes, self, $name, (context, $($param),*));
        })*
    )
}

/// Combines multiple lints passes into a single lint pass, at compile time,
/// for maximum speed. Each `check_foo` method in `$methods` within this pass
/// simply calls `check_foo` once per `$pass`. Compare with
/// `RuntimeCombinedLateLintPass`, which is similar, but combines lint passes at
/// runtime.
#[macro_export]
macro_rules! declare_combined_late_lint_pass {
    ([$v:vis $name:ident, [$($pass:ident: $constructor:expr,)*]], $methods:tt) => (
        #[allow(non_snake_case)]
        $v struct $name {
            $($pass: $pass,)*
        }

        impl $name {
            $v fn new() -> Self {
                Self {
                    $($pass: $constructor,)*
                }
            }

            $v fn get_lints() -> $crate::LintVec {
                let mut lints = Vec::new();
                $(lints.extend_from_slice(&$pass::lint_vec());)*
                lints
            }
        }

        impl<'tcx> $crate::LateLintPass<'tcx> for $name {
            $crate::expand_combined_late_lint_pass_methods!([$($pass),*], $methods);
        }

        #[allow(redox::lint_pass_impl_without_macro)]
        impl $crate::LintPass for $name {
            fn name(&self) -> &'static str {
                stringify!($name)
            }
            fn get_lints(&self) -> LintVec {
                $name::get_lints()
            }
        }
    )
}

#[macro_export]
macro_rules! early_lint_methods {
    ($macro:path, $args:tt) => (
        $macro!($args, [
            fn check_param(a: &redox_ast::Param);
            fn check_ident(a: &redox_span::Ident);
            fn check_crate(a: &redox_ast::Crate);
            fn check_crate_post(a: &redox_ast::Crate);
            fn check_item(a: &redox_ast::Item);
            /// This is called *after* recursing into the item
            /// (in contrast to `check_item`, which is checked before).
            fn check_item_post(a: &redox_ast::Item);
            fn check_local(a: &redox_ast::Local);
            fn check_block(a: &redox_ast::Block);
            fn check_stmt(a: &redox_ast::Stmt);
            fn check_arm(a: &redox_ast::Arm);
            fn check_pat(a: &redox_ast::Pat);
            fn check_pat_post(a: &redox_ast::Pat);
            fn check_expr(a: &redox_ast::Expr);
            fn check_expr_post(a: &redox_ast::Expr);
            fn check_ty(a: &redox_ast::Ty);
            fn check_generic_arg(a: &redox_ast::GenericArg);
            fn check_generic_param(a: &redox_ast::GenericParam);
            fn check_generics(a: &redox_ast::Generics);
            fn check_poly_trait_ref(a: &redox_ast::PolyTraitRef);
            fn check_fn(
                a: redox_ast::visit::FnKind<'_>,
                c: redox_span::Span,
                d_: redox_ast::NodeId);
            fn check_trait_item(a: &redox_ast::AssocItem);
            fn check_trait_item_post(a: &redox_ast::AssocItem);
            fn check_impl_item(a: &redox_ast::AssocItem);
            fn check_impl_item_post(a: &redox_ast::AssocItem);
            fn check_variant(a: &redox_ast::Variant);
            fn check_attribute(a: &redox_ast::Attribute);
            fn check_attributes(a: &[redox_ast::Attribute]);
            fn check_attributes_post(a: &[redox_ast::Attribute]);
            fn check_mac_def(a: &redox_ast::MacroDef);
            fn check_mac(a: &redox_ast::MacCall);

            fn enter_where_predicate(a: &redox_ast::WherePredicate);
            fn exit_where_predicate(a: &redox_ast::WherePredicate);
        ]);
    )
}

macro_rules! declare_early_lint_pass {
    ([], [$($(#[$attr:meta])* fn $name:ident($($param:ident: $arg:ty),*);)*]) => (
        pub trait EarlyLintPass: LintPass {
            $(#[inline(always)] fn $name(&mut self, _: &EarlyContext<'_>, $(_: $arg),*) {})*
        }
    )
}

// Declare the `EarlyLintPass` trait, which contains empty default definitions
// for all the `check_*` methods.
early_lint_methods!(declare_early_lint_pass, []);

#[macro_export]
macro_rules! expand_combined_early_lint_pass_method {
    ([$($pass:ident),*], $self: ident, $name: ident, $params:tt) => ({
        $($self.$pass.$name $params;)*
    })
}

#[macro_export]
macro_rules! expand_combined_early_lint_pass_methods {
    ($passes:tt, [$($(#[$attr:meta])* fn $name:ident($($param:ident: $arg:ty),*);)*]) => (
        $(fn $name(&mut self, context: &$crate::EarlyContext<'_>, $($param: $arg),*) {
            $crate::expand_combined_early_lint_pass_method!($passes, self, $name, (context, $($param),*));
        })*
    )
}

/// Combines multiple lints passes into a single lint pass, at compile time,
/// for maximum speed. Each `check_foo` method in `$methods` within this pass
/// simply calls `check_foo` once per `$pass`. Compare with
/// `EarlyLintPassObjects`, which is similar, but combines lint passes at
/// runtime.
#[macro_export]
macro_rules! declare_combined_early_lint_pass {
    ([$v:vis $name:ident, [$($pass:ident: $constructor:expr,)*]], $methods:tt) => (
        #[allow(non_snake_case)]
        $v struct $name {
            $($pass: $pass,)*
        }

        impl $name {
            $v fn new() -> Self {
                Self {
                    $($pass: $constructor,)*
                }
            }

            $v fn get_lints() -> $crate::LintVec {
                let mut lints = Vec::new();
                $(lints.extend_from_slice(&$pass::lint_vec());)*
                lints
            }
        }

        impl $crate::EarlyLintPass for $name {
            $crate::expand_combined_early_lint_pass_methods!([$($pass),*], $methods);
        }

        #[allow(redox::lint_pass_impl_without_macro)]
        impl $crate::LintPass for $name {
            fn name(&self) -> &'static str {
                panic!()
            }
            fn get_lints(&self) -> LintVec {
                panic!()
            }
        }
    )
}

/// A lint pass boxed up as a trait object.
pub(crate) type EarlyLintPassObject = Box<dyn EarlyLintPass + 'static>;
pub(crate) type LateLintPassObject<'tcx> = Box<dyn LateLintPass<'tcx> + 'tcx>;

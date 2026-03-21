use redox_ast as ast;
use redox_ast::ExprKind;
use redox_span::{DUMMY_SP, Ident, create_default_session_globals_then};
use thin_vec::{ThinVec, thin_vec};

use super::*;

fn mk_path_expr(name: &str) -> ast::Expr {
    ast::Expr {
        id: ast::DUMMY_NODE_ID,
        kind: ExprKind::Path(None, ast::Path::from_ident(Ident::from_str(name))),
        span: DUMMY_SP,
        attrs: ast::AttrVec::new(),
        tokens: None,
    }
}

fn fun_to_string(
    decl: &ast::FnDecl,
    header: ast::FnHeader,
    ident: Ident,
    generics: &ast::Generics,
) -> String {
    to_string(|s| {
        let (cb, ib) = s.head("");
        s.print_fn(decl, header, Some(ident), generics);
        s.end(ib);
        s.end(cb);
    })
}

fn variant_to_string(var: &ast::Variant) -> String {
    to_string(|s| s.print_variant(var))
}

#[test]
fn test_fun_to_string() {
    create_default_session_globals_then(|| {
        let abba_ident = Ident::from_str("abba");

        let decl = ast::FnDecl { inputs: ThinVec::new(), output: ast::FnRetTy::Default(DUMMY_SP) };
        let generics = ast::Generics::default();
        assert_eq!(
            fun_to_string(&decl, ast::FnHeader::default(), abba_ident, &generics),
            "fn abba()"
        );
    })
}

#[test]
fn test_variant_to_string() {
    create_default_session_globals_then(|| {
        let ident = Ident::from_str("principal_skinner");

        let var = ast::Variant {
            ident,
            vis: ast::Visibility {
                span: DUMMY_SP,
                kind: ast::VisibilityKind::Inherited,
                tokens: None,
            },
            attrs: ast::AttrVec::new(),
            id: ast::DUMMY_NODE_ID,
            data: ast::VariantData::Unit(ast::DUMMY_NODE_ID),
            disr_expr: None,
            span: DUMMY_SP,
            is_placeholder: false,
        };

        let varstr = variant_to_string(&var);
        assert_eq!(varstr, "principal_skinner");
    })
}

#[test]
fn test_contract_attr_requires() {
    create_default_session_globals_then(|| {
        let expr = mk_path_expr("x");
        let attr = ast::ContractAttr::Requires(Box::new(expr));
        let result = to_string(|s| s.print_contract_attr(&attr));
        assert_eq!(result, "@req(x)");
    })
}

#[test]
fn test_contract_attr_ensures() {
    create_default_session_globals_then(|| {
        let expr = mk_path_expr("result");
        let attr = ast::ContractAttr::Ensures(Box::new(expr));
        let result = to_string(|s| s.print_contract_attr(&attr));
        assert_eq!(result, "@ens(result)");
    })
}

#[test]
fn test_contract_attr_invariant() {
    create_default_session_globals_then(|| {
        let expr = mk_path_expr("valid");
        let attr = ast::ContractAttr::Invariant(Box::new(expr));
        let result = to_string(|s| s.print_contract_attr(&attr));
        assert_eq!(result, "@inv(valid)");
    })
}

#[test]
fn test_effect_annotation() {
    create_default_session_globals_then(|| {
        let ann = ast::EffectAnnotation {
            effects: thin_vec![Ident::from_str("io"), Ident::from_str("net")],
            span: DUMMY_SP,
        };
        let result = to_string(|s| s.print_effect_annotation(&ann));
        assert_eq!(result, "@fx(io, net)");
    })
}

#[test]
fn test_spec_block() {
    create_default_session_globals_then(|| {
        let req_expr = mk_path_expr("x");
        let ens_expr = mk_path_expr("result");
        let spec = ast::SpecBlock {
            clauses: thin_vec![
                ast::SpecClause {
                    kind: ast::SpecClauseKind::Requires(Box::new(req_expr)),
                    span: DUMMY_SP,
                },
                ast::SpecClause {
                    kind: ast::SpecClauseKind::Ensures(Box::new(ens_expr)),
                    span: DUMMY_SP,
                },
            ],
            span: DUMMY_SP,
        };
        let result = to_string(|s| s.print_spec_block(&spec));
        assert!(result.contains("@req(x)"));
        assert!(result.contains("@ens(result)"));
    })
}

#[test]
fn test_spec_clause_perf() {
    create_default_session_globals_then(|| {
        let expr = mk_path_expr("n");
        let clause =
            ast::SpecClause { kind: ast::SpecClauseKind::Perf(Box::new(expr)), span: DUMMY_SP };
        let result = to_string(|s| s.print_spec_clause(&clause));
        assert_eq!(result, "@perf(n)");
    })
}

#[test]
fn test_spec_clause_effects() {
    create_default_session_globals_then(|| {
        let clause = ast::SpecClause {
            kind: ast::SpecClauseKind::Effects(thin_vec![
                Ident::from_str("io"),
                Ident::from_str("net"),
            ]),
            span: DUMMY_SP,
        };
        let result = to_string(|s| s.print_spec_clause(&clause));
        assert_eq!(result, "@fx(io, net)");
    })
}

#[test]
fn test_capability_block() {
    create_default_session_globals_then(|| {
        let cap = ast::CapabilityBlock {
            capabilities: thin_vec![Ident::from_str("fs_read"), Ident::from_str("net_connect"),],
            span: DUMMY_SP,
        };
        let result = to_string(|s| s.print_capability_block(&cap));
        assert_eq!(result, "@cap(fs_read, net_connect)");
    })
}

#[test]
fn test_perf_annotation_force_inline() {
    create_default_session_globals_then(|| {
        let result = to_string(|s| s.print_perf_annotation(&ast::PerfAnnotation::ForceInline));
        assert_eq!(result, "@pi!");
    })
}

#[test]
fn test_perf_annotation_no_block() {
    create_default_session_globals_then(|| {
        let result = to_string(|s| s.print_perf_annotation(&ast::PerfAnnotation::NoBlock));
        assert_eq!(result, "@pnb");
    })
}

#[test]
fn test_perf_annotation_vectorize() {
    create_default_session_globals_then(|| {
        let result = to_string(|s| s.print_perf_annotation(&ast::PerfAnnotation::Vectorize(8)));
        assert_eq!(result, "@pv(8)");
    })
}

#[test]
fn test_perf_annotation_pure() {
    create_default_session_globals_then(|| {
        let result = to_string(|s| s.print_perf_annotation(&ast::PerfAnnotation::Pure));
        assert_eq!(result, "@pp");
    })
}

#[test]
fn test_perf_annotation_repr_target_optimal() {
    create_default_session_globals_then(|| {
        let result =
            to_string(|s| s.print_perf_annotation(&ast::PerfAnnotation::ReprTargetOptimal));
        assert_eq!(result, "#[repr(target_optimal)]");
    })
}

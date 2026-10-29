use clippy_utils::diagnostics::{span_lint_and_help, span_lint_and_note};
use clippy_utils::macros::{is_panic, root_macro_call};
use clippy_utils::res::MaybeDef as _;
use clippy_utils::visitors::is_local_used;
use clippy_utils::{higher, is_in_const_context, is_wild, peel_blocks, peel_blocks_with_stmt};
use rustc_attr_ir::LangItem;
use rustc_hir::def::Res;
use rustc_hir::{Arm, Expr, ExprKind, HirId, LetStmt, Pat, PatKind, QPath, intravisit};
use rustc_lint::LateContext;
use rustc_span::symbol::{kw, sym};

use super::MATCH_WILD_ERR_ARM;

pub(crate) fn check<'tcx>(cx: &LateContext<'tcx>, ex: &Expr<'tcx>, arms: &[Arm<'tcx>]) {
    // `unwrap`/`expect` is not (yet) const, so we want to allow this in const contexts for now
    if is_in_const_context(cx) {
        return;
    }

    let ex_ty = cx.typeck_results().expr_ty(ex).peel_refs();
    if ex_ty.is_diag_item(cx, sym::Result) {
        for arm in arms {
            if let PatKind::TupleStruct(ref path, inner, _) = arm.pat.kind {
                let path_str = rustc_hir_pretty::qpath_to_string(
                    #[allow(trivial_casts)]
                    &(&cx.tcx as &dyn intravisit::HirTyCtxt<'_>),
                    path,
                );
                if path_str == "Err" {
                    let mut matching_wild = inner.iter().any(is_wild);
                    let mut ident_bind_name = kw::Underscore;
                    if !matching_wild {
                        // Looking for unused bindings (i.e.: `_e`)
                        for pat in inner {
                            if let PatKind::Binding(_, id, ident, None) = pat.kind
                                && ident.as_str().starts_with('_')
                                && !is_local_used(cx, arm.body, id)
                            {
                                ident_bind_name = ident.name;
                                matching_wild = true;
                            }
                        }
                    }
                    if matching_wild
                        && let Some(macro_call) = root_macro_call(peel_blocks_with_stmt(arm.body).span)
                        && is_panic(cx, macro_call.def_id)
                    {
                        // `Err(_)` or `Err(_e)` arm with `panic!` found
                        span_lint_and_note(
                            cx,
                            MATCH_WILD_ERR_ARM,
                            arm.pat.span,
                            format!("`Err({ident_bind_name})` matches all errors"),
                            None,
                            "match each error separately or use the error output, or use `.expect(msg)` if the error case is unreachable",
                        );
                    }
                }
            }
        }
    }
}

fn is_result_ctor_pattern(cx: &LateContext<'_>, pat: &Pat<'_>, item: LangItem) -> bool {
    if let PatKind::TupleStruct(ref qpath, ..) = pat.kind {
        cx.qpath_res(qpath, pat.hir_id).ctor_parent(cx).is_lang_item(cx, item)
    } else {
        false
    }
}

fn is_ok_pattern(cx: &LateContext<'_>, pat: &Pat<'_>) -> bool {
    is_result_ctor_pattern(cx, pat, LangItem::ResultOk)
}

fn is_err_pattern(cx: &LateContext<'_>, pat: &Pat<'_>) -> bool {
    is_result_ctor_pattern(cx, pat, LangItem::ResultErr)
}

/// If `expr` is a path to a bare local binding (e.g. `sc`), returns its `HirId`.
///
/// Intentionally does not look through field/index projections, so two
/// different places rooted at the same local (`x.a` vs `x.b`) are not treated
/// as equal.
fn as_local(expr: &Expr<'_>) -> Option<HirId> {
    if let ExprKind::Path(QPath::Resolved(None, path)) = expr.kind
        && let Res::Local(hir_id) = path.res
    {
        Some(hir_id)
    } else {
        None
    }
}

/// Returns `true` when the error of an `if let Ok(..) = <local>` is bound in a
/// chained `else if let Err(..) = <same local>` arm.
///
/// In that case the same `Result` value is examined in both arms and the `Err`
/// is bound, so the error is not discarded and the construct should not lint.
/// The scrutinees must be the *same* local binding: two separate calls such as
/// `if let Ok(_) = f() { .. } else if let Err(e) = f() { .. }` are distinct
/// evaluations, and the first call's error really is discarded.
fn err_bound_in_else_chain(cx: &LateContext<'_>, if_let: &higher::IfLet<'_>) -> bool {
    if let Some(else_expr) = if_let.if_else
        && let Some(else_if_let) = higher::IfLet::hir(cx, peel_blocks(else_expr))
        && is_err_pattern(cx, else_if_let.let_pat)
        && let Some(ok_scrutinee) = as_local(if_let.let_expr)
        && let Some(err_scrutinee) = as_local(else_if_let.let_expr)
    {
        ok_scrutinee == err_scrutinee
    } else {
        false
    }
}

/// Checks `if let Ok(..) = expr` and `while let Ok(..) = expr`.
pub(crate) fn check_if_while_let<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
    if let Some(if_let) = higher::IfLet::hir(cx, expr)
        && is_ok_pattern(cx, if_let.let_pat)
        && !err_bound_in_else_chain(cx, &if_let)
    {
        span_lint_and_help(
            cx,
            MATCH_WILD_ERR_ARM,
            expr.span,
            "this `if let Ok(...)` discards the `Err` variant",
            None,
            "consider using `match` and binding the `Err` value for logging or recovery",
        );
    } else if let Some(while_let) = higher::WhileLet::hir(expr)
        && is_ok_pattern(cx, while_let.let_pat)
    {
        span_lint_and_help(
            cx,
            MATCH_WILD_ERR_ARM,
            expr.span,
            "this `while let Ok(...)` discards the `Err` variant",
            None,
            "consider using `loop` + `match` and binding the `Err` value for logging or recovery",
        );
    }
}

/// Checks `let Ok(..) = expr else { .. }`.
pub(crate) fn check_let_else<'tcx>(cx: &LateContext<'tcx>, local: &'tcx LetStmt<'_>) {
    if local.els.is_some() && is_ok_pattern(cx, local.pat) {
        span_lint_and_help(
            cx,
            MATCH_WILD_ERR_ARM,
            local.span,
            "this `let Ok(...) = ... else` discards the `Err` variant",
            None,
            "consider using `match` and binding the `Err` value for logging or recovery",
        );
    }
}

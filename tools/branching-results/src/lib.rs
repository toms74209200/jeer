#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{BinOpKind, Expr, ExprKind, UnOp};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::{ExpnKind, sym};

dylint_linting::declare_late_lint! {
    pub BRANCHING_RESULTS,
    Warn,
    "a function's result is branched on to recover a fact it had already established"
}

enum Returned {
    Bool,
    OptionOrResult,
    Other,
}

enum Branching {
    Condition,
    Compared,
    Reduced,
}

fn local_call(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<Returned> {
    let def_id = match expr.kind {
        ExprKind::Call(callee, _) => match callee.kind {
            ExprKind::Path(ref qpath) => match cx.qpath_res(qpath, callee.hir_id) {
                Res::Def(DefKind::Fn | DefKind::AssocFn, def_id) => Some(def_id),
                _ => None,
            },
            _ => None,
        },
        ExprKind::MethodCall(..) => cx.typeck_results().type_dependent_def_id(expr.hir_id),
        _ => None,
    }?;
    let in_trait = cx
        .tcx
        .trait_of_assoc(def_id)
        .or(cx.tcx.trait_impl_of_assoc(def_id));
    let returned = match cx.typeck_results().expr_ty(expr).kind() {
        ty::Bool => Returned::Bool,
        ty::Adt(adt, _) => match cx.tcx.get_diagnostic_name(adt.did()) {
            Some(sym::Option | sym::Result) => Returned::OptionOrResult,
            _ => Returned::Other,
        },
        _ => Returned::Other,
    };
    match (def_id.as_local(), in_trait) {
        (Some(_), None) => Some(returned),
        _ => None,
    }
}

fn emit(cx: &LateContext<'_>, expr: &Expr<'_>, branching: Branching) {
    let message = match branching {
        Branching::Condition => {
            "the caller branches on a `bool` result to recover a fact the function had established"
        }
        Branching::Compared => {
            "the caller compares a result to recover a fact the function had established"
        }
        Branching::Reduced => {
            "the caller reduces an `Option` or `Result` to `bool` to recover a fact the function had established"
        }
    };
    span_lint_and_help(
        cx,
        BRANCHING_RESULTS,
        expr.span,
        message,
        None,
        "return the fact as a type (an enum or a type only a parse function makes) and match on it",
    );
}

fn check_condition(cx: &LateContext<'_>, expr: &Expr<'_>) {
    match expr.kind {
        ExprKind::DropTemps(inner) | ExprKind::Unary(UnOp::Not, inner) => {
            check_condition(cx, inner)
        }
        ExprKind::Binary(op, left, right) if matches!(op.node, BinOpKind::And | BinOpKind::Or) => {
            check_condition(cx, left);
            check_condition(cx, right);
        }
        _ => match local_call(cx, expr) {
            Some(Returned::Bool) => emit(cx, expr, Branching::Condition),
            Some(Returned::OptionOrResult | Returned::Other) | None => {}
        },
    }
}

impl<'tcx> LateLintPass<'tcx> for BranchingResults {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if let ExpnKind::Macro(..) = expr.span.ctxt().outer_expn_data().kind {
            return;
        }
        match expr.kind {
            ExprKind::If(cond, _, _) => check_condition(cx, cond),
            ExprKind::Match(scrutinee, arms, _) => {
                check_condition(cx, scrutinee);
                for guard in arms.iter().filter_map(|arm| arm.guard) {
                    check_condition(cx, guard);
                }
            }
            ExprKind::Binary(op, left, right)
                if matches!(op.node, BinOpKind::Eq | BinOpKind::Ne) =>
            {
                for side in [left, right] {
                    match local_call(cx, side) {
                        Some(Returned::Bool | Returned::OptionOrResult) => {
                            emit(cx, side, Branching::Compared)
                        }
                        Some(Returned::Other) | None => {}
                    }
                }
            }
            ExprKind::MethodCall(segment, receiver, [], _)
                if matches!(
                    segment.ident.as_str(),
                    "is_some" | "is_none" | "is_ok" | "is_err"
                ) =>
            {
                match local_call(cx, receiver) {
                    Some(Returned::OptionOrResult) => emit(cx, receiver, Branching::Reduced),
                    Some(Returned::Bool | Returned::Other) | None => {}
                }
            }
            _ => {}
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

use chili_ast::{ast, workspace::BindingInfoIdx};
use chili_error::DiagnosticResult;
use chili_span::Span;
use codespan_reporting::diagnostic::{Diagnostic, Label};

use crate::{
    lvalue_access::check_lvalue_access,
    sess::{InitState, Sess},
};

pub(crate) fn check_id_access(
    sess: &Sess,
    binding_info_idx: BindingInfoIdx,
    span: Span,
) -> DiagnosticResult<()> {
    if let Some(state) = sess.init_scopes.get(binding_info_idx) {
        if state.is_not_init() {
            let binding_info = sess.workspace.get_binding_info(binding_info_idx).unwrap();

            let msg = format!(
                "use of possibly uninitialized value `{}`",
                binding_info.symbol
            );

            return Err(Diagnostic::error()
                .with_message(msg.clone())
                .with_labels(vec![
                    Label::primary(span.file_id, span.range()).with_message(msg),
                    Label::secondary(binding_info.span.file_id, binding_info.span.range())
                        .with_message("defined here"),
                ]));
        }
    }

    Ok(())
}

pub(crate) fn check_assign_lvalue_id_access(
    sess: &mut Sess,
    lvalue: &ast::Expr,
    binding_info_idx: BindingInfoIdx,
) -> DiagnosticResult<()> {
    let binding_info = sess.workspace.get_binding_info(binding_info_idx).unwrap();
    let init_state = sess.init_scopes.get(binding_info_idx).unwrap();

    if init_state.is_init() && !binding_info.is_mutable {
        let msg = format!(
            "cannot assign twice to immutable variable `{}`",
            binding_info.symbol
        );
        return Err(Diagnostic::error()
            .with_message(msg.clone())
            .with_labels(vec![
                Label::primary(lvalue.span.file_id, lvalue.span.range().clone()).with_message(msg),
                Label::secondary(binding_info.span.file_id, binding_info.span.range().clone())
                    .with_message("defined here"),
            ]));
    }

    if init_state.is_init() {
        check_lvalue_access(lvalue, lvalue.span)?;
    } else {
        // set binding as init in the current scope
        *sess.init_scopes.get_mut(binding_info_idx).unwrap() = InitState::Init;
    }

    Ok(())
}

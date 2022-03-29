use crate::{
    normalize::NormalizeTy,
    ty_ctx::TyCtx,
    unify::{UnifyTyErr, UnifyTyResult},
};
use chili_error::DiagnosticResult;
use chili_span::Span;
use codespan_reporting::diagnostic::{Diagnostic, Label};

pub trait DisplayTy {
    fn display(&self, tycx: &TyCtx) -> String;
}

impl<T: NormalizeTy> DisplayTy for T {
    fn display(&self, tycx: &TyCtx) -> String {
        self.normalize(tycx).to_string()
    }
}

pub(crate) trait OrReportErr {
    fn or_report_err(
        self,
        tycx: &TyCtx,
        expected: impl DisplayTy,
        found: impl DisplayTy,
        span: Span,
    ) -> DiagnosticResult<()>;
}

impl OrReportErr for UnifyTyResult {
    fn or_report_err(
        self,
        tycx: &TyCtx,
        expected: impl DisplayTy,
        found: impl DisplayTy,
        span: Span,
    ) -> DiagnosticResult<()> {
        self.map_err(|e| {
            let expected = expected.display(tycx);
            let found = found.display(tycx);

            match e {
                UnifyTyErr::Mismatch => Diagnostic::error()
                    .with_message(format!(
                        "mismatched types - expected {}, but found {}",
                        expected, found
                    ))
                    .with_labels(vec![Label::primary(span.file_id, span.range().clone())
                        .with_message(format!("expected {}", expected))]),
                UnifyTyErr::Occurs => Diagnostic::error()
                    .with_message(format!(
                        "found recursive type - {} is equal to {}",
                        expected, found
                    ))
                    .with_labels(vec![Label::primary(span.file_id, span.range().clone())
                        .with_message(format!("expected {}", expected))]),
            }
        })
    }
}

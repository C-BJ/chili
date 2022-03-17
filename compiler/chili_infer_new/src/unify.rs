use crate::{
    normalize::NormalizeTy,
    tycx::{TyBinding, TyContext},
};
use chili_ast::{ty::*, workspace::Workspace};
use chili_span::Span;

pub(crate) type TyUnifyResult = Result<(), TyUnifyErr>;

#[derive(Debug)]
pub(crate) enum TyUnifyErr {
    Mismatch(TyKind, TyKind),
    Occurs(TyKind, TyKind),
}

pub(crate) trait Unify<T>
where
    Self: Sized,
    T: Sized,
{
    fn unify(
        &self,
        other: &T,
        tycx: &mut TyContext,
        workspace: &Workspace,
        span: Span,
    ) -> TyUnifyResult;
}

impl Unify<Ty> for Ty {
    fn unify(
        &self,
        other: &Ty,
        tycx: &mut TyContext,
        workspace: &Workspace,
        span: Span,
    ) -> TyUnifyResult {
        let t1 = TyKind::Var(*self);
        let t2 = TyKind::Var(*other);
        t1.unify(&t2, tycx, workspace, span)
    }
}

impl Unify<TyKind> for Ty {
    fn unify(
        &self,
        other: &TyKind,
        tycx: &mut TyContext,
        workspace: &Workspace,
        span: Span,
    ) -> TyUnifyResult {
        let ty = TyKind::Var(*self);
        ty.unify(other, tycx, workspace, span)
    }
}

impl Unify<Ty> for TyKind {
    fn unify(
        &self,
        other: &Ty,
        tycx: &mut TyContext,
        workspace: &Workspace,
        span: Span,
    ) -> TyUnifyResult {
        let other = TyKind::Var(*other);
        self.unify(&other, tycx, workspace, span)
    }
}

impl Unify<TyKind> for TyKind {
    fn unify(
        &self,
        other: &TyKind,
        tycx: &mut TyContext,
        workspace: &Workspace,
        span: Span,
    ) -> TyUnifyResult {
        match (self, other) {
            (TyKind::Unit, TyKind::Unit) => Ok(()),
            (TyKind::Bool, TyKind::Bool) => Ok(()),
            (TyKind::Int(t1), TyKind::Int(t2)) if t1 == t2 => Ok(()),
            (TyKind::UInt(t1), TyKind::UInt(t2)) if t1 == t2 => Ok(()),
            (TyKind::Float(t1), TyKind::Float(t2)) if t1 == t2 => Ok(()),

            (TyKind::AnyInt(var), ty @ TyKind::Int(_))
            | (ty @ TyKind::Int(_), TyKind::AnyInt(var))
            | (TyKind::AnyInt(var), ty @ TyKind::UInt(_))
            | (ty @ TyKind::UInt(_), TyKind::AnyInt(var))
            | (TyKind::AnyInt(var), ty @ TyKind::Float(_))
            | (ty @ TyKind::Float(_), TyKind::AnyInt(var))
            | (TyKind::AnyFloat(var), ty @ TyKind::Float(_))
            | (ty @ TyKind::Float(_), TyKind::AnyFloat(var)) => {
                tycx.bind(*var, ty.clone());
                Ok(())
            }

            (TyKind::Var(var), _) => unify_var_type(*var, self, other, tycx, workspace, span),
            (_, TyKind::Var(var)) => unify_var_type(*var, other, self, tycx, workspace, span),

            (TyKind::Never, _) | (_, TyKind::Never) => Ok(()),

            _ => Err(TyUnifyErr::Mismatch(self.clone(), other.clone())),
        }
    }
}

fn unify_var_type(
    var: Ty,
    t1: &TyKind,
    t2: &TyKind,
    tycx: &mut TyContext,
    workspace: &Workspace,
    span: Span,
) -> TyUnifyResult {
    match tycx.find_type_binding(var) {
        TyBinding::Bound(t) => t.unify(t2, tycx, workspace, span),
        TyBinding::Unbound => {
            let normalized = t2.normalize(tycx);

            if *t1 != normalized {
                if occurs(var, &normalized, tycx, workspace) {
                    Err(TyUnifyErr::Occurs(t1.clone(), t2.clone()))
                } else {
                    tycx.bind(var, normalized);
                    Ok(())
                }
            } else {
                Ok(())
            }
        }
    }
}

fn occurs(var: Ty, ty: &TyKind, tycx: &TyContext, workspace: &Workspace) -> bool {
    match ty {
        TyKind::Var(other) => match tycx.find_type_binding(*other) {
            TyBinding::Bound(ty) => occurs(var, &ty, tycx, workspace),
            TyBinding::Unbound => var == *other,
        },
        TyKind::Fn(f) => {
            f.params.iter().any(|p| occurs(var, &p.ty, tycx, workspace))
                || occurs(var, &f.ret, tycx, workspace)
        }
        TyKind::Pointer(ty, _)
        | TyKind::MultiPointer(ty, _)
        | TyKind::Array(ty, _)
        | TyKind::Slice(ty, _) => occurs(var, ty, tycx, workspace),
        TyKind::Tuple(tys) => tys.iter().any(|ty| occurs(var, ty, tycx, workspace)),
        TyKind::Struct(st) => st
            .fields
            .iter()
            .any(|f| occurs(var, &f.ty, tycx, workspace)),
        _ => false,
    }
}

// NOTE (Ron): checks that mutability rules are equal
fn can_coerce_mut(from: bool, to: bool) -> bool {
    from == to || (!from && to)
}

use crate::Resolver;
use chili_ast::{
    ast::{self, BindingKind, ForeignLibrary, Visibility},
    workspace::Workspace,
};
use chili_error::{DiagnosticResult, SyntaxError};
use chili_span::Span;
use codespan_reporting::diagnostic::{Diagnostic, Label};
use ustr::UstrMap;

// Trait for resolving binding/import uses
pub(crate) trait Resolve<'w> {
    fn resolve(
        &mut self,
        resolver: &mut Resolver,
        workspace: &mut Workspace<'w>,
    ) -> DiagnosticResult<()>;
}

impl<'w, T: Resolve<'w>> Resolve<'w> for Vec<T> {
    fn resolve(
        &mut self,
        resolver: &mut Resolver,
        workspace: &mut Workspace<'w>,
    ) -> DiagnosticResult<()> {
        for element in self {
            element.resolve(resolver, workspace)?;
        }
        Ok(())
    }
}

impl<'w, T: Resolve<'w>> Resolve<'w> for Option<T> {
    fn resolve(
        &mut self,
        resolver: &mut Resolver,
        workspace: &mut Workspace<'w>,
    ) -> DiagnosticResult<()> {
        if let Some(e) = self {
            e.resolve(resolver, workspace)?;
        }
        Ok(())
    }
}

impl<'w, T: Resolve<'w>> Resolve<'w> for Box<T> {
    fn resolve(
        &mut self,
        resolver: &mut Resolver,
        workspace: &mut Workspace<'w>,
    ) -> DiagnosticResult<()> {
        self.as_mut().resolve(resolver, workspace)
    }
}

impl<'w> Resolve<'w> for ast::Ast {
    fn resolve(
        &mut self,
        resolver: &mut Resolver,
        workspace: &mut Workspace<'w>,
    ) -> DiagnosticResult<()> {
        self.imports.resolve(resolver, workspace)?;
        self.bindings.resolve(resolver, workspace)?;
        Ok(())
    }
}

impl<'w> Resolve<'w> for ast::Import {
    fn resolve(
        &mut self,
        resolver: &mut Resolver,
        workspace: &mut Workspace<'w>,
    ) -> DiagnosticResult<()> {
        if !resolver.in_global_scope() {
            self.module_idx =
                workspace.find_module_info(self.module_info).unwrap();

            self.binding_info_idx = resolver.add_binding(
                workspace,
                self.alias,
                self.visibility,
                false,
                BindingKind::Import,
                self.span,
                true,
            );
        }

        Ok(())
    }
}

impl<'w> Resolve<'w> for ast::Binding {
    fn resolve(
        &mut self,
        resolver: &mut Resolver,
        workspace: &mut Workspace<'w>,
    ) -> DiagnosticResult<()> {
        self.ty_expr.resolve(resolver, workspace)?;
        self.value.resolve(resolver, workspace)?;

        // Collect foreign libraries to be linked later
        if let Some(lib) = self.lib_name {
            workspace.foreign_libraries.insert(ForeignLibrary::from_str(
                &lib,
                resolver.module_info.file_path,
                self.pattern.span(),
            )?);
        }

        if !resolver.in_global_scope() {
            resolver.add_binding_with_pattern(
                workspace,
                &mut self.pattern,
                self.visibility,
                self.kind,
                false,
            );
        }

        Ok(())
    }
}

impl<'w> Resolve<'w> for ast::Expr {
    fn resolve(
        &mut self,
        resolver: &mut Resolver,
        workspace: &mut Workspace<'w>,
    ) -> DiagnosticResult<()> {
        match &mut self.kind {
            ast::ExprKind::Import(imports) => {
                for import in imports {
                    import.resolve(resolver, workspace)?;
                }
            }
            ast::ExprKind::Foreign(bindings) => {
                for binding in bindings {
                    binding.resolve(resolver, workspace)?;
                }
            }
            ast::ExprKind::Binding(binding) => {
                binding.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Defer(expr) => {
                expr.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Assign { lvalue, rvalue } => {
                lvalue.resolve(resolver, workspace)?;
                rvalue.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Cast(cast) => {
                cast.expr.resolve(resolver, workspace)?;
                cast.type_expr.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Builtin(builtin) => match builtin {
                ast::Builtin::SizeOf(expr) | ast::Builtin::AlignOf(expr) => {
                    expr.resolve(resolver, workspace)?;
                }
                ast::Builtin::Panic(msg) => {
                    msg.resolve(resolver, workspace)?;
                }
            },
            ast::ExprKind::Fn(f) => {
                f.resolve(resolver, workspace)?;
            }
            ast::ExprKind::While { cond, expr } => {
                cond.resolve(resolver, workspace)?;
                expr.resolve(resolver, workspace)?;
            }
            ast::ExprKind::For {
                iter_name,
                iter_index_name,
                iterator,
                expr,
            } => {
                // TODO: add iter_id
                // TODO: add iter_ty
                // TODO: add iter_index_id
                // TODO: add iter_name to current scope
                // TODO: add iter_index_name to current scope

                match iterator {
                    ast::ForIter::Range(start, end) => {
                        start.resolve(resolver, workspace)?;
                        end.resolve(resolver, workspace)?;
                    }
                    ast::ForIter::Value(value) => {
                        value.resolve(resolver, workspace)?;
                    }
                }

                expr.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Break { deferred } => {
                deferred.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Continue { deferred } => {
                deferred.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Return { expr, deferred } => {
                expr.resolve(resolver, workspace)?;
                deferred.resolve(resolver, workspace)?;
            }
            ast::ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                cond.resolve(resolver, workspace)?;
                then_expr.resolve(resolver, workspace)?;
                else_expr.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Block(block) => {
                block.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Binary { lhs, op: _, rhs } => {
                lhs.resolve(resolver, workspace)?;
                rhs.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Unary { op: _, lhs } => {
                lhs.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Subscript { expr, index } => {
                expr.resolve(resolver, workspace)?;
                index.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Slice { expr, low, high } => {
                expr.resolve(resolver, workspace)?;
                low.resolve(resolver, workspace)?;
                high.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Call(call) => {
                call.resolve(resolver, workspace)?;
            }
            ast::ExprKind::MemberAccess { expr, member: _ } => {
                expr.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Id {
                symbol,
                is_mutable: _,
                binding_span: _,
                binding_info_idx,
            } => match resolver.lookup_binding(workspace, *symbol) {
                Some(id) => {
                    let info = workspace.get_binding_info(id).unwrap();

                    if info.level < resolver.function_scope_level {
                        return Err(Diagnostic::error()
                            .with_message(
                                "can't capture dynamic environment in a fn",
                            )
                            .with_labels(vec![Label::primary(
                                self.span.file_id,
                                self.span.range().clone(),
                            )]));
                    }

                    *binding_info_idx = id
                }
                None => {
                    return Err(Diagnostic::error()
                        .with_message(format!(
                            "cannot find value `{}` in this scope",
                            symbol
                        ))
                        .with_labels(vec![Label::primary(
                            self.span.file_id,
                            self.span.range(),
                        )
                        .with_message("not found in this scope")]))
                }
            },
            ast::ExprKind::ArrayLiteral(kind) => match kind {
                ast::ArrayLiteralKind::List(list) => {
                    list.resolve(resolver, workspace)?;
                }
                ast::ArrayLiteralKind::Fill { len, expr } => {
                    len.resolve(resolver, workspace)?;
                    expr.resolve(resolver, workspace)?;
                }
            },
            ast::ExprKind::TupleLiteral(lit) => {
                lit.resolve(resolver, workspace)?;
            }
            ast::ExprKind::StructLiteral { type_expr, fields } => {
                type_expr.resolve(resolver, workspace)?;

                for field in fields {
                    field.value.resolve(resolver, workspace)?;
                }
            }
            ast::ExprKind::PointerType(inner, _) => {
                inner.resolve(resolver, workspace)?;
            }
            ast::ExprKind::MultiPointerType(inner, _) => {
                inner.resolve(resolver, workspace)?;
            }
            ast::ExprKind::ArrayType(inner, _) => {
                inner.resolve(resolver, workspace)?;
            }
            ast::ExprKind::SliceType(inner, _) => {
                inner.resolve(resolver, workspace)?;
            }
            ast::ExprKind::StructType(typ) => {
                let mut field_map = UstrMap::default();

                for field in typ.fields.iter_mut() {
                    field.ty.resolve(resolver, workspace)?;

                    if let Some(already_defined_span) =
                        field_map.insert(field.name, field.span)
                    {
                        return Err(SyntaxError::duplicate_symbol(
                            already_defined_span,
                            field.span,
                            field.name,
                        ));
                    }
                }
            }
            ast::ExprKind::FnType(proto) => {
                for param in proto.params.iter_mut() {
                    param.ty.resolve(resolver, workspace)?;
                }

                proto.ret.resolve(resolver, workspace)?;
            }
            ast::ExprKind::Literal(_)
            | ast::ExprKind::SelfType
            | ast::ExprKind::NeverType
            | ast::ExprKind::UnitType
            | ast::ExprKind::PlaceholderType
            | ast::ExprKind::Noop => (),
        }

        Ok(())
    }
}

impl<'w> Resolve<'w> for ast::Block {
    fn resolve(
        &mut self,
        resolver: &mut Resolver,
        workspace: &mut Workspace<'w>,
    ) -> DiagnosticResult<()> {
        resolver.push_scope();

        self.exprs.resolve(resolver, workspace)?;
        self.deferred.resolve(resolver, workspace)?;

        resolver.pop_scope();

        Ok(())
    }
}

impl<'w> Resolve<'w> for ast::Fn {
    fn resolve(
        &mut self,
        resolver: &mut Resolver,
        workspace: &mut Workspace<'w>,
    ) -> DiagnosticResult<()> {
        let old_scope_level = resolver.function_scope_level;

        let mut param_map = UstrMap::default();

        for param in self.proto.params.iter_mut() {
            param.ty.resolve(resolver, workspace)?;

            resolver.add_binding_with_pattern(
                workspace,
                &mut param.pattern,
                Visibility::Private,
                BindingKind::Let,
                false,
            );

            for pat in param.pattern.symbols() {
                if let Some(already_defined_span) =
                    param_map.insert(pat.symbol, pat.span)
                {
                    return Err(SyntaxError::duplicate_symbol(
                        already_defined_span,
                        pat.span,
                        pat.symbol,
                    ));
                }
            }
        }

        self.proto.ret.resolve(resolver, workspace)?;

        resolver.push_named_scope(self.proto.name);
        resolver.function_scope_level = resolver.scope_level;

        self.body.resolve(resolver, workspace)?;

        resolver.pop_scope();
        resolver.function_scope_level = old_scope_level;

        if !self.proto.name.is_empty() {
            resolver.add_binding(
                workspace,
                self.proto.name,
                Visibility::Private,
                false,
                BindingKind::Let,
                Span::unknown(),
                true,
            );
        }

        Ok(())
    }
}

impl<'w> Resolve<'w> for ast::Call {
    fn resolve(
        &mut self,
        resolver: &mut Resolver,
        workspace: &mut Workspace<'w>,
    ) -> DiagnosticResult<()> {
        self.callee.resolve(resolver, workspace)?;

        // TODO: check there are no duplicate arguments
        for arg in self.args.iter_mut() {
            arg.value.resolve(resolver, workspace)?;
        }

        Ok(())
    }
}

use std::path::PathBuf;

use crate::Parser;
use chilic_error::{DiagnosticResult, SyntaxError};
use chilic_ir::{
    entity::Visibility,
    module::ModuleInfo,
    path::AsModuleName,
    use_decl::{UseDecl, UsePath, UsePathNode},
};
use chilic_span::{Span, Spanned};
use chilic_token::TokenType::*;
use codespan_reporting::diagnostic::{Diagnostic, Label};
use common::{
    builtin::{MOD_FILE_NAME, SOURCE_FILE_EXT},
    compiler_info,
};
use ustr::{ustr, Ustr};

impl Parser {
    pub(crate) fn parse_use(
        &mut self,
        visibility: Visibility,
    ) -> DiagnosticResult<Vec<UseDecl>> {
        let uses = self.parse_use_internal(visibility)?;

        uses.iter().for_each(|use_| {
            self.used_modules.push(use_.module_info);
        });

        Ok(uses)
    }

    fn parse_use_internal(
        &mut self,
        visibility: Visibility,
    ) -> DiagnosticResult<Vec<UseDecl>> {
        if self.match_one(Tilde) {
            self.consume(Dot)?;
            todo!("implement `from_root` use: `use ~.foo.bar`");
        }

        let id_token = self.consume_id()?.clone();
        let name = id_token.symbol().as_str();

        match name {
            compiler_info::STD => {
                let info = compiler_info::std_module_info();

                let path_buf = PathBuf::from(info.file_path.as_str());

                let module = info.name;
                let alias = info.name;

                self.parse_use_postfix(
                    path_buf,
                    module,
                    alias,
                    visibility,
                    id_token.span.clone(),
                )
            }
            _ => {
                let mut path_buf = PathBuf::from(&self.current_dir);
                path_buf.push(name);

                let module = path_buf.as_module_name(&self.root_dir);
                let module = ustr(&module);
                let alias = ustr(name);

                if path_buf.with_extension(SOURCE_FILE_EXT).is_file() {
                    path_buf.set_extension(SOURCE_FILE_EXT);
                    check_path_is_under_root_or_std(
                        &self.root_dir,
                        &path_buf,
                        &id_token.span,
                    )?;

                    self.parse_use_postfix(
                        path_buf,
                        module,
                        alias,
                        visibility,
                        id_token.span,
                    )
                } else if path_buf.is_dir() {
                    check_path_is_under_root_or_std(
                        &self.root_dir,
                        &path_buf,
                        &id_token.span,
                    )?;

                    let mut mod_path = path_buf.clone();

                    mod_path.push(MOD_FILE_NAME);
                    mod_path.set_extension(SOURCE_FILE_EXT);

                    if mod_path.exists() && mod_path.is_file() {
                        self.parse_use_postfix(
                            mod_path,
                            module,
                            alias,
                            visibility,
                            id_token.span,
                        )
                    } else {
                        Err(module_not_found_err(
                            &path_buf,
                            &module,
                            &id_token.span,
                        ))
                    }
                } else {
                    Err(module_not_found_err(
                        &path_buf,
                        &module,
                        &id_token.span,
                    ))
                }
            }
        }
    }

    fn parse_use_postfix(
        &mut self,
        path_buf: PathBuf,
        module: Ustr,
        alias: Ustr,
        visibility: Visibility,
        module_name_span: Span,
    ) -> DiagnosticResult<Vec<UseDecl>> {
        let uses = self.parse_use_postfix_internal(
            ustr(path_buf.to_str().unwrap()),
            module,
            alias,
            visibility,
            module_name_span,
            &mut vec![],
        )?;

        Ok(uses)
    }

    fn parse_use_postfix_internal(
        &mut self,
        path: Ustr,
        module: Ustr,
        alias: Ustr,
        visibility: Visibility,
        span: Span,
        use_path: &mut UsePath,
    ) -> DiagnosticResult<Vec<UseDecl>> {
        if self.match_one(Dot) {
            if self.match_id() {
                // single child, i.e: `use other.foo`

                let id_token = self.previous();
                let id_token_span = id_token.span.clone();
                let alias = id_token.symbol();

                use_path.push(Spanned::new(
                    UsePathNode::Symbol(alias),
                    id_token.span.clone(),
                ));

                self.parse_use_postfix_internal(
                    path,
                    module,
                    alias,
                    visibility,
                    id_token_span,
                    use_path,
                )
            } else if self.match_one(OpenCurly) {
                // multiple children, i.e: `use other.{foo, bar}`

                let mut uses = vec![];

                while !self.match_one(CloseCurly) {
                    let id_token = self.consume_id()?.clone();
                    let alias = id_token.symbol();

                    let mut local_use_path = use_path.clone();
                    local_use_path.push(Spanned::new(
                        UsePathNode::Symbol(alias),
                        id_token.span.clone(),
                    ));

                    let use_decl = self.parse_use_postfix_internal(
                        path,
                        module,
                        alias,
                        visibility,
                        id_token.span,
                        &mut local_use_path,
                    )?;

                    uses.extend(use_decl);

                    if !self.match_one(Comma) {
                        self.consume(CloseCurly)?;
                        break;
                    }
                }

                Ok(uses)
            } else if self.match_one(QuestionMark) {
                use_path.push(Spanned::new(
                    UsePathNode::Wildcard,
                    self.previous().span.clone(),
                ));
                Ok(vec![UseDecl {
                    module_info: ModuleInfo::new(module, path),
                    alias: ustr(""),
                    use_path: use_path.clone(),
                    visibility,
                    span,
                }])
            } else {
                Err(SyntaxError::expected(
                    self.span_ref(),
                    "an identifier, { or ?",
                ))
            }
        } else {
            let alias = if self.match_one(Colon) {
                self.consume_id()?.symbol()
            } else {
                alias
            };

            Ok(vec![UseDecl {
                module_info: ModuleInfo::new(module, path),
                alias,
                use_path: use_path.clone(),
                visibility,
                span,
            }])
        }
    }
}

fn module_not_found_err(
    path_buf: &PathBuf,
    module: &str,
    span: &Span,
) -> Diagnostic<usize> {
    Diagnostic::error()
        .with_message(format!("couldn't find module `{}`", module))
        .with_labels(vec![Label::primary(span.file_id, span.range.clone())])
        .with_notes(vec![format!(
            "tried to resolve this path: {}",
            path_buf.display()
        )])
}

fn check_path_is_under_root_or_std(
    root_path: &str,
    path_buf: &PathBuf,
    span: &Span,
) -> DiagnosticResult<()> {
    if path_buf.starts_with(root_path)
        || path_buf.starts_with(compiler_info::std_module_root_dir())
    {
        Ok(())
    } else {
        Err(Diagnostic::error()
            .with_message("cannot use modules outside of the root module scope")
            .with_labels(vec![Label::primary(
                span.file_id,
                span.range.clone(),
            )]))
    }
}

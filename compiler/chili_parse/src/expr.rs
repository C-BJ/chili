use std::vec;

use crate::*;
use chili_ast::{
    ast::{
        self, BinaryOp, Block, BuiltinKind, Expr, ExprKind, ForIter, NameAndId, UnaryOp, Visibility,
    },
    ty::StructTyKind,
};
use chili_error::{
    diagnostic::{Diagnostic, Label},
    *,
};
use chili_span::{Span, To};
use chili_token::TokenKind::*;
use ustr::ustr;

macro_rules! parse_binary {
    ($parser:expr, pattern = $(|) ? $($pattern : pat_param) | +, next_fn = $next:expr) => {{
        let mut expr = $next($parser)?;
        let start_span = expr.span;

        while eat!($parser, $( $pattern )|+) {
            let op: BinaryOp = $parser.previous().kind.into();
            let rhs = $next($parser)?;
            let span = start_span.to($parser.previous_span());

            expr = Expr::new(
                ExprKind::Binary(ast::Binary {
                    lhs: Box::new(expr),
                    op,
                    rhs: Box::new(rhs),
                    span,
                }),
                span,
            );
        }

        Ok(expr)
    }};
}

impl Parser {
    pub(crate) fn parse_expr(&mut self) -> DiagnosticResult<Expr> {
        self.with_res(Restrictions::empty(), |p| p.parse_expr_inner(ustr("")))
    }

    pub(crate) fn parse_expr_with_res(
        &mut self,
        restrictions: Restrictions,
    ) -> DiagnosticResult<Expr> {
        self.with_res(restrictions, |p| p.parse_expr_inner(ustr("")))
    }

    pub(crate) fn parse_decl_expr(&mut self, decl_name: Ustr) -> DiagnosticResult<Expr> {
        self.with_res(Restrictions::empty(), |p| p.parse_expr_inner(decl_name))
    }

    fn parse_expr_inner(&mut self, decl_name: Ustr) -> DiagnosticResult<Expr> {
        let is_stmt = self.restrictions.contains(Restrictions::STMT_EXPR);

        self.decl_name_frames.push(decl_name);

        let expr = if is_stmt {
            if eat!(self, Defer) {
                let span = self.span();
                let expr = self.parse_expr()?;
                Ok(Expr::new(
                    ExprKind::Defer(ast::Defer {
                        expr: Box::new(expr),
                    }),
                    span,
                ))
            } else if eat!(self, Let) {
                let start_span = self.previous_span();

                if eat!(self, Extern) {
                    let binding = self.parse_extern(Visibility::Private, start_span)?;

                    Ok(Expr::new(
                        ExprKind::Extern(vec![binding]),
                        start_span.to(self.previous_span()),
                    ))
                } else {
                    let binding = self.parse_binding(Visibility::Private, false)?;

                    Ok(Expr::new(
                        ExprKind::Binding(Box::new(binding)),
                        start_span.to(self.previous_span()),
                    ))
                }
            } else if eat!(self, Extern) {
                let start_span = self.previous_span();
                let bindings = self.parse_extern_block()?;

                Ok(Expr::new(
                    ExprKind::Extern(bindings),
                    start_span.to(self.previous_span()),
                ))
            } else {
                self.parse_logic_or()
            }
        } else {
            self.parse_logic_or()
        }?;

        let expr = self.parse_postfix_expr(expr)?;

        self.decl_name_frames.pop();

        Ok(expr)
    }

    pub(crate) fn parse_if(&mut self) -> DiagnosticResult<Expr> {
        let token = self.previous();
        let span = token.span;

        let cond = self.parse_expr_with_res(Restrictions::NO_STRUCT_LITERAL)?;

        require!(self, OpenCurly, "{")?;
        let then = self.parse_block_expr()?;

        let otherwise = if eat!(self, Else) {
            let expr = if eat!(self, If) {
                self.parse_if()?
            } else {
                require!(self, OpenCurly, "{")?;
                self.parse_block_expr()?
            };

            Some(Box::new(expr))
        } else {
            None
        };

        Ok(Expr::new(
            ExprKind::If(ast::If {
                cond: Box::new(cond),
                then: Box::new(then),
                otherwise,
            }),
            span.to(self.previous_span()),
        ))
    }

    pub(crate) fn parse_block(&mut self) -> DiagnosticResult<Block> {
        let start_span = self.previous_span();

        let mut exprs = vec![];
        let mut yields = false;

        while !eat!(self, CloseCurly) && !self.is_end() {
            self.skip_semicolons();

            exprs.push(self.parse_expr_with_res(Restrictions::STMT_EXPR)?);

            if eat!(self, Semicolon) {
                yields = false;
                continue;
            } else if eat!(self, CloseCurly) {
                yields = true;
                break;
            } else if exprs.last().map_or(false, expr_doesnt_require_semicolon) {
                continue;
            } else {
                let span = Parser::get_missing_delimiter_span(self.previous_span());
                return Err(SyntaxError::expected(span, ";"));
            }
        }

        let span = start_span.to(self.previous_span());

        Ok(Block {
            exprs,
            deferred: vec![],
            yields,
            span,
        })
    }

    pub(crate) fn parse_block_expr(&mut self) -> DiagnosticResult<Expr> {
        let block = self.parse_block()?;
        let span = block.span;
        Ok(Expr::new(ExprKind::Block(block), span))
    }

    pub(crate) fn parse_logic_or(&mut self) -> DiagnosticResult<Expr> {
        parse_binary!(self, pattern = BarBar, next_fn = Parser::parse_logic_and)
    }

    pub(crate) fn parse_logic_and(&mut self) -> DiagnosticResult<Expr> {
        parse_binary!(self, pattern = AmpAmp, next_fn = Parser::parse_comparison)
    }

    pub(crate) fn parse_comparison(&mut self) -> DiagnosticResult<Expr> {
        parse_binary!(
            self,
            pattern = BangEq | EqEq | Gt | GtEq | Lt | LtEq,
            next_fn = Parser::parse_bitwise_or
        )
    }

    pub(crate) fn parse_bitwise_or(&mut self) -> DiagnosticResult<Expr> {
        parse_binary!(self, pattern = Bar, next_fn = Parser::parse_bitwise_xor)
    }

    pub(crate) fn parse_bitwise_xor(&mut self) -> DiagnosticResult<Expr> {
        parse_binary!(self, pattern = Caret, next_fn = Parser::parse_bitwise_and)
    }

    pub(crate) fn parse_bitwise_and(&mut self) -> DiagnosticResult<Expr> {
        parse_binary!(self, pattern = Amp, next_fn = Parser::parse_bitshift)
    }

    pub(crate) fn parse_bitshift(&mut self) -> DiagnosticResult<Expr> {
        parse_binary!(self, pattern = LtLt | GtGt, next_fn = Parser::parse_term)
    }

    pub(crate) fn parse_term(&mut self) -> DiagnosticResult<Expr> {
        parse_binary!(self, pattern = Minus | Plus, next_fn = Parser::parse_factor)
    }

    pub(crate) fn parse_factor(&mut self) -> DiagnosticResult<Expr> {
        parse_binary!(
            self,
            pattern = Star | FwSlash | Percent,
            next_fn = Parser::parse_unary
        )
    }

    pub(crate) fn parse_unary(&mut self) -> DiagnosticResult<Expr> {
        if eat!(self, Amp | AmpAmp | Bang | Minus | Plus) {
            let start_span = self.previous_span();
            let token = self.previous().kind;

            let op = match token {
                Amp => UnaryOp::Ref(eat!(self, Mut)),
                Star => UnaryOp::Deref,
                Minus => UnaryOp::Neg,
                Plus => UnaryOp::Plus,
                Bang => UnaryOp::Not,
                t => panic!("{} is not a unary op", t),
            };

            let lhs = self.parse_unary()?;

            let span = start_span.to(self.previous_span());

            let expr = Expr::new(
                ExprKind::Unary(ast::Unary {
                    op,
                    lhs: Box::new(lhs),
                    span,
                }),
                span,
            );

            Ok(expr)
        } else {
            self.parse_primary()
        }
    }

    pub(crate) fn parse_primary(&mut self) -> DiagnosticResult<Expr> {
        let expr = if eat!(self, Ident(_)) {
            const SELF_SYMBOL: &str = "Self";

            let token = self.previous();
            let symbol = token.symbol();

            let kind = if symbol == SELF_SYMBOL {
                ExprKind::SelfType
            } else {
                ExprKind::Ident(ast::Ident {
                    symbol,
                    binding_info_id: Default::default(),
                })
            };

            Expr::new(kind, token.span)
        } else if eat!(self, Placeholder) {
            Expr::new(ExprKind::Placeholder, self.previous_span())
        } else if eat!(self, Star) {
            let start_span = self.previous_span();
            let is_mutable = eat!(self, Mut);
            let expr = self.parse_expr()?;

            Expr::new(
                ExprKind::PointerType(ast::ExprAndMut {
                    inner: Box::new(expr),
                    is_mutable,
                }),
                start_span.to(self.previous_span()),
            )
        } else if eat!(self, If) {
            self.parse_if()?
        } else if eat!(self, While) {
            self.parse_while()?
        } else if eat!(self, For) {
            self.parse_for()?
        } else if eat!(self, OpenCurly) {
            self.parse_block_expr()?
        } else if eat!(self, OpenBracket) {
            self.parse_array_type()?
        } else if eat!(self, Dot) {
            let start_span = self.previous_span();

            if eat!(self, OpenCurly) {
                // anonymous struct literal
                self.parse_struct_literal(None, start_span)?
            } else if eat!(self, OpenBracket) {
                // array literal
                self.parse_array_literal(start_span)?
            } else {
                return Err(SyntaxError::expected(
                    self.span(),
                    &format!("an expression, got `{}`", self.peek().lexeme),
                ));
            }
        } else if eat!(self, At) {
            self.parse_builtin()?
        } else if eat!(self, Break | Continue | Return) {
            self.parse_terminator()?
        } else if eat!(
            self,
            Nil | True | False | Int(_) | Float(_) | Str(_) | Char(_)
        ) {
            self.parse_literal()?
        } else if eat!(self, OpenParen) {
            let start_span = self.previous_span();

            if eat!(self, CloseParen) {
                let span = start_span.to(self.previous_span());
                Expr::new(
                    ExprKind::TupleLiteral(ast::TupleLiteral { elements: vec![] }),
                    span,
                )
            } else {
                let mut expr = self.parse_expr()?;

                if eat!(self, Comma) {
                    self.parse_tuple_literal(expr, start_span)?
                } else {
                    require!(self, CloseParen, ")")?;

                    expr.span.range().start -= 1;
                    expr.span = Span::to(&expr.span, self.previous_span());

                    expr
                }
            }
        } else if eat!(self, Fn) {
            self.parse_fn(ast::FunctionKind::Orphan)?
        } else if eat!(self, Struct) {
            self.parse_struct_type()?
        } else if eat!(self, Union) {
            self.parse_struct_union_type()?
        } else {
            return Err(SyntaxError::expected(
                self.span(),
                &format!("an expression, got `{}`", self.peek().lexeme),
            ));
        };

        self.parse_postfix_expr(expr)
    }

    fn parse_array_type(&mut self) -> DiagnosticResult<Expr> {
        let start_span = self.previous_span();

        if eat!(self, Star) {
            // multi-pointer type
            // Note (Ron): multi-pointers will be removed once we have:
            //     1. parametric polymorphism
            //     2. methods
            //     3. ptr offset builtin function

            let is_mutable = eat!(self, Mut);

            require!(self, CloseBracket, "]")?;

            let inner = self.parse_expr()?;

            let ty = Expr::new(
                ExprKind::MultiPointerType(ast::ExprAndMut {
                    inner: Box::new(inner),
                    is_mutable,
                }),
                start_span.to(self.previous_span()),
            );

            Ok(ty)
        } else if eat!(self, CloseBracket) {
            // slice type
            // Note (Ron): syntax for mut slices will probably be removed once we have unsized types

            let is_mutable = eat!(self, Mut);
            let ty = self.parse_expr()?;

            Ok(Expr::new(
                ExprKind::SliceType(ast::ExprAndMut {
                    inner: Box::new(ty),
                    is_mutable,
                }),
                start_span.to(self.previous_span()),
            ))
        } else {
            // array type

            let size = self.parse_expr()?;
            require!(self, CloseBracket, "]")?;
            let ty = self.parse_expr()?;

            Ok(Expr::new(
                ExprKind::ArrayType(ast::ArrayType {
                    inner: Box::new(ty),
                    size: Box::new(size),
                }),
                start_span.to(self.previous_span()),
            ))
        }
    }

    fn parse_struct_type(&mut self) -> DiagnosticResult<Expr> {
        let start_span = self.previous_span();
        let name = self.get_decl_name();

        let kind = if eat!(self, OpenParen) {
            const SYM_PACKED: &str = "packed";

            let token = require!(self, Ident(_), SYM_PACKED)?;

            if token.symbol() != SYM_PACKED {
                return Err(SyntaxError::expected(token.span, "packed"));
            }

            require!(self, CloseParen, ")")?;

            StructTyKind::PackedStruct
        } else {
            StructTyKind::Struct
        };

        require!(self, OpenCurly, "{")?;

        let fields = self.parse_struct_type_fields()?;

        Ok(Expr::new(
            ExprKind::StructType(ast::StructType {
                name,
                fields,
                kind,
                binding_info_id: Default::default(),
            }),
            start_span.to(self.previous_span()),
        ))
    }

    fn parse_struct_union_type(&mut self) -> DiagnosticResult<Expr> {
        let start_span = self.previous_span();
        let name = self.get_decl_name();

        require!(self, OpenCurly, "{")?;

        let fields = self.parse_struct_type_fields()?;

        Ok(Expr::new(
            ExprKind::StructType(ast::StructType {
                name,
                fields,
                kind: StructTyKind::Union,
                binding_info_id: Default::default(),
            }),
            start_span.to(self.previous_span()),
        ))
    }

    fn parse_struct_type_fields(&mut self) -> DiagnosticResult<Vec<ast::StructTypeField>> {
        let fields = parse_delimited_list!(
            self,
            CloseCurly,
            Comma,
            {
                let id = require!(self, Ident(_), "an identifier")?;
                let name = id.symbol();

                require!(self, Colon, ":")?;

                let ty = self.parse_expr()?;

                ast::StructTypeField {
                    name,
                    ty,
                    span: id.span,
                }
            },
            ", or }"
        );

        Ok(fields)
    }

    fn parse_builtin(&mut self) -> DiagnosticResult<Expr> {
        let start_span = self.previous_span();
        let id_token = require!(self, Ident(_), "an identifier")?;
        let symbol = id_token.symbol();

        require!(self, OpenParen, "(")?;

        let builtin = match symbol.as_str() {
            "import" => self.parse_builtin_import()?,
            "lang_item" => BuiltinKind::LangItem(require!(self, Str(_), "string")?.symbol()),
            "size_of" => BuiltinKind::SizeOf(Box::new(self.parse_expr()?)),
            "align_of" => BuiltinKind::AlignOf(Box::new(self.parse_expr()?)),
            "panic" => BuiltinKind::Panic(if is!(self, CloseParen) {
                None
            } else {
                Some(Box::new(self.parse_expr()?))
            }),
            "run" => BuiltinKind::Run(Box::new(self.parse_expr()?), None),
            name => {
                return Err(Diagnostic::error()
                    .with_message(format!("unknown builtin function `{}`", name))
                    .with_label(Label::primary(start_span.to(id_token.span), "")))
            }
        };

        require!(self, CloseParen, ")")?;

        Ok(Expr::new(
            ExprKind::Builtin(builtin),
            start_span.to(self.previous_span()),
        ))
    }

    pub(crate) fn parse_while(&mut self) -> DiagnosticResult<Expr> {
        let start_span = self.previous_span();

        let cond = self.parse_expr_with_res(Restrictions::NO_STRUCT_LITERAL)?;

        require!(self, OpenCurly, "{")?;
        let block = self.parse_block()?;

        Ok(Expr::new(
            ExprKind::While(ast::While {
                cond: Box::new(cond),
                block,
            }),
            start_span.to(self.previous_span()),
        ))
    }

    pub(crate) fn parse_for(&mut self) -> DiagnosticResult<Expr> {
        let start_span = self.previous_span();

        let iter_name = require!(self, Ident(_), "an identifier")?.symbol();

        let iter_index_name = if eat!(self, Comma) {
            Some(require!(self, Ident(_), "an identifier")?.symbol())
        } else {
            None
        };

        require!(self, In, "in")?;

        let iter_start = self.parse_expr_with_res(Restrictions::NO_STRUCT_LITERAL)?;

        let iterator = if eat!(self, DotDot) {
            let iter_end = self.parse_expr_with_res(Restrictions::NO_STRUCT_LITERAL)?;
            ForIter::Range(Box::new(iter_start), Box::new(iter_end))
        } else {
            ForIter::Value(Box::new(iter_start))
        };

        require!(self, OpenCurly, "{")?;
        let block = self.parse_block()?;

        Ok(Expr::new(
            ExprKind::For(ast::For {
                iter_binding: NameAndId::new(iter_name),
                index_binding: iter_index_name.map(|name| NameAndId::new(name)),
                iterator,
                block,
            }),
            start_span.to(self.previous_span()),
        ))
    }

    pub(crate) fn parse_terminator(&mut self) -> DiagnosticResult<Expr> {
        let token = self.previous();
        let span = token.span;

        let kind = match token.kind {
            Break => ExprKind::Break(ast::Terminator { deferred: vec![] }),
            Continue => ExprKind::Continue(ast::Terminator { deferred: vec![] }),
            Return => {
                let expr = if !self.peek().kind.is_expr_start() && is!(self, Semicolon) {
                    None
                } else {
                    let expr = self.parse_expr()?;
                    Some(Box::new(expr))
                };

                ExprKind::Return(ast::Return {
                    expr,
                    deferred: vec![],
                })
            }
            _ => panic!("got an invalid terminator"),
        };

        Ok(Expr::new(kind, span.to(self.previous_span())))
    }
}

#[inline(always)]
fn expr_doesnt_require_semicolon(expr: &ast::Expr) -> bool {
    match &expr.kind {
        ast::ExprKind::For(_)
        | ast::ExprKind::While(_)
        | ast::ExprKind::If(_)
        | ast::ExprKind::Block(_) => true,
        _ => false,
    }
}

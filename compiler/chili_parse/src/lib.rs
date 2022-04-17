mod binding;
mod expr;
mod foreign;
mod func;
mod import;
mod literal;
mod pattern;
mod postfix_expr;
mod top_level;
mod ty;

use bitflags::bitflags;
use chili_ast::{ast::Ast, workspace::ModuleInfo};
use chili_error::{DiagnosticResult, Diagnostics, SyntaxError};
use chili_span::Span;
use chili_token::{Token, TokenKind::*};
use std::{collections::HashSet, path::Path};
use ustr::{ustr, Ustr};

bitflags! {
    struct Restrictions : u8 {
        const STMT_EXPR = 1 << 0;
        const NO_STRUCT_LITERAL = 1 << 1;
    }
}

macro_rules! token_is {
    ($parser:expr, $(|) ? $($pattern : pat_param) | +) => {
        if $parser.is_end() {
            false
        } else {
            match &$parser.peek().kind {
                $( $pattern )|+ => true,
                _ => false
            }
        }
    };
}

macro_rules! eat {
    ($parser:expr, $(|) ? $($pattern : pat_param) | +) => {
        if token_is!($parser, $( $pattern )|+) {
            $parser.bump();
            true
        } else {
            false
        }
    };
}

macro_rules! expect {
    ($parser:expr, $(|) ? $($pattern : pat_param) | +, $msg:literal) => {
        if token_is!($parser, $( $pattern )|+) {
            Ok($parser.bump().clone())
        } else {
            Err(SyntaxError::expected($parser.span(), $msg))
        }
    };
}

macro_rules! parse_delimited_list {
    ($parser:expr, $close_delim:pat, $sep:pat, $parse:expr, $msg:literal) => {{
        let mut items = vec![];

        while !eat!($parser, $close_delim) && !$parser.is_end() {
            items.push($parse);

            if eat!($parser, $sep) {
                continue;
            } else if eat!($parser, $close_delim) {
                break;
            } else {
                return Err(SyntaxError::expected($parser.span(), $msg));
            }
        }

        items
    }};
}

pub(crate) use eat;
pub(crate) use expect;
pub(crate) use parse_delimited_list;
pub(crate) use token_is;

pub struct Parser<'p> {
    tokens: Vec<Token>,
    current: usize,
    marked: Vec<usize>,
    module_info: ModuleInfo,
    root_dir: &'p Path,
    std_dir: &'p Path,
    current_dir: String,
    decl_name_frames: Vec<Ustr>,
    used_modules: HashSet<ModuleInfo>,
    restrictions: Restrictions,
    diagnostics: &'p mut Diagnostics,
}

pub struct ParserResult {
    pub ast: Ast,
    pub imports: HashSet<ModuleInfo>,
}

impl<'p> Parser<'p> {
    pub fn new(
        tokens: Vec<Token>,
        module_info: ModuleInfo,
        root_dir: &'p Path,
        std_dir: &'p Path,
        current_dir: String,
        diagnostics: &'p mut Diagnostics,
    ) -> Self {
        Self {
            tokens,
            current: 0,
            marked: Default::default(),
            module_info,
            root_dir,
            std_dir,
            current_dir,
            decl_name_frames: Default::default(),
            used_modules: Default::default(),
            restrictions: Restrictions::empty(),
            diagnostics,
        }
    }

    pub fn parse(&mut self) -> ParserResult {
        let mut ast = Ast::new(self.module_info);

        while !self.is_end() {
            let _ = self.parse_top_level(&mut ast);
            self.skip_trailing_semicolons();
        }

        ParserResult {
            ast,
            imports: self.used_modules.clone(),
        }
    }

    pub(crate) fn with_res<T>(
        &mut self,
        restrictions: Restrictions,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let old = self.restrictions;
        self.restrictions = restrictions;
        let res = f(self);
        self.restrictions = old;
        res
    }

    pub(crate) fn get_decl_name(&self) -> Ustr {
        if !self.decl_name_frames.is_empty() {
            *self.decl_name_frames.last().unwrap()
        } else {
            ustr("")
        }
    }

    #[inline]
    pub(crate) fn bump(&mut self) -> &Token {
        if !self.is_end() {
            self.current += 1;
        }

        self.previous()
    }

    #[inline]
    pub(crate) fn is_end(&self) -> bool {
        self.peek().kind == Eof
    }

    pub(crate) fn mark(&mut self, offset: isize) {
        self.marked.push((self.current as isize + offset) as usize);
    }

    pub(crate) fn reset_to_mark(&mut self) {
        self.current = self.marked.pop().unwrap();
    }

    pub(crate) fn pop_mark(&mut self) {
        self.marked.pop();
    }

    #[inline]
    pub(crate) fn peek(&self) -> &Token {
        self.tokens.get(self.current).unwrap()
    }

    #[inline]
    pub(crate) fn previous(&self) -> &Token {
        self.tokens.get(self.current - 1).unwrap()
    }

    #[inline]
    pub(crate) fn span(&self) -> Span {
        self.peek().span
    }

    #[inline]
    pub(crate) fn previous_span(&self) -> Span {
        self.previous().span
    }

    #[inline]
    pub(crate) fn skip_trailing_semicolons(&mut self) {
        while token_is!(self, Semicolon) {
            self.bump();
        }
    }

    pub(crate) fn try_recover_from_err(&mut self) {
        while !self.is_end() && !token_is!(self, Semicolon) {
            self.bump();
        }
    }
}

pub(crate) trait OrRecover<T> {
    fn or_recover(self, parser: &mut Parser) -> Result<T, ()>;
}

impl<T> OrRecover<T> for DiagnosticResult<T> {
    fn or_recover(self, parser: &mut Parser) -> Result<T, ()> {
        self.map_err(|diag| {
            parser.diagnostics.add(diag);
            parser.try_recover_from_err();
        })
    }
}

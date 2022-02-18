mod cursor;
pub mod lexer;
mod source;
mod unescape;

use chilic_span::Span;
use std::fmt::Display;
use ustr::{ustr, Ustr};

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: Ustr,
    pub span: Span,
}

impl Token {
    pub fn is(&self, other: TokenType) -> bool {
        self.token_type.is(other)
    }

    pub fn into_id(&self) -> Ustr {
        match self.token_type {
            TokenType::Id(name) => name,
            _ => unreachable!(),
        }
    }

    pub fn symbol(&self) -> Ustr {
        match &self.token_type {
            TokenType::Id(name) => *name,
            TokenType::Str(value) => ustr(value),
            _ => panic!("BUG! only call get_name for identifiers and strings"),
        }
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.token_type)
    }
}

#[derive(strum_macros::Display, Debug, PartialEq, Clone)]
pub enum TokenType {
    At,

    Semicolon,
    Colon,

    OpenParen,
    CloseParen,

    OpenCurly,
    CloseCurly,

    OpenBracket,
    CloseBracket,

    Plus,
    PlusEq,

    Minus,
    MinusEq,

    Star,
    StarEq,

    FwSlash,
    FwSlashEq,

    Percent,
    PercentEq,

    QuestionMark,

    Comma,

    Amp,
    AmpEq,

    AmpAmp,
    AmpAmpEq,

    Bar,
    BarEq,

    BarBar,
    BarBarEq,

    Tilde,

    Caret,
    CaretEq,

    Bang,
    BangEq,

    Eq,
    EqEq,

    Lt,
    LtEq,

    LtLt,
    LtLtEq,

    Gt,
    GtEq,
    GtGt,
    GtGtEq,

    Dot,
    DotDot,

    RightArrow,

    If,
    Else,
    While,
    For,
    Break,
    Continue,
    Return,
    Defer,
    Let,
    Type,
    Fn,
    Foreign,
    Use,
    Pub,
    Mut,
    In,
    As,
    Union,
    Match,

    Placeholder,

    Id(Ustr),

    Nil,
    True,
    False,
    Int(i64),
    Float(f64),
    Str(Ustr),
    Char(char),

    Comment(Ustr),
    Unknown(char),
    Eof,
}

impl TokenType {
    pub fn is(&self, other: TokenType) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(&other)
    }

    pub fn lexeme(&self) -> &str {
        use TokenType::*;

        match self {
            At => "@",
            Semicolon => ";",
            Colon => ":",
            OpenParen => "(",
            CloseParen => ")",
            OpenCurly => "{",
            CloseCurly => "}",
            OpenBracket => "[",
            CloseBracket => "]",
            Plus => "+",
            PlusEq => "+=",
            Minus => "-",
            MinusEq => "-=",
            Star => "*",
            StarEq => "*=",
            FwSlash => "/",
            FwSlashEq => "/=",
            Percent => "%",
            PercentEq => "%=",
            QuestionMark => "?",
            Comma => ",",
            Amp => "&",
            AmpEq => "&=",
            AmpAmp => "&&",
            AmpAmpEq => "&&=",
            Bar => "|",
            BarEq => "|=",
            BarBar => "||",
            BarBarEq => "||=",
            Tilde => "~",
            Caret => "^",
            CaretEq => "^=",
            Bang => "!",
            BangEq => "!=",
            Eq => "=",
            EqEq => "==",
            Lt => "<",
            LtEq => "<=",
            LtLt => "<<",
            LtLtEq => "<<=",
            Gt => ">",
            GtEq => ">=",
            GtGt => ">>",
            GtGtEq => ">>=",
            Dot => ".",
            DotDot => "..",
            RightArrow => "->",
            If => "if",
            Else => "else",
            While => "while",
            For => "for",
            Break => "break",
            Continue => "continue",
            Return => "return",
            Defer => "defer",
            Let => "let",
            Type => "type",
            Fn => "fn",
            Foreign => "foreign",
            Use => "use",
            Pub => "pub",
            Mut => "mut",
            In => "in",
            As => "as",
            Union => "union",
            Match => "match",
            Placeholder => "_",
            Id(_) => "identifier",
            Nil => "nil",
            True => "true",
            False => "false",
            Int(_) => "{integer}",
            Float(_) => "{float}",
            Str(_) => "{string}",
            Char(_) => "{char}",
            Unknown(_) => "???",
            Comment(_) => "comment",
            Eof => "EOF",
        }
    }
}

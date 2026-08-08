//! Token definitions for the Poly language.

use std::fmt;

/// A span represents a location in source code (start offset, end offset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// A token with its kind and span in source code.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// All possible token kinds in the Poly language.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // === Literals ===
    /// Integer literal (decimal, hex 0x, binary 0b, octal 0o)
    IntLiteral(String),
    /// Float literal (e.g., 3.14, 1.0e10)
    FloatLiteral(String),
    /// ASCII string literal ("hello")
    StringLiteral(String),
    /// Unicode string literal (u"hello")
    UnicodeStringLiteral(String),
    /// Byte literal ([0x48, 0x65])
    ByteLiteral(Vec<u8>),
    /// Boolean literal (true, false)
    BoolLiteral(bool),

    // === Identifiers & Keywords ===
    /// User-defined identifier
    Identifier(String),

    // Keywords
    Var,
    Let,
    Const,
    Fn,
    End,
    If,
    Then,
    Else,
    While,
    Loop,
    For,
    In,
    Match,
    Break,
    Continue,
    Return,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
    Use,
    Pub,
    As,
    Where,
    Unsafe,
    Async,
    Await,
    Spawn,
    Move,
    Type,
    Macro,
    Try,
    Panic,
    Null,
    Addr,
    Deref,
    Step,
    Capture,

    // Assembly-style operations
    Add,
    Sub,
    Inc,
    Dec,

    // I/O commands
    Put,
    Get,
    Error,
    Warn,
    Info,
    With,
    Validate,
    Complete,
    Encoding,
    Timeout,
    Default,
    Mask,
    Bytes,

    // Type keywords
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    I128,
    U128,
    F32,
    F64,
    ISize,
    USize,
    Char,
    String,
    UChar,
    UString,
    Byte,
    Ptr,
    Vec,
    Option,
    Result,

    // === Operators ===
    // Arithmetic
    Plus,    // +
    Minus,   // -
    Star,    // *
    Slash,   // /
    Percent, // %

    // Comparison
    EqEq,  // ==
    NotEq, // !=
    Lt,    // <
    Gt,    // >
    LtEq,  // <=
    GtEq,  // >=

    // Logical
    AndAnd, // &&
    OrOr,   // ||
    Not,    // !

    // Bitwise
    Amp,   // &
    Pipe,  // |
    Caret, // ^
    Tilde, // ~
    LtLt,  // <<
    GtGt,  // >>

    // Assignment
    Eq,        // =
    PlusEq,    // +=
    MinusEq,   // -=
    StarEq,    // *=
    SlashEq,   // /=
    PercentEq, // %=
    AmpEq,     // &=
    PipeEq,    // |=
    CaretEq,   // ^=
    LtLtEq,    // <<=
    GtGtEq,    // >>=

    // Arrow & fat arrow
    Arrow,    // ->
    FatArrow, // =>

    // Range
    DotDot,   // ..
    DotDotEq, // ..=

    // Path separator
    ColonColon, // ::

    // === Delimiters ===
    LParen,    // (
    RParen,    // )
    LBracket,  // [
    RBracket,  // ]
    LBrace,    // {
    RBrace,    // }
    Comma,     // ,
    Semicolon, // ;
    Colon,     // :
    Dot,       // .

    // Note: File I/O (<, >, >>) uses Lt, Gt, GtGt tokens.
    // The parser determines file I/O context from these tokens.

    // === Special ===
    /// Newline
    Newline,
    /// EOF
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Literals
            TokenKind::IntLiteral(s) => write!(f, "{}", s),
            TokenKind::FloatLiteral(s) => write!(f, "{}", s),
            TokenKind::StringLiteral(s) => write!(f, "\"{}\"", s),
            TokenKind::UnicodeStringLiteral(s) => write!(f, "u\"{}\"", s),
            TokenKind::ByteLiteral(bytes) => {
                write!(f, "[")?;
                for (i, b) in bytes.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "0x{:02X}", b)?;
                }
                write!(f, "]")
            }
            TokenKind::BoolLiteral(b) => write!(f, "{}", b),

            // Identifiers
            TokenKind::Identifier(s) => write!(f, "{}", s),

            // Keywords
            TokenKind::Var => write!(f, "var"),
            TokenKind::Let => write!(f, "let"),
            TokenKind::Const => write!(f, "const"),
            TokenKind::Fn => write!(f, "fn"),
            TokenKind::End => write!(f, "end"),
            TokenKind::If => write!(f, "if"),
            TokenKind::Then => write!(f, "then"),
            TokenKind::Else => write!(f, "else"),
            TokenKind::While => write!(f, "while"),
            TokenKind::Loop => write!(f, "loop"),
            TokenKind::For => write!(f, "for"),
            TokenKind::In => write!(f, "in"),
            TokenKind::Match => write!(f, "match"),
            TokenKind::Break => write!(f, "break"),
            TokenKind::Continue => write!(f, "continue"),
            TokenKind::Return => write!(f, "return"),
            TokenKind::Struct => write!(f, "struct"),
            TokenKind::Enum => write!(f, "enum"),
            TokenKind::Trait => write!(f, "trait"),
            TokenKind::Impl => write!(f, "impl"),
            TokenKind::Module => write!(f, "module"),
            TokenKind::Use => write!(f, "use"),
            TokenKind::Pub => write!(f, "pub"),
            TokenKind::As => write!(f, "as"),
            TokenKind::Where => write!(f, "where"),
            TokenKind::Unsafe => write!(f, "unsafe"),
            TokenKind::Async => write!(f, "async"),
            TokenKind::Await => write!(f, "await"),
            TokenKind::Spawn => write!(f, "spawn"),
            TokenKind::Move => write!(f, "move"),
            TokenKind::Type => write!(f, "type"),
            TokenKind::Macro => write!(f, "macro"),
            TokenKind::Try => write!(f, "try"),
            TokenKind::Panic => write!(f, "panic"),
            TokenKind::Null => write!(f, "null"),
            TokenKind::Addr => write!(f, "addr"),
            TokenKind::Deref => write!(f, "deref"),
            TokenKind::Step => write!(f, "step"),
            TokenKind::Capture => write!(f, "capture"),

            // Assembly ops
            TokenKind::Add => write!(f, "add"),
            TokenKind::Sub => write!(f, "sub"),
            TokenKind::Inc => write!(f, "inc"),
            TokenKind::Dec => write!(f, "dec"),

            // I/O
            TokenKind::Put => write!(f, "put"),
            TokenKind::Get => write!(f, "get"),
            TokenKind::Error => write!(f, "error"),
            TokenKind::Warn => write!(f, "warn"),
            TokenKind::Info => write!(f, "info"),
            TokenKind::With => write!(f, "with"),
            TokenKind::Validate => write!(f, "validate"),
            TokenKind::Complete => write!(f, "complete"),
            TokenKind::Encoding => write!(f, "encoding"),
            TokenKind::Timeout => write!(f, "timeout"),
            TokenKind::Default => write!(f, "default"),
            TokenKind::Mask => write!(f, "mask"),
            TokenKind::Bytes => write!(f, "bytes"),

            // Types
            TokenKind::Bool => write!(f, "bool"),
            TokenKind::I8 => write!(f, "i8"),
            TokenKind::U8 => write!(f, "u8"),
            TokenKind::I16 => write!(f, "i16"),
            TokenKind::U16 => write!(f, "u16"),
            TokenKind::I32 => write!(f, "i32"),
            TokenKind::U32 => write!(f, "u32"),
            TokenKind::I64 => write!(f, "i64"),
            TokenKind::U64 => write!(f, "u64"),
            TokenKind::I128 => write!(f, "i128"),
            TokenKind::U128 => write!(f, "u128"),
            TokenKind::F32 => write!(f, "f32"),
            TokenKind::F64 => write!(f, "f64"),
            TokenKind::ISize => write!(f, "isize"),
            TokenKind::USize => write!(f, "usize"),
            TokenKind::Char => write!(f, "char"),
            TokenKind::String => write!(f, "string"),
            TokenKind::UChar => write!(f, "uchar"),
            TokenKind::UString => write!(f, "ustring"),
            TokenKind::Byte => write!(f, "byte"),
            TokenKind::Ptr => write!(f, "ptr"),
            TokenKind::Vec => write!(f, "Vec"),
            TokenKind::Option => write!(f, "Option"),
            TokenKind::Result => write!(f, "Result"),

            // Operators
            TokenKind::Plus => write!(f, "+"),
            TokenKind::Minus => write!(f, "-"),
            TokenKind::Star => write!(f, "*"),
            TokenKind::Slash => write!(f, "/"),
            TokenKind::Percent => write!(f, "%"),
            TokenKind::EqEq => write!(f, "=="),
            TokenKind::NotEq => write!(f, "!="),
            TokenKind::Lt => write!(f, "<"),
            TokenKind::Gt => write!(f, ">"),
            TokenKind::LtEq => write!(f, "<="),
            TokenKind::GtEq => write!(f, ">="),
            TokenKind::AndAnd => write!(f, "&&"),
            TokenKind::OrOr => write!(f, "||"),
            TokenKind::Not => write!(f, "!"),
            TokenKind::Amp => write!(f, "&"),
            TokenKind::Pipe => write!(f, "|"),
            TokenKind::Caret => write!(f, "^"),
            TokenKind::Tilde => write!(f, "~"),
            TokenKind::LtLt => write!(f, "<<"),
            TokenKind::GtGt => write!(f, ">>"),
            TokenKind::Eq => write!(f, "="),
            TokenKind::PlusEq => write!(f, "+="),
            TokenKind::MinusEq => write!(f, "-="),
            TokenKind::StarEq => write!(f, "*="),
            TokenKind::SlashEq => write!(f, "/="),
            TokenKind::PercentEq => write!(f, "%="),
            TokenKind::AmpEq => write!(f, "&="),
            TokenKind::PipeEq => write!(f, "|="),
            TokenKind::CaretEq => write!(f, "^="),
            TokenKind::LtLtEq => write!(f, "<<="),
            TokenKind::GtGtEq => write!(f, ">>="),
            TokenKind::Arrow => write!(f, "->"),
            TokenKind::FatArrow => write!(f, "=>"),
            TokenKind::DotDot => write!(f, ".."),
            TokenKind::DotDotEq => write!(f, "..="),
            TokenKind::ColonColon => write!(f, "::"),

            // Delimiters
            TokenKind::LParen => write!(f, "("),
            TokenKind::RParen => write!(f, ")"),
            TokenKind::LBracket => write!(f, "["),
            TokenKind::RBracket => write!(f, "]"),
            TokenKind::LBrace => write!(f, "{{"),
            TokenKind::RBrace => write!(f, "}}"),
            TokenKind::Comma => write!(f, ","),
            TokenKind::Semicolon => write!(f, ";"),
            TokenKind::Colon => write!(f, ":"),
            TokenKind::Dot => write!(f, "."),

            // Special
            TokenKind::Newline => write!(f, "\\n"),
            TokenKind::Eof => write!(f, "EOF"),
        }
    }
}

impl TokenKind {
    /// Check if this token is a keyword that could also be an identifier.
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Var
                | TokenKind::Let
                | TokenKind::Const
                | TokenKind::Fn
                | TokenKind::End
                | TokenKind::If
                | TokenKind::Then
                | TokenKind::Else
                | TokenKind::While
                | TokenKind::Loop
                | TokenKind::For
                | TokenKind::In
                | TokenKind::Match
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Return
                | TokenKind::Struct
                | TokenKind::Enum
                | TokenKind::Trait
                | TokenKind::Impl
                | TokenKind::Module
                | TokenKind::Use
                | TokenKind::Pub
                | TokenKind::As
                | TokenKind::Where
                | TokenKind::Unsafe
                | TokenKind::Async
                | TokenKind::Await
                | TokenKind::Spawn
                | TokenKind::Move
                | TokenKind::Type
                | TokenKind::Macro
                | TokenKind::Try
                | TokenKind::Panic
                | TokenKind::Null
                | TokenKind::Addr
                | TokenKind::Deref
                | TokenKind::Step
                | TokenKind::Capture
        )
    }

    /// Check if this token is a type keyword.
    pub fn is_type_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Bool
                | TokenKind::I8
                | TokenKind::U8
                | TokenKind::I16
                | TokenKind::U16
                | TokenKind::I32
                | TokenKind::U32
                | TokenKind::I64
                | TokenKind::U64
                | TokenKind::I128
                | TokenKind::U128
                | TokenKind::F32
                | TokenKind::F64
                | TokenKind::ISize
                | TokenKind::USize
                | TokenKind::Char
                | TokenKind::String
                | TokenKind::UChar
                | TokenKind::UString
                | TokenKind::Byte
                | TokenKind::Bytes
                | TokenKind::Ptr
        )
    }

    /// Check if this token is a literal.
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            TokenKind::IntLiteral(_)
                | TokenKind::FloatLiteral(_)
                | TokenKind::StringLiteral(_)
                | TokenKind::UnicodeStringLiteral(_)
                | TokenKind::ByteLiteral(_)
                | TokenKind::BoolLiteral(_)
        )
    }

    /// Check if this token is an assignment operator.
    pub fn is_assignment_op(&self) -> bool {
        matches!(
            self,
            TokenKind::Eq
                | TokenKind::PlusEq
                | TokenKind::MinusEq
                | TokenKind::StarEq
                | TokenKind::SlashEq
                | TokenKind::PercentEq
                | TokenKind::AmpEq
                | TokenKind::PipeEq
                | TokenKind::CaretEq
                | TokenKind::LtLtEq
                | TokenKind::GtGtEq
        )
    }
}

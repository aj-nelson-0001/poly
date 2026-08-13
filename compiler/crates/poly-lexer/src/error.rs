//! Lexer error types.

use std::fmt;

use crate::token::Span;

/// Errors that can occur during lexing.
#[derive(Debug, Clone, PartialEq)]
pub struct LexerError {
    pub kind: LexerErrorKind,
    pub span: Span,
    pub message: String,
}

impl LexerError {
    /// Construct a diagnostic while keeping the offending source span attached.
    pub fn new(kind: LexerErrorKind, span: Span, message: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
        }
    }
}

impl fmt::Display for LexerError {
    // Keep formatting compact because CLI callers often print several lexer
    // diagnostics in one pass during error recovery.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:?}] at {}..{}: {}",
            self.kind, self.span.start, self.span.end, self.message
        )
    }
}

impl std::error::Error for LexerError {}

/// Kinds of lexer errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LexerErrorKind {
    /// Unexpected character
    UnexpectedCharacter,
    /// Unterminated string
    UnterminatedString,
    /// Invalid number format
    InvalidNumber,
    /// Invalid escape sequence
    InvalidEscape,
    /// Invalid byte literal
    InvalidByteLiteral,
    /// Invalid unicode string prefix
    InvalidUnicodePrefix,
    /// Unterminated block comment
    UnterminatedComment,
    /// Invalid token
    InvalidToken,
}

impl fmt::Display for LexerErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexerErrorKind::UnexpectedCharacter => write!(f, "Unexpected character"),
            LexerErrorKind::UnterminatedString => write!(f, "Unterminated string"),
            LexerErrorKind::InvalidNumber => write!(f, "Invalid number"),
            LexerErrorKind::InvalidEscape => write!(f, "Invalid escape sequence"),
            LexerErrorKind::InvalidByteLiteral => write!(f, "Invalid byte literal"),
            LexerErrorKind::InvalidUnicodePrefix => write!(f, "Invalid unicode prefix"),
            LexerErrorKind::UnterminatedComment => write!(f, "Unterminated block comment"),
            LexerErrorKind::InvalidToken => write!(f, "Invalid token"),
        }
    }
}

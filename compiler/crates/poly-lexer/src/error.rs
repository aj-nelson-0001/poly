//! Lexer error types.
//!
//! When the lexer sees something it cannot turn into a sensible token (an
//! unterminated string, a stray `@`, a bad number) it records a `LexerError`
//! instead of crashing. Errors are collected in a list so the compiler can
//! report several problems in a single pass rather than stopping at the first.

use std::fmt;

// A `Span` says *where* in the source the problem is.
use crate::token::Span;

/// A single problem found while lexing.
///
/// A `LexerError` bundles three things: a machine-friendly category (`kind`),
/// the exact source range (`span`), and a human-readable explanation
/// (`message`). Keeping the category separate from the message means tools can
/// branch on the kind without parsing English text.
#[derive(Debug, Clone, PartialEq)]
pub struct LexerError {
    /// Stable category used by tools to distinguish malformed literals from
    /// unexpected characters without parsing the human-readable message.
    pub kind: LexerErrorKind,
    /// Exact byte range used to underline the offending source text.
    pub span: Span,
    /// Human-readable detail suitable for CLI and editor diagnostics.
    pub message: String,
}

impl LexerError {
    /// Build an error value from its three parts.
    ///
    /// `message` accepts anything that converts into a `String` (such as a
    /// `&str` or a `format!` result) so callers can pass either a fixed
    /// message or one with values spliced in.
    pub fn new(kind: LexerErrorKind, span: Span, message: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
        }
    }
}

impl fmt::Display for LexerError {
    /// Render a compact single-line form like
    /// `[UnterminatedString] at 4..10: Unterminated string literal`.
    /// The CLI often prints several of these in a row, so the format stays
    /// terse.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:?}] at {}..{}: {}",
            self.kind, self.span.start, self.span.end, self.message
        )
    }
}

// Implementing the standard `Error` trait lets a `LexerError` be returned with
// the `?` operator from functions that return `Box<dyn Error>`, and lets other
// crates treat it as a normal Rust error.
impl std::error::Error for LexerError {}

/// The categories of lexer error.
///
/// These are deliberately coarse: they describe *what kind* of problem
/// occurred, not the specific text. Tools switch on this to decide how to
/// react; the `message` field carries the details for humans.
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
    /// A short human-facing label for each category, used when an error is
    /// printed without a custom message.
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

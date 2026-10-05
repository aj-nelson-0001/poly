//! Poly Language Lexer
//!
//! This crate is the front door of the Poly compiler. Its only job is to turn
//! raw source text (a `&str`) into a flat list of *tokens* — the small,
//! meaningful pieces like keywords, identifiers, numbers, and punctuation.
//! The parser crate then reads that token list instead of the raw text.
//!
//! Think of it as the "reader" stage: it doesn't understand what the program
//! means, it just recognises the individual words and symbols.

// The four modules that make up the lexer. Each one is declared here so the
// rest of the compiler can reach its contents.
pub mod diagnostics; // Turns byte offsets into line/column and pretty error output.
pub mod error; // The `LexerError` type: what went wrong and where.
pub mod lexer; // The main `Lexer` struct that walks the source text.
pub mod token; // The `Token`, `TokenKind`, and `Span` types.

// Re-export the most commonly used items at the crate root. This lets callers
// write `use poly_lexer::Lexer;` instead of `use poly_lexer::lexer::Lexer;`,
// keeping import lines short and stable even if modules are reorganised.
pub use diagnostics::{line_col, render_error, source_line};
pub use error::LexerError;
pub use lexer::Lexer;
pub use token::{Span, Token, TokenKind};

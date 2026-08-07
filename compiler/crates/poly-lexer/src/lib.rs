//! Poly Language Lexer
//!
//! Tokenizer for the Poly programming language. Converts source code into a stream of tokens
//! that can be consumed by the parser.

pub mod token;
pub mod lexer;
pub mod error;

pub use token::{Token, TokenKind, Span};
pub use lexer::Lexer;
pub use error::LexerError;

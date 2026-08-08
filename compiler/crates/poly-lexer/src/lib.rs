//! Poly Language Lexer
//!
//! Tokenizer for the Poly programming language. Converts source code into a stream of tokens
//! that can be consumed by the parser.

pub mod error;
pub mod lexer;
pub mod token;

pub use error::LexerError;
pub use lexer::Lexer;
pub use token::{Span, Token, TokenKind};

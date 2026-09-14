//! Poly Language Parser
//!
//! Converts a stream of tokens into an Abstract Syntax Tree (AST).

pub mod ast;
pub mod entry_point;
pub mod error;
pub mod parser;

pub use ast::*;
pub use entry_point::require_explicit_main;
pub use error::ParseError;
pub use parser::Parser;

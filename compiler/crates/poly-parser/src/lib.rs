//! Poly Language Parser
//!
//! Converts a stream of tokens into an Abstract Syntax Tree (AST).

pub mod ast;
pub mod ast_dump;
pub mod depth_limit;
pub mod entry_point;
pub mod error;
pub mod parser;

pub use ast::*;
pub use ast_dump::{render_program, write_program};
pub use depth_limit::{ast_depth, MAX_AST_DEPTH};
pub use entry_point::require_explicit_main;
pub use error::ParseError;
pub use parser::{
    is_valid_dependency_name, is_valid_dependency_version, Parser, MAX_EXPRESSION_DEPTH,
    MAX_OPERAND_CHAIN,
};

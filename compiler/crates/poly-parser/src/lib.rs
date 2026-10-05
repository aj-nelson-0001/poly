//! Poly Language Parser
//!
//! Converts a stream of tokens (produced by the lexer) into an Abstract Syntax
//! Tree (AST) — a structured tree that mirrors the shape of the program.
//!
//! Where the lexer only knows "this is an `if` keyword, here is a name", the
//! parser understands that `if cond ... end if` is a conditional statement and
//! builds a node describing it. Later stages (type checking, code generation)
//! walk this tree.

// Sub-modules that make up the parser.
pub mod ast; // The AST node and type definitions.
pub mod ast_dump; // Serialises an AST back to a readable text form.
pub mod depth_limit; // Guards against pathologically deep nested input.
pub mod entry_point; // Checks that the program declares `fn main`.
pub mod error; // The `ParseError` type (with helpful suggestions).
pub mod parser; // The main `Parser` that builds the AST.

// Re-export the AST node types so callers can use them without a nested path.
pub use ast::*;
// Handy top-level functions for rendering a parsed program.
pub use ast_dump::{render_program, write_program};
// Depth utilities used to reject extremely deep input safely.
pub use depth_limit::{ast_depth, MAX_AST_DEPTH};
// The entry-point check used by the CLI before code generation.
pub use entry_point::require_explicit_main;
// The parser error type.
pub use error::ParseError;
// The parser itself plus its limits and dependency-name validators.
pub use parser::{
    is_valid_dependency_name, is_valid_dependency_version, Parser, MAX_EXPRESSION_DEPTH,
    MAX_OPERAND_CHAIN,
};

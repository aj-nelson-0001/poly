//! Poly Language Transpiler
//!
//! Generates Rust code from Poly source code.

pub mod checker;
pub mod codegen;
pub mod source_map;

pub use checker::{check_program, TypeCheckError, TypeChecker};
pub use codegen::Transpiler;
pub use source_map::SourceMap;

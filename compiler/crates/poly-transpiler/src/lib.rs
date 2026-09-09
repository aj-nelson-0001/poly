//! Poly Language Transpiler
//!
//! Generates Rust code from Poly source code.

pub mod checker;
pub mod codegen;
pub mod source_map;

pub use checker::{
    check_program, check_program_strict_foreign, check_program_strict_foreign_with_warnings,
    check_program_with_warnings, TypeCheckError, TypeChecker,
};
pub use codegen::Transpiler;
pub use source_map::SourceMap;

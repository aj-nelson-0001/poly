//! Poly Language Transpiler
//!
//! Generates Rust code from Poly source code.
//!
//! This crate is the middle of the pipeline: it runs semantic checking
//! ([`checker`]) and then lowers the program through the intermediate
//! representation to Rust ([`codegen`]). It also builds source maps
//! ([`source_map`]) so generated Rust can be traced back to Poly source.
//!
//! Although the crate is named after Rust output, it also fans out to the C,
//! assembly, and JavaScript backends by delegating to their crates.

pub mod checker;
pub mod codegen;
pub mod source_map;

// Re-export the checking entry points and the transpiler itself so callers can
// `use poly_transpiler::check_program;` and friends directly.
pub use checker::{
    check_program, check_program_strict_foreign, check_program_strict_foreign_with_warnings,
    check_program_with_warnings, TypeCheckError, TypeChecker,
};
pub use codegen::Transpiler;
pub use source_map::SourceMap;

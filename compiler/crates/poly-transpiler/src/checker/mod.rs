//! Semantic type checking for Poly programs.
//!
//! The parser deliberately accepts syntax without needing to know what names
//! or types mean.  This module performs the next compiler phase: it builds a
//! symbol table, infers expression types, and reports semantic errors before
//! Rust code generation is attempted.
//!
//! The implementation is split across two files:
//!
//! - **`helpers`** — pure type-classification and generic-substitution
//!   utilities that depend only on `PolyType` and the AST.  Extracted here
//!   so the checker file stays focused on the `TypeChecker` state machine.
//! - **`checker`** — the `TypeChecker` struct, its `check_program` driver,
//!   and every method that reads or writes checker state.

mod helpers;
mod type_checker;

pub use self::type_checker::{
    check_program, check_program_strict_foreign, check_program_strict_foreign_with_warnings,
    check_program_with_warnings, TypeCheckError, TypeChecker,
};

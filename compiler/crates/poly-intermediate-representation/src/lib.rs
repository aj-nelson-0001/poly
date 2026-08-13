//! Poly intermediate representation (intermediate representation) and optimization passes.
//!
//! The intermediate representation is the compiler's middle layer: the parser produces an AST, the
//! generator lowers it to intermediate representation, optimization passes rewrite the intermediate representation, and the intermediate representation
//! code generator produces Rust.  Keeping a dedicated intermediate representation crate lets each phase
//! be developed, tested, and benchmarked independently.

pub mod codegen;
pub mod generator;
pub mod intermediate_representation;
pub mod optimizer;

pub use codegen::IntermediateRepresentationCodeGen;
pub use generator::generate;
pub use intermediate_representation::*;
pub use optimizer::{optimize, OptimizationPass};

/// Convert Poly source directly to Rust through the intermediate representation pipeline (without
/// optimization).  Useful for consumers that want a single-call entry point.
pub fn transpile(source: &str) -> Result<String, String> {
    let (tokens, errors) = poly_lexer::Lexer::lex(source);
    if !errors.is_empty() {
        return Err(format!("Lexer errors: {:?}", errors));
    }
    let mut parser = poly_parser::Parser::new(&tokens);
    let program = parser.parse().map_err(|e| e.to_string())?;
    let intermediate_representation = generator::generate(&program);
    let mut codegen = IntermediateRepresentationCodeGen::new();
    Ok(codegen.generate(&intermediate_representation))
}

/// Convert Poly source to Rust through the intermediate representation pipeline with optimization.
pub fn transpile_optimized(source: &str) -> Result<String, String> {
    let (tokens, errors) = poly_lexer::Lexer::lex(source);
    if !errors.is_empty() {
        return Err(format!("Lexer errors: {:?}", errors));
    }
    let mut parser = poly_parser::Parser::new(&tokens);
    let program = parser.parse().map_err(|e| e.to_string())?;
    let mut intermediate_representation = generator::generate(&program);
    optimizer::optimize(&mut intermediate_representation);
    let mut codegen = IntermediateRepresentationCodeGen::new();
    Ok(codegen.generate(&intermediate_representation))
}

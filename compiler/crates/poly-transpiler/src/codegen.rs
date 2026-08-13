//! Rust code generator facade for the Poly language.
//!
//! This module exposes the [`Transpiler`] public API.  Code generation itself
//! lives in the shared intermediate-representation pipeline
//! (`poly-intermediate-representation`): the parser produces an AST, the
//! generator lowers it to a flat intermediate representation, and a single
//! intermediate-representation code generator emits idiomatic Rust.
//!
//! Routing every backend through one code generator (instead of maintaining a
//! second, near-identical AST walker) keeps the two historical backends from
//! drifting apart and lets optimization passes benefit the default pipeline.

use poly_lexer::Lexer;
use poly_parser::{Parser, Statement};

use crate::checker::check_program;
use crate::source_map::SourceMap;

/// The Poly-to-Rust transpiler.
pub struct Transpiler {
    /// Source map for debugging
    #[allow(dead_code)]
    source_map: Option<SourceMap>,
}

impl Transpiler {
    /// Create a new transpiler instance.
    pub fn new() -> Self {
        Self { source_map: None }
    }

    /// Create a new transpiler with source map generation enabled.
    pub fn with_source_map() -> Self {
        Self {
            source_map: Some(SourceMap::new("", "")),
        }
    }

    /// Transpile Poly source code to Rust code.
    ///
    /// Keep this low-level API permissive for callers that want to inspect
    /// generated Rust; build-oriented entry points use `transpile_checked`.
    pub fn transpile(&self, source: &str) -> Result<String, String> {
        poly_intermediate_representation::transpile(source)
    }

    /// Transpile Poly source code after running semantic type checking.
    ///
    /// The pipeline is intentionally lex -> parse -> check -> generate so
    /// invalid programs fail before they create misleading Rust artifacts.
    pub fn transpile_checked(&self, source: &str) -> Result<String, String> {
        let (tokens, errors) = Lexer::lex(source);
        if !errors.is_empty() {
            return Err(format!("Lexer errors: {:?}", errors));
        }

        let mut parser = Parser::new(&tokens);
        let program = parser.parse().map_err(|e| e.to_string())?;
        check_program(&program).map_err(|errors| {
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        })?;

        let intermediate_representation =
            poly_intermediate_representation::generator::generate(&program);
        let mut codegen =
            poly_intermediate_representation::IntermediateRepresentationCodeGen::new();
        Ok(codegen.generate(&intermediate_representation))
    }

    /// Transpile Poly source through the intermediate representation pipeline
    /// with optimization.
    ///
    /// This entry point exposes the optimizer that the default pipeline skips.
    pub fn transpile_with_intermediate_representation(
        &self,
        source: &str,
    ) -> Result<String, String> {
        poly_intermediate_representation::transpile_optimized(source)
    }

    /// Transpile Poly source through the intermediate representation pipeline
    /// without optimization.
    pub fn transpile_with_intermediate_representation_unoptimized(
        &self,
        source: &str,
    ) -> Result<String, String> {
        poly_intermediate_representation::transpile(source)
    }

    /// Transpile Poly source code to Rust code with source map.
    ///
    /// Top-level statements now carry exact source spans, so symbol locations
    /// are registered with precise columns.  Line mappings remain best-effort:
    /// generated lines are mapped to the source line with the same 1-based
    /// index, which is exact for one-statement-per-line programs and degrades
    /// gracefully as statements span lines.
    pub fn transpile_with_source_map(
        &mut self,
        source: &str,
    ) -> Result<(String, SourceMap), String> {
        let rust_code = self.transpile(source)?;

        let mut source_map = SourceMap::new(source, &rust_code);
        let source_line_count = source.lines().count().max(1);

        for (index, _) in rust_code.lines().enumerate() {
            // Best-effort line mapping; see the struct docs for the caveat.
            source_map.add_mapping(index + 1, (index + 1).min(source_line_count));
        }

        // Register symbol locations from the statement spans the parser
        // attached, so diagnostics can point at the exact declaration column.
        self.register_statement_symbols(source, &mut source_map);

        // Retain the map so `source_map()` returns the latest transpilation.
        self.source_map = Some(source_map.clone());

        Ok((rust_code, source_map))
    }

    /// Register declaration symbols (functions, structs, enums, variables)
    /// with their exact source locations using the parsed statement spans.
    fn register_statement_symbols(&self, source: &str, source_map: &mut SourceMap) {
        use poly_lexer::diagnostics::line_col;

        let (tokens, errors) = Lexer::lex(source);
        if !errors.is_empty() {
            return;
        }
        let mut parser = Parser::new(&tokens);
        let (program, _) = parser.parse_with_recovery();

        for spanned in &program.statements {
            let (line, column) = line_col(source, spanned.span.start);
            match &spanned.node {
                Statement::FunctionDeclaration(function) => {
                    source_map.add_symbol(&function.name, line, column, function.name.len());
                }
                Statement::StructDeclaration(decl) => {
                    source_map.add_symbol(&decl.name, line, column, decl.name.len());
                }
                Statement::EnumDeclaration(decl) => {
                    source_map.add_symbol(&decl.name, line, column, decl.name.len());
                }
                Statement::VarDeclaration { name, .. } | Statement::LetDeclaration { name, .. } => {
                    source_map.add_symbol(name, line, column, name.len());
                }
                Statement::ConstDeclaration { name, .. } => {
                    source_map.add_symbol(name, line, column, name.len());
                }
                _ => {}
            }
        }
    }

    /// Get the source map from the last transpilation with a source map.
    pub fn source_map(&self) -> Option<&SourceMap> {
        self.source_map.as_ref()
    }
}

impl Default for Transpiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transpile_routes_through_shared_pipeline() {
        let t = Transpiler::new();
        let rust = t.transpile("var x i32 := 42").unwrap();
        assert!(rust.contains("let mut x: i32 = 42"));
    }

    #[test]
    fn transpile_checked_rejects_type_errors() {
        let t = Transpiler::new();
        // Assigning a String to an i32 variable is a semantic error the
        // checker flags before any Rust is generated.
        let result = t.transpile_checked("var x i32 := \"hello\"");
        assert!(result.is_err(), "type mismatch should fail checking");
    }

    #[test]
    fn source_map_maps_lines_best_effort() {
        let mut t = Transpiler::with_source_map();
        let (rust, map) = t.transpile_with_source_map("put \"hi\"").unwrap();
        assert!(rust.contains("println!"));
        assert!(map.lookup_target_line(1).is_some());
    }
}

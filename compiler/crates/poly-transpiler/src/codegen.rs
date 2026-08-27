//! Rust code generator facade for the Poly language.
//!
//! This module exposes the default Poly-to-Rust API and target-aware parsing
//! shared by the CLI and alternate backends.

use poly_lexer::Lexer;
use poly_parser::{Parser, Program};

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

    /// Parse Poly source into an AST.
    pub fn parse(source: &str) -> Result<Program, String> {
        let (tokens, errors) = Lexer::lex(source);
        if !errors.is_empty() {
            return Err(format!("Lexer errors: {:?}", errors));
        }
        let mut parser = Parser::new(&tokens);
        parser.parse().map_err(|e| e.to_string())
    }

    /// Parse Poly source and keep only foreign blocks for the selected target.
    pub fn parse_target(source: &str, target: &str) -> Result<Program, String> {
        Self::select_target(Self::parse(source)?, target)
    }

    /// Transpile Poly source code to Rust code.
    pub fn transpile(&self, source: &str) -> Result<String, String> {
        let program = Self::parse_target(source, "rust")?;
        self.generate_rust(&program)
    }

    /// Transpile Poly source code after running semantic type checking.
    pub fn transpile_checked(&self, source: &str) -> Result<String, String> {
        let program = Self::parse_target(source, "rust")?;
        check_program(&program).map_err(|errors| {
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        })?;
        self.generate_rust(&program)
    }

    /// Transpile a parsed program to C after checking Poly semantics.
    pub fn transpile_c_checked(&self, source: &str) -> Result<String, String> {
        let program = Self::parse_target(source, "c")?;
        check_program(&program).map_err(|errors| {
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        })?;
        poly_c_codegen::transpile(&program)
    }

    /// Transpile Poly source to the selected target (`rust` or `c`).
    pub fn transpile_target(&self, source: &str, target: &str) -> Result<String, String> {
        match target {
            "rust" | "rs" => self.transpile_checked(source),
            "c" => self.transpile_c_checked(source),
            other => Err(format!(
                "Unknown target '{}'. Supported targets: rust, c",
                other
            )),
        }
    }

    fn generate_rust(&self, program: &Program) -> Result<String, String> {
        let intermediate_representation =
            poly_intermediate_representation::generator::generate(program);
        let mut codegen =
            poly_intermediate_representation::IntermediateRepresentationCodeGen::new();
        Ok(codegen.generate(&intermediate_representation))
    }

    fn select_target(mut program: Program, language: &str) -> Result<Program, String> {
        // Filter only after parsing the complete source: this preserves useful
        // diagnostics for mixed-target files while ensuring the checker and
        // backend cannot accidentally see a helper from another language.
        if program.statements.iter().any(|statement| {
            matches!(
                &statement.node,
                poly_parser::Statement::ForeignBlock { language: block_language, .. }
                    if block_language == "cpp"
            )
        }) {
            return Err(
                "#cpp blocks are reserved for a future backend; supported targets are rust and c"
                    .to_string(),
            );
        }
        // Non-selected foreign blocks and extern declarations are removed
        // before semantic checking; ordinary Poly statements are retained for
        // every target.
        program
            .statements
            .retain(|statement| match &statement.node {
                poly_parser::Statement::ForeignBlock {
                    language: block_language,
                    ..
                } => block_language == language,
                poly_parser::Statement::ExternFunctionDeclaration(declaration) => {
                    declaration.target == language
                }
                _ => true,
            });
        Ok(program)
    }

    /// Transpile Poly source through the intermediate representation pipeline
    /// with optimization.
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
    pub fn transpile_with_source_map(
        &mut self,
        source: &str,
    ) -> Result<(String, SourceMap), String> {
        let rust_code = self.transpile(source)?;
        let mut source_map = SourceMap::new(source, &rust_code);
        let source_line_count = source.lines().count().max(1);
        for (index, _) in rust_code.lines().enumerate() {
            source_map.add_mapping(index + 1, (index + 1).min(source_line_count));
        }
        self.register_statement_symbols(source, &mut source_map);
        self.source_map = Some(source_map.clone());
        Ok((rust_code, source_map))
    }

    /// Record top-level names against their parser spans for editor and CLI
    /// diagnostics; malformed programs simply produce no symbol entries.
    fn register_statement_symbols(&self, source: &str, source_map: &mut SourceMap) {
        use poly_lexer::diagnostics::line_col;
        let Ok(program) = Self::parse(source) else {
            return;
        };
        for spanned in &program.statements {
            let (line, column) = line_col(source, spanned.span.start);
            match &spanned.node {
                poly_parser::Statement::FunctionDeclaration(function) => {
                    source_map.add_symbol(&function.name, line, column, function.name.len());
                }
                poly_parser::Statement::StructDeclaration(decl) => {
                    source_map.add_symbol(&decl.name, line, column, decl.name.len());
                }
                poly_parser::Statement::EnumDeclaration(decl) => {
                    source_map.add_symbol(&decl.name, line, column, decl.name.len());
                }
                poly_parser::Statement::VarDeclaration { name, .. }
                | poly_parser::Statement::LetDeclaration { name, .. }
                | poly_parser::Statement::ConstDeclaration { name, .. } => {
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

    #[test]
    fn target_dispatch_rejects_unknown_backends() {
        let error = Transpiler::new()
            .transpile_target("put 1", "cpp")
            .unwrap_err();
        assert!(error.contains("Supported targets: rust, c"));
    }

    #[test]
    fn cpp_is_explicitly_rejected_until_a_backend_exists() {
        let source = "#cpp\nint value() { return 1; }\n#endcpp\nput value()";
        let error = Transpiler::new().transpile_target(source, "c").unwrap_err();
        assert!(error.contains("#cpp blocks are reserved"));
    }

    #[test]
    fn target_selection_keeps_only_matching_foreign_blocks() {
        let source = "extern rust fn rust_value(): i32\nextern c fn c_value(): i32\n#rust\nfn rust_value() -> i32 { 1 }\n#endrust\n#c\nint c_value(void) { return 2; }\n#endc\nvar result i32 := rust_value()\nput result";
        let rust = Transpiler::new().transpile_target(source, "rust").unwrap();
        assert!(rust.contains("rust_value"));
        assert!(!rust.contains("c_value"));

        let c_source = source.replace("rust_value()", "c_value()");
        let c = Transpiler::new().transpile_target(&c_source, "c").unwrap();
        assert!(c.contains("c_value"));
        assert!(!c.contains("rust_value"));
    }
}

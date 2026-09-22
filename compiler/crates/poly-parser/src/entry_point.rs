//! Program entry-point validation.
//!
//! Poly requires an explicit entry point: a program must declare
//! `fn main()` ... `end fn` (optionally `async fn main()`). Top-level
//! executable statements — variables, `put`, loops, assignments, expressions —
//! are rejected so every artifact starts from the declared entry point.
//!
//! Declaration statements (functions, structs, enums, traits, impls, modules,
//! consts, aliases, `use`, foreign blocks, extern declarations) are allowed at
//! the top level. `const` declarations are grouped with declarations because
//! the code generators hoist them to file/namespace scope; only statements
//! that would run *inside* the generated entry point are rejected.

use crate::ast::{Program, Statement};

/// Validate that a parsed program declares an explicit entry point.
///
/// Returns a human-readable diagnostic when the program has top-level
/// executable statements instead of `fn main() ... end fn`.
pub fn require_explicit_main(program: &Program) -> Result<(), String> {
    let has_main = program.statements.iter().any(|statement| {
        matches!(
            &statement.node,
            Statement::FunctionDeclaration(function)
                if function.name == "main" && function.params.is_empty()
        )
    });

    if has_main {
        return Ok(());
    }

    for statement in &program.statements {
        if is_executable_statement(&statement.node) {
            return Err(format!(
                "program has no `fn main() ... end fn` entry point: top-level executable statements are not allowed (first offender at byte offset {})",
                statement.span.start
            ));
        }
    }

    Ok(())
}

/// True for statements that would execute at program start if kept at the top
/// level. Declarations of any kind return `false`.
fn is_executable_statement(statement: &Statement) -> bool {
    match statement {
        // Declarations allowed at module scope.
        Statement::FunctionDeclaration(_)
        | Statement::StructDeclaration(_)
        | Statement::EnumDeclaration(_)
        | Statement::TraitDeclaration(_)
        | Statement::ImplDeclaration(_)
        | Statement::ModuleDeclaration(_)
        | Statement::UseDeclaration(_)
        | Statement::DependencyDeclaration(_)
        | Statement::TypeDeclaration(_)
        | Statement::ConstDeclaration { .. }
        | Statement::ForeignBlock { .. }
        | Statement::ExternFunctionDeclaration(_) => false,

        // Everything else would execute inside the generated entry point.
        Statement::VarDeclaration { .. }
        | Statement::LetDeclaration { .. }
        | Statement::Assignment { .. }
        | Statement::ExpressionStatement(_)
        | Statement::ReturnStatement(_)
        | Statement::BreakStatement
        | Statement::ContinueStatement
        | Statement::PutStatement { .. }
        | Statement::ErrorStatement(_)
        | Statement::WarnStatement(_)
        | Statement::InfoStatement(_)
        | Statement::Block(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Parser;
    use poly_lexer::Lexer;

    fn parse(source: &str) -> Program {
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty(), "lexer errors: {errors:?}");
        let mut parser = Parser::new(&tokens);
        parser
            .parse()
            .unwrap_or_else(|e| panic!("parse error: {e}"))
    }

    #[test]
    fn accepts_explicit_main() -> Result<(), Box<dyn std::error::Error>> {
        let program = parse("fn main()\n    put \"hi\"\nend fn\n");
        assert!(require_explicit_main(&program).is_ok());
        Ok(())
    }

    #[test]
    fn accepts_declarations_without_executable_statements() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = "const N := 5\nfn helper(): i32\n    return N\nend fn\n";
        let program = parse(source);
        assert!(require_explicit_main(&program).is_ok());
        Ok(())
    }

    #[test]
    fn rejects_top_level_put() -> Result<(), Box<dyn std::error::Error>> {
        let program = parse("put \"hello\"\n");
        assert!(require_explicit_main(&program).is_err());
        Ok(())
    }

    #[test]
    fn rejects_top_level_var() -> Result<(), Box<dyn std::error::Error>> {
        let program = parse("var x i32 := 1\n");
        assert!(require_explicit_main(&program).is_err());
        Ok(())
    }

    #[test]
    fn rejects_program_with_functions_but_no_main() -> Result<(), Box<dyn std::error::Error>> {
        let source = "fn helper(): i32\n    return 1\nend fn\nput helper()\n";
        let program = parse(source);
        assert!(require_explicit_main(&program).is_err());
        Ok(())
    }

    #[test]
    fn accepts_async_main() -> Result<(), Box<dyn std::error::Error>> {
        let program = parse("async fn main()\n    put \"hi\"\nend fn\n");
        assert!(require_explicit_main(&program).is_ok());
        Ok(())
    }
}

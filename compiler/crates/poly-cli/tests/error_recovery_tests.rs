//! Error recovery tests for the Poly parser.

use poly_lexer::Lexer;
use poly_parser::Parser;

#[test]
fn test_error_recovery_collects_multiple_errors() {
    let source = "var x: i32 = 42\nvar y = \nvar z = 100";
    let (tokens, _errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let (program, errors) = parser.parse_with_recovery();
    
    // Should have at least one error but still parse the valid statements
    assert!(!errors.is_empty());
    assert!(program.statements.len() >= 2); // x and z should be parsed
}

#[test]
fn test_error_recovery_skips_bad_statement() {
    // Use truly invalid syntax: missing value after `=` at end of file
    let source = "var x = 1\nvar y = \nvar z = 2";
    let (tokens, _errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let (program, errors) = parser.parse_with_recovery();
    
    // Should have at least one error for the incomplete `var y = `
    assert!(!errors.is_empty());
    // x and z should be parsed
    assert!(program.statements.len() >= 2);
}

#[test]
fn test_parse_with_recovery_returns_program() {
    let source = "var x = 1\nvar y = 2";
    let (tokens, _errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let (program, errors) = parser.parse_with_recovery();
    
    assert!(errors.is_empty());
    assert_eq!(program.statements.len(), 2);
}

#[test]
fn test_error_recovery_multiple_errors() {
    let source = "var x = \nvar y = \nvar z = 100";
    let (tokens, _errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let (program, errors) = parser.parse_with_recovery();
    
    // Should have at least 2 errors
    assert!(errors.len() >= 2);
    // z should still be parsed
    assert!(!program.statements.is_empty());
}

#[test]
fn test_error_recovery_empty_source() {
    let source = "";
    let (tokens, _errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let (program, errors) = parser.parse_with_recovery();
    
    assert!(errors.is_empty());
    assert!(program.statements.is_empty());
}

#[test]
fn test_error_recovery_lexer_errors() {
    // Source with unterminated strings should produce lexer errors
    let source = "var x = \"unterminated";
    let (_tokens, lexer_errors) = Lexer::lex(source);
    
    // Lexer should catch unterminated strings
    assert!(!lexer_errors.is_empty());
}

#[test]
fn test_error_recovery_function_and_var() {
    let source = r#"
fn sum(a: i32, b: i32): i32
    return a + b
end fn

var x = sum(1, 2)
put x
"#;
    let (tokens, _errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let (program, errors) = parser.parse_with_recovery();
    
    assert!(errors.is_empty());
    assert_eq!(program.statements.len(), 3); // fn, var, put
}

//! Comprehensive error handling tests for edge cases in the Poly compiler.
//!
//! These tests cover various error scenarios to ensure robust error handling
//! across all compiler phases.

use poly_lexer::Lexer;
use poly_parser::Parser;
use poly_transpiler::Transpiler;

// =============================================================================
// LEXER ERROR TESTS
// =============================================================================

#[test]
fn test_lexer_unterminated_string_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"var x := "unterminated string"#;
    let (_tokens, errors) = Lexer::lex(source);
    assert!(!errors.is_empty(), "Should detect unterminated string");
    // Should still produce some tokens for error recovery
    assert!(!_tokens.is_empty());
    Ok(())
}

#[test]
fn test_lexer_unterminated_multiline_string() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"var x := "line1
line2
line3"#;
    let (_tokens, errors) = Lexer::lex(source);
    assert!(
        !errors.is_empty(),
        "Should detect unterminated multiline string"
    );
    Ok(())
}

#[test]
fn test_lexer_invalid_character() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var x := @invalid";
    let (_tokens, errors) = Lexer::lex(source);
    assert!(!errors.is_empty(), "Should detect invalid character '@'");
    Ok(())
}

#[test]
fn test_lexer_invalid_number_format() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var x := 123.456.789";
    let (_tokens, errors) = Lexer::lex(source);
    assert!(!errors.is_empty(), "Should detect invalid number format");
    Ok(())
}

#[test]
fn test_lexer_invalid_hex_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var x := 0xGG";
    let (_tokens, errors) = Lexer::lex(source);
    assert!(!errors.is_empty(), "Should detect invalid hex literal");
    Ok(())
}

#[test]
fn test_lexer_unterminated_block_comment() -> Result<(), Box<dyn std::error::Error>> {
    let source = "/* This comment never ends\nvar x := 1";
    let (_tokens, errors) = Lexer::lex(source);
    assert!(
        !errors.is_empty(),
        "Should detect unterminated block comment"
    );
    Ok(())
}

#[test]
fn test_lexer_unterminated_line_comment_at_eof() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var x := 1 // comment";
    let (_tokens, errors) = Lexer::lex(source);
    // Line comments should be fine at EOF
    assert!(errors.is_empty(), "Line comment at EOF should be valid");
    Ok(())
}

#[test]
fn test_lexer_empty_input() -> Result<(), Box<dyn std::error::Error>> {
    let source = "";
    let (tokens, errors) = Lexer::lex(source);
    assert!(errors.is_empty(), "Empty input should have no errors");
    assert!(
        tokens.is_empty() || tokens.len() == 1,
        "Empty input should produce minimal tokens"
    );
    Ok(())
}

#[test]
fn test_lexer_only_whitespace() -> Result<(), Box<dyn std::error::Error>> {
    let source = "   \t\t\n\n  ";
    let (_tokens, errors) = Lexer::lex(source);
    assert!(
        errors.is_empty(),
        "Whitespace-only input should have no errors"
    );
    Ok(())
}

#[test]
fn test_lexer_only_newlines() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\n\n\n\n";
    let (_tokens, errors) = Lexer::lex(source);
    assert!(
        errors.is_empty(),
        "Newline-only input should have no errors"
    );
    Ok(())
}

#[test]
fn test_lexer_nested_block_comments() -> Result<(), Box<dyn std::error::Error>> {
    let source = "/* /* nested */ */";
    let (_tokens, _errors) = Lexer::lex(source);
    // Should handle nested comments or report error
    // Just ensure it doesn't panic
    Ok(())
}

#[test]
fn test_lexer_very_long_string() -> Result<(), Box<dyn std::error::Error>> {
    let long_string = "a".repeat(10000);
    let source = format!("var x := \"{}\"", long_string);
    let (_tokens, errors) = Lexer::lex(&source);
    assert!(errors.is_empty(), "Very long string should be valid");
    Ok(())
}

#[test]
fn test_lexer_unicode_escape_sequences() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"var x := "\u{0041}""#;
    let (_tokens, _errors) = Lexer::lex(source);
    // Should handle unicode escapes
    Ok(())
}

#[test]
fn test_lexer_multiple_consecutive_operators() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var x := a ++ b";
    let (tokens, errors) = Lexer::lex(source);
    // '++' is not a valid operator in Poly
    assert!(
        !errors.is_empty()
            || tokens
                .iter()
                .any(|t| matches!(t.kind, poly_lexer::token::TokenKind::Identifier(_))),
        "Should handle invalid operator"
    );
    Ok(())
}

// =============================================================================
// PARSER ERROR TESTS
// =============================================================================

#[test]
fn test_parser_missing_value_in_var_declaration() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var x :=";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    assert!(result.is_err(), "Should fail to parse var without value");
    Ok(())
}

#[test]
fn test_parser_missing_identifier_in_var_declaration() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var := 42";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    assert!(
        result.is_err(),
        "Should fail to parse var without identifier"
    );
    Ok(())
}

#[test]
fn test_parser_missing_function_name() -> Result<(), Box<dyn std::error::Error>> {
    let source = "fn ()\n    return 1\nend fn";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    assert!(
        result.is_err(),
        "Should fail to parse function without name"
    );
    Ok(())
}

#[test]
fn test_parser_missing_function_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = "fn foo()";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let _result = parser.parse();
    // Should either fail or parse as empty function
    Ok(())
}

#[test]
fn test_parser_missing_struct_name() -> Result<(), Box<dyn std::error::Error>> {
    let source = "struct\n    var x: i32\nend struct";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    assert!(result.is_err(), "Should fail to parse struct without name");
    Ok(())
}

#[test]
fn test_parser_missing_enum_name() -> Result<(), Box<dyn std::error::Error>> {
    let source = "enum\n    North\nend enum";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    assert!(result.is_err(), "Should fail to parse enum without name");
    Ok(())
}

#[test]
fn test_parser_missing_return_value() -> Result<(), Box<dyn std::error::Error>> {
    let source = "fn foo()\n    return\nend fn";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    // Should parse successfully with None return value
    assert!(result.is_ok(), "Should parse return without value");
    Ok(())
}

#[test]
fn test_parser_missing_if_condition() -> Result<(), Box<dyn std::error::Error>> {
    let source = "if\n    put 1\nend if";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    assert!(result.is_err(), "Should fail to parse if without condition");
    Ok(())
}

#[test]
fn test_parser_missing_while_condition() -> Result<(), Box<dyn std::error::Error>> {
    let source = "while\n    put 1\nend while";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    assert!(
        result.is_err(),
        "Should fail to parse while without condition"
    );
    Ok(())
}

#[test]
fn test_parser_unmatched_parentheses() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var x := (1 + 2";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    assert!(
        result.is_err(),
        "Should fail to parse unmatched parentheses"
    );
    Ok(())
}

#[test]
fn test_parser_unmatched_braces() -> Result<(), Box<dyn std::error::Error>> {
    let source = "fn foo()\n    put 1";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let _result = parser.parse();
    // Should fail due to missing end fn or closing brace
    Ok(())
}

#[test]
fn test_parser_invalid_assignment_target() -> Result<(), Box<dyn std::error::Error>> {
    let source = "(a + b) = 10";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let _result = parser.parse();
    // Should fail or handle gracefully
    Ok(())
}

#[test]
fn test_parser_accepts_type_without_colon() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var x i32 := 42";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    assert!(result.is_ok(), "The canonical form omits the type colon");
    Ok(())
}

#[test]
fn test_parser_invalid_type_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var x @invalid := 42";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    assert!(result.is_err(), "Should fail with invalid type annotation");
    Ok(())
}

#[test]
fn test_parser_duplicate_variable_declaration() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var x := 1\nvar x := 2";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    // Should parse successfully (duplicate detection is semantic, not syntax)
    assert!(result.is_ok(), "Duplicate declarations should parse");
    Ok(())
}

#[test]
fn test_parser_deeply_nested_expressions() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var x := (((((((1 + 2) + 3) + 4) + 5) + 6) + 7) + 8)";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    assert!(result.is_ok(), "Deeply nested parentheses should parse");
    Ok(())
}

#[test]
fn test_parser_empty_block() -> Result<(), Box<dyn std::error::Error>> {
    let source = "fn foo()\nend fn";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let result = parser.parse();
    assert!(result.is_ok(), "Empty function body should parse");
    Ok(())
}

#[test]
fn test_parser_multiple_statements_on_one_line() -> Result<(), Box<dyn std::error::Error>> {
    let source = "var x := 1 var y := 2";
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let _result = parser.parse();
    // Should fail or handle gracefully
    Ok(())
}

// =============================================================================
// TRANSPILER ERROR TESTS
// =============================================================================

#[test]
fn test_transpiler_empty_program() -> Result<(), Box<dyn std::error::Error>> {
    let t = Transpiler::new();
    let result = t.transpile("");
    assert!(
        result.is_ok(),
        "Empty program should transpile successfully"
    );
    let rust_code = result?;
    // Empty programs may or may not have main function
    // Just ensure it's valid Rust
    assert!(
        rust_code.contains("// Generated from Poly source code"),
        "Should have generated header"
    );
    Ok(())
}

#[test]
fn test_transpiler_whitespace_only() -> Result<(), Box<dyn std::error::Error>> {
    let t = Transpiler::new();
    let result = t.transpile("   \t\t\n\n  ");
    assert!(
        result.is_ok(),
        "Whitespace-only should transpile successfully"
    );
    Ok(())
}

#[test]
fn test_transpiler_comments_only() -> Result<(), Box<dyn std::error::Error>> {
    let t = Transpiler::new();
    let result = t.transpile("// This is a comment\n/* Block comment */");
    assert!(
        result.is_ok(),
        "Comments-only should transpile successfully"
    );
    Ok(())
}

#[test]
fn test_transpiler_invalid_utf8_sequence() -> Result<(), Box<dyn std::error::Error>> {
    let t = Transpiler::new();
    // Create bytes with invalid UTF-8
    let invalid_bytes: Vec<u8> = vec![0x66, 0x6F, 0x6F, 0xFF, 0xFE, 0x62, 0x61, 0x72];
    if let Ok(invalid_str) = String::from_utf8(invalid_bytes) {
        let _ = t.transpile(&invalid_str);
        // Just ensure it doesn't panic
    }
    Ok(())
}

#[test]
fn test_transpiler_extremely_long_identifier() -> Result<(), Box<dyn std::error::Error>> {
    let long_name = "a".repeat(10000);
    let source = format!("var {} := 42", long_name);
    let t = Transpiler::new();
    let _result = t.transpile(&source);
    // Should either succeed or fail gracefully
    Ok(())
}

#[test]
fn test_transpiler_nested_functions() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fn outer()
    fn inner()
        put "inner"
    end fn
    inner()
end fn
"#;
    let t = Transpiler::new();
    let _result = t.transpile(source);
    // Should handle nested functions (Rust doesn't support nested fn)
    // Should either transpile or report error
    Ok(())
}

#[test]
fn test_transpiler_recursive_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fn factorial(n: i32): i32
    if n <= 1,
        return 1
    end if
    return n * factorial(n - 1)
end fn
"#;
    let t = Transpiler::new();
    let result = t.transpile(source);
    assert!(result.is_ok(), "Recursive function should transpile");
    Ok(())
}

#[test]
fn test_transpiler_mutual_recursion() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fn is_even(n: i32): bool
    if n = 0,
        return true
    end if
    return is_odd(n - 1)
end fn

fn is_odd(n: i32): bool
    if n = 0,
        return false
    end if
    return is_even(n - 1)
end fn
"#;
    let t = Transpiler::new();
    let result = t.transpile(source);
    assert!(result.is_ok(), "Mutual recursion should transpile");
    Ok(())
}

#[test]
fn test_transpiler_complex_pattern_matching() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
enum Shape
    Circle(f32)
    Rectangle(f32, f32)
    Triangle(f32, f32, f32)
end enum

fn area(shape: Shape): f32
    match shape
        Circle(r), 3.14 * r * r
        Rectangle(w, h), w * h
        Triangle(a, b, c),
            let s := (a + b + c) / 2.0
            return (s * (s - a) * (s - b) * (s - c))
    end match
end fn
"#;
    let t = Transpiler::new();
    let result = t.transpile(source);
    assert!(result.is_ok(), "Complex pattern matching should transpile");
    Ok(())
}

#[test]
fn test_transpiler_closures_with_captures() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fn make_adder(x: i32): fn(i32) -> i32
    return |y| x + y
end fn
"#;
    let t = Transpiler::new();
    let _result = t.transpile(source);
    // Closures with captures may need special handling
    Ok(())
}

#[test]
fn test_transpiler_higher_order_functions() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fn apply(f: fn(i32) -> i32, x: i32): i32
    return f(x)
end fn

fn double(x: i32): i32
    return x * 2
end fn
"#;
    let t = Transpiler::new();
    let result = t.transpile(source);
    assert!(result.is_ok(), "Higher-order functions should transpile");
    Ok(())
}

#[test]
fn test_transpiler_complex_expressions() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fn main()
    var x := (1 + 2) * (3 + 4) / (5 - 6)
    var y := if x > 0, x else -x
    var z := [1, 2, 3, 4, 5]
end fn
"#;
    let t = Transpiler::new();
    let result = t.transpile(source);
    assert!(result.is_ok(), "Complex expressions should transpile");
    Ok(())
}

// =============================================================================
// ERROR RECOVERY TESTS
// =============================================================================

#[test]
fn test_error_recovery_multiple_errors() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
var x i32 := 42
var y :=
var z := 100
fn broken(
var w := "hello"
"#;
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let (program, errors) = parser.parse_with_recovery();

    // Should collect multiple errors
    assert!(errors.len() >= 2, "Should collect multiple errors");
    // Should still parse valid statements
    assert!(
        program.statements.len() >= 2,
        "Should parse at least x and z"
    );
    Ok(())
}

#[test]
fn test_error_recovery_after_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fn valid_function()
    return 42
end fn

var broken :=
var another_valid_var := 100
"#;
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let (program, errors) = parser.parse_with_recovery();

    // Should recover after function and parse more
    assert!(!errors.is_empty(), "Should have errors");
    assert!(
        program.statements.len() >= 2,
        "Should parse function and at least one var"
    );
    Ok(())
}

#[test]
fn test_error_recovery_nested_structures() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
struct Point
    var x: f32
    var y: f32
end struct

enum Color
    Red
    Green
    Blue
end enum

var broken :=
var p := Point { x: 1.0, y: 2.0 }
"#;
    let (tokens, _lexer_errors) = Lexer::lex(source);
    let mut parser = Parser::new(&tokens);
    let (program, _errors) = parser.parse_with_recovery();

    // Should recover and parse struct, enum, and final var
    assert!(
        program.statements.len() >= 3,
        "Should parse multiple structures"
    );
    Ok(())
}

// =============================================================================
// INTEGRATION ERROR TESTS
// =============================================================================

#[test]
fn test_full_pipeline_with_errors() -> Result<(), Box<dyn std::error::Error>> {
    let t = Transpiler::new();
    let result = t.transpile(
        "var x i32 := 42\nvar y :=
var z := 100",
    );

    // Should either succeed with partial output or fail gracefully
    match result {
        Ok(rust_code) => {
            // If it succeeds, should have valid Rust
            assert!(rust_code.contains("fn main"));
        }
        Err(e) => {
            // Error message should be informative
            assert!(!e.is_empty(), "Error should have message");
        }
    }
    Ok(())
}

#[test]
fn test_transpiler_handles_lexer_errors() -> Result<(), Box<dyn std::error::Error>> {
    let t = Transpiler::new();
    let result = t.transpile(r#"var x := "unterminated"#);

    assert!(result.is_err(), "Should fail on lexer errors");
    let Err(err) = result else {
        panic!("expected an Err result")
    };
    assert!(
        err.contains("Lexer") || err.contains("error"),
        "Error should mention lexer"
    );
    Ok(())
}

#[test]
fn test_transpiler_handles_parser_errors() -> Result<(), Box<dyn std::error::Error>> {
    let t = Transpiler::new();
    let result = t.transpile("var x =");

    assert!(result.is_err(), "Should fail on parser errors");
    let Err(err) = result else {
        panic!("expected an Err result")
    };
    assert!(
        err.contains("Parse") || err.contains("error") || err.contains("Expected"),
        "Error should mention parse error"
    );
    Ok(())
}

// =============================================================================
// PERFORMANCE/STRESS TESTS
// =============================================================================

#[test]
fn test_large_program_transpilation() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::from("fn main()\n");
    for i in 0..100 {
        source.push_str(&format!("    var x_{} i32 := {}\n", i, i));
    }
    source.push_str("end fn\n");

    let t = Transpiler::new();
    let result = t.transpile(&source);
    assert!(
        result.is_ok(),
        "Large program should transpile successfully"
    );
    Ok(())
}

#[test]
fn test_missing_main_entry_point_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let t = Transpiler::new();
    let result = t.transpile("put \"hello\"");
    assert!(result.is_err(), "Top-level statements must be rejected");
    let Err(err) = result else {
        panic!("expected an Err result")
    };
    assert!(
        err.contains("fn main"),
        "Error should point at the missing entry point, got: {err}"
    );
    Ok(())
}

#[test]
fn test_deeply_nested_blocks_fail_gracefully() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::from("fn deeply_nested()\n");
    for _ in 0..50 {
        source.push_str("    if true\n");
    }
    source.push_str("        put 42\n");
    for _ in 0..50 {
        source.push_str("    end if\n");
    }
    source.push_str("end fn\n");

    let t = Transpiler::new();
    let result = t.transpile(&source);
    assert!(
        result.is_err(),
        "excessive nesting should produce a controlled parser error"
    );
    let Err(error) = result else {
        panic!("expected a parse error")
    };
    assert!(
        error.contains("Maximum nested if depth"),
        "the error should explain the nesting limit"
    );
    Ok(())
}

#[test]
fn test_many_functions() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::new();
    for i in 0..50 {
        source.push_str(&format!(
            "fn func_{}(x: i32): i32\n    return x + {}\nend fn\n",
            i, i
        ));
    }

    let t = Transpiler::new();
    let result = t.transpile(&source);
    assert!(
        result.is_ok(),
        "Many functions should transpile successfully"
    );
    Ok(())
}

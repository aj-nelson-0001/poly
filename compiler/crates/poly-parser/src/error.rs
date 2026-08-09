//! Parser error types with helpful suggestions.

use std::fmt;

use poly_lexer::token::Span;

/// Errors that can occur during parsing.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub suggestion: Option<String>,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            suggestion: None,
        }
    }

    pub fn with_suggestion(
        message: impl Into<String>,
        span: Span,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            span,
            suggestion: Some(suggestion.into()),
        }
    }

    /// Generate helpful error suggestions based on the error message.
    pub fn generate_suggestion(&mut self) {
        if self.suggestion.is_some() {
            return;
        }

        let msg = self.message.to_lowercase();

        self.suggestion = if msg.contains("expected 'end'") || msg.contains("expected end") {
            Some(
                "Poly uses 'end' to close blocks. Did you forget 'end fn', 'end if', etc.?"
                    .to_string(),
            )
        } else if msg.contains("expected identifier") && msg.contains("after 'fn'") {
            Some("Function declarations need a name: fn my_function(...)".to_string())
        } else if msg.contains("expected ':'") && msg.contains("parameter") {
            Some("Parameters need type annotations: fn foo(x: i32)".to_string())
        } else if msg.contains("Comma") && msg.contains("Expected") {
            Some("If statements require comma: if condition, ... end if".to_string())
        } else if msg.contains("expected 'in'") {
            Some("For loops use 'in': for item in collection ... end for".to_string())
        } else if msg.contains("unexpected token") && msg.contains("return") {
            Some("Return statements: return value or return (no value)".to_string())
        } else if msg.contains("expected type") {
            Some("Valid types: i32, f64, string, bool, char, Vec<T>, Option<T>".to_string())
        } else if msg.contains("expected ','") {
            Some("Separate parameters/arguments with commas: fn foo(a: i32, b: i32)".to_string())
        } else if msg.contains("expected '='") && msg.contains("let") {
            Some("'let' declarations require initialization: let x = value".to_string())
        } else if msg.contains("expected '") && msg.contains("struct") {
            Some("Struct fields: var field_name: type".to_string())
        } else if msg.contains("expected '") && msg.contains("enum") {
            Some("Enum variants: VariantName or VariantName(type)".to_string())
        } else {
            None
        };
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Parse error at {}..{}: {}",
            self.span.start, self.span.end, self.message
        )?;
        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\n  💡 {}", suggestion)?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseError {}

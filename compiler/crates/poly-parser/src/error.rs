//! Parser error types with helpful suggestions.
//!
//! When the parser cannot make sense of the token stream it produces a
//! `ParseError`. Besides a message and the source location, a parse error can
//! carry a *suggestion* — a short hint like "Poly uses 'end' to close blocks".
//! These hints turn cryptic syntax errors into something a learner can act on.

use std::fmt;

// A span records where in the source the error occurred.
use poly_lexer::token::Span;

/// Errors that can occur during parsing.
#[derive(Debug, Clone)]
pub struct ParseError {
    // What went wrong, written for a human.
    pub message: String,
    // Where in the source it happened.
    pub span: Span,
    // An optional, friendlier hint about how to fix it.
    pub suggestion: Option<String>,
}

impl ParseError {
    /// Create an error without a migration hint. The span remains attached so
    /// CLI and editor clients can render the diagnostic against source text.
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            suggestion: None,
        }
    }

    /// Create an error with a targeted replacement suggestion. Keeping the
    /// suggestion on the error avoids duplicating syntax-specific wording in
    /// every caller that displays parser failures.
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
    ///
    /// This is a small "expert system": it inspects the (lower-cased) message
    /// text for patterns and attaches the matching hint. If a suggestion was
    /// already supplied, this does nothing.
    pub fn generate_suggestion(&mut self) {
        // Don't overwrite an explicit suggestion.
        if self.suggestion.is_some() {
            return;
        }

        // Matching is done on lower-case text so casing in the message doesn't
        // matter.
        let msg = self.message.to_lowercase();

        self.suggestion = if msg.contains("expected 'end'") || msg.contains("expected end") {
            // The classic Poly beginner mistake: forgetting `end <keyword>`.
            Some(
                "Poly uses 'end' to close blocks. Did you forget 'end fn', 'end if', etc.?"
                    .to_string(),
            )
        } else if msg.contains("expected identifier") && msg.contains("after 'fn'") {
            Some("Function declarations need a name: fn my_function(...)".to_string())
        } else if msg.contains("expected ':'") && msg.contains("parameter") {
            Some("Parameters need type annotations: fn foo(x: i32)".to_string())
        } else if msg.contains("comma") && msg.contains("expected") {
            Some("If statements use `if condition ... end if`; a comma is accepted for compatibility.".to_string())
        } else if msg.contains("expected 'in'") {
            Some("For loops use 'in': for item in collection ... end for".to_string())
        } else if msg.contains("unexpected token") && msg.contains("return") {
            Some("Return statements: return value or return (no value)".to_string())
        } else if msg.contains("expected type") {
            Some("Valid types: i32, f64, string, bool, char, Vec<T>, Option<T>".to_string())
        } else if msg.contains("expected ','") {
            Some("Separate parameters/arguments with commas: fn foo(a: i32, b: i32)".to_string())
        } else if msg.contains("expected '='") && msg.contains("let") {
            Some("Declarations require initialization with `:=`: let x := value".to_string())
        } else if msg.contains("expected '") && msg.contains("struct") {
            Some("Struct fields use `var field_name: type` inside the struct body".to_string())
        } else if msg.contains("expected '") && msg.contains("enum") {
            Some("Enum variants: VariantName or VariantName(type)".to_string())
        } else {
            // No matching pattern: leave the suggestion empty.
            None
        };
    }
}

impl fmt::Display for ParseError {
    /// Print the error as `Parse error at start..end: message`, followed by a
    /// bullet-pointed suggestion when one exists.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Parse error at {}..{}: {}",
            self.span.start, self.span.end, self.message
        )?;
        // Only print the suggestion line if there is one.
        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\n  💡 {}", suggestion)?;
        }
        Ok(())
    }
}

// Allows `ParseError` to flow through `?` in functions returning boxed errors.
impl std::error::Error for ParseError {}

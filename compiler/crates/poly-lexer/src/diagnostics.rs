//! Source diagnostics for Poly compiler errors.
//!
//! Lexer and parser errors carry byte-offset [`Span`]s into the original
//! source.  This module converts those offsets into 1-based line/column
//! positions and renders human-friendly, rustc-style diagnostics with the
//! offending source line and a caret.  Both the CLI and the language server
//! use these helpers so diagnostics stay consistent across tools.

use crate::token::Span;

/// Convert a byte offset into a 1-based (line, column) pair.
///
/// Lines are split on `\n`; a column is the number of characters (not bytes)
/// since the start of the line, plus one.
pub fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (index, character) in source.char_indices() {
        if index >= offset {
            break;
        }
        if character == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// The text of the 1-based line `line` in `source`, if it exists.
pub fn source_line(source: &str, line: usize) -> Option<&str> {
    source.lines().nth(line.checked_sub(1)?)
}

/// Render a rustc-style diagnostic with the source line and a caret.
///
/// ```text
/// error: message
///   --> <label>:<line>:<col>
///    |
/// LL | source text
///    |    ^^^^
/// ```
pub fn render_error(source: &str, span: Span, label: &str, message: &str) -> String {
    let (line, col) = line_col(source, span.start);
    let (end_line, end_col) = line_col(source, span.end);
    // A span can point just past the final newline (EOF); clamp it to the last
    // real line so the caret lands on visible source text.
    let line = line.min(source.lines().count().max(1));
    let line_text = source_line(source, line).unwrap_or("<unknown source line>");

    let gutter_width = line.to_string().len();
    let gutter = " ".repeat(gutter_width);

    // Caret width: at least one character, clamped to the source line length
    // so the caret never extends past the end of the displayed line.
    let caret_width = if end_line == line {
        (end_col.saturating_sub(col)).max(1)
    } else {
        1
    };
    let caret_padding = " ".repeat(col.saturating_sub(1).min(line_text.chars().count()));

    format!(
        "error: {message}\n  --> {label}:{line}:{col}\n{gutter} |\n{line:>width$} | {line_text}\n{gutter} | {caret_padding}{caret}",
        width = gutter_width,
        caret = "^".repeat(caret_width),
    )
}

/// Render a lexer or parser error with the source context of its span.
pub fn render_span_error(source: &str, span: Span, label: &str, message: &str) -> String {
    render_error(source, span, label, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_counts_first_line() {
        assert_eq!(line_col("var x := 42", 0), (1, 1));
        assert_eq!(line_col("var x := 42", 10), (1, 11));
    }

    #[test]
    fn line_col_crosses_newlines() {
        let source = "var x := 1\nvar y := 2";
        let newline_index = source.find('\n').unwrap();
        assert_eq!(line_col(source, newline_index + 1), (2, 1));
        assert_eq!(line_col(source, newline_index + 4), (2, 4));
    }

    #[test]
    fn line_col_past_end_clamps_to_last_position() {
        let source = "abc";
        assert_eq!(line_col(source, 100), (1, 4));
    }

    #[test]
    fn source_line_returns_requested_line() {
        let source = "one\ntwo\nthree";
        assert_eq!(source_line(source, 2), Some("two"));
        assert_eq!(source_line(source, 5), None);
    }

    #[test]
    fn render_error_includes_position_and_caret() {
        let source = "var x := 42\n";
        let rendered = render_error(source, Span::new(0, 3), "test.poly", "oops");
        assert!(rendered.contains("--> test.poly:1:1"));
        assert!(rendered.contains("var x := 42"));
        assert!(rendered.contains("^^^"));
    }

    #[test]
    fn render_error_multi_line_span_clamps_caret() {
        let source = "var x := 1\nvar y := 2";
        let rendered = render_error(source, Span::new(0, source.len()), "t.poly", "wide span");
        assert!(rendered.contains("t.poly:1:1"));
    }
}

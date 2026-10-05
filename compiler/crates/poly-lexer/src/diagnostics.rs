//! Source diagnostics for Poly compiler errors.
//!
//! Lexer and parser errors carry byte-offset [`Span`]s into the original
//! source.  This module converts those offsets into 1-based line/column
//! positions and renders human-friendly, rustc-style diagnostics with the
//! offending source line and a caret.  Both the CLI and the language server
//! use these helpers so diagnostics stay consistent across tools.
//!
//! In other words: errors elsewhere in the compiler know *which bytes* are
//! wrong, and this module turns those byte numbers into something a person
//! can read — "line 3, column 12, here's the line, and the caret points at
//! the problem".

// A span marks the start/end byte offsets of the offending source text.
use crate::token::Span;

/// Convert a byte offset into a 1-based (line, column) pair.
///
/// Lines are split on `\n`; a column is the number of characters (not bytes)
/// since the start of the line, plus one. Counting characters rather than
/// bytes matters for source containing multi-byte UTF-8 (for example accented
/// letters or emoji), so the reported column matches what the user sees.
pub fn line_col(source: &str, offset: usize) -> (usize, usize) {
    // Both counters start at 1 because humans number lines and columns from 1.
    let mut line = 1usize;
    let mut col = 1usize;
    // Walk the source one character at a time, tracking the byte index too.
    for (index, character) in source.char_indices() {
        // Stop as soon as we reach (or pass) the target offset.
        if index >= offset {
            break;
        }
        if character == '\n' {
            // A newline moves us to the next line and resets the column.
            line += 1;
            col = 1;
        } else {
            // Any other character advances the column by one.
            col += 1;
        }
    }
    (line, col)
}

/// The text of the 1-based line `line` in `source`, if it exists.
///
/// Returns `None` for line 0 or a line number past the end of the file.
pub fn source_line(source: &str, line: usize) -> Option<&str> {
    // `checked_sub(1)` turns the 1-based line number into a 0-based index and
    // yields `None` when `line` is 0, avoiding an underflow.
    source.lines().nth(line.checked_sub(1)?)
}

/// Render a rustc-style diagnostic with the source line and a caret.
///
/// The output looks like this:
///
/// ```text
/// error: message
///   --> <label>:<line>:<col>
///    |
/// LL | source text
///    |    ^^^^
/// ```
pub fn render_error(source: &str, span: Span, label: &str, message: &str) -> String {
    // Convert both ends of the span to line/column so we can size the caret.
    let (line, col) = line_col(source, span.start);
    let (end_line, end_col) = line_col(source, span.end);
    // A span can point just past the final newline (EOF); clamp it to the last
    // real line so the caret lands on visible source text.
    let line = line.min(source.lines().count().max(1));
    // Fetch the actual text of that line; fall back to a placeholder if the
    // line number is somehow out of range.
    let line_text = source_line(source, line).unwrap_or("<unknown source line>");

    // The left gutter holds the line number; width it to fit the largest
    // possible number so the `|` characters stay aligned.
    let gutter_width = line.to_string().len();
    let gutter = " ".repeat(gutter_width);

    // Caret width: at least one character, clamped to the source line length
    // so the caret never extends past the end of the displayed line. If the
    // span crosses multiple lines we simply show a single caret.
    let caret_width = if end_line == line {
        (end_col.saturating_sub(col)).max(1)
    } else {
        1
    };
    // Indent the caret so it sits under the first character of the span,
    // without running off the end of the displayed line.
    let caret_padding = " ".repeat(col.saturating_sub(1).min(line_text.chars().count()));

    format!(
        "error: {message}\n  --> {label}:{line}:{col}\n{gutter} |\n{line:>width$} | {line_text}\n{gutter} | {caret_padding}{caret}",
        width = gutter_width,
        caret = "^".repeat(caret_width),
    )
}

/// Render a lexer or parser error with the source context of its span.
///
/// This is a thin wrapper over [`render_error`] that exists so callers reading
/// a lexer/parser error don't need to know about the more general function.
pub fn render_span_error(source: &str, span: Span, label: &str, message: &str) -> String {
    render_error(source, span, label, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_counts_first_line() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(line_col("var x := 42", 0), (1, 1));
        assert_eq!(line_col("var x := 42", 10), (1, 11));
        Ok(())
    }

    #[test]
    fn line_col_crosses_newlines() -> Result<(), Box<dyn std::error::Error>> {
        let source = "var x := 1\nvar y := 2";
        let newline_index = source.find('\n').ok_or("source should contain a newline")?;
        assert_eq!(line_col(source, newline_index + 1), (2, 1));
        assert_eq!(line_col(source, newline_index + 4), (2, 4));
        Ok(())
    }

    #[test]
    fn line_col_past_end_clamps_to_last_position() -> Result<(), Box<dyn std::error::Error>> {
        let source = "abc";
        assert_eq!(line_col(source, 100), (1, 4));
        Ok(())
    }

    #[test]
    fn source_line_returns_requested_line() -> Result<(), Box<dyn std::error::Error>> {
        let source = "one\ntwo\nthree";
        assert_eq!(source_line(source, 2), Some("two"));
        assert_eq!(source_line(source, 5), None);
        Ok(())
    }

    #[test]
    fn render_error_includes_position_and_caret() -> Result<(), Box<dyn std::error::Error>> {
        let source = "var x := 42\n";
        let rendered = render_error(source, Span::new(0, 3), "test.poly", "oops");
        assert!(rendered.contains("--> test.poly:1:1"));
        assert!(rendered.contains("var x := 42"));
        assert!(rendered.contains("^^^"));
        Ok(())
    }

    #[test]
    fn render_error_multi_line_span_clamps_caret() -> Result<(), Box<dyn std::error::Error>> {
        let source = "var x := 1\nvar y := 2";
        let rendered = render_error(source, Span::new(0, source.len()), "t.poly", "wide span");
        assert!(rendered.contains("t.poly:1:1"));
        Ok(())
    }
}

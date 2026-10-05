//! The Poly language lexer.
//!
//! Converts Poly source code into a stream of tokens.
//!
//! The lexer walks the source text from left to right. At each position it
//! decides what kind of token starts there (a number, a name, a symbol, ...)
//! and hands off to a small scanner method that consumes the whole token. When
//! it cannot recognise something it records an error but keeps going, so one
//! pass can surface several problems.

// Error types produced while lexing (and the categories they belong to).
use crate::error::{LexerError, LexerErrorKind};
// The token/span types the lexer builds.
use crate::token::{Span, Token, TokenKind};

/// The Poly language lexer.
///
/// Converts source code into a stream of tokens.
///
/// The fields are simply the source split into characters, a current position
/// into that list, and the results accumulated so far (tokens and errors).
pub struct Lexer {
    // The source, stored as a `Vec<char>` so indexing is O(1) and we never
    // split a multi-byte UTF-8 character by accident.
    chars: Vec<char>,
    // How many characters have been consumed so far.
    pos: usize,
    // The tokens produced, in source order.
    tokens: Vec<Token>,
    // Any errors encountered; the lexer does not stop at the first one.
    errors: Vec<LexerError>,
}

impl Lexer {
    /// Create a new lexer for the given source code.
    ///
    /// This only sets up the state; call [`Lexer::tokenize`] to actually do the
    /// work, or use the convenience [`Lexer::lex`] method.
    pub fn new(source: &str) -> Self {
        Self {
            // Collect the characters once; all later work indexes this list.
            chars: source.chars().collect(),
            pos: 0,
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Lex the entire source code and return tokens and errors.
    ///
    /// This is the simplest entry point for callers: give it source text, get
    /// back the token list and any lexing errors.
    pub fn lex(source: &str) -> (Vec<Token>, Vec<LexerError>) {
        // Build a lexer, run it to completion, then hand back the results.
        let mut lexer = Lexer::new(source);
        lexer.tokenize();
        (lexer.tokens, lexer.errors)
    }

    /// Tokenize the source code.
    pub fn tokenize(&mut self) {
        // Scan until the sentinel; individual scanners are responsible for
        // consuming the complete token and advancing `pos`.
        while !self.is_at_end() {
            self.scan_token();
        }
        // Every token stream ends with an EOF marker so the parser can rely on
        // always having one more token to look at.
        self.tokens
            .push(Token::new(TokenKind::Eof, Span::new(self.pos, self.pos)));
    }

    /// Get the tokens produced by lexing.
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Get any errors produced during lexing.
    pub fn errors(&self) -> &[LexerError] {
        &self.errors
    }

    // === Character navigation ===
    // These small helpers are the lexer's "cursor". They hide the details of
    // bounds checking so the scanning code can read like English.

    /// Have we consumed every character yet?
    fn is_at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    /// The character under the cursor, or a NUL (`'\0'`) sentinel at the end.
    /// Using a sentinel avoids a bounds check at every call site.
    fn current(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.chars[self.pos]
        }
    }

    /// The character just after the cursor, or NUL at the end.
    /// Used for two-character operators such as `==` and `->`.
    fn peek_next(&self) -> char {
        if self.pos + 1 >= self.chars.len() {
            '\0'
        } else {
            self.chars[self.pos + 1]
        }
    }

    /// Consume the character under the cursor and return it.
    fn advance(&mut self) -> char {
        let ch = self.current();
        self.pos += 1;
        ch
    }

    /// Consume the next character only if it equals `expected`.
    /// Returns `true` when it matched (and was consumed). This is how the lexer
    /// recognises optional second characters like the `=` in `<=`.
    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.current() != expected {
            false
        } else {
            self.pos += 1;
            true
        }
    }

    /// Skip over spaces, tabs, blank lines, and comments.
    fn skip_whitespace(&mut self) {
        // Newlines are discarded here because Poly block termination is
        // expressed with `end <keyword>`, not line-sensitive indentation.
        while !self.is_at_end() {
            match self.current() {
                // Ordinary horizontal whitespace: just step over it.
                ' ' | '\r' | '\t' => {
                    self.advance();
                }
                '\n' => {
                    // Don't add Newline tokens - they're only meaningful
                    // in specific contexts (which the parser handles)
                    self.advance();
                }
                '#' => {
                    // `#` introduces either a foreign-language block (which is
                    // a real token) or a one-line comment. Stop so the block
                    // opener reaches `scan_token`; otherwise treat it as a
                    // comment and keep skipping.
                    if self.foreign_block_language_at_current().is_some() {
                        break;
                    }
                    self.skip_comment();
                }
                // `//` and `/* ... */` are also comment openers.
                '/' if self.peek_next() == '*' || self.peek_next() == '/' => {
                    self.skip_comment();
                }
                // Anything else is the start of a real token.
                _ => break,
            }
        }
    }

    /// Consume one comment. Handles the three comment styles Poly accepts.
    fn skip_comment(&mut self) {
        if self.current() == '#' {
            // Single-line comment: skip the #
            self.advance();
            // Consume everything up to (but not including) the newline.
            while !self.is_at_end() && self.current() != '\n' {
                self.advance();
            }
        } else if self.current() == '/' && self.peek_next() == '/' {
            // C++-style single-line comment: skip the //
            self.advance();
            self.advance();
            while !self.is_at_end() && self.current() != '\n' {
                self.advance();
            }
        } else if self.current() == '/' && self.peek_next() == '*' {
            // Block comment: skip /* ... */
            self.advance(); // /
            self.advance(); // *
                            // Remember where the comment body begins so we can point a
                            // diagnostic at it if the closing `*/` never arrives.
            let start = self.pos;
            while !self.is_at_end() {
                if self.current() == '*' && self.peek_next() == '/' {
                    self.advance(); // *
                    self.advance(); // /
                    return;
                }
                self.advance();
            }
            // Reached the end of input without `*/`: report it.
            self.errors.push(LexerError::new(
                LexerErrorKind::UnterminatedComment,
                Span::new(start, self.pos),
                "Unterminated block comment",
            ));
        }
    }

    // === Token scanning ===

    /// Recognise and consume exactly one token, starting at the cursor.
    ///
    /// The big `match` below is the lexer's dispatch table: the first character
    /// decides which scanner runs. Scanners called from here are responsible
    /// for consuming the *whole* token and leaving `pos` just past it.
    fn scan_token(&mut self) {
        // Whitespace/comments may consume the rest of the input, so always
        // re-check EOF before recording the next token's starting offset.
        self.skip_whitespace();
        if self.is_at_end() {
            return;
        }

        // `start` is the byte offset of this token's first character; it is
        // passed to `add_token` (which spans from `start` to the current pos).
        let start = self.pos;
        // Consume the first character; the match decides what to do with it.
        let ch = self.advance();

        match ch {
            // Single character tokens: the character alone is the whole token.
            '(' => self.add_token(TokenKind::LParen, start),
            ')' => self.add_token(TokenKind::RParen, start),
            '[' => self.add_token(TokenKind::LBracket, start),
            ']' => self.add_token(TokenKind::RBracket, start),
            '{' => self.add_token(TokenKind::LBrace, start),
            '}' => self.add_token(TokenKind::RBrace, start),
            ',' => self.add_token(TokenKind::Comma, start),
            ';' => self.add_token(TokenKind::Semicolon, start),
            // `:` can begin `::` or `:=`, otherwise it is a lone colon.
            ':' => {
                if self.match_char(':') {
                    self.add_token(TokenKind::ColonColon, start);
                } else if self.match_char('=') {
                    self.add_token(TokenKind::ColonEq, start);
                } else {
                    self.add_token(TokenKind::Colon, start);
                }
            }
            // `.` can begin `..=`, `..`, or stand alone (field access).
            '.' => {
                if self.match_char('.') {
                    if self.match_char('=') {
                        self.add_token(TokenKind::DotDotEq, start);
                    } else {
                        self.add_token(TokenKind::DotDot, start);
                    }
                } else {
                    self.add_token(TokenKind::Dot, start);
                }
            }
            '~' => self.add_token(TokenKind::Tilde, start),

            // String and character literals
            '"' => self.scan_string(start),
            '\'' => self.scan_unicode_char(start),
            // A leading `u` might introduce a legacy `u"..."` string, otherwise
            // it starts an ordinary identifier/keyword.
            'u' => {
                if self.current() == '"' {
                    self.advance(); // consume the legacy prefix quote
                    self.scan_unicode_string(start);
                } else {
                    self.scan_identifier_or_keyword(start);
                }
            }

            // Numbers. `0` may start a base-prefixed literal (0x/0b/0o).
            '0' => {
                if self.current() == 'x' || self.current() == 'X' {
                    self.scan_hex_number(start);
                } else if self.current() == 'b' || self.current() == 'B' {
                    self.scan_binary_number(start);
                } else if self.current() == 'o' || self.current() == 'O' {
                    self.scan_octal_number(start);
                } else {
                    self.scan_number(start);
                }
            }
            '1'..='9' => self.scan_number(start),

            // Identifiers and keywords (including a lone `_`).
            'a'..='z' | 'A'..='Z' | '_' => self.scan_identifier_or_keyword(start),

            // Operators
            '+' => self.add_token(TokenKind::Plus, start),
            // `-` may be the start of the arrow `->`.
            '-' => {
                if self.match_char('>') {
                    self.add_token(TokenKind::Arrow, start);
                } else {
                    self.add_token(TokenKind::Minus, start);
                }
            }
            '*' => self.add_token(TokenKind::Star, start),
            '/' => self.add_token(TokenKind::Slash, start),
            '%' => self.add_token(TokenKind::Percent, start),
            // `=` may be `==` or, in closure/match context, the fat arrow `=>`.
            '=' => {
                if self.match_char('=') {
                    self.add_token(TokenKind::EqEq, start);
                } else if self.match_char('>') {
                    self.add_token(TokenKind::FatArrow, start);
                } else {
                    self.add_token(TokenKind::Eq, start);
                }
            }
            '!' => {
                if self.match_char('=') {
                    self.add_token(TokenKind::NotEq, start);
                } else {
                    // Standalone `!` is the retired spelling of `not`; a
                    // dedicated token lets the parser reject it precisely
                    // while the `not` keyword keeps TokenKind::Not.
                    self.add_token(TokenKind::Bang, start);
                }
            }
            // `<` may be `<=`, `<<`, or plain less-than.
            '<' => {
                if self.match_char('=') {
                    self.add_token(TokenKind::LtEq, start);
                } else if self.match_char('<') {
                    self.add_token(TokenKind::LtLt, start);
                } else {
                    self.add_token(TokenKind::Lt, start);
                }
            }
            // `>` similarly may be `>=`, `>>`, or plain greater-than.
            '>' => {
                if self.match_char('=') {
                    self.add_token(TokenKind::GtEq, start);
                } else if self.match_char('>') {
                    self.add_token(TokenKind::GtGt, start);
                } else {
                    self.add_token(TokenKind::Gt, start);
                }
            }
            // Logical/bitwise operators are keyword-spelled now; the symbol
            // forms get distinct token kinds so the parser can reject them
            // with a migration diagnostic (except << >>, which stay valid).
            '&' => {
                if self.match_char('&') {
                    self.add_token(TokenKind::AndAnd, start);
                } else {
                    self.add_token(TokenKind::Amp, start);
                }
            }
            '|' => {
                if self.match_char('|') {
                    self.add_token(TokenKind::OrOr, start);
                } else {
                    self.add_token(TokenKind::Pipe, start);
                }
            }
            '^' => self.add_token(TokenKind::Caret, start),

            // Foreign passthrough block: #<language> ... #end<language>
            '#' => {
                if let Some(language) = self.foreign_block_language_at_current() {
                    self.scan_foreign_block(start, language);
                } else {
                    // Regular Poly comment — consume to end of line.
                    while !self.is_at_end() && self.current() != '\n' {
                        self.advance();
                    }
                }
            }

            // Anything else is not part of Poly. Record an error but keep the
            // character in the stream as an `Error` token.
            _ => {
                self.errors.push(LexerError::new(
                    LexerErrorKind::UnexpectedCharacter,
                    Span::new(start, self.pos),
                    format!("Unexpected character: '{}'", ch),
                ));
                // Preserve an invalid character in the token stream so the
                // parser cannot silently accept the remainder of malformed
                // source after the lexer reports an error.
                self.add_token(TokenKind::Error, start);
            }
        }
    }

    /// Record a finished token. `start` is its first character's offset and the
    /// current `pos` is just past its last character.
    fn add_token(&mut self, kind: TokenKind, start: usize) {
        // All scanners use one constructor so spans consistently cover the
        // source from the first character through the final consumed character.
        self.tokens
            .push(Token::new(kind, Span::new(start, self.pos)));
    }

    /// Return the language after `#` when the marker is a standalone block
    /// opener. This is called after `#` has already been consumed.
    ///
    /// For example `#rust` on its own line yields `Some("rust")`, whereas an
    /// ordinary `# a comment` yields `None` (the text after `#` isn't a known
    /// language name followed only by whitespace).
    fn foreign_block_language_at_current(&self) -> Option<String> {
        // If the `#` is still under the cursor, look just past it; otherwise
        // (the common case here) it has already been consumed and the marker
        // text starts at the cursor.
        let marker_start = if self.current() == '#' {
            self.pos + 1
        } else {
            self.pos
        };
        // Find the end of the current line, i.e. the next newline (or EOF).
        let line_end = self
            .chars
            .iter()
            .enumerate()
            .skip(marker_start)
            .find_map(|(index, ch)| (*ch == '\n').then_some(index))
            .unwrap_or(self.chars.len());
        // The text between `#` and end-of-line is the candidate marker.
        let marker: String = self.chars[marker_start..line_end].iter().collect();
        // Only a handful of languages are supported for passthrough blocks.
        let language = ["rust", "cpp", "c", "asm", "js"]
            .iter()
            .find(|language| marker.starts_with(**language))?;
        // It counts as a block opener only if nothing but whitespace follows
        // the language name on that line (so `#rusty` is not a `rust` block).
        if marker[language.len()..].trim().is_empty() {
            Some((*language).to_string())
        } else {
            None
        }
    }

    /// Scan a `#<language> ... #end<language>` block. The opening `#` has
    /// already been consumed. End markers are recognized only at line starts,
    /// so strings and comments inside foreign code remain opaque.
    ///
    /// The whole block becomes a single `ForeignBlock` token whose `content` is
    /// the raw text between the opening line and the end marker.
    fn scan_foreign_block(&mut self, start: usize, language: String) {
        // Skip past the language name (the `#` is already gone).
        for _ in 0..language.chars().count() {
            self.advance();
        }
        // Skip the rest of the opening line.
        while !self.is_at_end() && self.current() != '\n' {
            self.advance();
        }
        if !self.is_at_end() {
            self.advance(); // consume the newline itself
        }

        // Collect the raw foreign source until we find the end marker.
        let mut content = String::new();
        let block_start = self.pos;
        let end_marker = format!("#end{language}");
        while !self.is_at_end() {
            // Work out whether the cursor is at the start of a line (ignoring
            // leading whitespace). End markers only count at line starts so
            // that a string like "#endrust" inside the foreign code is ignored.
            let mut line_start = self.pos;
            while line_start > 0 && self.chars[line_start - 1] != '\n' {
                line_start -= 1;
            }
            let at_line_start = self.chars[line_start..self.pos]
                .iter()
                .all(|ch| ch.is_whitespace());
            if at_line_start {
                let remaining: String = self.chars[self.pos..].iter().collect();
                let marker_end = end_marker.chars().count();
                // The marker must be followed by a newline or end-of-input, so
                // `#endrusty` does not accidentally close a `rust` block.
                if remaining.starts_with(&end_marker)
                    && remaining
                        .chars()
                        .nth(marker_end)
                        .is_none_or(|ch| ch == '\n' || ch == '\r')
                {
                    // Consume the end marker and the rest of its line.
                    for _ in 0..marker_end {
                        self.advance();
                    }
                    while !self.is_at_end() && self.current() != '\n' {
                        self.advance();
                    }
                    self.add_token(TokenKind::ForeignBlock { language, content }, start);
                    return;
                }
            }
            // Not the end marker: this character is part of the foreign code.
            content.push(self.advance());
        }

        // Ran out of input before finding the end marker.
        self.errors.push(LexerError::new(
            LexerErrorKind::UnterminatedComment,
            Span::new(block_start, self.pos),
            format!("Unterminated foreign block; missing {end_marker}"),
        ));
        self.add_token(TokenKind::ForeignBlock { language, content }, start);
    }

    // === Scanners ===
    // Each scanner below is called with the token's first character already
    // consumed, and must consume the rest of the token (or report an error).

    /// Scan an ASCII string literal. A handful of backslash escapes are
    /// recognised; anything else is reported as an invalid escape.
    fn scan_string(&mut self, start: usize) {
        // Store decoded characters in the AST-facing value; the source span
        // still retains the original spelling for diagnostics.
        let mut value = String::new();
        // Read until the closing quote (or end of input).
        while !self.is_at_end() && self.current() != '"' {
            if self.current() == '\\' {
                // Escape: consume the backslash and decode the next character.
                self.advance();
                if self.is_at_end() {
                    break;
                }
                let escaped = self.advance();
                match escaped {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '\\' => value.push('\\'),
                    '"' => value.push('"'),
                    '\'' => value.push('\''),
                    '0' => value.push('\0'),
                    _ => {
                        // Unknown escape: keep going but note the problem.
                        self.errors.push(LexerError::new(
                            LexerErrorKind::InvalidEscape,
                            Span::new(self.pos - 2, self.pos),
                            format!("Invalid escape sequence: \\{}", escaped),
                        ));
                    }
                }
            } else {
                // Ordinary character: copy it through.
                value.push(self.advance());
            }
        }

        // Reached the end without a closing quote.
        if self.is_at_end() {
            self.errors.push(LexerError::new(
                LexerErrorKind::UnterminatedString,
                Span::new(start, self.pos),
                "Unterminated string literal",
            ));
            return;
        }

        self.advance(); // closing "
        self.add_token(TokenKind::StringLiteral(value), start);
    }

    /// Scan a `unicode '...'` character literal. Unlike an ordinary string it
    /// must contain exactly one character, and it may be escaped.
    fn scan_unicode_char(&mut self, start: usize) {
        // The opening quote has already been consumed by the dispatcher.
        // Empty literal or a stray newline is an error.
        let value = if self.is_at_end() || self.current() == '\n' {
            self.errors.push(LexerError::new(
                LexerErrorKind::InvalidToken,
                Span::new(start, self.pos),
                "Unicode character literal must contain exactly one character",
            ));
            return;
        } else if self.current() == '\'' {
            // Immediate closing quote means the literal is empty.
            self.advance(); // consume the empty literal's closing quote
            self.errors.push(LexerError::new(
                LexerErrorKind::InvalidToken,
                Span::new(start, self.pos),
                "Unicode character literal must contain exactly one character",
            ));
            return;
        } else if self.current() == '\\' {
            // Escaped character: decode it the same way strings are decoded.
            self.advance();
            if self.is_at_end() {
                self.errors.push(LexerError::new(
                    LexerErrorKind::UnterminatedString,
                    Span::new(start, self.pos),
                    "Unterminated Unicode character literal",
                ));
                return;
            }
            let escaped = self.advance();
            match escaped {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '\\' => '\\',
                '\'' => '\'',
                '"' => '"',
                '0' => '\0',
                _ => {
                    // Unknown escape: report, then skip to the closing quote.
                    self.errors.push(LexerError::new(
                        LexerErrorKind::InvalidEscape,
                        Span::new(self.pos - 2, self.pos),
                        format!("Invalid escape sequence: \\{}", escaped),
                    ));
                    while !self.is_at_end() && self.current() != '\'' && self.current() != '\n' {
                        self.advance();
                    }
                    if self.current() == '\'' {
                        self.advance();
                    }
                    return;
                }
            }
        } else {
            // A single ordinary (possibly multi-byte) character.
            self.advance()
        };

        // The opening value was consumed; the next character must be the
        // closing quote, otherwise the literal held more than one character.
        if self.current() != '\'' {
            while !self.is_at_end() && self.current() != '\'' && self.current() != '\n' {
                self.advance();
            }
            if self.current() == '\'' {
                self.advance();
            }
            self.errors.push(LexerError::new(
                LexerErrorKind::InvalidToken,
                Span::new(start, self.pos),
                "Unicode character literal must contain exactly one character",
            ));
            return;
        }

        self.advance(); // closing '
        self.add_token(TokenKind::UnicodeCharLiteral(value.to_string()), start);
    }

    /// Scan a legacy `u"..."` Unicode string. The body may contain any
    /// characters, including multi-byte ones, and the usual escapes apply.
    fn scan_unicode_string(&mut self, start: usize) {
        let mut value = String::new();
        while !self.is_at_end() && self.current() != '"' {
            if self.current() == '\\' {
                // Same escape handling as an ordinary string literal.
                self.advance();
                if self.is_at_end() {
                    break;
                }
                let escaped = self.advance();
                match escaped {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '\\' => value.push('\\'),
                    '"' => value.push('"'),
                    '\'' => value.push('\''),
                    '0' => value.push('\0'),
                    _ => {
                        self.errors.push(LexerError::new(
                            LexerErrorKind::InvalidEscape,
                            Span::new(self.pos - 2, self.pos),
                            format!("Invalid escape sequence: \\{}", escaped),
                        ));
                    }
                }
            } else {
                value.push(self.advance());
            }
        }

        if self.is_at_end() {
            self.errors.push(LexerError::new(
                LexerErrorKind::UnterminatedString,
                Span::new(start, self.pos),
                "Unterminated unicode string literal",
            ));
            return;
        }

        self.advance(); // closing "
        self.add_token(TokenKind::UnicodeStringLiteral(value), start);
    }

    /// Scan a decimal integer or float. The first digit was already consumed.
    fn scan_number(&mut self, start: usize) {
        // Numeric text is preserved rather than parsed here. The semantic phase
        // can then apply the surrounding declaration's target type.
        while !self.is_at_end() && self.current().is_ascii_digit() {
            self.advance();
        }

        // A number that directly follows a `.` is a tuple index (`pair.0`,
        // `nested.1.0`), so the following dot must not be consumed as a
        // decimal point — exactly like Rust's lexer, where `t.1.0` lexes as
        // `t`, `.`, `1`, `.`, `0`.
        let after_dot = matches!(self.tokens.last().map(|t| &t.kind), Some(TokenKind::Dot));

        // Look for decimal point
        if !after_dot && self.current() == '.' && self.peek_next().is_ascii_digit() {
            self.advance(); // consume '.'
            while !self.is_at_end() && self.current().is_ascii_digit() {
                self.advance();
            }

            // Look for exponent
            if self.current() == 'e' || self.current() == 'E' {
                self.advance();
                if self.current() == '+' || self.current() == '-' {
                    self.advance();
                }
                let exponent_start = self.pos;
                while !self.is_at_end() && self.current().is_ascii_digit() {
                    self.advance();
                }
                if self.pos == exponent_start {
                    self.errors.push(LexerError::new(
                        LexerErrorKind::InvalidNumber,
                        Span::new(start, self.pos),
                        "Exponent must contain at least one digit",
                    ));
                }
            }

            // A second decimal point followed by digits cannot begin a new
            // token; report the complete input as one malformed number.
            if self.current() == '.' && self.peek_next().is_ascii_digit() {
                while !self.is_at_end()
                    && (self.current().is_ascii_digit()
                        || self.current() == '.'
                        || self.current() == 'e'
                        || self.current() == 'E'
                        || self.current() == '+'
                        || self.current() == '-')
                {
                    self.advance();
                }
                self.errors.push(LexerError::new(
                    LexerErrorKind::InvalidNumber,
                    Span::new(start, self.pos),
                    "Number contains multiple decimal points",
                ));
            }

            // A decimal point was consumed earlier, so this is a float.
            let text: String = self.chars[start..self.pos].iter().collect();
            self.add_token(TokenKind::FloatLiteral(text), start);
        } else {
            // Plain digits with no decimal point: an integer.
            let text: String = self.chars[start..self.pos].iter().collect();
            self.add_token(TokenKind::IntLiteral(text), start);
        }
    }

    /// Scan a hexadecimal literal such as `0xFF`. The `0` was consumed by the
    /// dispatcher; this starts at the `x`.
    fn scan_hex_number(&mut self, start: usize) {
        self.advance(); // consume 'x'
                        // Remember where the digits begin so we can detect an empty literal.
        let digit_start = self.pos;
        while !self.is_at_end() && self.current().is_ascii_hexdigit() {
            self.advance();
        }
        // A letter/digit/underscore right after the digits means something the
        // hex literal cannot contain (e.g. `0xG` or `0x1z`).
        let invalid_digit =
            !self.is_at_end() && (self.current().is_ascii_alphanumeric() || self.current() == '_');
        if self.pos == digit_start || invalid_digit {
            // Gobble the bad suffix so the whole literal is one error token.
            while !self.is_at_end()
                && (self.current().is_ascii_alphanumeric() || self.current() == '_')
            {
                self.advance();
            }
            self.errors.push(LexerError::new(
                LexerErrorKind::InvalidNumber,
                Span::new(start, self.pos),
                "Invalid hexadecimal literal",
            ));
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        self.add_token(TokenKind::IntLiteral(text), start);
    }

    /// Scan a binary literal such as `0b1010`.
    fn scan_binary_number(&mut self, start: usize) {
        self.advance(); // consume 'b'
                        // Only 0 and 1 are binary digits.
        while !self.is_at_end() && (self.current() == '0' || self.current() == '1') {
            self.advance();
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        self.add_token(TokenKind::IntLiteral(text), start);
    }

    /// Scan an octal literal such as `0o755`.
    fn scan_octal_number(&mut self, start: usize) {
        self.advance(); // consume 'o'
                        // `is_digit(8)` accepts 0-7.
        while !self.is_at_end() && self.current().is_digit(8) {
            self.advance();
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        self.add_token(TokenKind::IntLiteral(text), start);
    }

    /// Scan a word, then classify it as a keyword, a literal, or an identifier.
    fn scan_identifier_or_keyword(&mut self, start: usize) {
        // Read the complete word before classification; prefixes such as `u`
        // need this to distinguish an identifier from a legacy literal.
        while !self.is_at_end() && (self.current().is_alphanumeric() || self.current() == '_') {
            self.advance();
        }

        // Now compare the collected word against the keyword table. Anything
        // not listed becomes a plain identifier carrying its text.
        let text: String = self.chars[start..self.pos].iter().collect();
        let kind = match text.as_str() {
            // Keywords
            "var" => TokenKind::Var,
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "fn" => TokenKind::Fn,
            "end" => TokenKind::End,
            "if" => TokenKind::If,
            "then" => TokenKind::Then,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "loop" => TokenKind::Loop,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "match" => TokenKind::Match,
            "to" => TokenKind::To,
            "from" => TokenKind::From,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "return" => TokenKind::Return,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "trait" => TokenKind::Trait,
            "impl" => TokenKind::Impl,
            "module" => TokenKind::Module,
            "use" => TokenKind::Use,
            "extern" => TokenKind::Extern,
            // `dep name = "version"` declares an external crate dependency;
            // consumed by `--project` and dependency-aware checking.
            "dep" => TokenKind::Dep,
            "pub" => TokenKind::Pub,
            "as" => TokenKind::As,
            "where" => TokenKind::Where,
            "unsafe" => TokenKind::Unsafe,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "spawn" => TokenKind::Spawn,
            "move" => TokenKind::Move,
            "type" => TokenKind::Type,
            "macro" => TokenKind::Macro,
            "try" => TokenKind::Try,
            "not" => TokenKind::Not,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "xor" => TokenKind::Xor,
            "mod" => TokenKind::Mod,
            "bitand" => TokenKind::BitAnd,
            "bitor" => TokenKind::BitOr,
            "bitnot" => TokenKind::BitNot,
            // `shift` is contextual: the parser reads the identifier
            // `left`/`right` after it, so those words stay usable as names.
            "shift" => TokenKind::Shift,
            "panic" => TokenKind::Panic,
            "null" => TokenKind::Null,
            "addr" => TokenKind::Addr,
            "deref" => TokenKind::Deref,
            "step" => TokenKind::Step,
            "capture" => TokenKind::Capture,
            "unicode" => TokenKind::Unicode,

            // Assembly ops are only keywords in specific contexts,
            // so we treat them as identifiers for now.
            // The parser will handle the context-dependent interpretation.

            // I/O commands
            "put" => TokenKind::Put,
            "get" => TokenKind::Get,
            "error" => TokenKind::Error,
            "warn" => TokenKind::Warn,
            "info" => TokenKind::Info,
            "with" => TokenKind::With,
            "validate" => TokenKind::Validate,
            "complete" => TokenKind::Complete,
            "encoding" => TokenKind::Encoding,
            "timeout" => TokenKind::Timeout,
            "default" => TokenKind::Default,
            "mask" => TokenKind::Mask,
            "bytes" => TokenKind::Bytes,

            // Type keywords
            "bool" => TokenKind::Bool,
            "i8" => TokenKind::I8,
            "u8" => TokenKind::U8,
            "i16" => TokenKind::I16,
            "u16" => TokenKind::U16,
            "i32" => TokenKind::I32,
            "u32" => TokenKind::U32,
            "i64" => TokenKind::I64,
            "u64" => TokenKind::U64,
            "i128" => TokenKind::I128,
            "u128" => TokenKind::U128,
            "f32" => TokenKind::F32,
            "f64" => TokenKind::F64,
            "isize" => TokenKind::ISize,
            "usize" => TokenKind::USize,
            "char" => TokenKind::Char,
            "string" => TokenKind::String,
            "uchar" => TokenKind::UChar,
            "ustring" => TokenKind::UString,
            "byte" => TokenKind::Byte,
            "ptr" => TokenKind::Ptr,
            "Vec" => TokenKind::Vec,
            "Option" => TokenKind::Option,
            "Result" => TokenKind::Result,

            // Boolean literals
            "true" => TokenKind::BoolLiteral(true),
            "false" => TokenKind::BoolLiteral(false),

            // Not a keyword or a boolean literal: a user-defined name.
            _ => TokenKind::Identifier(text),
        };

        self.add_token(kind, start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let source = "var x i32 := 42";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens.len(), 6); // var, x, i32, :=, 42, Eof
        assert_eq!(tokens[0].kind, TokenKind::Var);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("x".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::I32);
        assert_eq!(tokens[3].kind, TokenKind::ColonEq);
        assert_eq!(tokens[4].kind, TokenKind::IntLiteral("42".to_string()));
        assert_eq!(tokens[5].kind, TokenKind::Eof);
        Ok(())
    }

    #[test]
    fn test_equality_and_initialization_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let (tokens, errors) = Lexer::lex("var x := 1\nif x = 1,");
        assert!(errors.is_empty());
        assert!(tokens.iter().any(|token| token.kind == TokenKind::ColonEq));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Eq));
        Ok(())
    }

    #[test]
    fn test_string_literal() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"put "Hello, World!""#;
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Put);
        assert_eq!(
            tokens[1].kind,
            TokenKind::StringLiteral("Hello, World!".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_unicode_string_prefix() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"put unicode "Hello, 世界! 🚀""#;
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Put);
        assert_eq!(tokens[1].kind, TokenKind::Unicode);
        assert_eq!(
            tokens[2].kind,
            TokenKind::StringLiteral("Hello, 世界! 🚀".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_unicode_character_literals() -> Result<(), Box<dyn std::error::Error>> {
        let (tokens, errors) = Lexer::lex("unicode '世' unicode '\\n' unicode '\\\\'");
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Unicode);
        assert_eq!(
            tokens[1].kind,
            TokenKind::UnicodeCharLiteral("世".to_string())
        );
        assert_eq!(tokens[2].kind, TokenKind::Unicode);
        assert_eq!(
            tokens[3].kind,
            TokenKind::UnicodeCharLiteral("\n".to_string())
        );
        assert_eq!(tokens[4].kind, TokenKind::Unicode);
        assert_eq!(
            tokens[5].kind,
            TokenKind::UnicodeCharLiteral("\\".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_invalid_unicode_character_literals() -> Result<(), Box<dyn std::error::Error>> {
        let (_tokens, errors) = Lexer::lex("'ab'");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].kind, LexerErrorKind::InvalidToken);
        Ok(())
    }

    #[test]
    fn test_invalid_unicode_character_escape_has_one_diagnostic(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_tokens, errors) = Lexer::lex("'\\q'");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].kind, LexerErrorKind::InvalidEscape);
        Ok(())
    }

    #[test]
    fn test_operators() -> Result<(), Box<dyn std::error::Error>> {
        let source = "+ - * / % == != < > <= >= && || ! & | ^ ~ << >>";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Plus);
        assert_eq!(tokens[1].kind, TokenKind::Minus);
        assert_eq!(tokens[2].kind, TokenKind::Star);
        assert_eq!(tokens[3].kind, TokenKind::Slash);
        assert_eq!(tokens[4].kind, TokenKind::Percent);
        assert_eq!(tokens[5].kind, TokenKind::EqEq);
        assert_eq!(tokens[6].kind, TokenKind::NotEq);
        assert_eq!(tokens[7].kind, TokenKind::Lt);
        assert_eq!(tokens[8].kind, TokenKind::Gt);
        assert_eq!(tokens[9].kind, TokenKind::LtEq);
        assert_eq!(tokens[10].kind, TokenKind::GtEq);
        assert_eq!(tokens[11].kind, TokenKind::AndAnd);
        assert_eq!(tokens[12].kind, TokenKind::OrOr);
        // Standalone `!` is the retired spelling of `not` and lexes as Bang;
        // the `not` keyword keeps TokenKind::Not.
        assert_eq!(tokens[13].kind, TokenKind::Bang);
        assert_eq!(tokens[14].kind, TokenKind::Amp);
        assert_eq!(tokens[15].kind, TokenKind::Pipe);
        assert_eq!(tokens[16].kind, TokenKind::Caret);
        assert_eq!(tokens[17].kind, TokenKind::Tilde);
        assert_eq!(tokens[18].kind, TokenKind::LtLt);
        assert_eq!(tokens[19].kind, TokenKind::GtGt);
        Ok(())
    }

    #[test]
    fn test_operator_keywords() -> Result<(), Box<dyn std::error::Error>> {
        // Logical, bitwise, and arithmetic operators have keyword spellings;
        // the parser maps them onto the same AST operator variants as the
        // retired symbol spellings did.
        let source = "and or xor mod bitand bitor bitnot shift left right not";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::And);
        assert_eq!(tokens[1].kind, TokenKind::Or);
        assert_eq!(tokens[2].kind, TokenKind::Xor);
        assert_eq!(tokens[3].kind, TokenKind::Mod);
        assert_eq!(tokens[4].kind, TokenKind::BitAnd);
        assert_eq!(tokens[5].kind, TokenKind::BitOr);
        assert_eq!(tokens[6].kind, TokenKind::BitNot);
        assert_eq!(tokens[7].kind, TokenKind::Shift);
        // `left`/`right` stay ordinary identifiers; only the parser, directly
        // after `shift`, interprets them as shift directions.
        assert_eq!(tokens[8].kind, TokenKind::Identifier("left".to_string()));
        assert_eq!(tokens[9].kind, TokenKind::Identifier("right".to_string()));
        assert_eq!(tokens[10].kind, TokenKind::Not);
        Ok(())
    }

    #[test]
    fn test_compound_operators_are_separate_tokens() -> Result<(), Box<dyn std::error::Error>> {
        // Compound operators like += are no longer single tokens;
        // they lex as two separate tokens: + and =
        let source = "x + = 5";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Identifier("x".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Eq);
        assert_eq!(tokens[3].kind, TokenKind::IntLiteral("5".to_string()));
        Ok(())
    }

    #[test]
    fn test_range_operators() -> Result<(), Box<dyn std::error::Error>> {
        let source = "0..10 0..=10";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::IntLiteral("0".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::DotDot);
        assert_eq!(tokens[2].kind, TokenKind::IntLiteral("10".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::IntLiteral("0".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::DotDotEq);
        assert_eq!(tokens[5].kind, TokenKind::IntLiteral("10".to_string()));
        Ok(())
    }

    #[test]
    fn test_function_declaration() -> Result<(), Box<dyn std::error::Error>> {
        let source = "fn sum(a: i32, b: i32): i32";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Fn);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("sum".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::LParen);
        assert_eq!(tokens[3].kind, TokenKind::Identifier("a".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Colon);
        assert_eq!(tokens[5].kind, TokenKind::I32);
        assert_eq!(tokens[6].kind, TokenKind::Comma);
        assert_eq!(tokens[7].kind, TokenKind::Identifier("b".to_string()));
        assert_eq!(tokens[8].kind, TokenKind::Colon);
        assert_eq!(tokens[9].kind, TokenKind::I32);
        assert_eq!(tokens[10].kind, TokenKind::RParen);
        assert_eq!(tokens[11].kind, TokenKind::Colon);
        assert_eq!(tokens[12].kind, TokenKind::I32);
        Ok(())
    }

    #[test]
    fn test_control_flow() -> Result<(), Box<dyn std::error::Error>> {
        let source = "if x > 0,\n    put x\nend if";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::If);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("x".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Gt);
        assert_eq!(tokens[3].kind, TokenKind::IntLiteral("0".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Comma);
        Ok(())
    }

    #[test]
    fn test_hex_number() -> Result<(), Box<dyn std::error::Error>> {
        let source = "var x i32 := 0xFF";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[4].kind, TokenKind::IntLiteral("0xFF".to_string()));
        Ok(())
    }

    #[test]
    fn test_binary_number() -> Result<(), Box<dyn std::error::Error>> {
        let source = "var x i32 := 0b1010";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[4].kind, TokenKind::IntLiteral("0b1010".to_string()));
        Ok(())
    }

    #[test]
    fn test_float_number() -> Result<(), Box<dyn std::error::Error>> {
        let source = "var pi f64 := 3.14159";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(
            tokens[4].kind,
            TokenKind::FloatLiteral("3.14159".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_tuple_index_after_dot_is_integer_not_float() -> Result<(), Box<dyn std::error::Error>> {
        // `nested.1.0` must lex as `nested`, `.`, `1`, `.`, `0` — the dot after
        // a field-access dot must not be consumed as a float decimal point.
        let source = "put nested.1.0";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        let kinds: Vec<&TokenKind> = tokens.iter().map(|t| &t.kind).collect();
        assert_eq!(kinds[1], &TokenKind::Identifier("nested".to_string()));
        assert_eq!(kinds[2], &TokenKind::Dot);
        assert_eq!(kinds[3], &TokenKind::IntLiteral("1".to_string()));
        assert_eq!(kinds[4], &TokenKind::Dot);
        assert_eq!(kinds[5], &TokenKind::IntLiteral("0".to_string()));
        Ok(())
    }

    #[test]
    fn test_struct_declaration() -> Result<(), Box<dyn std::error::Error>> {
        let source = "struct Point\n    var x: f32\n    var y: f32\nend struct";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Struct);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("Point".to_string()));
        Ok(())
    }

    #[test]
    fn test_enum_declaration() -> Result<(), Box<dyn std::error::Error>> {
        let source = "enum Direction\n    North\n    South\n    East\n    West\nend enum";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Enum);
        assert_eq!(
            tokens[1].kind,
            TokenKind::Identifier("Direction".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_match_expression() -> Result<(), Box<dyn std::error::Error>> {
        let source = "match x\n    0, put \"zero\"\n    _, put \"other\"\nend match";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Match);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("x".to_string()));
        Ok(())
    }

    #[test]
    fn test_closure() -> Result<(), Box<dyn std::error::Error>> {
        let source = "var square := |x: i32| x * x";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Var);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("square".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::ColonEq);
        assert_eq!(tokens[3].kind, TokenKind::Pipe);
        Ok(())
    }

    #[test]
    fn test_arrow_and_fat_arrow() -> Result<(), Box<dyn std::error::Error>> {
        let source = "-> =>";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Arrow);
        assert_eq!(tokens[1].kind, TokenKind::FatArrow);
        Ok(())
    }

    #[test]
    fn test_colon_colon() -> Result<(), Box<dyn std::error::Error>> {
        let source = "Shape::Circle";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Identifier("Shape".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::ColonColon);
        assert_eq!(tokens[2].kind, TokenKind::Identifier("Circle".to_string()));
        Ok(())
    }

    #[test]
    fn test_comments() -> Result<(), Box<dyn std::error::Error>> {
        let source = "# This is a comment\nvar x := 1";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Var);
        Ok(())
    }

    #[test]
    fn test_foreign_blocks_are_line_delimited_and_language_tagged(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = "#rust\nconst VALUE: i32 = 1;\nlet text = \"#endrust\";\n#endrust\n#c\nint answer(void) { return 42; }\n#endc";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(matches!(
            tokens.first().map(|token| &token.kind),
            Some(TokenKind::ForeignBlock { language, content })
                if language == "rust" && content.contains("#endrust")
        ));
        assert!(matches!(
            tokens.get(1).map(|token| &token.kind),
            Some(TokenKind::ForeignBlock { language, .. }) if language == "c"
        ));
        Ok(())
    }

    #[test]
    fn test_foreign_end_marker_must_start_a_line() -> Result<(), Box<dyn std::error::Error>> {
        let source = "#rust\nlet x = \"#endrust\";\n#endrust\nput 1";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(tokens.len(), 4);
        Ok(())
    }

    #[test]
    fn test_block_comment() -> Result<(), Box<dyn std::error::Error>> {
        let source = "/* block comment */ var x := 1";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Var);
        Ok(())
    }

    #[test]
    fn test_error_unterminated_string() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"var x = "hello"#;
        let (_tokens, errors) = Lexer::lex(source);
        assert!(!errors.is_empty());
        assert_eq!(errors[0].kind, LexerErrorKind::UnterminatedString);
        Ok(())
    }

    #[test]
    fn test_error_unexpected_character() -> Result<(), Box<dyn std::error::Error>> {
        let source = "var x := @";
        let (_tokens, errors) = Lexer::lex(source);
        assert!(!errors.is_empty());
        assert_eq!(errors[0].kind, LexerErrorKind::UnexpectedCharacter);
        Ok(())
    }

    #[test]
    fn test_put_with_flags() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"put -n "Loading...""#;
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Put);
        assert_eq!(tokens[1].kind, TokenKind::Minus);
        assert_eq!(tokens[2].kind, TokenKind::Identifier("n".to_string()));
        assert_eq!(
            tokens[3].kind,
            TokenKind::StringLiteral("Loading...".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_get_with_flags() -> Result<(), Box<dyn std::error::Error>> {
        let source = "get --timeout 3000";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Get);
        assert_eq!(tokens[1].kind, TokenKind::Minus);
        assert_eq!(tokens[2].kind, TokenKind::Minus);
        assert_eq!(tokens[3].kind, TokenKind::Timeout);
        assert_eq!(tokens[4].kind, TokenKind::IntLiteral("3000".to_string()));
        Ok(())
    }

    #[test]
    fn test_file_io_operators() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"put "hello" > "file.txt""#;
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Put);
        assert_eq!(
            tokens[1].kind,
            TokenKind::StringLiteral("hello".to_string())
        );
        assert_eq!(tokens[2].kind, TokenKind::Gt);
        assert_eq!(
            tokens[3].kind,
            TokenKind::StringLiteral("file.txt".to_string())
        );
        Ok(())
    }
}

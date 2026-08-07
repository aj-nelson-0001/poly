//! The Poly language lexer.
//!
//! Converts Poly source code into a stream of tokens.

use crate::error::{LexerError, LexerErrorKind};
use crate::token::{Span, Token, TokenKind};

/// The Poly language lexer.
///
/// Converts source code into a stream of tokens.
pub struct Lexer<'a> {
    source: &'a str,
    chars: Vec<char>,
    pos: usize,
    tokens: Vec<Token>,
    errors: Vec<LexerError>,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given source code.
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars().collect(),
            pos: 0,
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Lex the entire source code and return tokens and errors.
    pub fn lex(source: &'a str) -> (Vec<Token>, Vec<LexerError>) {
        let mut lexer = Lexer::new(source);
        lexer.tokenize();
        (lexer.tokens, lexer.errors)
    }

    /// Tokenize the source code.
    pub fn tokenize(&mut self) {
        while !self.is_at_end() {
            self.scan_token();
        }
        self.tokens.push(Token::new(
            TokenKind::Eof,
            Span::new(self.pos, self.pos),
        ));
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

    fn is_at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn current(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.chars[self.pos]
        }
    }

    fn peek(&self) -> char {
        self.current()
    }

    fn peek_next(&self) -> char {
        if self.pos + 1 >= self.chars.len() {
            '\0'
        } else {
            self.chars[self.pos + 1]
        }
    }

    fn advance(&mut self) -> char {
        let ch = self.current();
        self.pos += 1;
        ch
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.current() != expected {
            false
        } else {
            self.pos += 1;
            true
        }
    }

    fn skip_whitespace(&mut self) {
        while !self.is_at_end() {
            match self.current() {
                ' ' | '\r' | '\t' => {
                    self.advance();
                }
                '\n' => {
                    // Don't add Newline tokens - they're only meaningful
                    // in specific contexts (which the parser handles)
                    self.advance();
                }
                '#' => {
                    self.skip_comment();
                }
                '/' if self.peek_next() == '*' => {
                    self.skip_comment();
                }
                _ => break,
            }
        }
    }

    fn skip_comment(&mut self) {
        if self.current() == '#' {
            // Single-line comment: skip the #
            self.advance();
            while !self.is_at_end() && self.current() != '\n' {
                self.advance();
            }
        } else if self.current() == '/' && self.peek_next() == '*' {
            // Block comment: skip /* ... */
            self.advance(); // /
            self.advance(); // *
            let start = self.pos;
            while !self.is_at_end() {
                if self.current() == '*' && self.peek_next() == '/' {
                    self.advance(); // *
                    self.advance(); // /
                    return;
                }
                self.advance();
            }
            self.errors.push(LexerError::new(
                LexerErrorKind::UnterminatedComment,
                Span::new(start, self.pos),
                "Unterminated block comment",
            ));
        }
    }

    // === Token scanning ===

    fn scan_token(&mut self) {
        self.skip_whitespace();
        if self.is_at_end() {
            return;
        }

        let start = self.pos;
        let ch = self.advance();

        match ch {
            // Single character tokens
            '(' => self.add_token(TokenKind::LParen, start),
            ')' => self.add_token(TokenKind::RParen, start),
            '[' => self.add_token(TokenKind::LBracket, start),
            ']' => self.add_token(TokenKind::RBracket, start),
            '{' => self.add_token(TokenKind::LBrace, start),
            '}' => self.add_token(TokenKind::RBrace, start),
            ',' => self.add_token(TokenKind::Comma, start),
            ';' => self.add_token(TokenKind::Semicolon, start),
            ':' => {
                if self.match_char(':') {
                    self.add_token(TokenKind::ColonColon, start);
                } else {
                    self.add_token(TokenKind::Colon, start);
                }
            }
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

            // String literals
            '"' => self.scan_string(start),
            'u' => {
                if self.peek() == '"' {
                    self.advance(); // consume the "
                    self.scan_unicode_string(start);
                } else {
                    self.scan_identifier_or_keyword(start);
                }
            }

            // Numbers
            '0' => {
                if self.peek() == 'x' || self.peek() == 'X' {
                    self.scan_hex_number(start);
                } else if self.peek() == 'b' || self.peek() == 'B' {
                    self.scan_binary_number(start);
                } else if self.peek() == 'o' || self.peek() == 'O' {
                    self.scan_octal_number(start);
                } else {
                    self.scan_number(start);
                }
            }
            '1'..='9' => self.scan_number(start),

            // Identifiers and keywords
            'a'..='z' | 'A'..='Z' | '_' => self.scan_identifier_or_keyword(start),

            // Operators
            '+' => {
                if self.match_char('=') {
                    self.add_token(TokenKind::PlusEq, start);
                } else {
                    self.add_token(TokenKind::Plus, start);
                }
            }
            '-' => {
                if self.match_char('>') {
                    self.add_token(TokenKind::Arrow, start);
                } else if self.match_char('=') {
                    self.add_token(TokenKind::MinusEq, start);
                } else {
                    self.add_token(TokenKind::Minus, start);
                }
            }
            '*' => {
                if self.match_char('=') {
                    self.add_token(TokenKind::StarEq, start);
                } else {
                    self.add_token(TokenKind::Star, start);
                }
            }
            '/' => {
                if self.match_char('=') {
                    self.add_token(TokenKind::SlashEq, start);
                } else {
                    self.add_token(TokenKind::Slash, start);
                }
            }
            '%' => {
                if self.match_char('=') {
                    self.add_token(TokenKind::PercentEq, start);
                } else {
                    self.add_token(TokenKind::Percent, start);
                }
            }
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
                    self.add_token(TokenKind::Not, start);
                }
            }
            '<' => {
                if self.match_char('=') {
                    self.add_token(TokenKind::LtEq, start);
                } else if self.match_char('<') {
                    if self.match_char('=') {
                        self.add_token(TokenKind::LtLtEq, start);
                    } else {
                        self.add_token(TokenKind::LtLt, start);
                    }
                } else {
                    self.add_token(TokenKind::Lt, start);
                }
            }
            '>' => {
                if self.match_char('=') {
                    self.add_token(TokenKind::GtEq, start);
                } else if self.match_char('>') {
                    if self.match_char('=') {
                        self.add_token(TokenKind::GtGtEq, start);
                    } else {
                        self.add_token(TokenKind::GtGt, start);
                    }
                } else {
                    self.add_token(TokenKind::Gt, start);
                }
            }
            '&' => {
                if self.match_char('&') {
                    self.add_token(TokenKind::AndAnd, start);
                } else if self.match_char('=') {
                    self.add_token(TokenKind::AmpEq, start);
                } else {
                    self.add_token(TokenKind::Amp, start);
                }
            }
            '|' => {
                if self.match_char('|') {
                    self.add_token(TokenKind::OrOr, start);
                } else if self.match_char('=') {
                    self.add_token(TokenKind::PipeEq, start);
                } else {
                    self.add_token(TokenKind::Pipe, start);
                }
            }
            '^' => {
                if self.match_char('=') {
                    self.add_token(TokenKind::CaretEq, start);
                } else {
                    self.add_token(TokenKind::Caret, start);
                }
            }

            // Unexpected character
            _ => {
                self.errors.push(LexerError::new(
                    LexerErrorKind::UnexpectedCharacter,
                    Span::new(start, self.pos),
                    format!("Unexpected character: '{}'", ch),
                ));
            }
        }
    }

    fn add_token(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token::new(kind, Span::new(start, self.pos)));
    }

    // === Scanners ===

    fn scan_string(&mut self, start: usize) {
        let mut value = String::new();
        while !self.is_at_end() && self.current() != '"' {
            if self.current() == '\\' {
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
                "Unterminated string literal",
            ));
            return;
        }

        self.advance(); // closing "
        self.add_token(TokenKind::StringLiteral(value), start);
    }

    fn scan_unicode_string(&mut self, start: usize) {
        let mut value = String::new();
        while !self.is_at_end() && self.current() != '"' {
            if self.current() == '\\' {
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

    fn scan_number(&mut self, start: usize) {
        while !self.is_at_end() && self.current().is_ascii_digit() {
            self.advance();
        }

        // Look for decimal point
        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            self.advance(); // consume '.'
            while !self.is_at_end() && self.current().is_ascii_digit() {
                self.advance();
            }

            // Look for exponent
            if self.peek() == 'e' || self.peek() == 'E' {
                self.advance();
                if self.peek() == '+' || self.peek() == '-' {
                    self.advance();
                }
                while !self.is_at_end() && self.current().is_ascii_digit() {
                    self.advance();
                }
            }

            let text: String = self.chars[start..self.pos].iter().collect();
            self.add_token(TokenKind::FloatLiteral(text), start);
        } else {
            let text: String = self.chars[start..self.pos].iter().collect();
            self.add_token(TokenKind::IntLiteral(text), start);
        }
    }

    fn scan_hex_number(&mut self, start: usize) {
        self.advance(); // consume 'x'
        while !self.is_at_end() && self.current().is_ascii_hexdigit() {
            self.advance();
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        self.add_token(TokenKind::IntLiteral(text), start);
    }

    fn scan_binary_number(&mut self, start: usize) {
        self.advance(); // consume 'b'
        while !self.is_at_end() && (self.current() == '0' || self.current() == '1') {
            self.advance();
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        self.add_token(TokenKind::IntLiteral(text), start);
    }

    fn scan_octal_number(&mut self, start: usize) {
        self.advance(); // consume 'o'
        while !self.is_at_end() && self.current().is_digit(8) {
            self.advance();
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        self.add_token(TokenKind::IntLiteral(text), start);
    }

    fn scan_identifier_or_keyword(&mut self, start: usize) {
        while !self.is_at_end() && (self.current().is_alphanumeric() || self.current() == '_') {
            self.advance();
        }

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
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "return" => TokenKind::Return,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "trait" => TokenKind::Trait,
            "impl" => TokenKind::Impl,
            "module" => TokenKind::Module,
            "use" => TokenKind::Use,
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
            "panic" => TokenKind::Panic,
            "null" => TokenKind::Null,
            "addr" => TokenKind::Addr,
            "deref" => TokenKind::Deref,
            "step" => TokenKind::Step,
            "capture" => TokenKind::Capture,

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

            // Default: identifier
            _ => TokenKind::Identifier(text),
        };

        self.add_token(kind, start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tokens() {
        let source = "var x: i32 = 42";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens.len(), 7); // var, x, :, i32, =, 42, Eof
        assert_eq!(tokens[0].kind, TokenKind::Var);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("x".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Colon);
        assert_eq!(tokens[3].kind, TokenKind::I32);
        assert_eq!(tokens[4].kind, TokenKind::Eq);
        assert_eq!(tokens[5].kind, TokenKind::IntLiteral("42".to_string()));
        assert_eq!(tokens[6].kind, TokenKind::Eof);
    }

    #[test]
    fn test_string_literal() {
        let source = r#"put "Hello, World!""#;
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Put);
        assert_eq!(
            tokens[1].kind,
            TokenKind::StringLiteral("Hello, World!".to_string())
        );
    }

    #[test]
    fn test_unicode_string() {
        let source = r#"put u"Hello, 世界! 🚀""#;
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Put);
        assert_eq!(
            tokens[1].kind,
            TokenKind::UnicodeStringLiteral("Hello, 世界! 🚀".to_string())
        );
    }

    #[test]
    fn test_operators() {
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
        assert_eq!(tokens[13].kind, TokenKind::Not);
        assert_eq!(tokens[14].kind, TokenKind::Amp);
        assert_eq!(tokens[15].kind, TokenKind::Pipe);
        assert_eq!(tokens[16].kind, TokenKind::Caret);
        assert_eq!(tokens[17].kind, TokenKind::Tilde);
        assert_eq!(tokens[18].kind, TokenKind::LtLt);
        assert_eq!(tokens[19].kind, TokenKind::GtGt);
    }

    #[test]
    fn test_assignment_operators() {
        let source = "x += 5 -= 3 *= 2 /= 4 %= 3";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Identifier("x".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::PlusEq);
        assert_eq!(tokens[2].kind, TokenKind::IntLiteral("5".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::MinusEq);
        assert_eq!(tokens[4].kind, TokenKind::IntLiteral("3".to_string()));
        assert_eq!(tokens[5].kind, TokenKind::StarEq);
        assert_eq!(tokens[6].kind, TokenKind::IntLiteral("2".to_string()));
        assert_eq!(tokens[7].kind, TokenKind::SlashEq);
        assert_eq!(tokens[8].kind, TokenKind::IntLiteral("4".to_string()));
        assert_eq!(tokens[9].kind, TokenKind::PercentEq);
        assert_eq!(tokens[10].kind, TokenKind::IntLiteral("3".to_string()));
    }

    #[test]
    fn test_range_operators() {
        let source = "0..10 0..=10";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::IntLiteral("0".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::DotDot);
        assert_eq!(tokens[2].kind, TokenKind::IntLiteral("10".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::IntLiteral("0".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::DotDotEq);
        assert_eq!(tokens[5].kind, TokenKind::IntLiteral("10".to_string()));
    }

    #[test]
    fn test_function_declaration() {
        let source = "fn add(a: i32, b: i32): i32";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Fn);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("add".to_string()));
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
    }

    #[test]
    fn test_control_flow() {
        let source = "if x > 0 then\n    put x\nend if";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::If);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("x".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Gt);
        assert_eq!(tokens[3].kind, TokenKind::IntLiteral("0".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Then);
    }

    #[test]
    fn test_hex_number() {
        let source = "var x: i32 = 0xFF";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(
            tokens[5].kind,
            TokenKind::IntLiteral("0xFF".to_string())
        );
    }

    #[test]
    fn test_binary_number() {
        let source = "var x: i32 = 0b1010";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(
            tokens[5].kind,
            TokenKind::IntLiteral("0b1010".to_string())
        );
    }

    #[test]
    fn test_float_number() {
        let source = "var pi: f64 = 3.14159";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(
            tokens[5].kind,
            TokenKind::FloatLiteral("3.14159".to_string())
        );
    }

    #[test]
    fn test_struct_declaration() {
        let source = "struct Point\n    var x: f32\n    var y: f32\nend struct";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Struct);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("Point".to_string()));
    }

    #[test]
    fn test_enum_declaration() {
        let source = "enum Direction\n    North\n    South\n    East\n    West\nend enum";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Enum);
        assert_eq!(
            tokens[1].kind,
            TokenKind::Identifier("Direction".to_string())
        );
    }

    #[test]
    fn test_match_expression() {
        let source = "match x\n    0 => put \"zero\"\n    _ => put \"other\"\nend match";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Match);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("x".to_string()));
    }

    #[test]
    fn test_closure() {
        let source = "var square = |x: i32| x * x";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Var);
        assert_eq!(
            tokens[1].kind,
            TokenKind::Identifier("square".to_string())
        );
        assert_eq!(tokens[2].kind, TokenKind::Eq);
        assert_eq!(tokens[3].kind, TokenKind::Pipe);
    }

    #[test]
    fn test_arrow_and_fat_arrow() {
        let source = "-> =>";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Arrow);
        assert_eq!(tokens[1].kind, TokenKind::FatArrow);
    }

    #[test]
    fn test_colon_colon() {
        let source = "Shape::Circle";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(
            tokens[0].kind,
            TokenKind::Identifier("Shape".to_string())
        );
        assert_eq!(tokens[1].kind, TokenKind::ColonColon);
        assert_eq!(
            tokens[2].kind,
            TokenKind::Identifier("Circle".to_string())
        );
    }

    #[test]
    fn test_comments() {
        let source = "# This is a comment\nvar x = 1";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Var);
    }

    #[test]
    fn test_block_comment() {
        let source = "/* block comment */ var x = 1";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Var);
    }

    #[test]
    fn test_error_unterminated_string() {
        let source = r#"var x = "hello"#;
        let (tokens, errors) = Lexer::lex(source);
        assert!(!errors.is_empty());
        assert_eq!(errors[0].kind, LexerErrorKind::UnterminatedString);
    }

    #[test]
    fn test_error_unexpected_character() {
        let source = "var x = @";
        let (tokens, errors) = Lexer::lex(source);
        assert!(!errors.is_empty());
        assert_eq!(errors[0].kind, LexerErrorKind::UnexpectedCharacter);
    }

    #[test]
    fn test_put_with_flags() {
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
    }

    #[test]
    fn test_get_with_flags() {
        let source = "get --timeout 3000";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].kind, TokenKind::Get);
        assert_eq!(tokens[1].kind, TokenKind::Minus);
        assert_eq!(tokens[2].kind, TokenKind::Minus);
        assert_eq!(tokens[3].kind, TokenKind::Timeout);
        assert_eq!(tokens[4].kind, TokenKind::IntLiteral("3000".to_string()));
    }

    #[test]
    fn test_file_io_operators() {
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
    }
}

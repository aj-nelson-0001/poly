//! Poly Language Parser
//!
//! Converts a stream of tokens into an Abstract Syntax Tree (AST).
//! This crate is a stub — the full parser will be implemented in a future milestone.

use poly_lexer::token::Token;

/// The Poly language parser.
pub struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    /// Create a new parser for the given token stream.
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Parse the token stream and return the result.
    pub fn parse(&mut self) -> Result<(), String> {
        // TODO: Implement full parser
        Ok(())
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> &Token {
        let token = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        token
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_stub() {
        let tokens = vec![];
        let mut parser = Parser::new(&tokens);
        assert!(parser.parse().is_ok());
    }
}

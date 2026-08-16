//! Recursive descent parser for the Poly language.

use std::collections::HashMap;

use poly_lexer::token::{Span, Token, TokenKind};

use crate::ast::*;
use crate::error::ParseError;

/// The Poly language parser.
pub struct Parser<'a> {
    // The parser borrows lexer output so parsing never needs to duplicate token data.
    tokens: &'a [Token],
    // `pos` always points at the next token to consume; `peek` reads without moving it.
    pos: usize,
    // Recovery keeps collecting diagnostics so callers can show more than one fix at a time.
    errors: Vec<ParseError>,
    // Chained conditionals recurse, so cap their depth before malformed input can exhaust the stack.
    if_depth: usize,
    // Statement macros defined so far; invocations expand during parsing.
    macros: HashMap<String, MacroDecl>,
    // While parsing `get` flag values, a `--` starts the next flag, so a
    // binary minus must not swallow it: `get --timeout 5000 --default 0`.
    stop_at_double_minus: bool,
}

impl<'a> Parser<'a> {
    /// Create a new parser for the given token stream.
    pub fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: Vec::new(),
            if_depth: 0,
            macros: HashMap::new(),
            stop_at_double_minus: false,
        }
    }

    /// Parse the token stream and return a Program AST.
    ///
    /// This compatibility entry point preserves the historical first-error API;
    /// tools that want all diagnostics should call `parse_with_recovery`.
    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            let start = self.pos;
            match self.parse_statement() {
                Ok(stmt) => statements.push(Spanned::new(stmt, self.statement_span(start))),
                Err(mut e) => {
                    e.generate_suggestion();
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        if self.errors.is_empty() {
            Ok(Program { statements })
        } else {
            // Return the first error for backward compatibility
            Err(self.errors[0].clone())
        }
    }

    /// Parse with error recovery, collecting all errors.
    ///
    /// A partially built program is returned alongside diagnostics so the CLI
    /// and editor integrations can continue analyzing valid later statements.
    pub fn parse_with_recovery(&mut self) -> (Program, Vec<ParseError>) {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            let start = self.pos;
            match self.parse_statement() {
                Ok(stmt) => statements.push(Spanned::new(stmt, self.statement_span(start))),
                Err(mut e) => {
                    e.generate_suggestion();
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        (Program { statements }, self.errors.clone())
    }

    /// Compute the source span of a successfully parsed statement.
    ///
    /// `start` is the token index where the statement began.  The span runs
    /// from the start of the first consumed token to the end of the last one;
    /// if nothing was consumed (defensive), it falls back to the token at
    /// `start` itself.
    fn statement_span(&self, start: usize) -> Span {
        let first = self.tokens.get(start).or_else(|| self.tokens.last());
        let last = if self.pos > start {
            self.tokens.get(self.pos - 1)
        } else {
            first
        };
        match (first, last) {
            (Some(first), Some(last)) => Span::new(first.span.start, last.span.end),
            _ => Span::new(0, 0),
        }
    }

    /// Get collected errors.
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// Synchronize the parser after an error by skipping tokens until a
    /// synchronization point (statement boundary).
    fn synchronize(&mut self) {
        while !self.is_at_end() {
            match self.peek() {
                // Statement boundary tokens
                TokenKind::Var
                | TokenKind::Let
                | TokenKind::Const
                | TokenKind::Fn
                | TokenKind::Struct
                | TokenKind::Enum
                | TokenKind::Trait
                | TokenKind::Impl
                | TokenKind::Module
                | TokenKind::Use
                | TokenKind::Type
                | TokenKind::While
                | TokenKind::Return
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Set
                | TokenKind::Add
                | TokenKind::Sub
                | TokenKind::Inc
                | TokenKind::Dec
                | TokenKind::Put
                | TokenKind::Error
                | TokenKind::Warn
                | TokenKind::Info
                | TokenKind::If
                | TokenKind::Match
                | TokenKind::Loop
                | TokenKind::For
                | TokenKind::Async => {
                    return;
                }
                // End tokens (skip nested blocks)
                TokenKind::End => {
                    self.advance();
                    return;
                }
                TokenKind::RBrace => {
                    self.advance();
                    return;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    // === Token navigation ===

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len() || self.current().kind == TokenKind::Eof
    }

    fn current(&self) -> &Token {
        if self.pos >= self.tokens.len() {
            &self.tokens[self.tokens.len().saturating_sub(1)]
        } else {
            &self.tokens[self.pos]
        }
    }

    fn peek(&self) -> &TokenKind {
        &self.current().kind
    }

    fn advance(&mut self) -> &Token {
        // The lexer always appends EOF. The increment guard keeps normal parsing
        // on the final sentinel, while callers still ensure a token is available
        // before advancing during recovery.
        let token = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        token
    }

    fn expect(&mut self, expected: &TokenKind) -> Result<&Token, ParseError> {
        if self.peek() == expected {
            Ok(self.advance())
        } else {
            Err(ParseError::new(
                format!("Expected {:?}, got {:?}", expected, self.peek()),
                self.current().span,
            ))
        }
    }

    fn expect_identifier(&mut self) -> Result<String, ParseError> {
        match self.peek().clone() {
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(name)
            }
            _ => Err(ParseError::new(
                format!("Expected identifier, got {:?}", self.peek()),
                self.current().span,
            )),
        }
    }

    fn match_token(&mut self, expected: &TokenKind) -> bool {
        if self.peek() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Accept an identifier or a keyword token whose spelling is a plausible
    /// method/field name (e.g. `map.get(...)`, `stream.put(...)`).  Method
    /// names are not reserved words in Poly, so `get`/`put`/`error` after a
    /// `.` must parse instead of being mistaken for statement keywords.
    fn expect_field_name(&mut self) -> Result<String, ParseError> {
        if let TokenKind::Identifier(name) = self.peek().clone() {
            self.advance();
            return Ok(name);
        }
        if let Some(name) = keyword_as_field_name(self.peek()) {
            self.advance();
            return Ok(name.to_string());
        }
        Err(ParseError::new(
            format!("Expected identifier, got {:?}", self.peek()),
            self.current().span,
        ))
    }

    // === Statement parsing ===

    // Dispatching on the first token keeps each declaration/control-flow parser
    // small and makes synchronization points explicit in `synchronize` above.
    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek().clone() {
            TokenKind::Var => self.parse_var_declaration(),
            TokenKind::Let => self.parse_let_declaration(),
            TokenKind::Const => self.parse_const_declaration(),
            TokenKind::Fn | TokenKind::Async => self
                .parse_function_declaration()
                .map(Statement::FunctionDeclaration),
            TokenKind::Struct => self
                .parse_struct_declaration()
                .map(Statement::StructDeclaration),
            TokenKind::Enum => self
                .parse_enum_declaration()
                .map(Statement::EnumDeclaration),
            TokenKind::Trait => self
                .parse_trait_declaration()
                .map(Statement::TraitDeclaration),
            TokenKind::Impl => self
                .parse_impl_declaration()
                .map(Statement::ImplDeclaration),
            TokenKind::Module => self
                .parse_module_declaration()
                .map(Statement::ModuleDeclaration),
            TokenKind::Use => self.parse_use_declaration().map(Statement::UseDeclaration),
            TokenKind::Type => self
                .parse_type_declaration()
                .map(Statement::TypeDeclaration),
            TokenKind::Macro => self.parse_macro_declaration(),
            TokenKind::While => self.parse_while_statement(),
            TokenKind::Return => self.parse_return_statement(),
            TokenKind::Break => {
                self.advance();
                Ok(Statement::BreakStatement)
            }
            TokenKind::Continue => {
                self.advance();
                Ok(Statement::ContinueStatement)
            }
            TokenKind::Set => self.parse_set_statement(),
            TokenKind::Add | TokenKind::Sub | TokenKind::Inc | TokenKind::Dec => {
                self.parse_mutation_statement()
            }
            TokenKind::Put => self.parse_put_statement(),
            TokenKind::Error => {
                self.advance();
                let expr = self.parse_expression()?;
                Ok(Statement::ErrorStatement(expr))
            }
            TokenKind::Warn => {
                self.advance();
                let expr = self.parse_expression()?;
                Ok(Statement::WarnStatement(expr))
            }
            TokenKind::Info => {
                self.advance();
                let expr = self.parse_expression()?;
                Ok(Statement::InfoStatement(expr))
            }
            TokenKind::Pub => {
                // Public modifier - parse the next statement and mark it as public
                self.advance(); // consume 'pub'
                let stmt = self.parse_statement()?;
                // For now, just parse it normally - the modifier is noted but not stored
                Ok(stmt)
            }
            TokenKind::Spawn => {
                // Spawn an async task: spawn expr
                self.advance(); // consume 'spawn'
                let expr = self.parse_expression()?;
                Ok(Statement::ExpressionStatement(Expression::Call {
                    func: Box::new(Expression::Identifier("spawn_task".into())),
                    args: vec![expr],
                }))
            }
            _ => {
                if self.starts_assignment_statement() {
                    return self.parse_assignment_statement();
                }
                let expr = self.parse_expression()?;
                if let Expression::Call { func, args } = &expr {
                    if let Expression::Identifier(name) = func.as_ref() {
                        if let Some(declaration) = self.macros.get(name).cloned() {
                            return self.expand_macro_call(&declaration, args);
                        }
                    }
                }
                Ok(Statement::ExpressionStatement(expr))
            }
        }
    }

    /// Parse the strict variable declaration syntax:
    /// `var name := value` or `var name Type := value`.
    fn parse_var_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume `var`
        let name = self.expect_identifier()?;

        if self.match_token(&TokenKind::Colon) {
            return Err(ParseError::with_suggestion(
                "Variable declarations no longer use `:` before the type",
                self.current().span,
                format!("Use `var {name} type := value` or `var {name} := value`"),
            ));
        }

        let ty = if matches!(self.peek(), TokenKind::ColonEq) {
            None
        } else if matches!(self.peek(), TokenKind::Eq) {
            return Err(ParseError::with_suggestion(
                "Variable declarations must use `:=` to initialize a value",
                self.current().span,
                format!("Use `var {name} := value`"),
            ));
        } else {
            Some(self.parse_type().map_err(|error| {
                ParseError::with_suggestion(
                    "Expected a type or `:=` after the variable name",
                    error.span,
                    format!("Use `var {name} type := value` or `var {name} := value`"),
                )
            })?)
        };

        if !self.match_token(&TokenKind::ColonEq) {
            return Err(ParseError::with_suggestion(
                "Variable declarations must initialize a value with `:=`",
                self.current().span,
                format!("Use `var {name} := value` or `var {name} type := value`"),
            ));
        }

        let value = self.parse_expression()?;
        Ok(Statement::VarDeclaration {
            name,
            ty,
            value: Some(value),
        })
    }

    fn parse_let_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'let'
        let name = self.expect_identifier()?;
        let ty = if self.match_token(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_initializer()?;
        let value = self.parse_expression()?;
        Ok(Statement::LetDeclaration { name, ty, value })
    }

    fn parse_const_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'const'
        let name = self.expect_identifier()?;
        self.expect_initializer()?;
        let value = self.parse_expression()?;
        Ok(Statement::ConstDeclaration { name, value })
    }

    fn parse_return_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'return'
        let value = if self.is_at_end()
            || matches!(
                self.peek(),
                TokenKind::Newline
                    | TokenKind::Eof
                    | TokenKind::End
                    | TokenKind::Else
                    | TokenKind::RBrace
            ) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        Ok(Statement::ReturnStatement(value))
    }

    fn parse_while_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'while'
        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        self.expect(&TokenKind::End)?;
        self.expect(&TokenKind::While)?;
        Ok(Statement::ExpressionStatement(Expression::IfExpression {
            condition: Box::new(condition),
            then_block: body,
            else_block: None,
        }))
    }

    fn parse_set_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume `set`
        let target = self.parse_expression()?;
        self.expect(&TokenKind::To)?;
        let value = self.parse_expression()?;

        if expression_contains_target(&value, &target) {
            return Err(ParseError::new(
                format!(
                    "Self-reference in `set {}` is not allowed; use an explicit mutation command",
                    expression_description(&target)
                ),
                self.current().span,
            ));
        }

        Ok(Statement::Set { target, value })
    }

    fn parse_mutation_statement(&mut self) -> Result<Statement, ParseError> {
        let op = match self.peek() {
            TokenKind::Add => MutationOp::Add,
            TokenKind::Sub => MutationOp::Sub,
            TokenKind::Inc => MutationOp::Inc,
            TokenKind::Dec => MutationOp::Dec,
            _ => {
                return Err(ParseError::new(
                    format!("Expected mutation command, got {:?}", self.peek()),
                    self.current().span,
                ));
            }
        };
        self.advance();
        let target = self.parse_expression()?;
        let value = if self.match_token(&TokenKind::Comma) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        if matches!(op, MutationOp::Inc | MutationOp::Dec) && value.is_some() {
            return Err(ParseError::new(
                "`inc` and `dec` do not take a value; use `add` or `sub` for a custom amount",
                self.current().span,
            ));
        }
        Ok(Statement::Mutation { target, op, value })
    }

    /// Accept the conventional `=` initializer and the legacy `:=` spelling.
    fn match_initializer(&mut self) -> bool {
        self.match_token(&TokenKind::Eq) || self.match_token(&TokenKind::ColonEq)
    }

    fn expect_initializer(&mut self) -> Result<(), ParseError> {
        if self.match_initializer() {
            Ok(())
        } else {
            Err(ParseError::new(
                format!("Expected `=` after declaration, got {:?}", self.peek()),
                self.current().span,
            ))
        }
    }

    /// Detect an assignment whose target is an identifier, field, or index expression.
    fn starts_assignment_statement(&self) -> bool {
        let mut index = self.pos;
        if !matches!(
            self.tokens.get(index).map(|token| &token.kind),
            Some(TokenKind::Identifier(_))
        ) {
            return false;
        }
        index += 1;

        loop {
            match self.tokens.get(index).map(|token| &token.kind) {
                Some(TokenKind::Dot) => {
                    index += 1;
                    if !matches!(
                        self.tokens.get(index).map(|token| &token.kind),
                        Some(TokenKind::Identifier(_)) | Some(TokenKind::Await)
                    ) {
                        return false;
                    }
                    index += 1;
                }
                Some(TokenKind::LBracket) => {
                    let mut depth = 1;
                    index += 1;
                    while depth > 0 {
                        match self.tokens.get(index).map(|token| &token.kind) {
                            Some(TokenKind::LBracket) => depth += 1,
                            Some(TokenKind::RBracket) => depth -= 1,
                            Some(TokenKind::Eof) | None => return false,
                            _ => {}
                        }
                        index += 1;
                    }
                }
                _ => break,
            }
        }

        matches!(
            self.tokens.get(index).map(|token| &token.kind),
            Some(
                TokenKind::Eq
                    | TokenKind::PlusEq
                    | TokenKind::MinusEq
                    | TokenKind::StarEq
                    | TokenKind::SlashEq
                    | TokenKind::PercentEq
                    | TokenKind::AmpEq
                    | TokenKind::PipeEq
                    | TokenKind::CaretEq
                    | TokenKind::LtLtEq
                    | TokenKind::GtGtEq
            )
        )
    }

    fn parse_assignment_target(&mut self) -> Result<Expression, ParseError> {
        let name = self.expect_identifier()?;
        let mut target = Expression::Identifier(name);

        loop {
            match self.peek() {
                TokenKind::Dot => {
                    self.advance();
                    let field = self.expect_identifier()?;
                    target = Expression::FieldAccess {
                        object: Box::new(target),
                        field,
                    };
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expression()?;
                    self.expect(&TokenKind::RBracket)?;
                    target = Expression::Index {
                        object: Box::new(target),
                        index: Box::new(index),
                    };
                }
                _ => break,
            }
        }

        Ok(target)
    }

    fn parse_assignment_statement(&mut self) -> Result<Statement, ParseError> {
        let target = self.parse_assignment_target()?;
        let operator = self.advance().kind.clone();
        let value = self.parse_expression()?;

        match operator {
            TokenKind::Eq => Ok(Statement::Set { target, value }),
            TokenKind::PlusEq => Ok(Statement::Mutation {
                target,
                op: MutationOp::Add,
                value: Some(value),
            }),
            TokenKind::MinusEq => Ok(Statement::Mutation {
                target,
                op: MutationOp::Sub,
                value: Some(value),
            }),
            TokenKind::StarEq => Ok(Statement::Set {
                target: target.clone(),
                value: Expression::BinaryOp {
                    op: BinaryOp::Mul,
                    left: Box::new(target),
                    right: Box::new(value),
                },
            }),
            TokenKind::SlashEq => Ok(Statement::Set {
                target: target.clone(),
                value: Expression::BinaryOp {
                    op: BinaryOp::Div,
                    left: Box::new(target),
                    right: Box::new(value),
                },
            }),
            TokenKind::PercentEq => Ok(Statement::Set {
                target: target.clone(),
                value: Expression::BinaryOp {
                    op: BinaryOp::Mod,
                    left: Box::new(target),
                    right: Box::new(value),
                },
            }),
            TokenKind::AmpEq => Ok(Statement::Set {
                target: target.clone(),
                value: Expression::BinaryOp {
                    op: BinaryOp::BitAnd,
                    left: Box::new(target),
                    right: Box::new(value),
                },
            }),
            TokenKind::PipeEq => Ok(Statement::Set {
                target: target.clone(),
                value: Expression::BinaryOp {
                    op: BinaryOp::BitOr,
                    left: Box::new(target),
                    right: Box::new(value),
                },
            }),
            TokenKind::CaretEq => Ok(Statement::Set {
                target: target.clone(),
                value: Expression::BinaryOp {
                    op: BinaryOp::BitXor,
                    left: Box::new(target),
                    right: Box::new(value),
                },
            }),
            TokenKind::LtLtEq => Ok(Statement::Set {
                target: target.clone(),
                value: Expression::BinaryOp {
                    op: BinaryOp::Shl,
                    left: Box::new(target),
                    right: Box::new(value),
                },
            }),
            TokenKind::GtGtEq => Ok(Statement::Set {
                target: target.clone(),
                value: Expression::BinaryOp {
                    op: BinaryOp::Shr,
                    left: Box::new(target),
                    right: Box::new(value),
                },
            }),
            _ => Err(ParseError::new(
                "Expected assignment operator",
                self.current().span,
            )),
        }
    }

    fn parse_put_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'put'
        let no_newline = if self.peek() == &TokenKind::Minus {
            match self.tokens.get(self.pos + 1).map(|token| &token.kind) {
                Some(TokenKind::Identifier(name)) if name == "n" => {
                    self.advance(); // consume '-'
                    self.advance(); // consume 'n'
                    true
                }
                Some(TokenKind::Identifier(name)) if name == "u" => {
                    // `-u "..."` is no longer a valid Unicode literal. Leave
                    // it for parse_expression so it receives a migration error.
                    false
                }
                _ => {
                    return Err(ParseError::with_suggestion(
                        "Unknown `put` flag",
                        self.current().span,
                        "Use `put -n value` for no-newline output or `put unicode \"text\"` for Unicode text",
                    ));
                }
            }
        } else {
            false
        };
        let expr = self.parse_expression()?;
        let redirect = if self.match_token(&TokenKind::Gt) {
            Some(Redirect::Write(self.parse_expression()?))
        } else if self.match_token(&TokenKind::GtGt) {
            Some(Redirect::Append(self.parse_expression()?))
        } else {
            None
        };
        Ok(Statement::PutStatement {
            no_newline,
            expr,
            redirect,
        })
    }

    // === Declaration parsing ===

    // Function, struct, enum, trait, and impl parsers all preserve the source
    // declaration shape so later phases can share one AST representation.
    fn parse_function_declaration(&mut self) -> Result<FunctionDecl, ParseError> {
        // Check for async modifier
        let is_async = self.match_token(&TokenKind::Async);
        self.advance(); // consume 'fn'
        let name = self.expect_identifier()?;
        let generics = self.parse_generic_params()?;
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;
        let return_type = if self.match_token(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = if self.peek() == &TokenKind::LBrace {
            self.advance();
            let stmts = self.parse_block()?;
            self.expect(&TokenKind::RBrace)?;
            Some(stmts)
        } else if self.peek() == &TokenKind::End {
            // An explicit empty body is still a complete function.
            self.advance();
            self.expect(&TokenKind::Fn)?;
            Some(Vec::new())
        } else if self.is_at_end() {
            // A declaration-only signature at EOF remains valid.
            None
        } else {
            // Parse body until 'end fn'
            let stmts = self.parse_block()?;
            self.expect(&TokenKind::End)?;
            self.expect(&TokenKind::Fn)?;
            Some(stmts)
        };
        Ok(FunctionDecl {
            name,
            params,
            return_type,
            body,
            is_async,
            generics,
        })
    }

    /// Parse generic parameter lists such as `<T: Bound1 + Bound2, U>` used by
    /// generic functions and structs. Returns an empty list when absent.
    fn parse_generic_params(&mut self) -> Result<Vec<GenericParam>, ParseError> {
        let mut params = Vec::new();
        if self.peek() != &TokenKind::Lt {
            return Ok(params);
        }
        self.advance(); // consume '<'
        loop {
            let name = self.expect_identifier()?;
            let mut bounds = Vec::new();
            if self.match_token(&TokenKind::Colon) {
                bounds.push(self.expect_identifier()?);
                while self.match_token(&TokenKind::Plus) {
                    bounds.push(self.expect_identifier()?);
                }
            }
            params.push(GenericParam { name, bounds });
            if !self.match_token(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::Gt)?;
        Ok(params)
    }

    fn parse_params(&mut self) -> Result<Vec<Parameter>, ParseError> {
        let mut params = Vec::new();
        if self.peek() == &TokenKind::RParen {
            return Ok(params);
        }
        loop {
            let name = self.expect_identifier()?;
            // Handle 'self' as a special case (no type annotation needed)
            if name == "self" {
                params.push(Parameter {
                    name,
                    ty: TypeAnnotation::Named("Self".into()),
                    default: None,
                });
            } else {
                self.expect(&TokenKind::Colon)?;
                let ty = self.parse_type()?;
                let default = if self.match_initializer() {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                params.push(Parameter { name, ty, default });
            }
            if !self.match_token(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    fn parse_struct_declaration(&mut self) -> Result<StructDecl, ParseError> {
        self.advance(); // consume 'struct'
        let name = self.expect_identifier()?;
        let generics = self.parse_generic_params()?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.match_token(&TokenKind::End) {
            if self.peek() == &TokenKind::Fn {
                methods.push(self.parse_function_declaration()?);
            } else {
                let mutable = self.match_token(&TokenKind::Var);
                let field_name = self.expect_identifier()?;
                self.expect(&TokenKind::Colon)?;
                let ty = self.parse_type()?;
                let default = if self.match_initializer() {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                fields.push(StructField {
                    mutable,
                    name: field_name,
                    ty,
                    default,
                });
            }
        }
        // consume 'struct'
        self.expect(&TokenKind::Struct)?;

        Ok(StructDecl {
            name,
            fields,
            methods,
            generics,
        })
    }

    fn parse_enum_declaration(&mut self) -> Result<EnumDecl, ParseError> {
        self.advance(); // consume 'enum'
        let name = self.expect_identifier()?;
        let mut variants = Vec::new();
        let mut methods = Vec::new();

        while !self.match_token(&TokenKind::End) {
            if self.peek() == &TokenKind::Fn {
                methods.push(self.parse_function_declaration()?);
            } else {
                let variant_name = self.expect_identifier()?;
                if self.match_token(&TokenKind::LParen) {
                    let mut fields = Vec::new();
                    if self.peek() != &TokenKind::RParen {
                        // Check if this is named tuple variant: Variant(param: type, ...)
                        // or plain tuple variant: Variant(type, type, ...)
                        if let TokenKind::Identifier(_) = self.peek() {
                            // Look ahead: is the next token ':' or ',' or ')'?
                            // If Identifier followed by ':' it's a named field
                            let saved_pos = self.pos;
                            let param_name = self.expect_identifier()?;
                            if self.match_token(&TokenKind::Colon) {
                                // Named field: param: type
                                let ty = self.parse_type()?;
                                fields.push((param_name, ty));
                            } else {
                                // Not a named field - rewind and parse as type
                                self.pos = saved_pos;
                                let ty = self.parse_type()?;
                                fields.push(("".to_string(), ty)); // unnamed field placeholder
                            }
                        } else {
                            let ty = self.parse_type()?;
                            fields.push(("".to_string(), ty)); // unnamed field placeholder
                        }
                        while self.match_token(&TokenKind::Comma) {
                            if self.peek() == &TokenKind::RParen {
                                break;
                            }
                            if let TokenKind::Identifier(_) = self.peek() {
                                let saved_pos = self.pos;
                                let param_name = self.expect_identifier()?;
                                if self.match_token(&TokenKind::Colon) {
                                    let ty = self.parse_type()?;
                                    fields.push((param_name, ty));
                                } else {
                                    self.pos = saved_pos;
                                    let ty = self.parse_type()?;
                                    fields.push(("".to_string(), ty));
                                }
                            } else {
                                let ty = self.parse_type()?;
                                fields.push(("".to_string(), ty));
                            }
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    // Check if all fields have names -> Struct variant, else Tuple variant
                    if fields.iter().all(|(name, _)| !name.is_empty()) {
                        variants.push(EnumVariant::Struct(variant_name, fields));
                    } else {
                        let types: Vec<TypeAnnotation> =
                            fields.into_iter().map(|(_, ty)| ty).collect();
                        variants.push(EnumVariant::Tuple(variant_name, types));
                    }
                } else if self.peek() == &TokenKind::LBrace {
                    self.advance();
                    let mut fields = Vec::new();
                    if self.peek() != &TokenKind::RBrace {
                        let fname = self.expect_identifier()?;
                        self.expect(&TokenKind::Colon)?;
                        let fty = self.parse_type()?;
                        fields.push((fname, fty));
                        while self.match_token(&TokenKind::Comma) {
                            if self.peek() == &TokenKind::RBrace {
                                break;
                            }
                            let fname = self.expect_identifier()?;
                            self.expect(&TokenKind::Colon)?;
                            let fty = self.parse_type()?;
                            fields.push((fname, fty));
                        }
                    }
                    self.expect(&TokenKind::RBrace)?;
                    variants.push(EnumVariant::Struct(variant_name, fields));
                } else {
                    variants.push(EnumVariant::Unit(variant_name));
                }
            }
        }
        // Empty enums are not valid Poly declarations. This also prevents
        // `enum` followed immediately by `end enum` from being accepted as
        // an enum whose name was accidentally taken from a variant line.
        if variants.is_empty() && methods.is_empty() {
            return Err(ParseError::new(
                "Enum declaration must contain at least one variant or method",
                self.current().span,
            ));
        }

        // consume 'enum'
        self.expect(&TokenKind::Enum)?;

        Ok(EnumDecl {
            name,
            variants,
            methods,
        })
    }

    fn parse_trait_declaration(&mut self) -> Result<TraitDecl, ParseError> {
        self.advance(); // consume 'trait'
        let name = self.expect_identifier()?;
        let mut methods = Vec::new();

        while !self.match_token(&TokenKind::End) {
            // Check for async modifier
            let is_async = self.match_token(&TokenKind::Async);
            self.expect(&TokenKind::Fn)?;
            let method_name = self.expect_identifier()?;
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;
            let return_type = if self.match_token(&TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            // Trait methods don't have bodies
            methods.push(FunctionDecl {
                name: method_name,
                params,
                return_type,
                body: None,
                is_async,
                generics: Vec::new(),
            });
        }
        // consume 'trait'
        self.expect(&TokenKind::Trait)?;

        Ok(TraitDecl { name, methods })
    }

    fn parse_impl_declaration(&mut self) -> Result<ImplDecl, ParseError> {
        self.advance(); // consume 'impl'
        let type_name = self.expect_identifier()?;

        // Check for `for` keyword (trait impl)
        let trait_name = if self.match_token(&TokenKind::For) {
            Some(type_name.clone())
        } else {
            None
        };

        let type_name = if trait_name.is_some() {
            self.expect_identifier()?
        } else {
            type_name
        };

        let mut methods = Vec::new();
        while !self.match_token(&TokenKind::End) {
            // Check for async modifier
            let is_async = self.match_token(&TokenKind::Async);
            self.expect(&TokenKind::Fn)?;
            let method_name = self.expect_identifier()?;
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;
            let return_type = if self.match_token(&TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            // Parse method body
            let body = if self.peek() == &TokenKind::LBrace {
                self.advance();
                let stmts = self.parse_block()?;
                self.expect(&TokenKind::RBrace)?;
                Some(stmts)
            } else if self.peek() == &TokenKind::End {
                None
            } else {
                let stmts = self.parse_block()?;
                if self.peek() == &TokenKind::End {
                    self.advance();
                    self.expect(&TokenKind::Fn)?;
                }
                Some(stmts)
            };
            methods.push(FunctionDecl {
                name: method_name,
                params,
                return_type,
                body,
                is_async,
                generics: Vec::new(),
            });
        }
        // consume 'impl'
        self.expect(&TokenKind::Impl)?;

        Ok(ImplDecl {
            trait_name,
            type_name,
            methods,
        })
    }

    fn parse_module_declaration(&mut self) -> Result<ModuleDecl, ParseError> {
        self.advance(); // consume 'module'
        let name = self.expect_identifier()?;
        let mut statements = Block::new();

        while !self.match_token(&TokenKind::End) {
            let start = self.pos;
            let statement = self.parse_statement()?;
            statements.push(Spanned::new(statement, self.statement_span(start)));
        }
        // consume 'module'
        self.expect(&TokenKind::Module)?;

        Ok(ModuleDecl { name, statements })
    }

    fn parse_use_declaration(&mut self) -> Result<UseDecl, ParseError> {
        self.advance(); // consume 'use'
        let mut path = vec![self.expect_identifier()?];
        while self.match_token(&TokenKind::ColonColon) {
            if self.peek() == &TokenKind::Star {
                self.advance();
                break;
            }
            path.push(self.expect_identifier()?);
        }
        let alias = if self.match_token(&TokenKind::As) {
            Some(self.expect_identifier()?)
        } else {
            None
        };
        Ok(UseDecl { path, alias })
    }

    fn parse_type_declaration(&mut self) -> Result<TypeDecl, ParseError> {
        self.advance(); // consume 'type'
        let name = self.expect_identifier()?;
        self.expect_initializer()?;
        let ty = self.parse_type()?;
        Ok(TypeDecl { name, ty })
    }

    // === Macro parsing ===

    // Statement macros expand during parsing: `macro name(params)` collects
    // body statements, and later `name(arg, ...)` calls are replaced by that
    // body with parameter identifiers substituted for the arguments.
    fn parse_macro_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'macro'
        let name = self.expect_identifier()?;
        let mut params = Vec::new();
        if self.match_token(&TokenKind::LParen) {
            if self.peek() != &TokenKind::RParen {
                params.push(self.expect_identifier()?);
                while self.match_token(&TokenKind::Comma) {
                    params.push(self.expect_identifier()?);
                }
            }
            self.expect(&TokenKind::RParen)?;
        }
        let body = self.parse_block()?;
        self.expect(&TokenKind::End)?;
        self.expect(&TokenKind::Macro)?;
        self.macros
            .insert(name.clone(), MacroDecl { name, params, body });
        // The declaration itself is not a statement; continue with the next one.
        self.parse_statement()
    }

    /// Expand a macro invocation into a block statement whose bodies have had
    /// parameter identifiers replaced by the supplied argument expressions.
    fn expand_macro_call(
        &self,
        declaration: &MacroDecl,
        args: &[Expression],
    ) -> Result<Statement, ParseError> {
        if declaration.params.len() != args.len() {
            return Err(ParseError::new(
                format!(
                    "macro `{}` expects {} arguments, got {}",
                    declaration.name,
                    declaration.params.len(),
                    args.len()
                ),
                self.current().span,
            ));
        }
        let mut substitutions = HashMap::new();
        for (param, arg) in declaration.params.iter().zip(args.iter()) {
            substitutions.insert(param.clone(), arg.clone());
        }
        let expanded = declaration
            .body
            .iter()
            .map(|statement| substitute_spanned_statement(statement, &substitutions))
            .collect();
        Ok(Statement::Block(expanded))
    }

    // === Type parsing ===

    // Type parsing is recursive because containers and function signatures nest
    // other annotations inside delimiters such as `<...>` and `(...)`.
    fn parse_type(&mut self) -> Result<TypeAnnotation, ParseError> {
        match self.peek().clone() {
            TokenKind::I8 => {
                self.advance();
                Ok(TypeAnnotation::Named("i8".into()))
            }
            TokenKind::U8 => {
                self.advance();
                Ok(TypeAnnotation::Named("u8".into()))
            }
            TokenKind::I16 => {
                self.advance();
                Ok(TypeAnnotation::Named("i16".into()))
            }
            TokenKind::U16 => {
                self.advance();
                Ok(TypeAnnotation::Named("u16".into()))
            }
            TokenKind::I32 => {
                self.advance();
                Ok(TypeAnnotation::Named("i32".into()))
            }
            TokenKind::U32 => {
                self.advance();
                Ok(TypeAnnotation::Named("u32".into()))
            }
            TokenKind::I64 => {
                self.advance();
                Ok(TypeAnnotation::Named("i64".into()))
            }
            TokenKind::U64 => {
                self.advance();
                Ok(TypeAnnotation::Named("u64".into()))
            }
            TokenKind::I128 => {
                self.advance();
                Ok(TypeAnnotation::Named("i128".into()))
            }
            TokenKind::U128 => {
                self.advance();
                Ok(TypeAnnotation::Named("u128".into()))
            }
            TokenKind::F32 => {
                self.advance();
                Ok(TypeAnnotation::Named("f32".into()))
            }
            TokenKind::F64 => {
                self.advance();
                Ok(TypeAnnotation::Named("f64".into()))
            }
            TokenKind::ISize => {
                self.advance();
                Ok(TypeAnnotation::Named("isize".into()))
            }
            TokenKind::USize => {
                self.advance();
                Ok(TypeAnnotation::Named("usize".into()))
            }
            TokenKind::Bool => {
                self.advance();
                Ok(TypeAnnotation::Named("bool".into()))
            }
            TokenKind::Char => {
                self.advance();
                Ok(TypeAnnotation::Named("char".into()))
            }
            TokenKind::String => {
                self.advance();
                Ok(TypeAnnotation::Named("string".into()))
            }
            TokenKind::UChar => {
                self.advance();
                Ok(TypeAnnotation::Named("uchar".into()))
            }
            TokenKind::UString => {
                self.advance();
                Ok(TypeAnnotation::Named("ustring".into()))
            }
            TokenKind::Byte => {
                self.advance();
                Ok(TypeAnnotation::Named("byte".into()))
            }
            TokenKind::Bytes => {
                self.advance();
                Ok(TypeAnnotation::Named("bytes".into()))
            }
            TokenKind::Ptr => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(TypeAnnotation::Pointer(Box::new(inner)))
            }
            TokenKind::Vec => {
                self.advance();
                self.expect(&TokenKind::Lt)?;
                let inner = self.parse_type()?;
                self.expect(&TokenKind::Gt)?;
                Ok(TypeAnnotation::Vec(Box::new(inner)))
            }
            TokenKind::Option => {
                self.advance();
                self.expect(&TokenKind::Lt)?;
                let inner = self.parse_type()?;
                self.expect(&TokenKind::Gt)?;
                Ok(TypeAnnotation::Option(Box::new(inner)))
            }
            TokenKind::Result => {
                self.advance();
                self.expect(&TokenKind::Lt)?;
                let ok_ty = self.parse_type()?;
                self.expect(&TokenKind::Comma)?;
                let err_ty = self.parse_type()?;
                self.expect(&TokenKind::Gt)?;
                Ok(TypeAnnotation::Result(Box::new(ok_ty), Box::new(err_ty)))
            }
            TokenKind::Pipe => {
                // Closure type annotation: `|x: i32, y: f64| bool`. Parameter
                // names are optional and discarded; only the types matter for
                // the annotation (they map to TypeAnnotation::Function).
                self.advance(); // consume '|'
                let mut params = Vec::new();
                if *self.peek() != TokenKind::Pipe {
                    loop {
                        // Optional `name: ` prefix before each parameter type.
                        if matches!(self.peek(), TokenKind::Identifier(_))
                            && self.tokens.get(self.pos + 1).map(|token| &token.kind)
                                == Some(&TokenKind::Colon)
                        {
                            self.advance(); // consume the parameter name
                            self.advance(); // consume ':'
                        }
                        params.push(self.parse_type()?);
                        if !self.match_token(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::Pipe)?;
                let ret = self.parse_type()?;
                Ok(TypeAnnotation::Function {
                    params,
                    ret: Box::new(ret),
                })
            }
            TokenKind::Fn => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let mut params = Vec::new();
                if self.peek() != &TokenKind::RParen {
                    params.push(self.parse_type()?);
                    while self.match_token(&TokenKind::Comma) {
                        params.push(self.parse_type()?);
                    }
                }
                self.expect(&TokenKind::RParen)?;
                self.expect(&TokenKind::Arrow)?;
                let ret = self.parse_type()?;
                Ok(TypeAnnotation::Function {
                    params,
                    ret: Box::new(ret),
                })
            }
            TokenKind::Identifier(name) => {
                self.advance();
                if self.match_token(&TokenKind::Lt) {
                    let mut args = vec![self.parse_type()?];
                    while self.match_token(&TokenKind::Comma) {
                        args.push(self.parse_type()?);
                    }
                    self.expect(&TokenKind::Gt)?;
                    Ok(TypeAnnotation::Generic { name, args })
                } else {
                    Ok(TypeAnnotation::Named(name))
                }
            }
            TokenKind::Amp => {
                self.advance();
                let mutability = if let TokenKind::Identifier(ref s) = self.peek() {
                    if s == "mut" {
                        self.advance();
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                let inner = self.parse_type()?;
                Ok(TypeAnnotation::Reference(mutability, Box::new(inner)))
            }
            TokenKind::LBracket => {
                self.advance();
                let inner = self.parse_type()?;
                self.expect(&TokenKind::Semicolon)?;
                let size = self.parse_expression()?;
                self.expect(&TokenKind::RBracket)?;
                Ok(TypeAnnotation::Array(Box::new(inner), Box::new(size)))
            }
            TokenKind::LParen => {
                // Tuple type: (Type, Type, ...)
                self.advance(); // (
                let mut types = vec![self.parse_type()?];
                while self.match_token(&TokenKind::Comma) {
                    types.push(self.parse_type()?);
                }
                self.expect(&TokenKind::RParen)?;
                Ok(TypeAnnotation::Tuple(types))
            }
            _ => Err(ParseError::new(
                format!("Expected type, got {:?}", self.peek()),
                self.current().span,
            )),
        }
    }

    // === Expression parsing ===

    // Each helper below represents one precedence level. Lower-precedence
    // helpers call higher-precedence helpers first, producing left-associative
    // trees without a separate precedence table.
    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_or_expression()
    }

    fn parse_or_expression(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_and_expression()?;
        while self.peek() == &TokenKind::OrOr {
            self.advance();
            let right = self.parse_and_expression()?;
            left = Expression::BinaryOp {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and_expression(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_comparison()?;
        while self.peek() == &TokenKind::AndAnd {
            self.advance();
            let right = self.parse_comparison()?;
            left = Expression::BinaryOp {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_bitwise_or()?;
        loop {
            match self.peek() {
                TokenKind::Eq | TokenKind::EqEq => {
                    self.advance();
                    let right = self.parse_bitwise_or()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::Eq,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                TokenKind::NotEq => {
                    self.advance();
                    let right = self.parse_bitwise_or()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::NotEq,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                TokenKind::Lt => {
                    self.advance();
                    let right = self.parse_bitwise_or()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::Lt,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                TokenKind::Gt => {
                    self.advance();
                    let right = self.parse_bitwise_or()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::Gt,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                TokenKind::LtEq => {
                    self.advance();
                    let right = self.parse_bitwise_or()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::LtEq,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                TokenKind::GtEq => {
                    self.advance();
                    let right = self.parse_bitwise_or()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::GtEq,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_bitwise_xor()?;
        while self.peek() == &TokenKind::Pipe {
            self.advance();
            let right = self.parse_bitwise_xor()?;
            left = Expression::BinaryOp {
                op: BinaryOp::BitOr,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_bitwise_and()?;
        while self.peek() == &TokenKind::Caret {
            self.advance();
            let right = self.parse_bitwise_and()?;
            left = Expression::BinaryOp {
                op: BinaryOp::BitXor,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_shift()?;
        while self.peek() == &TokenKind::Amp {
            self.advance();
            let right = self.parse_shift()?;
            left = Expression::BinaryOp {
                op: BinaryOp::BitAnd,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_add_sub()?;
        loop {
            match self.peek() {
                TokenKind::LtLt => {
                    self.advance();
                    let right = self.parse_add_sub()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::Shl,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                TokenKind::GtGt => {
                    self.advance();
                    let right = self.parse_add_sub()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::Shr,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_add_sub(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_mul_div()?;
        loop {
            match self.peek() {
                TokenKind::Plus => {
                    self.advance();
                    let right = self.parse_mul_div()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::Add,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                TokenKind::Minus => {
                    // Inside a `get` flag value, `--` begins the next flag
                    // rather than a subtraction of a negative operand.
                    if self.stop_at_double_minus
                        && matches!(
                            self.tokens.get(self.pos + 1).map(|token| &token.kind),
                            Some(TokenKind::Minus)
                        )
                    {
                        break;
                    }
                    self.advance();
                    let right = self.parse_mul_div()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::Sub,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_mul_div(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.parse_unary()?;
        loop {
            match self.peek() {
                TokenKind::Star => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::Mul,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                TokenKind::Slash => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::Div,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                TokenKind::Percent => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expression::BinaryOp {
                        op: BinaryOp::Mod,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expression, ParseError> {
        // Prefix operators recurse into unary parsing so chains such as `!!x`
        // and `*ptr` bind tighter than every binary operator.
        match self.peek() {
            TokenKind::Minus => {
                if matches!(self.tokens.get(self.pos + 1).map(|token| &token.kind), Some(TokenKind::Identifier(name)) if name == "u")
                    && matches!(
                        self.tokens.get(self.pos + 2).map(|token| &token.kind),
                        Some(TokenKind::StringLiteral(_))
                    )
                {
                    return Err(ParseError::with_suggestion(
                        "The `unicode \"...\"` Unicode string syntax has been removed",
                        self.current().span,
                        "Use `unicode \"...\"` instead",
                    ));
                }

                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expression::UnaryOp {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Not => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expression::UnaryOp {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Tilde => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expression::UnaryOp {
                    op: UnaryOp::BitNot,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Star => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expression::UnaryOp {
                    op: UnaryOp::Deref,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Pipe => {
                // Closure: |params| body
                self.parse_closure()
            }
            TokenKind::Try => {
                self.advance(); // consume 'try'
                let expr = self.parse_unary()?;
                Ok(Expression::TryExpression(Box::new(expr)))
            }
            TokenKind::Await => {
                self.advance(); // consume 'await'
                let expr = self.parse_unary()?;
                // Represent as a method call: expr.await()
                Ok(Expression::MethodCall {
                    object: Box::new(expr),
                    method: "await".into(),
                    args: vec![],
                })
            }
            TokenKind::Move => {
                self.advance(); // consume 'move'
                                // Parse as closure with move semantics
                if *self.peek() == TokenKind::Pipe {
                    let closure = self.parse_closure()?;
                    // Mark as move closure (for transpilation)
                    Ok(closure)
                } else {
                    // move expr - just return the expression
                    self.parse_unary()
                }
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_closure(&mut self) -> Result<Expression, ParseError> {
        self.advance(); // consume first |
        let mut params = Vec::new();
        if *self.peek() != TokenKind::Pipe {
            loop {
                let name = self.expect_identifier()?;
                let ty = if self.match_token(&TokenKind::Colon) {
                    self.parse_type()?
                } else {
                    TypeAnnotation::Named("_".into()) // untyped
                };
                params.push(Parameter {
                    name,
                    ty,
                    default: None,
                });
                if !self.match_token(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::Pipe)?;
        let body = self.parse_expression()?;
        Ok(Expression::Closure {
            params,
            body: Box::new(body),
        })
    }

    fn parse_loop_expression(&mut self) -> Result<Expression, ParseError> {
        self.advance(); // consume 'loop'

        // Check for range loop: loop: i 0..10 or loop: i 1..3, 7, 19..21 step 2
        if self.peek() == &TokenKind::Colon {
            self.advance(); // consume ':'

            // Check for tuple destructuring: loop: (a, b) in collection
            if *self.peek() == TokenKind::LParen {
                // Parse tuple destructuring variables
                self.advance(); // (
                let mut vars = Vec::new();
                loop {
                    let name = self.expect_identifier()?;
                    vars.push(name);
                    if !self.match_token(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen)?;

                // Expect 'in' keyword
                self.expect(&TokenKind::In)?;

                // Parse the collection expression
                let collection = self.parse_expression()?;

                let body = self.parse_block()?;
                self.expect(&TokenKind::End)?;
                self.expect(&TokenKind::Loop)?;

                // The tuple pattern travels in the loop variable; the checker
                // registers each destructured name and codegen emits it as a
                // Rust tuple pattern: `for (a, b) in collection.iter() { ... }`.
                return Ok(Expression::LoopRange {
                    variable: format!("({})", vars.join(", ")),
                    ranges: vec![LoopRangePart::Value(Box::new(collection))],
                    body,
                });
            }

            // The loop variable is explicit: `loop: i 1..3, 7, 19..21`.
            let variable = self.expect_identifier()?;

            // `loop: item in collection` is the collection form of the same
            // syntax; keep it explicit rather than deriving a name from the
            // collection identifier.
            if self.match_token(&TokenKind::In) {
                let collection = self.parse_expression()?;
                let body = self.parse_block()?;
                self.expect(&TokenKind::End)?;
                self.expect(&TokenKind::Loop)?;
                return Ok(Expression::LoopRange {
                    variable,
                    ranges: vec![LoopRangePart::Value(Box::new(collection))],
                    body,
                });
            }

            // Parse the first range/value part.
            let mut range_parts = Vec::new();

            loop {
                let part = self.parse_loop_range_part()?;
                range_parts.push(part);

                if !self.match_token(&TokenKind::Comma) {
                    break;
                }
            }

            let body = self.parse_block()?;
            self.expect(&TokenKind::End)?;
            self.expect(&TokenKind::Loop)?;

            return Ok(Expression::LoopRange {
                variable,
                ranges: range_parts,
                body,
            });
        }

        // Infinite loop: `loop` ... `end loop` with a block body.
        let body = self.parse_block()?;
        self.expect(&TokenKind::End)?;
        self.expect(&TokenKind::Loop)?;
        Ok(Expression::InfiniteLoop(body))
    }

    /// Parse a single part of a loop range spec.
    /// This can be: 0..10, 0..=10, 0..10 step 2, or just 5 (a single value).
    /// Unlike ordinary range expressions, loop ranges include both endpoints.
    fn parse_loop_range_part(&mut self) -> Result<LoopRangePart, ParseError> {
        let first = self.parse_expression()?;

        // If parse_expression already consumed a range (e.g., 0..10), extract it
        if let Expression::Range { start, end, .. } = first {
            // Poly loop ranges include both endpoints. `..=` remains accepted
            // as an explicit spelling of the same inclusive behavior.
            let inclusive = true;
            // Check for optional step: ..10 step 2
            let step = if self.peek() == &TokenKind::Step {
                self.advance(); // consume 'step'
                Some(self.parse_expression()?)
            } else {
                None
            };
            Ok(LoopRangePart::Range {
                start,
                end,
                inclusive,
                step,
            })
        } else if self.peek() == &TokenKind::DotDot || self.peek() == &TokenKind::DotDotEq {
            // Loop ranges are inclusive even when written with `..`.
            let inclusive = true;
            self.advance(); // consume .. or ..=
            let end = self.parse_expression()?;

            // Check for optional step: ..10 step 2
            let step = if self.peek() == &TokenKind::Step {
                self.advance(); // consume 'step'
                Some(self.parse_expression()?)
            } else {
                None
            };

            Ok(LoopRangePart::Range {
                start: Box::new(first),
                end: Box::new(end),
                inclusive,
                step,
            })
        } else {
            // Single value
            Ok(LoopRangePart::Value(Box::new(first)))
        }
    }

    fn parse_for_expression(&mut self) -> Result<Expression, ParseError> {
        self.advance(); // consume 'for'
        let variable = self.expect_identifier()?;
        self.expect(&TokenKind::In)?;
        let iter = self.parse_expression()?;
        let body = self.parse_block()?;
        self.expect(&TokenKind::End)?;
        self.expect(&TokenKind::For)?;
        Ok(Expression::ForLoop {
            variable,
            iterable: Box::new(iter),
            body,
        })
    }

    fn parse_postfix(&mut self) -> Result<Expression, ParseError> {
        // Start with an atom, then repeatedly attach calls, indexing, and field
        // access. Repetition is what permits expressions like `a.b()[i].c`.
        let mut expr = self.parse_primary()?;

        loop {
            match self.peek() {
                TokenKind::Dot => {
                    self.advance();
                    // Check for .await
                    if *self.peek() == TokenKind::Await {
                        self.advance(); // consume 'await'
                        expr = Expression::MethodCall {
                            object: Box::new(expr),
                            method: "await".into(),
                            args: vec![],
                        };
                    } else {
                        let field = self.expect_field_name()?;
                        expr = Expression::FieldAccess {
                            object: Box::new(expr),
                            field,
                        };
                    }
                }
                TokenKind::LParen => {
                    self.advance();
                    let mut args = Vec::new();
                    if self.peek() != &TokenKind::RParen {
                        args.push(self.parse_expression()?);
                        while self.match_token(&TokenKind::Comma) {
                            args.push(self.parse_expression()?);
                        }
                    }
                    self.expect(&TokenKind::RParen)?;

                    // Check if this is a method call (object.method())
                    if let Expression::FieldAccess { object, field } = expr {
                        expr = Expression::MethodCall {
                            object,
                            method: field,
                            args,
                        };
                    } else {
                        expr = Expression::Call {
                            func: Box::new(expr),
                            args,
                        };
                    }
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expression()?;
                    self.expect(&TokenKind::RBracket)?;
                    expr = Expression::Index {
                        object: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                _ => break,
            }
        }

        // Check for `as` expression
        if self.peek() == &TokenKind::As {
            self.advance();
            let ty = self.parse_type()?;
            expr = Expression::AsExpression {
                expr: Box::new(expr),
                ty: Box::new(ty),
            };
        }

        // Check for `try` expression
        if self.peek() == &TokenKind::Try {
            self.advance();
            expr = Expression::TryExpression(Box::new(expr));
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expression, ParseError> {
        // Primary expressions are the leaves of the expression tree: literals,
        // names, grouped values, and language constructs such as `match`.
        match self.peek().clone() {
            TokenKind::IntLiteral(val) => {
                self.advance();
                // Check for range: 0..10 or 0..=10
                if self.peek() == &TokenKind::DotDot {
                    self.advance();
                    let end = self.parse_expression()?;
                    return Ok(Expression::Range {
                        start: Box::new(Expression::IntLiteral(val)),
                        end: Box::new(end),
                        inclusive: false,
                    });
                } else if self.peek() == &TokenKind::DotDotEq {
                    self.advance();
                    let end = self.parse_expression()?;
                    return Ok(Expression::Range {
                        start: Box::new(Expression::IntLiteral(val)),
                        end: Box::new(end),
                        inclusive: true,
                    });
                }
                Ok(Expression::IntLiteral(val))
            }
            TokenKind::FloatLiteral(val) => {
                self.advance();
                Ok(Expression::FloatLiteral(val))
            }
            TokenKind::StringLiteral(val) => {
                self.advance();
                Ok(Expression::StringLiteral(val))
            }
            TokenKind::UnicodeStringLiteral(_) => Err(ParseError::with_suggestion(
                "The legacy Unicode string syntax is no longer accepted",
                self.current().span,
                "Use `unicode \"...\"` instead",
            )),
            TokenKind::UnicodeCharLiteral(_) => Err(ParseError::with_suggestion(
                "Character literals must use the explicit `unicode` keyword",
                self.current().span,
                "Use `unicode 'c'` instead",
            )),
            TokenKind::Unicode => self.parse_unicode_literal(),
            TokenKind::BoolLiteral(val) => {
                self.advance();
                Ok(Expression::BoolLiteral(val))
            }
            TokenKind::ByteLiteral(val) => {
                self.advance();
                Ok(Expression::ByteLiteral(val))
            }
            TokenKind::Identifier(name) => {
                if name == "u"
                    && matches!(
                        self.tokens.get(self.pos + 1).map(|token| &token.kind),
                        Some(TokenKind::StringLiteral(_)) | Some(TokenKind::UnicodeCharLiteral(_))
                    )
                {
                    return Err(ParseError::with_suggestion(
                        "The legacy Unicode literal syntax is no longer accepted",
                        self.current().span,
                        "Use `unicode \"...\"` or `unicode 'c'` instead",
                    ));
                }
                self.advance();

                // Check for enum variant: EnumName::Variant
                if self.peek() == &TokenKind::ColonColon {
                    self.advance();
                    let variant = self.expect_identifier()?;
                    let data = if self.peek() == &TokenKind::LParen {
                        self.advance();
                        let mut args = Vec::new();
                        if self.peek() != &TokenKind::RParen {
                            args.push(self.parse_expression()?);
                            while self.match_token(&TokenKind::Comma) {
                                args.push(self.parse_expression()?);
                            }
                        }
                        self.expect(&TokenKind::RParen)?;
                        Some(args)
                    } else {
                        None
                    };
                    return Ok(Expression::EnumVariant {
                        enum_name: name,
                        variant,
                        data,
                    });
                }

                // Check for struct literal: StructName { ... }
                if self.peek() == &TokenKind::LBrace {
                    self.advance();
                    let mut fields = Vec::new();
                    if self.peek() != &TokenKind::RBrace {
                        let fname = self.expect_identifier()?;
                        self.expect(&TokenKind::Colon)?;
                        let fval = self.parse_expression()?;
                        fields.push((fname, fval));
                        while self.match_token(&TokenKind::Comma) {
                            if self.peek() == &TokenKind::RBrace {
                                break;
                            }
                            let fname = self.expect_identifier()?;
                            self.expect(&TokenKind::Colon)?;
                            let fval = self.parse_expression()?;
                            fields.push((fname, fval));
                        }
                    }
                    self.expect(&TokenKind::RBrace)?;
                    return Ok(Expression::StructLiteral { name, fields });
                }

                Ok(Expression::Identifier(name))
            }
            TokenKind::LParen => {
                self.advance();
                if self.peek() == &TokenKind::RParen {
                    self.advance();
                    return Ok(Expression::TupleLiteral(vec![]));
                }
                let first = self.parse_expression()?;
                if self.peek() == &TokenKind::Comma {
                    // Tuple
                    self.advance();
                    let mut elements = vec![first];
                    while self.peek() != &TokenKind::RParen {
                        elements.push(self.parse_expression()?);
                        if !self.match_token(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    Ok(Expression::TupleLiteral(elements))
                } else {
                    self.expect(&TokenKind::RParen)?;
                    Ok(Expression::Parenthesized(Box::new(first)))
                }
            }
            TokenKind::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                if self.peek() != &TokenKind::RBracket {
                    elements.push(self.parse_expression()?);
                    while self.match_token(&TokenKind::Comma) {
                        if self.peek() == &TokenKind::RBracket {
                            break;
                        }
                        elements.push(self.parse_expression()?);
                    }
                }
                self.expect(&TokenKind::RBracket)?;
                Ok(Expression::ArrayLiteral(elements))
            }
            TokenKind::Get => self.parse_get_expression(),
            TokenKind::If => self.parse_if_expression(),
            TokenKind::Match => self.parse_match_expression(),
            TokenKind::Loop => self.parse_loop_expression(),
            TokenKind::For => {
                // For loop: for item in collection
                self.parse_for_expression()
            }
            TokenKind::Unsafe => {
                self.advance();
                let stmts = self.parse_block()?;
                self.expect(&TokenKind::End)?;
                self.expect(&TokenKind::Unsafe)?;
                Ok(Expression::UnsafeBlock(stmts))
            }
            _ => Err(ParseError::new(
                format!("Unexpected token: {:?}", self.peek()),
                self.current().span,
            )),
        }
    }

    fn parse_unicode_literal(&mut self) -> Result<Expression, ParseError> {
        self.advance(); // consume `unicode`
        match self.peek().clone() {
            TokenKind::StringLiteral(value) => {
                self.advance();
                Ok(Expression::UnicodeStringLiteral(value))
            }
            TokenKind::UnicodeCharLiteral(value) => {
                self.advance();
                let character = value
                    .chars()
                    .next()
                    .expect("lexer guarantees one Unicode character");
                Ok(Expression::UnicodeCharLiteral(character))
            }
            _ => Err(ParseError::with_suggestion(
                "Expected a quoted string or character after `unicode`",
                self.current().span,
                "Use `unicode \"text\"` or `unicode 'c'`",
            )),
        }
    }

    fn parse_if_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_if_expression_inner(true)
    }

    fn starts_block_statement(&self) -> bool {
        matches!(
            self.peek(),
            TokenKind::Var
                | TokenKind::Let
                | TokenKind::Const
                | TokenKind::Fn
                | TokenKind::Async
                | TokenKind::Struct
                | TokenKind::Enum
                | TokenKind::Trait
                | TokenKind::Impl
                | TokenKind::Module
                | TokenKind::Use
                | TokenKind::Type
                | TokenKind::While
                | TokenKind::If
                | TokenKind::Return
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Set
                | TokenKind::Add
                | TokenKind::Sub
                | TokenKind::Inc
                | TokenKind::Dec
                | TokenKind::Put
                | TokenKind::Error
                | TokenKind::Warn
                | TokenKind::Info
                | TokenKind::Match
                | TokenKind::Loop
                | TokenKind::For
        )
    }

    /// Parse an if expression while optionally consuming its shared `end if`.
    /// Chained `else if` branches share one terminator, owned by the outermost
    /// if expression. A finite limit keeps malformed or machine-generated input
    /// from overflowing the parser thread's stack.
    fn parse_if_expression_inner(
        &mut self,
        consume_terminator: bool,
    ) -> Result<Expression, ParseError> {
        const MAX_IF_DEPTH: usize = 32;
        if self.if_depth >= MAX_IF_DEPTH {
            return Err(ParseError::new(
                format!("Maximum nested if depth ({MAX_IF_DEPTH}) exceeded"),
                self.current().span,
            ));
        }

        self.if_depth += 1;
        let result = self.parse_if_expression_inner_impl(consume_terminator);
        self.if_depth -= 1;
        result
    }

    fn parse_if_expression_inner_impl(
        &mut self,
        consume_terminator: bool,
    ) -> Result<Expression, ParseError> {
        self.advance(); // consume 'if'
        let condition = self.parse_expression()?;
        // New syntax: use comma instead of 'then'. Support the compact
        // expression form `if condition, value else other_value` as well
        // as the block form used by ordinary statements.
        // A comma is accepted for compatibility, but the canonical form is
        // now simply `if condition` followed by its block or expression.
        self.match_token(&TokenKind::Comma);
        let expression_start = self.pos;
        if !self.starts_block_statement() {
            if let Ok(then_expr) = self.parse_expression() {
                let then_span = self.statement_span(expression_start);
                if self.match_token(&TokenKind::Else) {
                    let else_start = self.pos;
                    if let Ok(else_expr) = self.parse_expression() {
                        let else_span = self.statement_span(else_start);
                        if consume_terminator && self.match_token(&TokenKind::End) {
                            self.expect(&TokenKind::If)?;
                        }
                        return Ok(Expression::IfExpression {
                            condition: Box::new(condition),
                            then_block: vec![Spanned::new(
                                Statement::ExpressionStatement(then_expr),
                                then_span,
                            )],
                            else_block: Some(vec![Spanned::new(
                                Statement::ExpressionStatement(else_expr),
                                else_span,
                            )]),
                        });
                    }
                    // `else` followed by a statement keyword belongs to the
                    // block form, not the compact expression form.
                    self.pos = else_start;
                }
            }
        }
        self.pos = expression_start;
        let then_block = self.parse_block()?;
        let else_block = if self.match_token(&TokenKind::Else) {
            // Consume comma after 'else' if present
            self.match_token(&TokenKind::Comma);
            if self.peek() == &TokenKind::If {
                // `else if` is one chained conditional, so the recursive
                // branch owns the final shared `end if`.
                let else_start = self.pos;
                let else_if = self.parse_if_expression_inner(true)?;
                vec![Spanned::new(
                    Statement::ExpressionStatement(else_if),
                    self.statement_span(else_start),
                )]
            } else {
                let block = self.parse_block()?;
                if consume_terminator {
                    self.expect(&TokenKind::End)?;
                    self.expect(&TokenKind::If)?;
                }
                block
            }
        } else {
            if consume_terminator {
                self.expect(&TokenKind::End)?;
                self.expect(&TokenKind::If)?;
            }
            vec![]
        };
        Ok(Expression::IfExpression {
            condition: Box::new(condition),
            then_block,
            else_block: Some(else_block),
        })
    }

    fn parse_match_expression(&mut self) -> Result<Expression, ParseError> {
        // Match arms are parsed until their closing `end match`; each arm owns
        // its pattern, optional guard, and either an expression or block body.
        self.advance(); // consume 'match'
        let scrutinee = self.parse_expression()?;
        let mut arms = Vec::new();

        while self.peek() != &TokenKind::End {
            let pattern = self.parse_pattern()?;
            let guard = if self.match_token(&TokenKind::If) {
                Some(self.parse_expression()?)
            } else {
                None
            };
            // Poly match arms use a comma between the pattern and body:
            // `pattern, expression`. The generated Rust uses `=>` later.
            self.expect(&TokenKind::Comma)?;
            let body = self.parse_match_arm_body()?;
            arms.push(MatchArm {
                pattern,
                guard,
                body,
            });
        }
        self.expect(&TokenKind::End)?;
        self.expect(&TokenKind::Match)?;

        Ok(Expression::MatchExpression {
            scrutinee: Box::new(scrutinee),
            arms,
        })
    }

    /// Parse the body of a match arm: either a single expression or a block of statements.
    fn parse_match_arm_body(&mut self) -> Result<MatchArmBody, ParseError> {
        // Parse statements until we hit something that starts a new match arm or end of match
        let mut stmts = Vec::new();

        loop {
            if self.is_at_end() || *self.peek() == TokenKind::End {
                break;
            }

            // Check if this looks like a new match arm pattern:
            // identifier, int literal, string literal, negative, or tuple pattern
            if self.could_be_pattern_start() {
                // Peek ahead: if we see a comma after a potential pattern,
                // this is a new arm.
                if self.peek_ahead_is_match_separator() {
                    break;
                }
            }

            let start = self.pos;
            let statement = self.parse_statement()?;
            stmts.push(Spanned::new(statement, self.statement_span(start)));
        }

        if stmts.is_empty() {
            // Shouldn't happen, but handle gracefully
            let expr = self.parse_expression()?;
            Ok(MatchArmBody::Expression(expr))
        } else if stmts.len() == 1 {
            if let Statement::ExpressionStatement(expr) = &stmts[0].node {
                Ok(MatchArmBody::Expression(expr.clone()))
            } else {
                Ok(MatchArmBody::Block(stmts))
            }
        } else {
            Ok(MatchArmBody::Block(stmts))
        }
    }

    /// Check if the current token could be the start of a match arm pattern.
    fn could_be_pattern_start(&self) -> bool {
        matches!(
            self.peek(),
            TokenKind::Identifier(_)
                | TokenKind::IntLiteral(_)
                | TokenKind::StringLiteral(_)
                | TokenKind::UnicodeStringLiteral(_)
                | TokenKind::UnicodeCharLiteral(_)
                | TokenKind::Unicode
                | TokenKind::BoolLiteral(_)
                | TokenKind::Minus
                | TokenKind::LParen
                | TokenKind::Pipe
        )
    }
    /// Parse a potential pattern and guard speculatively, then check for the
    /// comma that separates it from its body. This avoids mistaking an
    /// expression such as `x + 1` for the start of the next arm.
    fn peek_ahead_is_match_separator(&mut self) -> bool {
        let saved_pos = self.pos;
        let is_separator = self.parse_pattern().is_ok()
            && (!self.match_token(&TokenKind::If) || self.parse_expression().is_ok())
            && self.peek() == &TokenKind::Comma;
        self.pos = saved_pos;
        is_separator
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        match self.peek().clone() {
            TokenKind::Minus => {
                // Negative literal pattern
                self.advance();
                if let TokenKind::IntLiteral(val) = self.peek().clone() {
                    self.advance();
                    Ok(Pattern::Literal(Expression::IntLiteral(format!(
                        "-{}",
                        val
                    ))))
                } else {
                    Err(ParseError::new(
                        "Expected integer after -",
                        self.current().span,
                    ))
                }
            }
            TokenKind::IntLiteral(_)
            | TokenKind::StringLiteral(_)
            | TokenKind::UnicodeStringLiteral(_)
            | TokenKind::UnicodeCharLiteral(_)
            | TokenKind::Unicode
            | TokenKind::BoolLiteral(_) => {
                let expr = self.parse_primary()?;
                Ok(Pattern::Literal(expr))
            }
            TokenKind::Identifier(name) => {
                self.advance();
                if name == "_" {
                    return Ok(Pattern::Wildcard);
                }
                if self.peek() == &TokenKind::ColonColon {
                    // Enum pattern: EnumName::Variant(args)
                    self.advance();
                    let variant = self.expect_identifier()?;
                    let inner = if self.peek() == &TokenKind::LParen {
                        self.advance();
                        let mut patterns = Vec::new();
                        if self.peek() != &TokenKind::RParen {
                            patterns.push(self.parse_pattern()?);
                            while self.match_token(&TokenKind::Comma) {
                                patterns.push(self.parse_pattern()?);
                            }
                        }
                        self.expect(&TokenKind::RParen)?;
                        Some(patterns)
                    } else {
                        None
                    };
                    Ok(Pattern::Enum {
                        enum_name: name,
                        variant,
                        inner,
                    })
                } else if self.peek() == &TokenKind::LParen {
                    // Variant pattern without :: (e.g., Ok(content), Error(e))
                    self.advance();
                    let mut patterns = Vec::new();
                    if self.peek() != &TokenKind::RParen {
                        patterns.push(self.parse_pattern()?);
                        while self.match_token(&TokenKind::Comma) {
                            patterns.push(self.parse_pattern()?);
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    // Treat as Enum pattern with empty enum_name for now
                    Ok(Pattern::Enum {
                        enum_name: String::new(),
                        variant: name,
                        inner: Some(patterns),
                    })
                } else {
                    Ok(Pattern::Identifier(name))
                }
            }

            TokenKind::LParen => {
                self.advance();
                let mut patterns = Vec::new();
                patterns.push(self.parse_pattern()?);
                while self.match_token(&TokenKind::Comma) {
                    patterns.push(self.parse_pattern()?);
                }
                self.expect(&TokenKind::RParen)?;
                Ok(Pattern::Tuple(patterns))
            }
            _ => Err(ParseError::new(
                format!("Unexpected pattern: {:?}", self.peek()),
                self.current().span,
            )),
        }
    }

    /// Parse a `get` flag value. Inside a flag value, `--` begins the next
    /// flag, so the expression parser stops at a double minus instead of
    /// treating it as subtraction of a negative operand.
    fn parse_flag_value(&mut self) -> Result<Expression, ParseError> {
        let saved = self.stop_at_double_minus;
        self.stop_at_double_minus = true;
        let result = self.parse_expression();
        self.stop_at_double_minus = saved;
        result
    }

    fn parse_get_expression(&mut self) -> Result<Expression, ParseError> {
        self.advance(); // consume 'get'
        let mut prompt = None;
        let mut source = None; // input redirection: < "file"
        let mut flags = Vec::new();
        let mut with_clause = None;

        // Check for input redirection: < "file"
        if *self.peek() == TokenKind::Lt {
            self.advance(); // consume '<'
            source = Some(Box::new(self.parse_primary()?));
        }

        if source.is_some() && self.peek() == &TokenKind::Unicode {
            return Err(ParseError::new(
                "A file input cannot also have a Unicode prompt",
                self.current().span,
            ));
        }

        // Prompts use an explicit `unicode` keyword.
        if source.is_none() && self.peek() == &TokenKind::Unicode {
            self.advance();
            let parsed_prompt = match self.peek().clone() {
                TokenKind::StringLiteral(value) => {
                    self.advance();
                    Expression::StringLiteral(value)
                }
                _ => {
                    return Err(ParseError::with_suggestion(
                        "`get unicode` expects a quoted prompt",
                        self.current().span,
                        "Use `get unicode \"Prompt:\"`",
                    ));
                }
            };
            prompt = Some(parsed_prompt);
        }

        if source.is_none()
            && matches!(
                self.peek(),
                TokenKind::StringLiteral(_) | TokenKind::UnicodeStringLiteral(_)
            )
        {
            return Err(ParseError::with_suggestion(
                "Input prompts must use `get unicode \"Prompt:\"`; do not place a string directly after `get`",
                self.current().span,
                "Use `get unicode \"Prompt:\"`",
            ));
        }

        // Parse flags
        loop {
            match self.peek() {
                TokenKind::Minus => {
                    if matches!(
                        self.tokens.get(self.pos + 1).map(|token| &token.kind),
                        Some(TokenKind::Identifier(name)) if name == "u"
                    ) {
                        return Err(ParseError::with_suggestion(
                            "The `get -u` prompt syntax has been removed",
                            self.current().span,
                            "Use `get unicode \"Prompt:\"` instead",
                        ));
                    }
                    self.advance();
                    if self.peek() == &TokenKind::Minus {
                        self.advance();
                        match self.peek() {
                            TokenKind::Timeout => {
                                self.advance();
                                flags.push(GetFlag::Timeout(self.parse_flag_value()?));
                            }
                            TokenKind::Default => {
                                self.advance();
                                flags.push(GetFlag::Default(self.parse_flag_value()?));
                            }
                            TokenKind::Mask => {
                                self.advance();
                                flags.push(GetFlag::Mask(self.parse_flag_value()?));
                            }
                            TokenKind::Identifier(ref s) if s == "until" => {
                                self.advance();
                                flags.push(GetFlag::Until(self.parse_flag_value()?));
                            }
                            TokenKind::Bytes => {
                                self.advance();
                                flags.push(GetFlag::Bytes(self.parse_flag_value()?));
                            }
                            TokenKind::As => {
                                self.advance();
                                flags.push(GetFlag::As(self.parse_type()?));
                            }
                            TokenKind::Identifier(ref s) if s == "bytes" => {
                                self.advance();
                                flags.push(GetFlag::Bytes(self.parse_flag_value()?));
                            }
                            _ => break,
                        }
                    } else {
                        return Err(ParseError::new(
                            "Unknown `get` flag; use `get unicode \"Prompt:\"` for prompts or a documented `--` option",
                            self.current().span,
                        ));
                    }
                }
                TokenKind::With => {
                    self.advance();
                    match self.peek() {
                        TokenKind::Validate => {
                            self.advance();
                            // Check if next is a closure
                            if *self.peek() == TokenKind::Pipe {
                                let closure = self.parse_closure()?;
                                with_clause = Some(WithClause::Validate(closure));
                            } else {
                                with_clause = Some(WithClause::Validate(self.parse_expression()?));
                            }
                        }
                        TokenKind::Complete => {
                            self.advance();
                            with_clause = Some(WithClause::Complete(self.parse_expression()?));
                        }
                        TokenKind::Encoding => {
                            self.advance();
                            with_clause = Some(WithClause::Encoding(self.parse_expression()?));
                        }
                        _ => break,
                    }
                }
                _ => break,
            }
        }

        Ok(Expression::GetExpression(Box::new(GetExpr {
            prompt: prompt.map(Box::new),
            source,
            flags,
            with_clause,
        })))
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        // Blocks stop before terminators so the caller can consume the exact
        // closing keyword (`end if`, `end fn`, and so on).  Every nested
        // statement gets its own span, exactly like top-level statements.
        let mut stmts = Block::new();
        while !self.is_at_end()
            && self.peek() != &TokenKind::End
            && self.peek() != &TokenKind::RBrace
            && self.peek() != &TokenKind::Else
        {
            let start = self.pos;
            let statement = self.parse_statement()?;
            stmts.push(Spanned::new(statement, self.statement_span(start)));
        }
        Ok(stmts)
    }
}

fn expression_description(expr: &Expression) -> String {
    match expr {
        Expression::Identifier(name) => name.clone(),
        Expression::FieldAccess { object, field } => {
            format!("{}.{}", expression_description(object), field)
        }
        Expression::Index { object, index } => {
            format!(
                "{}[{}]",
                expression_description(object),
                expression_description(index)
            )
        }
        _ => "target".to_string(),
    }
}

/// Map a keyword token to its source spelling when the keyword is also a
/// plausible method/field name (used after `.`, e.g. `map.get(...)`).
fn keyword_as_field_name(token: &TokenKind) -> Option<&'static str> {
    use TokenKind::*;
    match token {
        Put => Some("put"),
        Get => Some("get"),
        Error => Some("error"),
        Warn => Some("warn"),
        Info => Some("info"),
        Try => Some("try"),
        Move => Some("move"),
        Set => Some("set"),
        Step => Some("step"),
        Type => Some("type"),
        To => Some("to"),
        With => Some("with"),
        Validate => Some("validate"),
        Complete => Some("complete"),
        Encoding => Some("encoding"),
        Timeout => Some("timeout"),
        Default => Some("default"),
        Mask => Some("mask"),
        Bytes => Some("bytes"),
        _ => None,
    }
}

fn expressions_match_target(left: &Expression, right: &Expression) -> bool {
    match (left, right) {
        (Expression::Identifier(left), Expression::Identifier(right)) => left == right,
        (
            Expression::FieldAccess {
                object: left_object,
                field: left_field,
            },
            Expression::FieldAccess {
                object: right_object,
                field: right_field,
            },
        ) => left_field == right_field && expressions_match_target(left_object, right_object),
        (
            Expression::Index {
                object: left_object,
                index: left_index,
            },
            Expression::Index {
                object: right_object,
                index: right_index,
            },
        ) => {
            expressions_match_target(left_object, right_object)
                && expressions_match_target(left_index, right_index)
        }
        (Expression::Parenthesized(left), right) => expressions_match_target(left, right),
        (left, Expression::Parenthesized(right)) => expressions_match_target(left, right),
        (Expression::IntLiteral(left), Expression::IntLiteral(right))
        | (Expression::FloatLiteral(left), Expression::FloatLiteral(right))
        | (Expression::StringLiteral(left), Expression::StringLiteral(right))
        | (Expression::UnicodeStringLiteral(left), Expression::UnicodeStringLiteral(right)) => {
            left == right
        }
        (Expression::UnicodeCharLiteral(left), Expression::UnicodeCharLiteral(right)) => {
            left == right
        }
        (Expression::BoolLiteral(left), Expression::BoolLiteral(right)) => left == right,
        _ => false,
    }
}

fn expression_contains_target(expr: &Expression, target: &Expression) -> bool {
    if expressions_match_target(expr, target) {
        return true;
    }

    match expr {
        Expression::BinaryOp { left, right, .. } => {
            expression_contains_target(left, target) || expression_contains_target(right, target)
        }
        Expression::UnaryOp { expr, .. }
        | Expression::Parenthesized(expr)
        | Expression::TryExpression(expr)
        | Expression::AsExpression { expr, .. } => expression_contains_target(expr, target),
        Expression::Call { func, args } => {
            expression_contains_target(func, target)
                || args
                    .iter()
                    .any(|arg| expression_contains_target(arg, target))
        }
        Expression::MethodCall { object, args, .. } => {
            expression_contains_target(object, target)
                || args
                    .iter()
                    .any(|arg| expression_contains_target(arg, target))
        }
        Expression::Index { object, index } => {
            expression_contains_target(object, target) || expression_contains_target(index, target)
        }
        Expression::FieldAccess { object, .. } => expression_contains_target(object, target),
        Expression::IfExpression {
            condition,
            then_block,
            else_block,
        } => {
            expression_contains_target(condition, target)
                || then_block
                    .iter()
                    .any(|stmt| statement_contains_target(&stmt.node, target))
                || else_block.as_ref().is_some_and(|block| {
                    block
                        .iter()
                        .any(|stmt| statement_contains_target(&stmt.node, target))
                })
        }
        Expression::ArrayLiteral(elements) | Expression::TupleLiteral(elements) => elements
            .iter()
            .any(|element| expression_contains_target(element, target)),
        Expression::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expression_contains_target(value, target)),
        Expression::EnumVariant { data, .. } => data.as_ref().is_some_and(|values| {
            values
                .iter()
                .any(|value| expression_contains_target(value, target))
        }),
        Expression::Range { start, end, .. } => {
            expression_contains_target(start, target) || expression_contains_target(end, target)
        }
        Expression::LoopRange { ranges, body, .. } => {
            ranges.iter().any(|range| match range {
                LoopRangePart::Range {
                    start, end, step, ..
                } => {
                    expression_contains_target(start, target)
                        || expression_contains_target(end, target)
                        || step
                            .as_ref()
                            .is_some_and(|value| expression_contains_target(value, target))
                }
                LoopRangePart::Value(value) => expression_contains_target(value, target),
            }) || body
                .iter()
                .any(|stmt| statement_contains_target(&stmt.node, target))
        }
        Expression::ForLoop { iterable, body, .. } => {
            expression_contains_target(iterable, target)
                || body
                    .iter()
                    .any(|stmt| statement_contains_target(&stmt.node, target))
        }
        Expression::InfiniteLoop(body) => body
            .iter()
            .any(|stmt| statement_contains_target(&stmt.node, target)),
        Expression::MatchExpression { scrutinee, arms } => {
            expression_contains_target(scrutinee, target)
                || arms.iter().any(|arm| match &arm.body {
                    MatchArmBody::Expression(value) => expression_contains_target(value, target),
                    MatchArmBody::Block(block) => block
                        .iter()
                        .any(|stmt| statement_contains_target(&stmt.node, target)),
                })
        }
        Expression::Closure { body, .. } => expression_contains_target(body, target),
        Expression::GetExpression(get) => {
            get.prompt
                .as_ref()
                .is_some_and(|value| expression_contains_target(value, target))
                || get
                    .source
                    .as_ref()
                    .is_some_and(|value| expression_contains_target(value, target))
                || get.flags.iter().any(|flag| match flag {
                    GetFlag::Timeout(value)
                    | GetFlag::Default(value)
                    | GetFlag::Mask(value)
                    | GetFlag::Until(value)
                    | GetFlag::Bytes(value) => expression_contains_target(value, target),
                    GetFlag::As(_) => false,
                })
                || get.with_clause.as_ref().is_some_and(|clause| match clause {
                    WithClause::Validate(value)
                    | WithClause::Complete(value)
                    | WithClause::Encoding(value) => expression_contains_target(value, target),
                })
        }
        Expression::UnsafeBlock(block) => block
            .iter()
            .any(|stmt| statement_contains_target(&stmt.node, target)),
        Expression::Identifier(_)
        | Expression::IntLiteral(_)
        | Expression::FloatLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::UnicodeStringLiteral(_)
        | Expression::UnicodeCharLiteral(_)
        | Expression::BoolLiteral(_)
        | Expression::ByteLiteral(_) => false,
    }
}

fn statement_contains_target(stmt: &Statement, target: &Expression) -> bool {
    match stmt {
        Statement::Set {
            target: value,
            value: expr,
        } => expression_contains_target(value, target) || expression_contains_target(expr, target),
        Statement::Mutation {
            target: value,
            value: expr,
            ..
        } => {
            expression_contains_target(value, target)
                || expr
                    .as_ref()
                    .is_some_and(|value| expression_contains_target(value, target))
        }
        Statement::ExpressionStatement(expr) => expression_contains_target(expr, target),
        Statement::ReturnStatement(value) => value
            .as_ref()
            .is_some_and(|value| expression_contains_target(value, target)),
        Statement::PutStatement { expr, redirect, .. } => {
            expression_contains_target(expr, target)
                || redirect.as_ref().is_some_and(|redirect| match redirect {
                    Redirect::Write(value) | Redirect::Append(value) => {
                        expression_contains_target(value, target)
                    }
                })
        }
        _ => false,
    }
}

// === Macro substitution ===

// Expanding a macro replaces every identifier in the captured body that names
// a macro parameter with the corresponding argument expression. The walk is
// exhaustive over the AST so substituted parameters inside nested blocks,
// match arms, and closures are rewritten consistently.

/// Substitute inside a spanned statement, preserving the original span.
fn substitute_spanned_statement(
    statement: &Spanned<Statement>,
    substitutions: &HashMap<String, Expression>,
) -> Spanned<Statement> {
    Spanned::new(
        substitute_statement(&statement.node, substitutions),
        statement.span,
    )
}

fn substitute_statement(
    statement: &Statement,
    substitutions: &HashMap<String, Expression>,
) -> Statement {
    match statement {
        Statement::VarDeclaration { name, ty, value } => Statement::VarDeclaration {
            name: name.clone(),
            ty: ty.clone(),
            value: value
                .as_ref()
                .map(|value| substitute_expression(value, substitutions)),
        },
        Statement::LetDeclaration { name, ty, value } => Statement::LetDeclaration {
            name: name.clone(),
            ty: ty.clone(),
            value: substitute_expression(value, substitutions),
        },
        Statement::ConstDeclaration { name, value } => Statement::ConstDeclaration {
            name: name.clone(),
            value: substitute_expression(value, substitutions),
        },
        Statement::Set { target, value } => Statement::Set {
            target: substitute_expression(target, substitutions),
            value: substitute_expression(value, substitutions),
        },
        Statement::Mutation { target, op, value } => Statement::Mutation {
            target: substitute_expression(target, substitutions),
            op: op.clone(),
            value: value
                .as_ref()
                .map(|value| substitute_expression(value, substitutions)),
        },
        Statement::FunctionDeclaration(declaration) => {
            Statement::FunctionDeclaration(FunctionDecl {
                name: declaration.name.clone(),
                params: declaration.params.clone(),
                return_type: declaration.return_type.clone(),
                body: declaration.body.as_ref().map(|body| {
                    body.iter()
                        .map(|statement| substitute_spanned_statement(statement, substitutions))
                        .collect()
                }),
                is_async: declaration.is_async,
                generics: declaration.generics.clone(),
            })
        }
        Statement::StructDeclaration(declaration) => Statement::StructDeclaration(StructDecl {
            name: declaration.name.clone(),
            fields: declaration.fields.clone(),
            methods: declaration
                .methods
                .iter()
                .map(|method| substitute_function(method, substitutions))
                .collect(),
            generics: declaration.generics.clone(),
        }),
        Statement::EnumDeclaration(declaration) => Statement::EnumDeclaration(EnumDecl {
            name: declaration.name.clone(),
            variants: declaration.variants.clone(),
            methods: declaration
                .methods
                .iter()
                .map(|method| substitute_function(method, substitutions))
                .collect(),
        }),
        Statement::TraitDeclaration(declaration) => Statement::TraitDeclaration(TraitDecl {
            name: declaration.name.clone(),
            methods: declaration
                .methods
                .iter()
                .map(|method| substitute_function(method, substitutions))
                .collect(),
        }),
        Statement::ImplDeclaration(declaration) => Statement::ImplDeclaration(ImplDecl {
            trait_name: declaration.trait_name.clone(),
            type_name: declaration.type_name.clone(),
            methods: declaration
                .methods
                .iter()
                .map(|method| substitute_function(method, substitutions))
                .collect(),
        }),
        Statement::ModuleDeclaration(declaration) => Statement::ModuleDeclaration(ModuleDecl {
            name: declaration.name.clone(),
            statements: declaration
                .statements
                .iter()
                .map(|statement| substitute_spanned_statement(statement, substitutions))
                .collect(),
        }),
        Statement::UseDeclaration(declaration) => Statement::UseDeclaration(declaration.clone()),
        Statement::TypeDeclaration(declaration) => Statement::TypeDeclaration(declaration.clone()),
        Statement::ExpressionStatement(expression) => {
            Statement::ExpressionStatement(substitute_expression(expression, substitutions))
        }
        Statement::ReturnStatement(value) => Statement::ReturnStatement(
            value
                .as_ref()
                .map(|value| substitute_expression(value, substitutions)),
        ),
        Statement::BreakStatement => Statement::BreakStatement,
        Statement::ContinueStatement => Statement::ContinueStatement,
        Statement::PutStatement {
            no_newline,
            expr,
            redirect,
        } => Statement::PutStatement {
            no_newline: *no_newline,
            expr: substitute_expression(expr, substitutions),
            redirect: redirect.as_ref().map(|redirect| match redirect {
                Redirect::Write(path) => {
                    Redirect::Write(substitute_expression(path, substitutions))
                }
                Redirect::Append(path) => {
                    Redirect::Append(substitute_expression(path, substitutions))
                }
            }),
        },
        Statement::ErrorStatement(expression) => {
            Statement::ErrorStatement(substitute_expression(expression, substitutions))
        }
        Statement::WarnStatement(expression) => {
            Statement::WarnStatement(substitute_expression(expression, substitutions))
        }
        Statement::InfoStatement(expression) => {
            Statement::InfoStatement(substitute_expression(expression, substitutions))
        }
        Statement::Block(statements) => Statement::Block(
            statements
                .iter()
                .map(|statement| substitute_spanned_statement(statement, substitutions))
                .collect(),
        ),
    }
}

fn substitute_function(
    function: &FunctionDecl,
    substitutions: &HashMap<String, Expression>,
) -> FunctionDecl {
    FunctionDecl {
        name: function.name.clone(),
        params: function.params.clone(),
        return_type: function.return_type.clone(),
        body: function.body.as_ref().map(|body| {
            body.iter()
                .map(|statement| substitute_spanned_statement(statement, substitutions))
                .collect()
        }),
        is_async: function.is_async,
        generics: function.generics.clone(),
    }
}

fn substitute_pattern(pattern: &Pattern, substitutions: &HashMap<String, Expression>) -> Pattern {
    match pattern {
        Pattern::Wildcard => Pattern::Wildcard,
        Pattern::Literal(expression) => {
            Pattern::Literal(substitute_expression(expression, substitutions))
        }
        Pattern::Identifier(name) => {
            if let Some(expression) = substitutions.get(name) {
                Pattern::Literal(expression.clone())
            } else {
                Pattern::Identifier(name.clone())
            }
        }
        Pattern::Tuple(patterns) => Pattern::Tuple(
            patterns
                .iter()
                .map(|pattern| substitute_pattern(pattern, substitutions))
                .collect(),
        ),
        Pattern::Enum {
            enum_name,
            variant,
            inner,
        } => Pattern::Enum {
            enum_name: enum_name.clone(),
            variant: variant.clone(),
            inner: inner.as_ref().map(|inner| {
                inner
                    .iter()
                    .map(|pattern| substitute_pattern(pattern, substitutions))
                    .collect()
            }),
        },
        Pattern::NamedFields { name, fields } => Pattern::NamedFields {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|(field, pattern)| (field.clone(), substitute_pattern(pattern, substitutions)))
                .collect(),
        },
        Pattern::Range {
            start,
            end,
            inclusive,
        } => Pattern::Range {
            start: Box::new(substitute_expression(start, substitutions)),
            end: Box::new(substitute_expression(end, substitutions)),
            inclusive: *inclusive,
        },
        Pattern::Binding { name, pattern } => Pattern::Binding {
            name: name.clone(),
            pattern: Box::new(substitute_pattern(pattern, substitutions)),
        },
    }
}

fn substitute_expression(
    expression: &Expression,
    substitutions: &HashMap<String, Expression>,
) -> Expression {
    match expression {
        Expression::IntLiteral(value) => Expression::IntLiteral(value.clone()),
        Expression::FloatLiteral(value) => Expression::FloatLiteral(value.clone()),
        Expression::StringLiteral(value) => Expression::StringLiteral(value.clone()),
        Expression::UnicodeStringLiteral(value) => Expression::UnicodeStringLiteral(value.clone()),
        Expression::UnicodeCharLiteral(value) => Expression::UnicodeCharLiteral(*value),
        Expression::BoolLiteral(value) => Expression::BoolLiteral(*value),
        Expression::ByteLiteral(value) => Expression::ByteLiteral(value.clone()),
        Expression::Identifier(name) => substitutions
            .get(name)
            .cloned()
            .unwrap_or_else(|| Expression::Identifier(name.clone())),
        Expression::BinaryOp { op, left, right } => Expression::BinaryOp {
            op: op.clone(),
            left: Box::new(substitute_expression(left, substitutions)),
            right: Box::new(substitute_expression(right, substitutions)),
        },
        Expression::UnaryOp { op, expr } => Expression::UnaryOp {
            op: op.clone(),
            expr: Box::new(substitute_expression(expr, substitutions)),
        },
        Expression::Call { func, args } => Expression::Call {
            func: Box::new(substitute_expression(func, substitutions)),
            args: args
                .iter()
                .map(|arg| substitute_expression(arg, substitutions))
                .collect(),
        },
        Expression::MethodCall {
            object,
            method,
            args,
        } => Expression::MethodCall {
            object: Box::new(substitute_expression(object, substitutions)),
            method: method.clone(),
            args: args
                .iter()
                .map(|arg| substitute_expression(arg, substitutions))
                .collect(),
        },
        Expression::Index { object, index } => Expression::Index {
            object: Box::new(substitute_expression(object, substitutions)),
            index: Box::new(substitute_expression(index, substitutions)),
        },
        Expression::FieldAccess { object, field } => Expression::FieldAccess {
            object: Box::new(substitute_expression(object, substitutions)),
            field: field.clone(),
        },
        Expression::Parenthesized(inner) => {
            Expression::Parenthesized(Box::new(substitute_expression(inner, substitutions)))
        }
        Expression::IfExpression {
            condition,
            then_block,
            else_block,
        } => Expression::IfExpression {
            condition: Box::new(substitute_expression(condition, substitutions)),
            then_block: then_block
                .iter()
                .map(|statement| substitute_spanned_statement(statement, substitutions))
                .collect(),
            else_block: else_block.as_ref().map(|block| {
                block
                    .iter()
                    .map(|statement| substitute_spanned_statement(statement, substitutions))
                    .collect()
            }),
        },
        Expression::MatchExpression { scrutinee, arms } => Expression::MatchExpression {
            scrutinee: Box::new(substitute_expression(scrutinee, substitutions)),
            arms: arms
                .iter()
                .map(|arm| MatchArm {
                    pattern: substitute_pattern(&arm.pattern, substitutions),
                    guard: arm
                        .guard
                        .as_ref()
                        .map(|guard| substitute_expression(guard, substitutions)),
                    body: match &arm.body {
                        MatchArmBody::Expression(body) => {
                            MatchArmBody::Expression(substitute_expression(body, substitutions))
                        }
                        MatchArmBody::Block(statements) => MatchArmBody::Block(
                            statements
                                .iter()
                                .map(|statement| {
                                    substitute_spanned_statement(statement, substitutions)
                                })
                                .collect(),
                        ),
                    },
                })
                .collect(),
        },
        Expression::Closure { params, body } => Expression::Closure {
            params: params.clone(),
            body: Box::new(substitute_expression(body, substitutions)),
        },
        Expression::ArrayLiteral(elements) => Expression::ArrayLiteral(
            elements
                .iter()
                .map(|element| substitute_expression(element, substitutions))
                .collect(),
        ),
        Expression::TupleLiteral(elements) => Expression::TupleLiteral(
            elements
                .iter()
                .map(|element| substitute_expression(element, substitutions))
                .collect(),
        ),
        Expression::StructLiteral { name, fields } => Expression::StructLiteral {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|(field, value)| (field.clone(), substitute_expression(value, substitutions)))
                .collect(),
        },
        Expression::EnumVariant {
            enum_name,
            variant,
            data,
        } => Expression::EnumVariant {
            enum_name: enum_name.clone(),
            variant: variant.clone(),
            data: data.as_ref().map(|values| {
                values
                    .iter()
                    .map(|value| substitute_expression(value, substitutions))
                    .collect()
            }),
        },
        Expression::LoopRange {
            variable,
            ranges,
            body,
        } => Expression::LoopRange {
            variable: variable.clone(),
            ranges: ranges
                .iter()
                .map(|range| match range {
                    LoopRangePart::Range {
                        start,
                        end,
                        inclusive,
                        step,
                    } => LoopRangePart::Range {
                        start: Box::new(substitute_expression(start, substitutions)),
                        end: Box::new(substitute_expression(end, substitutions)),
                        inclusive: *inclusive,
                        step: step
                            .as_ref()
                            .map(|step| substitute_expression(step, substitutions)),
                    },
                    LoopRangePart::Value(value) => {
                        LoopRangePart::Value(Box::new(substitute_expression(value, substitutions)))
                    }
                })
                .collect(),
            body: body
                .iter()
                .map(|statement| substitute_spanned_statement(statement, substitutions))
                .collect(),
        },
        Expression::ForLoop {
            variable,
            iterable,
            body,
        } => Expression::ForLoop {
            variable: variable.clone(),
            iterable: Box::new(substitute_expression(iterable, substitutions)),
            body: body
                .iter()
                .map(|statement| substitute_spanned_statement(statement, substitutions))
                .collect(),
        },
        Expression::InfiniteLoop(body) => Expression::InfiniteLoop(
            body.iter()
                .map(|statement| substitute_spanned_statement(statement, substitutions))
                .collect(),
        ),
        Expression::AsExpression { expr, ty } => Expression::AsExpression {
            expr: Box::new(substitute_expression(expr, substitutions)),
            ty: ty.clone(),
        },
        Expression::TryExpression(expr) => {
            Expression::TryExpression(Box::new(substitute_expression(expr, substitutions)))
        }
        Expression::GetExpression(get) => Expression::GetExpression(Box::new(GetExpr {
            prompt: get
                .prompt
                .as_ref()
                .map(|prompt| Box::new(substitute_expression(prompt, substitutions))),
            source: get
                .source
                .as_ref()
                .map(|source| Box::new(substitute_expression(source, substitutions))),
            flags: get
                .flags
                .iter()
                .map(|flag| match flag {
                    GetFlag::Timeout(value) => {
                        GetFlag::Timeout(substitute_expression(value, substitutions))
                    }
                    GetFlag::Default(value) => {
                        GetFlag::Default(substitute_expression(value, substitutions))
                    }
                    GetFlag::Mask(value) => {
                        GetFlag::Mask(substitute_expression(value, substitutions))
                    }
                    GetFlag::Until(value) => {
                        GetFlag::Until(substitute_expression(value, substitutions))
                    }
                    GetFlag::Bytes(value) => {
                        GetFlag::Bytes(substitute_expression(value, substitutions))
                    }
                    GetFlag::As(ty) => GetFlag::As(ty.clone()),
                })
                .collect(),
            with_clause: get.with_clause.clone(),
        })),
        Expression::UnsafeBlock(statements) => Expression::UnsafeBlock(
            statements
                .iter()
                .map(|statement| substitute_spanned_statement(statement, substitutions))
                .collect(),
        ),
        Expression::Range {
            start,
            end,
            inclusive,
        } => Expression::Range {
            start: Box::new(substitute_expression(start, substitutions)),
            end: Box::new(substitute_expression(end, substitutions)),
            inclusive: *inclusive,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poly_lexer::Lexer;

    fn parse_source(source: &str) -> Result<Program, ParseError> {
        let (tokens, _errors) = Lexer::lex(source);
        let mut parser = Parser::new(&tokens);
        parser.parse()
    }

    #[test]
    fn statement_spans_cover_exact_source_ranges() {
        let source =
            "var x i32 := 42\nfn sum(a: i32, b: i32): i32\n    return a + b\nend fn\nput x";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty());
        let mut parser = Parser::new(&tokens);
        let (program, parse_errors) = parser.parse_with_recovery();
        assert!(parse_errors.is_empty());

        assert_eq!(program.statements.len(), 3);

        // Statement spans must slice back to the exact source text.
        let first = &program.statements[0];
        assert_eq!(&source[first.span.start..first.span.end], "var x i32 := 42");

        let function = &program.statements[1];
        assert_eq!(
            &source[function.span.start..function.span.end],
            "fn sum(a: i32, b: i32): i32\n    return a + b\nend fn"
        );

        let put = &program.statements[2];
        assert_eq!(&source[put.span.start..put.span.end], "put x");
    }

    #[test]
    fn test_parse_generic_type_annotations() {
        let prog =
            parse_source("var scores Map<ustring, i32> := []\nvar seen Set<i32> := []").unwrap();
        assert_eq!(prog.statements.len(), 2);
        match &prog.statements[0].node {
            Statement::VarDeclaration { ty: Some(ty), .. } => match ty {
                TypeAnnotation::Generic { name, args } => {
                    assert_eq!(name, "Map");
                    assert_eq!(args.len(), 2);
                }
                other => panic!("expected generic type, got {other:?}"),
            },
            other => panic!("expected var declaration, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_generic_function_and_struct() {
        let prog = parse_source(
            "fn identity<T>(value: T): T\n    return value\nend fn\n\
             struct Pair<T>\n    var first: T\n    var second: T\nend struct",
        )
        .unwrap();
        match &prog.statements[0].node {
            Statement::FunctionDeclaration(function) => {
                assert_eq!(function.generics.len(), 1);
                assert_eq!(function.generics[0].name, "T");
            }
            other => panic!("expected function declaration, got {other:?}"),
        }
        match &prog.statements[1].node {
            Statement::StructDeclaration(struct_decl) => {
                assert_eq!(struct_decl.generics.len(), 1);
                assert_eq!(struct_decl.generics[0].name, "T");
            }
            other => panic!("expected struct declaration, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_generic_function_with_trait_bound() {
        let prog =
            parse_source("fn process<T: DataFetcher>(fetcher: T)\n    put unicode \"ok\"\nend fn")
                .unwrap();
        match &prog.statements[0].node {
            Statement::FunctionDeclaration(function) => {
                assert_eq!(function.generics.len(), 1);
                assert_eq!(function.generics[0].name, "T");
                assert_eq!(function.generics[0].bounds, vec!["DataFetcher"]);
            }
            other => panic!("expected function declaration, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_macro_expansion() {
        let prog = parse_source(
            "macro swap_values(a, b)\n    var temp := a\n    set a to b\n    set b to temp\nend macro\n\
             var x i32 := 1\nvar y i32 := 2\nswap_values(x, y)",
        )
        .unwrap();
        // The macro declaration is consumed; only the variables and the
        // expanded call remain.
        assert_eq!(prog.statements.len(), 3);
        match &prog.statements[2].node {
            Statement::Block(statements) => {
                assert_eq!(statements.len(), 3);
                match &statements[0].node {
                    Statement::VarDeclaration { name, .. } => assert_eq!(name, "temp"),
                    other => panic!("expected var declaration, got {other:?}"),
                }
            }
            other => panic!("expected expanded block, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_macro_inside_expansion_binds_arguments() {
        let prog = parse_source(
            "macro double(value)\n    var result := value + value\n    put result\nend macro\n\
             double(21)",
        )
        .unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::Block(statements) => match &statements[0].node {
                Statement::VarDeclaration {
                    value: Some(value), ..
                } => {
                    assert!(matches!(
                        value,
                        Expression::BinaryOp {
                            op: BinaryOp::Add,
                            left,
                            right,
                        } if matches!(**left, Expression::IntLiteral(ref v) if v == "21")
                            && matches!(**right, Expression::IntLiteral(ref v) if v == "21")
                    ));
                }
                other => panic!("expected var declaration, got {other:?}"),
            },
            other => panic!("expected expanded block, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_var_declaration() {
        let prog = parse_source("var x i32 := 42").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::VarDeclaration { name, ty, value } => {
                assert_eq!(name, "x");
                assert!(ty.is_some());
                assert!(value.is_some());
            }
            _ => panic!("Expected VarDeclaration"),
        }
    }

    #[test]
    fn test_parse_strict_var_syntax_and_prompt_errors() {
        assert!(parse_source("var inferred := 42").is_ok());
        assert!(parse_source("var typed i32 := 42").is_ok());

        let colon_error = parse_source("var typed: i32 := 42").unwrap_err();
        assert!(colon_error.message.contains("no longer use"));
        assert!(colon_error
            .suggestion
            .unwrap()
            .contains("var typed type := value"));

        let equals_error = parse_source("var typed = 42").unwrap_err();
        assert!(equals_error.message.contains("must use `:=`"));

        let prompt_error = parse_source(r#"var name := get "Prompt: ""#).unwrap_err();
        assert!(prompt_error.message.contains("get unicode"));
        assert!(parse_source(r#"var name := get unicode "Prompt: ""#).is_ok());
        assert!(parse_source(r#"var name := get -u "Prompt: ""#).is_err());
        assert!(parse_source(r#"var greeting := unicode "Hello, 世界!""#).is_ok());
        assert!(parse_source(r#"var marker := unicode '✓'"#).is_ok());
        assert!(parse_source(r#"put unicode "Hello, 世界!""#).is_ok());
        assert!(parse_source(r#"var file := get < "input.txt" unicode "Prompt: ""#).is_err());
        assert!(parse_source(r#"var legacy_char := u'✓'"#).is_err());
        assert!(parse_source(
            r#"match unicode "yes"
    unicode "yes", put "ok"
end match"#
        )
        .is_ok());

        let legacy_source = format!("var greeting := {}{}{}{}", "u", '"', "Hello", '"');
        let legacy_unicode_error = parse_source(&legacy_source).unwrap_err();
        assert!(legacy_unicode_error.message.contains("no longer accepted"));
        assert!(legacy_unicode_error
            .suggestion
            .unwrap()
            .contains("unicode \"...\""));
    }

    #[test]
    fn test_parse_function() {
        let prog = parse_source("fn sum(a: i32, b: i32): i32").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::FunctionDeclaration(decl) => {
                assert_eq!(decl.name, "sum");
                assert_eq!(decl.params.len(), 2);
                assert!(decl.return_type.is_some());
            }
            _ => panic!("Expected FunctionDeclaration"),
        }
    }

    #[test]
    fn test_parse_equality_and_assignment() {
        let equality = parse_source("var same := x == 1").unwrap();
        assert!(matches!(
            equality.statements.first().map(|s| &s.node),
            Some(Statement::VarDeclaration {
                value: Some(Expression::BinaryOp {
                    op: BinaryOp::Eq,
                    ..
                }),
                ..
            })
        ));
        assert!(parse_source("var x := 0\nx = x + 1").is_ok());
        assert!(parse_source("var x := 0\nx += 1").is_ok());
        assert!(parse_source("var x := 0\nset x to x + 1").is_err());
        assert!(parse_source("var x := 0\nadd x").is_ok());
        assert!(parse_source("var x := 0\nset x to 1").is_ok());
    }

    #[test]
    fn test_parse_set_target_shapes_and_self_reference() {
        assert!(parse_source("set total to 10").is_ok());
        assert!(parse_source("set item.value to 10").is_ok());
        assert!(parse_source("set items[0] to 10").is_ok());

        assert!(parse_source("set item.value to item.value + 1").is_err());
        assert!(parse_source("set items[0] to items[0] + 1").is_err());
        assert!(parse_source("set item.value to other.value").is_ok());
    }

    #[test]
    fn test_parse_accepts_conventional_equality_and_compound_mutation() {
        assert!(parse_source("var x := a == b").is_ok());
        assert!(parse_source("var x := 0\nx += 1").is_ok());
        assert!(parse_source("var x := 0\nx -= 1").is_ok());
    }

    #[test]
    fn test_parse_binary_expression() {
        let prog = parse_source("var x := a + b * 2").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::VarDeclaration {
                value: Some(expr), ..
            } => {
                // Should be a + (b * 2) due to precedence
                match expr {
                    Expression::BinaryOp {
                        op: BinaryOp::Add, ..
                    } => {}
                    _ => panic!("Expected Add operation"),
                }
            }
            _ => panic!("Expected VarDeclaration"),
        }
    }

    #[test]
    fn test_parse_put_statement() {
        let prog = parse_source(r#"put "Hello""#).unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::PutStatement {
                no_newline, expr, ..
            } => {
                assert!(!no_newline);
                match expr {
                    Expression::StringLiteral(s) => assert_eq!(s, "Hello"),
                    _ => panic!("Expected string literal"),
                }
            }
            _ => panic!("Expected PutStatement"),
        }
    }

    #[test]
    fn test_parse_unicode_string_and_character_literals() {
        let program =
            parse_source("var greeting := unicode \"Hello, 世界\"\nvar marker := unicode '✓'")
                .unwrap();
        assert_eq!(program.statements.len(), 2);
        match &program.statements[0].node {
            Statement::VarDeclaration {
                value: Some(Expression::UnicodeStringLiteral(value)),
                ..
            } => {
                assert_eq!(value, "Hello, 世界");
            }
            _ => panic!("Expected Unicode string literal"),
        }
        match &program.statements[1].node {
            Statement::VarDeclaration {
                value: Some(Expression::UnicodeCharLiteral(value)),
                ..
            } => {
                assert_eq!(*value, '✓');
            }
            _ => panic!("Expected Unicode character literal"),
        }
    }

    #[test]
    fn test_parse_put_no_newline() {
        let prog = parse_source(r#"put -n "Hello""#).unwrap();
        match &prog.statements[0].node {
            Statement::PutStatement { no_newline, .. } => {
                assert!(*no_newline);
            }
            _ => panic!("Expected PutStatement"),
        }
    }

    #[test]
    fn test_reject_legacy_unicode_string_literal() {
        let error = parse_source(r#"var greeting := u"Hello""#).unwrap_err();
        assert!(error.message.contains("no longer accepted"));
        assert!(error
            .suggestion
            .as_deref()
            .is_some_and(|suggestion| suggestion.contains("unicode")));
    }

    #[test]
    fn test_parse_struct() {
        let prog =
            parse_source("struct Point\n    var x: f32\n    var y: f32\nend struct").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::StructDeclaration(decl) => {
                assert_eq!(decl.name, "Point");
                assert_eq!(decl.fields.len(), 2);
            }
            _ => panic!("Expected StructDeclaration"),
        }
    }

    #[test]
    fn test_parse_enum() {
        let prog = parse_source("enum Direction\n    North\n    South\nend enum").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::EnumDeclaration(decl) => {
                assert_eq!(decl.name, "Direction");
                assert_eq!(decl.variants.len(), 2);
            }
            _ => panic!("Expected EnumDeclaration"),
        }
    }

    #[test]
    fn test_parse_if_expression() {
        let prog = parse_source("if x > 0,\n    put x\nend if").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::IfExpression { .. }) => {}
            _ => panic!("Expected IfExpression"),
        }
    }

    #[test]
    fn test_parse_closure() {
        let prog = parse_source("var square := |x: i32| x * x").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::VarDeclaration {
                value: Some(Expression::Closure { params, .. }),
                ..
            } => {
                assert_eq!(params.len(), 1);
            }
            _ => panic!("Expected closure"),
        }
    }

    #[test]
    fn test_parse_closure_type_annotation() {
        // Closure types may appear in parameter positions: `|x: i32| i32`.
        let prog = parse_source("fn apply(f: |x: i32| i32, v: i32): i32\n    return f(v)\nend fn")
            .unwrap();
        match &prog.statements[0].node {
            Statement::FunctionDeclaration(function) => {
                assert_eq!(function.params.len(), 2);
                match &function.params[0].ty {
                    TypeAnnotation::Function { params, ret } => {
                        assert_eq!(params.len(), 1);
                        assert!(matches!(params[0], TypeAnnotation::Named(ref n) if n == "i32"));
                        assert!(matches!(**ret, TypeAnnotation::Named(ref n) if n == "i32"));
                    }
                    other => panic!("Expected function type, got {other:?}"),
                }
            }
            _ => panic!("Expected function declaration"),
        }
    }

    #[test]
    fn test_parse_bare_closure_type_annotation() {
        // Parameter names are optional inside closure types: `|i32| i32`.
        let prog =
            parse_source("fn twice(f: |i32| i32, v: i32): i32\n    return f(v)\nend fn").unwrap();
        match &prog.statements[0].node {
            Statement::FunctionDeclaration(function) => match &function.params[0].ty {
                TypeAnnotation::Function { params, ret } => {
                    assert_eq!(params.len(), 1);
                    assert!(matches!(params[0], TypeAnnotation::Named(ref n) if n == "i32"));
                    assert!(matches!(**ret, TypeAnnotation::Named(ref n) if n == "i32"));
                }
                other => panic!("Expected function type, got {other:?}"),
            },
            _ => panic!("Expected function declaration"),
        }
    }

    #[test]
    fn test_parse_range() {
        let prog = parse_source("var r := 0..10").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::VarDeclaration {
                value: Some(Expression::Range { inclusive, .. }),
                ..
            } => {
                assert!(!inclusive);
            }
            _ => panic!("Expected range"),
        }
    }

    #[test]
    fn test_parse_loop_range() {
        let prog = parse_source("loop: i 0..10\n    put i\nend loop").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::LoopRange {
                variable,
                ranges,
                body,
            }) => {
                assert_eq!(variable, "i");
                assert_eq!(ranges.len(), 1);
                assert_eq!(body.len(), 1);
            }
            _ => panic!("Expected LoopRange"),
        }
    }

    #[test]
    fn test_parse_multi_range_loop() {
        let prog = parse_source("loop: value 1..3, 7, 19..21\n    put value\nend loop").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::LoopRange {
                variable, ranges, ..
            }) => {
                assert_eq!(variable, "value");
                assert_eq!(ranges.len(), 3);
            }
            _ => panic!("Expected LoopRange with 3 parts"),
        }
    }

    #[test]
    fn test_parse_step_range() {
        let prog = parse_source("loop: n 0..10 step 2\n    put n\nend loop").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::LoopRange { ranges, .. }) => {
                assert_eq!(ranges.len(), 1);
                assert!(matches!(
                    ranges[0],
                    LoopRangePart::Range {
                        inclusive: true,
                        ..
                    }
                ));
                match &ranges[0] {
                    LoopRangePart::Range { step, .. } => {
                        assert!(step.is_some());
                    }
                    _ => panic!("Expected range with step"),
                }
            }
            _ => panic!("Expected LoopRange"),
        }
    }

    #[test]
    fn test_parse_loop_range_requires_explicit_variable() {
        assert!(parse_source("loop: 0..10\n    put i\nend loop").is_err());
    }

    #[test]
    fn test_parse_infinite_loop_keeps_body() {
        let prog = parse_source("loop\n    put 1\n    break\nend loop").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::InfiniteLoop(body)) => {
                assert_eq!(body.len(), 2);
            }
            _ => panic!("Expected InfiniteLoop with body"),
        }
    }

    #[test]
    fn test_parse_multiple_get_flags() {
        let prog =
            parse_source("var x ustring := get --timeout 5000 --default unicode \"0\"").unwrap();
        match &prog.statements[0].node {
            Statement::VarDeclaration { value, .. } => match value {
                Some(Expression::GetExpression(get)) => {
                    assert_eq!(get.flags.len(), 2);
                }
                _ => panic!("Expected GetExpression"),
            },
            _ => panic!("Expected VarDeclaration"),
        }
    }

    #[test]
    fn test_parse_tuple_destructuring_loop() {
        let prog = parse_source(
            "loop: (index, fruit) in fruits.enumerate()\n    put index\n    put fruit\nend loop",
        )
        .unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::LoopRange {
                variable,
                ranges,
                body,
            }) => {
                assert_eq!(variable, "(index, fruit)");
                assert_eq!(ranges.len(), 1);
                assert!(matches!(ranges[0], LoopRangePart::Value(_)));
                assert_eq!(body.len(), 2);
            }
            _ => panic!("Expected LoopRange with tuple binding"),
        }
    }

    #[test]
    fn test_parse_match_with_block_body() {
        let prog = parse_source(
            "match x\n    1,\n        put \"one\"\n        x + 1\n    _, 0\nend match",
        )
        .unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::MatchExpression { arms, .. }) => {
                assert_eq!(arms.len(), 2);
                // First arm should have a block body
                match &arms[0].body {
                    MatchArmBody::Block(stmts) => {
                        assert_eq!(stmts.len(), 2);
                    }
                    _ => panic!("Expected block body"),
                }
            }
            _ => panic!("Expected MatchExpression"),
        }
    }

    #[test]
    fn test_parse_variant_pattern_without_colons() {
        let prog =
            parse_source("match x\n    Ok(content), put content\n    Error(e), error e\nend match")
                .unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::MatchExpression { arms, .. }) => {
                assert_eq!(arms.len(), 2);
            }
            _ => panic!("Expected MatchExpression"),
        }
    }

    #[test]
    fn test_parse_match_rejects_fat_arrow() {
        assert!(parse_source("match x\n    1 => put \"one\"\nend match").is_err());
    }

    #[test]
    fn test_parse_match_with_comma_separator_and_guard() {
        let prog = parse_source("match x\n    n if n > 0, put n\n    _, put 0\nend match").unwrap();
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::MatchExpression { arms, .. }) => {
                assert_eq!(arms.len(), 2);
                assert!(arms[0].guard.is_some());
            }
            _ => panic!("Expected MatchExpression"),
        }
    }

    #[test]
    fn test_parse_named_enum_variant() {
        let prog = parse_source("enum Error\n    NotFound(message: ustring)\nend enum").unwrap();
        match &prog.statements[0].node {
            Statement::EnumDeclaration(decl) => {
                assert_eq!(decl.variants.len(), 1);
                match &decl.variants[0] {
                    EnumVariant::Struct(name, fields) => {
                        assert_eq!(name, "NotFound");
                        assert_eq!(fields.len(), 1);
                    }
                    _ => panic!("Expected struct variant"),
                }
            }
            _ => panic!("Expected EnumDeclaration"),
        }
    }

    #[test]
    fn test_parse_if_else_if() {
        let prog = parse_source("if x > 0,\n    put x\nelse if x < 0,\n    put \"negative\"\nelse,\n    put \"zero\"\nend if").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::IfExpression {
                then_block,
                else_block,
                ..
            }) => {
                assert_eq!(then_block.len(), 1);
                assert!(else_block.is_some());
                let else_block = else_block.as_ref().unwrap();
                assert_eq!(else_block.len(), 1); // else if is wrapped as single expression
            }
            _ => panic!("Expected IfExpression"),
        }
    }

    #[test]
    fn test_parse_inline_if() {
        let prog = parse_source("var y := if x > 0, x else -x end if").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::VarDeclaration {
                value: Some(Expression::IfExpression { .. }),
                ..
            } => {}
            _ => panic!("Expected VarDeclaration with IfExpression"),
        }
    }

    #[test]
    fn test_parse_nested_if() {
        let prog = parse_source(
            "if x > 0,\n    if y > 0,\n        put \"both positive\"\n    end if\nend if",
        )
        .unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::IfExpression { then_block, .. }) => {
                // The inner if should be in the then_block
                assert_eq!(then_block.len(), 1);
            }
            _ => panic!("Expected IfExpression"),
        }
    }

    #[test]
    fn test_parse_if_without_else() {
        let prog = parse_source("if x > 0\n    put x\nend if").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::IfExpression {
                then_block,
                else_block,
                ..
            }) => {
                assert_eq!(then_block.len(), 1);
                // else_block should be Some with empty vec when there's no else
                assert!(else_block.is_some());
                assert!(else_block.as_ref().unwrap().is_empty());
            }
            _ => panic!("Expected IfExpression"),
        }
    }

    #[test]
    fn test_parse_if_else_only() {
        let prog =
            parse_source("if x > 0,\n    put x\nelse,\n    put \"negative\"\nend if").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::IfExpression {
                then_block,
                else_block,
                ..
            }) => {
                assert_eq!(then_block.len(), 1);
                assert!(else_block.is_some());
                let else_block = else_block.as_ref().unwrap();
                assert_eq!(else_block.len(), 1); // else block has one statement
            }
            _ => panic!("Expected IfExpression"),
        }
    }

    #[test]
    fn test_parse_multiple_else_if() {
        let source = "if x > 10,\n    put \"high\"\nelse if x > 5,\n    put \"medium\"\nelse if x > 0,\n    put \"low\"\nelse,\n    put \"zero\"\nend if";
        let prog = parse_source(source).unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0].node {
            Statement::ExpressionStatement(Expression::IfExpression {
                then_block,
                else_block,
                ..
            }) => {
                assert_eq!(then_block.len(), 1);
                assert!(else_block.is_some());
                // The else block should contain a nested if-else-if chain
                let else_block = else_block.as_ref().unwrap();
                assert_eq!(else_block.len(), 1);
            }
            _ => panic!("Expected IfExpression"),
        }
    }
}

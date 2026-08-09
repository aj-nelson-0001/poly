//! Recursive descent parser for the Poly language.

use poly_lexer::token::{Token, TokenKind};

use crate::ast::*;
use crate::error::ParseError;

/// The Poly language parser.
pub struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    /// Create a new parser for the given token stream.
    pub fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: Vec::new(),
        }
    }

    /// Parse the token stream and return a Program AST.
    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
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
    pub fn parse_with_recovery(&mut self) -> (Program, Vec<ParseError>) {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(mut e) => {
                    e.generate_suggestion();
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        (Program { statements }, self.errors.clone())
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

    // === Statement parsing ===

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
            TokenKind::Add => {
                self.advance(); // consume 'add'
                let name = self.expect_identifier()?;
                // add x means x = x + 1
                Ok(Statement::Assignment {
                    target: Expression::Identifier(name),
                    op: AssignmentOp::PlusEq,
                    value: Expression::IntLiteral("1".to_string()),
                })
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
                // Try expression statement or assignment
                let expr = self.parse_expression()?;

                // Check for assignment
                if self.peek().is_assignment_op() {
                    let op = self.parse_assignment_op()?;
                    let value = self.parse_expression()?;
                    Ok(Statement::Assignment {
                        target: expr,
                        op,
                        value,
                    })
                } else {
                    Ok(Statement::ExpressionStatement(expr))
                }
            }
        }
    }

    fn parse_var_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'var'
        let name = self.expect_identifier()?;
        let ty = if self.match_token(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let value = if self.match_token(&TokenKind::Eq) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        Ok(Statement::VarDeclaration { name, ty, value })
    }

    fn parse_let_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'let'
        let name = self.expect_identifier()?;
        let ty = if self.match_token(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expression()?;
        Ok(Statement::LetDeclaration { name, ty, value })
    }

    fn parse_const_declaration(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'const'
        let name = self.expect_identifier()?;
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expression()?;
        Ok(Statement::ConstDeclaration { name, value })
    }

    fn parse_return_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'return'
        let value =
            if self.is_at_end() || matches!(self.peek(), TokenKind::Newline | TokenKind::Eof) {
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

    fn parse_put_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'put'
        let no_newline = self.match_token(&TokenKind::Minus);
        if no_newline {
            // Consume 'n' after -n
            self.expect_identifier()?;
        }
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

    fn parse_function_declaration(&mut self) -> Result<FunctionDecl, ParseError> {
        // Check for async modifier
        let is_async = self.match_token(&TokenKind::Async);
        self.advance(); // consume 'fn'
        let name = self.expect_identifier()?;
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
            // Function with no body (just signature)
            None
        } else {
            // Parse body until 'end fn'
            let stmts = self.parse_block()?;
            // Consume 'end fn'
            if self.peek() == &TokenKind::End {
                self.advance();
                self.expect(&TokenKind::Fn)?;
            }
            Some(stmts)
        };
        Ok(FunctionDecl {
            name,
            params,
            return_type,
            body,
            is_async,
        })
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
                let default = if self.match_token(&TokenKind::Eq) {
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
                let default = if self.match_token(&TokenKind::Eq) {
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
        let mut statements = Vec::new();

        while !self.match_token(&TokenKind::End) {
            statements.push(self.parse_statement()?);
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
        self.expect(&TokenKind::Eq)?;
        let ty = self.parse_type()?;
        Ok(TypeDecl { name, ty })
    }

    // === Type parsing ===

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
            TokenKind::Identifier(name) => {
                self.advance();
                if self.match_token(&TokenKind::Lt) {
                    let mut args = vec![self.parse_type()?];
                    while self.match_token(&TokenKind::Comma) {
                        args.push(self.parse_type()?);
                    }
                    self.expect(&TokenKind::Gt)?;
                    // For now, just return as named with generic args
                    Ok(TypeAnnotation::Named(format!("{}<{}>", name, args.len())))
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
                TokenKind::EqEq => {
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
        match self.peek() {
            TokenKind::Minus => {
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

        // Check for range loop: loop: 0..10 or loop: 1..3, 7, 19..21 step 2
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

                let _body = self.parse_block()?;
                self.expect(&TokenKind::End)?;
                self.expect(&TokenKind::Loop)?;

                // Return as a for-each call with destructuring
                return Ok(Expression::Call {
                    func: Box::new(Expression::Identifier("for_each_destructure".into())),
                    args: vec![
                        Expression::TupleLiteral(
                            vars.into_iter()
                                .map(Expression::Identifier)
                                .collect(),
                        ),
                        collection,
                    ],
                });
            }

            // Check for collection iteration: loop: collection
            // or multi-range: loop: 1..3, 7, 19..21
            // First, parse the first part
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
                ranges: range_parts,
                body,
            });
        }

        // Infinite loop
        let _body = self.parse_block()?;
        self.expect(&TokenKind::End)?;
        self.expect(&TokenKind::Loop)?;
        Ok(Expression::Call {
            func: Box::new(Expression::Identifier("loop".into())),
            args: vec![],
        })
    }

    /// Parse a single part of a loop range spec.
    /// This can be: 0..10, 0..=10, 0..10 step 2, or just 5 (a single value).
    fn parse_loop_range_part(&mut self) -> Result<LoopRangePart, ParseError> {
        let first = self.parse_expression()?;

        // If parse_expression already consumed a range (e.g., 0..10), extract it
        if let Expression::Range {
            start,
            end,
            inclusive,
        } = first
        {
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
            let inclusive = self.peek() == &TokenKind::DotDotEq;
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
        let _body = self.parse_block()?;
        self.expect(&TokenKind::End)?;
        self.expect(&TokenKind::For)?;
        // Represent as a call for now
        Ok(Expression::Call {
            func: Box::new(Expression::Identifier("for_loop".into())),
            args: vec![Expression::Identifier(variable), iter],
        })
    }

    fn parse_postfix(&mut self) -> Result<Expression, ParseError> {
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
                        let field = self.expect_identifier()?;
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
            TokenKind::UnicodeStringLiteral(val) => {
                self.advance();
                Ok(Expression::UnicodeStringLiteral(val))
            }
            TokenKind::BoolLiteral(val) => {
                self.advance();
                Ok(Expression::BoolLiteral(val))
            }
            TokenKind::ByteLiteral(val) => {
                self.advance();
                Ok(Expression::ByteLiteral(val))
            }
            TokenKind::Identifier(name) => {
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

    fn parse_if_expression(&mut self) -> Result<Expression, ParseError> {
        self.advance(); // consume 'if'
        let condition = self.parse_expression()?;
        // New syntax: use comma instead of 'then'
        self.expect(&TokenKind::Comma)?;
        let then_block = self.parse_block()?;
        let else_block = if self.match_token(&TokenKind::Else) {
            // Consume comma after 'else' if present
            self.match_token(&TokenKind::Comma);
            if self.peek() == &TokenKind::If {
                // else if - parse as nested if expression
                let else_if = self.parse_if_expression()?;
                vec![Statement::ExpressionStatement(else_if)]
            } else {
                let block = self.parse_block()?;
                self.expect(&TokenKind::End)?;
                self.expect(&TokenKind::If)?;
                block
            }
        } else {
            self.expect(&TokenKind::End)?;
            self.expect(&TokenKind::If)?;
            vec![]
        };
        Ok(Expression::IfExpression {
            condition: Box::new(condition),
            then_block,
            else_block: Some(else_block),
        })
    }

    fn parse_match_expression(&mut self) -> Result<Expression, ParseError> {
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
            self.expect(&TokenKind::FatArrow)?;
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
                // Peek ahead: if we see FatArrow after a potential pattern, this is a new arm
                if self.peek_ahead_is_fatarrow() {
                    break;
                }
            }

            stmts.push(self.parse_statement()?);
        }

        if stmts.is_empty() {
            // Shouldn't happen, but handle gracefully
            let expr = self.parse_expression()?;
            Ok(MatchArmBody::Expression(expr))
        } else if stmts.len() == 1 {
            if let Statement::ExpressionStatement(expr) = &stmts[0] {
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
                | TokenKind::BoolLiteral(_)
                | TokenKind::Minus
                | TokenKind::LParen
                | TokenKind::Pipe
        )
    }

    /// Peek ahead to check if a FatArrow follows (indicating a new match arm).
    /// This does a limited lookahead.
    fn peek_ahead_is_fatarrow(&mut self) -> bool {
        let saved_pos = self.pos;
        let mut found_fatarrow = false;

        // Skip over what looks like a pattern
        match self.peek().clone() {
            TokenKind::Identifier(_) => {
                self.advance();
                // Check for Enum::Variant pattern
                if *self.peek() == TokenKind::ColonColon {
                    self.advance(); // ::
                    if matches!(self.peek(), TokenKind::Identifier(_)) {
                        self.advance(); // variant name
                    }
                    // Check for (args)
                    if *self.peek() == TokenKind::LParen {
                        self.advance(); // (
                                        // Skip until matching )
                        let mut depth = 1;
                        while depth > 0 && !self.is_at_end() {
                            match self.peek() {
                                TokenKind::LParen => depth += 1,
                                TokenKind::RParen => depth -= 1,
                                _ => {}
                            }
                            self.advance();
                        }
                    }
                } else if *self.peek() == TokenKind::LParen {
                    // Variant pattern without :: (e.g., Ok(content), Error(e))
                    self.advance(); // (
                    let mut depth = 1;
                    while depth > 0 && !self.is_at_end() {
                        match self.peek() {
                            TokenKind::LParen => depth += 1,
                            TokenKind::RParen => depth -= 1,
                            _ => {}
                        }
                        self.advance();
                    }
                }
            }
            TokenKind::IntLiteral(_) => {
                self.advance();
            }
            TokenKind::StringLiteral(_) => {
                self.advance();
            }
            TokenKind::UnicodeStringLiteral(_) => {
                self.advance();
            }
            TokenKind::BoolLiteral(_) => {
                self.advance();
            }
            TokenKind::Minus => {
                self.advance();
                if matches!(self.peek(), TokenKind::IntLiteral(_)) {
                    self.advance();
                }
            }
            TokenKind::LParen => {
                // Tuple pattern: skip to matching )
                self.advance();
                let mut depth = 1;
                while depth > 0 && !self.is_at_end() {
                    match self.peek() {
                        TokenKind::LParen => depth += 1,
                        TokenKind::RParen => depth -= 1,
                        _ => {}
                    }
                    self.advance();
                }
            }
            _ => {}
        }

        // Now check for optional guard (if condition)
        if *self.peek() == TokenKind::If {
            self.advance();
            // Skip expression until FatArrow
            // (simplified: just skip tokens until we find =>)
        }

        if *self.peek() == TokenKind::FatArrow {
            found_fatarrow = true;
        }

        // Restore position
        self.pos = saved_pos;
        found_fatarrow
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

        // Check for prompt (string literal right after get)
        if source.is_none()
            && matches!(
                self.peek(),
                TokenKind::StringLiteral(_) | TokenKind::UnicodeStringLiteral(_)
            )
        {
            prompt = Some(self.parse_expression()?);
        }

        // Parse flags
        loop {
            match self.peek() {
                TokenKind::Minus => {
                    self.advance();
                    if self.peek() == &TokenKind::Minus {
                        self.advance();
                        match self.peek() {
                            TokenKind::Timeout => {
                                self.advance();
                                flags.push(GetFlag::Timeout(self.parse_expression()?));
                            }
                            TokenKind::Default => {
                                self.advance();
                                flags.push(GetFlag::Default(self.parse_expression()?));
                            }
                            TokenKind::Mask => {
                                self.advance();
                                flags.push(GetFlag::Mask(self.parse_expression()?));
                            }
                            TokenKind::Identifier(ref s) if s == "until" => {
                                self.advance();
                                flags.push(GetFlag::Until(self.parse_expression()?));
                            }
                            TokenKind::Bytes => {
                                self.advance();
                                flags.push(GetFlag::Bytes(self.parse_expression()?));
                            }
                            TokenKind::Identifier(ref s) if s == "bytes" => {
                                self.advance();
                                flags.push(GetFlag::Bytes(self.parse_expression()?));
                            }
                            _ => break,
                        }
                    } else {
                        // -n flag
                        self.expect_identifier()?;
                        // This is for put, not get
                        break;
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

    fn parse_assignment_op(&mut self) -> Result<AssignmentOp, ParseError> {
        match self.peek() {
            TokenKind::Eq => {
                self.advance();
                Ok(AssignmentOp::Eq)
            }
            TokenKind::PlusEq => {
                self.advance();
                Ok(AssignmentOp::PlusEq)
            }
            TokenKind::MinusEq => {
                self.advance();
                Ok(AssignmentOp::MinusEq)
            }
            TokenKind::StarEq => {
                self.advance();
                Ok(AssignmentOp::StarEq)
            }
            TokenKind::SlashEq => {
                self.advance();
                Ok(AssignmentOp::SlashEq)
            }
            TokenKind::PercentEq => {
                self.advance();
                Ok(AssignmentOp::PercentEq)
            }
            TokenKind::AmpEq => {
                self.advance();
                Ok(AssignmentOp::AmpEq)
            }
            TokenKind::PipeEq => {
                self.advance();
                Ok(AssignmentOp::PipeEq)
            }
            TokenKind::CaretEq => {
                self.advance();
                Ok(AssignmentOp::CaretEq)
            }
            TokenKind::LtLtEq => {
                self.advance();
                Ok(AssignmentOp::LtLtEq)
            }
            TokenKind::GtGtEq => {
                self.advance();
                Ok(AssignmentOp::GtGtEq)
            }
            _ => Err(ParseError::new(
                format!("Expected assignment operator, got {:?}", self.peek()),
                self.current().span,
            )),
        }
    }

    fn parse_block(&mut self) -> Result<Vec<Statement>, ParseError> {
        let mut stmts = Vec::new();
        while !self.is_at_end()
            && self.peek() != &TokenKind::End
            && self.peek() != &TokenKind::RBrace
            && self.peek() != &TokenKind::Else
        {
            stmts.push(self.parse_statement()?);
        }
        Ok(stmts)
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
    fn test_parse_var_declaration() {
        let prog = parse_source("var x: i32 = 42").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
            Statement::VarDeclaration { name, ty, value } => {
                assert_eq!(name, "x");
                assert!(ty.is_some());
                assert!(value.is_some());
            }
            _ => panic!("Expected VarDeclaration"),
        }
    }

    #[test]
    fn test_parse_function() {
        let prog = parse_source("fn sum(a: i32, b: i32): i32").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
            Statement::FunctionDeclaration(decl) => {
                assert_eq!(decl.name, "sum");
                assert_eq!(decl.params.len(), 2);
                assert!(decl.return_type.is_some());
            }
            _ => panic!("Expected FunctionDeclaration"),
        }
    }

    #[test]
    fn test_parse_binary_expression() {
        let prog = parse_source("var x = a + b * 2").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
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
        match &prog.statements[0] {
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
    fn test_parse_put_no_newline() {
        let prog = parse_source(r#"put -n "Hello""#).unwrap();
        match &prog.statements[0] {
            Statement::PutStatement { no_newline, .. } => {
                assert!(*no_newline);
            }
            _ => panic!("Expected PutStatement"),
        }
    }

    #[test]
    fn test_parse_struct() {
        let prog =
            parse_source("struct Point\n    var x: f32\n    var y: f32\nend struct").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
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
        match &prog.statements[0] {
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
        match &prog.statements[0] {
            Statement::ExpressionStatement(Expression::IfExpression { .. }) => {}
            _ => panic!("Expected IfExpression"),
        }
    }

    #[test]
    fn test_parse_closure() {
        let prog = parse_source("var square = |x: i32| x * x").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
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
    fn test_parse_range() {
        let prog = parse_source("var r = 0..10").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
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
        let prog = parse_source("loop: 0..10\n    put i\nend loop").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
            Statement::ExpressionStatement(Expression::LoopRange { ranges, body }) => {
                assert_eq!(ranges.len(), 1);
                assert_eq!(body.len(), 1);
            }
            _ => panic!("Expected LoopRange"),
        }
    }

    #[test]
    fn test_parse_multi_range_loop() {
        let prog = parse_source("loop: 1..3, 7, 19..21\n    put i\nend loop").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
            Statement::ExpressionStatement(Expression::LoopRange { ranges, .. }) => {
                assert_eq!(ranges.len(), 3);
            }
            _ => panic!("Expected LoopRange with 3 parts"),
        }
    }

    #[test]
    fn test_parse_step_range() {
        let prog = parse_source("loop: 0..10 step 2\n    put i\nend loop").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
            Statement::ExpressionStatement(Expression::LoopRange { ranges, .. }) => {
                assert_eq!(ranges.len(), 1);
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
    fn test_parse_match_with_block_body() {
        let prog = parse_source(
            "match x\n    1 =>\n        put \"one\"\n        x + 1\n    _ => 0\nend match",
        )
        .unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
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
        let prog = parse_source(
            "match x\n    Ok(content) => put content\n    Error(e) => error e\nend match",
        )
        .unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
            Statement::ExpressionStatement(Expression::MatchExpression { arms, .. }) => {
                assert_eq!(arms.len(), 2);
            }
            _ => panic!("Expected MatchExpression"),
        }
    }

    #[test]
    fn test_parse_named_enum_variant() {
        let prog = parse_source("enum Error\n    NotFound(message: ustring)\nend enum").unwrap();
        match &prog.statements[0] {
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
        match &prog.statements[0] {
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
        let prog = parse_source("var y = if x > 0, x else -x end if").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
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
        match &prog.statements[0] {
            Statement::ExpressionStatement(Expression::IfExpression { then_block, .. }) => {
                // The inner if should be in the then_block
                assert_eq!(then_block.len(), 1);
            }
            _ => panic!("Expected IfExpression"),
        }
    }

    #[test]
    fn test_parse_if_without_else() {
        let prog = parse_source("if x > 0,\n    put x\nend if").unwrap();
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
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
        match &prog.statements[0] {
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
        match &prog.statements[0] {
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

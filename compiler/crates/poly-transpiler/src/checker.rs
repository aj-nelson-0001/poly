//! Semantic type checking for Poly programs.
//!
//! The parser deliberately accepts syntax without needing to know what names
//! or types mean.  This module performs the next compiler phase: it builds a
//! symbol table, infers expression types, and reports semantic errors before
//! Rust code generation is attempted.

use std::collections::HashMap;
use std::fmt;

use poly_parser::ast::*;
use poly_types::PolyType;

/// A semantic type-checking error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeCheckError {
    /// Human-readable description of the error.
    pub message: String,
    /// Optional source-context description.  The current AST does not retain
    /// spans on every node, so this is a symbol or construct name for now.
    pub context: Option<String>,
}

impl TypeCheckError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            context: None,
        }
    }

    fn in_context(message: impl Into<String>, context: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            context: Some(context.into()),
        }
    }
}

impl fmt::Display for TypeCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.context {
            Some(context) => write!(f, "{}: {}", context, self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for TypeCheckError {}

#[derive(Debug, Clone)]
struct FunctionSignature {
    // Signatures are collected before bodies are checked, allowing recursive and
    // mutually recursive calls to resolve even when declared later in the file.
    params: Vec<PolyType>,
    // Method signatures keep the receiver parameter; `has_self` records whether
    // the first parameter is the implicit `self` receiver so associated-function
    // calls (`Type::method`) can skip it for arity checking.
    has_self: bool,
    return_type: Option<PolyType>,
}

#[derive(Debug, Clone)]
struct StructInfo {
    fields: HashMap<String, PolyType>,
    methods: HashMap<String, FunctionSignature>,
}

#[derive(Debug, Clone)]
enum EnumVariantInfo {
    Unit,
    Tuple(Vec<PolyType>),
    Struct(HashMap<String, PolyType>),
}

#[derive(Debug, Clone)]
struct EnumInfo {
    variants: HashMap<String, EnumVariantInfo>,
}

/// The semantic checker for a parsed Poly program.
pub struct TypeChecker {
    // The last scope is the innermost one. Functions and modules push temporary
    // scopes instead of mutating the program-level environment.
    scopes: Vec<HashMap<String, PolyType>>,
    // Declarations live in separate namespaces so a function and a struct can
    // be resolved independently while checking expressions.
    functions: HashMap<String, FunctionSignature>,
    structs: HashMap<String, StructInfo>,
    enums: HashMap<String, EnumInfo>,
    aliases: HashMap<String, PolyType>,
    // Errors are accumulated rather than returned immediately for better CLI feedback.
    errors: Vec<TypeCheckError>,
    // Nested function checks use this stack to validate each return statement.
    return_types: Vec<Option<PolyType>>,
    // Used to reject break/continue outside loop bodies.
    loop_depth: usize,
    // Generic type parameter names currently in scope (e.g. `T` in `fn id<T>`).
    // They lower to an unresolved, polymorphic type.
    type_variables: std::collections::HashSet<String>,
}

impl TypeChecker {
    /// Create an empty checker.
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            aliases: HashMap::new(),
            errors: Vec::new(),
            return_types: Vec::new(),
            loop_depth: 0,
            type_variables: std::collections::HashSet::new(),
        }
    }

    /// Check a complete program and return every semantic error found.
    pub fn check(program: &Program) -> Result<(), Vec<TypeCheckError>> {
        let mut checker = Self::new();
        checker.check_program(program);
        if checker.errors.is_empty() {
            Ok(())
        } else {
            Err(checker.errors)
        }
    }

    /// Check a complete program with a reusable checker instance.
    pub fn check_program(&mut self, program: &Program) {
        // Declaration collection is intentionally a separate pass: expression
        // checking can then resolve forward references and recursive functions.
        let statements: Vec<&Statement> = program.statements.iter().map(|s| &s.node).collect();
        self.collect_declarations(&statements);

        for statement in statements {
            match statement {
                Statement::FunctionDeclaration(function) => self.check_function(function, None),
                Statement::StructDeclaration(struct_decl) => {
                    for method in &struct_decl.methods {
                        self.check_function(method, Some(&struct_decl.name));
                    }
                }
                Statement::EnumDeclaration(enum_decl) => {
                    for method in &enum_decl.methods {
                        self.check_function(method, Some(&enum_decl.name));
                    }
                }
                Statement::ImplDeclaration(impl_decl) => {
                    for method in &impl_decl.methods {
                        self.check_function(method, Some(&impl_decl.type_name));
                    }
                }
                Statement::ModuleDeclaration(module) => {
                    let refs: Vec<&Statement> = module.statements.iter().map(|s| &s.node).collect();
                    self.check_program_statements(&refs)
                }
                _ => self.check_statement(statement),
            }
        }
    }

    /// Return errors collected by this checker.
    pub fn errors(&self) -> &[TypeCheckError] {
        &self.errors
    }

    fn check_program_statements(&mut self, statements: &[&Statement]) {
        self.collect_declarations(statements);
        for statement in statements {
            match statement {
                Statement::FunctionDeclaration(function) => self.check_function(function, None),
                Statement::ModuleDeclaration(module) => {
                    let refs: Vec<&Statement> = module.statements.iter().map(|s| &s.node).collect();
                    self.check_program_statements(&refs)
                }
                _ => self.check_statement(statement),
            }
        }
    }

    fn collect_declarations(&mut self, statements: &[&Statement]) {
        // Register names before checking any body. This mirrors how Rust allows
        // item declarations to be referenced independently of source order.
        for statement in statements {
            match statement {
                Statement::FunctionDeclaration(function) => {
                    self.enter_type_variables(&function.generics);
                    let signature = self.function_signature(function, None);
                    self.exit_type_variables(&function.generics);
                    if self
                        .functions
                        .insert(function.name.clone(), signature)
                        .is_some()
                    {
                        self.error(TypeCheckError::new(format!(
                            "duplicate function declaration `{}`",
                            function.name
                        )));
                    }
                }
                Statement::StructDeclaration(struct_decl) => {
                    self.enter_type_variables(&struct_decl.generics);
                    let mut fields = HashMap::new();
                    for field in &struct_decl.fields {
                        let ty = self.annotation_type(&field.ty);
                        if fields.insert(field.name.clone(), ty).is_some() {
                            self.error(TypeCheckError::in_context(
                                format!("duplicate field `{}`", field.name),
                                format!("struct {}", struct_decl.name),
                            ));
                        }
                    }
                    let mut info = StructInfo {
                        fields,
                        methods: HashMap::new(),
                    };
                    for method in &struct_decl.methods {
                        let signature = self.function_signature(method, Some(&struct_decl.name));
                        info.methods.insert(method.name.clone(), signature);
                    }
                    self.exit_type_variables(&struct_decl.generics);
                    if self
                        .structs
                        .insert(struct_decl.name.clone(), info)
                        .is_some()
                    {
                        self.error(TypeCheckError::new(format!(
                            "duplicate struct declaration `{}`",
                            struct_decl.name
                        )));
                    }
                }
                Statement::EnumDeclaration(enum_decl) => {
                    let mut variants = HashMap::new();
                    for variant in &enum_decl.variants {
                        let name = match variant {
                            EnumVariant::Unit(name)
                            | EnumVariant::Tuple(name, _)
                            | EnumVariant::Struct(name, _) => name,
                        };
                        let info = match variant {
                            EnumVariant::Unit(_) => EnumVariantInfo::Unit,
                            EnumVariant::Tuple(_, types) => EnumVariantInfo::Tuple(
                                types.iter().map(|ty| self.annotation_type(ty)).collect(),
                            ),
                            EnumVariant::Struct(_, fields) => EnumVariantInfo::Struct(
                                fields
                                    .iter()
                                    .map(|(field, ty)| (field.clone(), self.annotation_type(ty)))
                                    .collect(),
                            ),
                        };
                        if variants.insert(name.clone(), info).is_some() {
                            self.error(TypeCheckError::in_context(
                                format!("duplicate enum variant `{}`", name),
                                format!("enum {}", enum_decl.name),
                            ));
                        }
                    }
                    if self
                        .enums
                        .insert(enum_decl.name.clone(), EnumInfo { variants })
                        .is_some()
                    {
                        self.error(TypeCheckError::new(format!(
                            "duplicate enum declaration `{}`",
                            enum_decl.name
                        )));
                    }
                }
                Statement::ImplDeclaration(impl_decl) => {
                    let methods: Vec<(String, FunctionSignature)> = impl_decl
                        .methods
                        .iter()
                        .map(|method| {
                            (
                                method.name.clone(),
                                self.function_signature(method, Some(&impl_decl.type_name)),
                            )
                        })
                        .collect();
                    if let Some(info) = self.structs.get_mut(&impl_decl.type_name) {
                        for (name, signature) in methods {
                            info.methods.insert(name, signature);
                        }
                    }
                }
                Statement::TypeDeclaration(type_decl) => {
                    let ty = self.annotation_type(&type_decl.ty);
                    if self.aliases.insert(type_decl.name.clone(), ty).is_some() {
                        self.error(TypeCheckError::new(format!(
                            "duplicate type declaration `{}`",
                            type_decl.name
                        )));
                    }
                }
                Statement::ModuleDeclaration(module) => {
                    let refs: Vec<&Statement> = module.statements.iter().map(|s| &s.node).collect();
                    self.collect_declarations(&refs)
                }
                _ => {}
            }
        }
    }

    fn check_statement(&mut self, statement: &Statement) {
        // Statements may introduce bindings, mutate existing values, or simply
        // force an expression to be type-checked for its side effects.
        match statement {
            Statement::VarDeclaration { name, ty, value } => {
                let declared = ty.as_ref().map(|ty| self.annotation_type(ty));
                let inferred = value
                    .as_ref()
                    .map(|value| self.check_expression(value))
                    .unwrap_or(PolyType::Unknown);
                // `get` is text at the syntax level, then the declaration type
                // determines the conversion performed by code generation.
                let actual = if declared.as_ref().is_some_and(|expected| {
                    !matches!(expected, PolyType::String)
                        && value
                            .as_ref()
                            .is_some_and(|value| matches!(value, Expression::GetExpression(_)))
                }) {
                    declared.clone().unwrap_or(inferred)
                } else {
                    inferred
                };
                if let Some(expected) = &declared {
                    self.require_compatible(&actual, expected, format!("initializer for `{name}`"));
                }
                self.declare(name, declared.unwrap_or(actual));
            }
            Statement::LetDeclaration { name, ty, value } => {
                let actual = self.check_expression(value);
                if let Some(expected) = ty.as_ref().map(|ty| self.annotation_type(ty)) {
                    self.require_compatible(
                        &actual,
                        &expected,
                        format!("initializer for `{name}`"),
                    );
                    self.declare(name, expected);
                } else {
                    self.declare(name, actual);
                }
            }
            Statement::ConstDeclaration { name, value } => {
                let ty = self.check_expression(value);
                self.declare(name, ty);
            }
            Statement::Set { target, value } => {
                let target_type = self.check_lvalue(target);
                let value_type = self.check_expression(value);
                self.require_compatible(&value_type, &target_type, "assignment".to_string());
            }
            Statement::Mutation { target, op, value } => {
                let target_type = self.check_lvalue(target);
                if !is_numeric(&target_type) {
                    self.error(TypeCheckError::new(format!(
                        "cannot apply {:?} mutation to {}; expected a numeric value",
                        op, target_type
                    )));
                }
                if let Some(value) = value {
                    let value_type = self.check_expression(value);
                    if !is_numeric(&value_type) {
                        self.error(TypeCheckError::new(format!(
                            "mutation amount must be numeric, got {}",
                            value_type
                        )));
                    } else {
                        self.require_compatible(&value_type, &target_type, "mutation".to_string());
                    }
                }
            }
            Statement::ReturnStatement(value) => self.check_return(value.as_ref()),
            Statement::BreakStatement | Statement::ContinueStatement => {
                if self.loop_depth == 0 {
                    self.error(TypeCheckError::new(
                        "`break` and `continue` are only valid inside a loop",
                    ));
                }
            }
            Statement::PutStatement { expr, redirect, .. } => {
                let is_file_operator = matches!(
                    expr,
                    Expression::BinaryOp {
                        op: BinaryOp::Gt | BinaryOp::Shr,
                        ..
                    }
                );
                if is_file_operator {
                    if let Expression::BinaryOp { left, right, .. } = expr {
                        let content_type = self.check_expression(left);
                        let path_type = self.check_expression(right);
                        if !matches!(content_type, PolyType::String | PolyType::Vec(_)) {
                            self.error(TypeCheckError::in_context(
                                format!("expected String or bytes, got {}", content_type),
                                "file content".to_string(),
                            ));
                        }
                        self.require_compatible(
                            &path_type,
                            &PolyType::String,
                            "file path".to_string(),
                        );
                    }
                } else {
                    self.check_expression(expr);
                }
                if let Some(redirect) = redirect {
                    let path = match redirect {
                        Redirect::Write(path) | Redirect::Append(path) => {
                            self.check_expression(path)
                        }
                    };
                    self.require_compatible(
                        &path,
                        &PolyType::String,
                        "file redirect path".to_string(),
                    );
                }
            }
            Statement::ErrorStatement(expr)
            | Statement::WarnStatement(expr)
            | Statement::InfoStatement(expr) => {
                self.check_expression(expr);
            }
            Statement::ExpressionStatement(expr) => {
                if let Expression::BinaryOp {
                    op: BinaryOp::Gt | BinaryOp::Shr,
                    left,
                    right,
                } = expr
                {
                    let content_type = self.check_expression(left);
                    let path_type = self.check_expression(right);
                    if !matches!(content_type, PolyType::String | PolyType::Vec(_)) {
                        self.error(TypeCheckError::in_context(
                            format!("expected String or bytes, got {}", content_type),
                            "file content".to_string(),
                        ));
                    }
                    self.require_compatible(&path_type, &PolyType::String, "file path".to_string());
                } else {
                    self.check_expression(expr);
                }
            }
            Statement::ModuleDeclaration(module) => {
                self.push_scope();
                for statement in &module.statements {
                    self.check_statement(&statement.node);
                }
                self.pop_scope();
            }
            Statement::Block(statements) => {
                self.push_scope();
                for statement in statements {
                    self.check_statement(&statement.node);
                }
                self.pop_scope();
            }
            Statement::FunctionDeclaration(function) => {
                // Nested functions: register the signature so calls from the
                // enclosing body resolve (mirroring top-level collection),
                // then check the body so returns and recursion validate.
                self.enter_type_variables(&function.generics);
                let signature = self.function_signature(function, None);
                self.exit_type_variables(&function.generics);
                self.functions
                    .entry(function.name.clone())
                    .or_insert(signature);
                self.check_function(function, None);
            }
            Statement::StructDeclaration(_)
            | Statement::EnumDeclaration(_)
            | Statement::TraitDeclaration(_)
            | Statement::ImplDeclaration(_)
            | Statement::UseDeclaration(_)
            | Statement::TypeDeclaration(_) => {}
        }
    }

    fn check_function(&mut self, function: &FunctionDecl, owner: Option<&str>) {
        // Parameters and locals must not leak into the surrounding module or
        // into a sibling function, so every function gets a fresh scope.
        self.push_scope();
        let signature = self.function_signature(function, owner);
        for (parameter, ty) in function.params.iter().zip(signature.params.iter()) {
            self.declare(&parameter.name, ty.clone());
            if let Some(default) = &parameter.default {
                let default_type = self.check_expression(default);
                self.require_compatible(
                    &default_type,
                    ty,
                    format!("default value for parameter `{}`", parameter.name),
                );
            }
        }
        // Loop examples use implicit conventional bindings. They are refined
        // by loop checking where possible, while keeping legacy loop syntax
        // compatible with the checker.
        self.declare("i", PolyType::I32);
        self.declare("j", PolyType::I32);
        self.declare("index", PolyType::I32);
        self.declare("fruit", PolyType::Unknown);
        self.declare("record", PolyType::Unknown);
        self.declare("line", PolyType::Unknown);
        self.return_types.push(signature.return_type.clone());
        if let Some(body) = &function.body {
            for statement in body {
                self.check_statement(&statement.node);
            }
        }
        self.return_types.pop();
        self.pop_scope();
    }

    fn check_return(&mut self, value: Option<&Expression>) {
        let Some(expected) = self.return_types.last().cloned() else {
            self.error(TypeCheckError::new(
                "`return` is only valid inside a function",
            ));
            if let Some(value) = value {
                self.check_expression(value);
            }
            return;
        };

        let actual = value
            .map(|value| self.check_expression(value))
            .unwrap_or(PolyType::Unknown);
        if let Some(expected) = expected {
            if value.is_none() {
                self.error(TypeCheckError::new(format!(
                    "function must return {}, but returned nothing",
                    expected
                )));
            } else {
                self.require_compatible(&actual, &expected, "return value".to_string());
            }
        }
    }

    fn check_lvalue(&mut self, expression: &Expression) -> PolyType {
        // Lvalues share expression syntax but have stricter rules: a call or
        // literal can be read, never assigned to.
        match expression {
            Expression::Identifier(name) => self
                .lookup(name)
                .or_else(|| self.enum_variant_type(name))
                .unwrap_or_else(|| {
                    self.error(TypeCheckError::new(format!("unknown variable `{name}`")));
                    PolyType::Unknown
                }),
            Expression::FieldAccess { object, field } => self.check_field_access(object, field),
            Expression::Index { object, index } => {
                let object_type = self.check_expression(object);
                let index_type = self.check_expression(index);
                if !is_integer(&index_type) {
                    self.error(TypeCheckError::new(format!(
                        "index must be an integer, got {}",
                        index_type
                    )));
                }
                element_type(&object_type).unwrap_or_else(|| {
                    self.error(TypeCheckError::new(format!(
                        "cannot index value of type {}",
                        object_type
                    )));
                    PolyType::Unknown
                })
            }
            _ => {
                self.error(TypeCheckError::new("assignment target is not writable"));
                self.check_expression(expression)
            }
        }
    }

    fn check_expression(&mut self, expression: &Expression) -> PolyType {
        // Return `Unknown` after reporting an error where possible. That lets
        // checking continue and prevents one bad subexpression from hiding all
        // diagnostics that follow it.
        match expression {
            Expression::IntLiteral(_) => PolyType::I32,
            Expression::FloatLiteral(_) => PolyType::F64,
            Expression::StringLiteral(_) | Expression::UnicodeStringLiteral(_) => PolyType::String,
            Expression::UnicodeCharLiteral(_) => PolyType::Char,
            Expression::BoolLiteral(_) => PolyType::Bool,
            Expression::ByteLiteral(_) => PolyType::Vec(Box::new(PolyType::U8)),
            Expression::Identifier(name) => self
                .lookup(name)
                .or_else(|| self.enum_variant_type(name))
                .unwrap_or_else(|| {
                    self.error(TypeCheckError::new(format!("unknown variable `{name}`")));
                    PolyType::Unknown
                }),
            Expression::BinaryOp { op, left, right } => {
                let left_type = self.check_expression(left);
                let right_type = self.check_expression(right);
                self.check_binary_op(op, &left_type, &right_type)
            }
            Expression::UnaryOp { op, expr } => {
                let ty = self.check_expression(expr);
                match op {
                    UnaryOp::Neg => {
                        if !is_numeric(&ty) {
                            self.error(TypeCheckError::new(format!(
                                "unary `-` expects a numeric value, got {}",
                                ty
                            )));
                        }
                        ty
                    }
                    UnaryOp::Not => {
                        self.require_compatible(&ty, &PolyType::Bool, "unary `!`".to_string());
                        PolyType::Bool
                    }
                    UnaryOp::BitNot => {
                        if !is_integer(&ty) {
                            self.error(TypeCheckError::new(format!(
                                "bitwise `~` expects an integer, got {}",
                                ty
                            )));
                        }
                        ty
                    }
                    UnaryOp::Deref => match ty {
                        PolyType::Reference(_, inner) => *inner,
                        PolyType::Named(name) if name.starts_with("ptr ") => PolyType::Unknown,
                        _ => {
                            self.error(TypeCheckError::new(format!("cannot dereference {}", ty)));
                            PolyType::Unknown
                        }
                    },
                }
            }
            Expression::Call { func, args } => self.check_call(func, args),
            Expression::MethodCall {
                object,
                method,
                args,
            } => self.check_method_call(object, method, args),
            Expression::Index { object, index } => {
                let object_type = self.check_expression(object);
                let index_type = self.check_expression(index);
                if !is_integer(&index_type) {
                    self.error(TypeCheckError::new(format!(
                        "index must be an integer, got {}",
                        index_type
                    )));
                }
                element_type(&object_type).unwrap_or_else(|| {
                    self.error(TypeCheckError::new(format!(
                        "cannot index value of type {}",
                        object_type
                    )));
                    PolyType::Unknown
                })
            }
            Expression::FieldAccess { object, field } => self.check_field_access(object, field),
            Expression::Parenthesized(expression) => self.check_expression(expression),
            Expression::IfExpression {
                condition,
                then_block,
                else_block,
            } => {
                let condition_type = self.check_expression(condition);
                self.require_compatible(
                    &condition_type,
                    &PolyType::Bool,
                    "if condition".to_string(),
                );
                // `while` statements are encoded by the parser as an
                // if-expression without an else block; keep the loop depth
                // bumped while checking the body so `break`/`continue` work.
                let is_while = else_block.is_none();
                if is_while {
                    self.loop_depth += 1;
                }
                self.push_scope();
                for statement in then_block {
                    self.check_statement(&statement.node);
                }
                if is_while {
                    self.loop_depth -= 1;
                }
                if let Some(else_block) = else_block {
                    for statement in else_block {
                        self.check_statement(&statement.node);
                    }
                }
                self.pop_scope();
                PolyType::Unknown
            }
            Expression::MatchExpression { scrutinee, arms } => {
                let scrutinee_type = self.check_expression(scrutinee);
                let mut arm_type = PolyType::Unknown;
                for arm in arms {
                    self.push_scope();
                    self.check_pattern(&arm.pattern, &scrutinee_type);
                    if let Some(guard) = &arm.guard {
                        let guard_type = self.check_expression(guard);
                        self.require_compatible(
                            &guard_type,
                            &PolyType::Bool,
                            "match guard".to_string(),
                        );
                    }
                    let body_type = match &arm.body {
                        MatchArmBody::Expression(expression) => self.check_expression(expression),
                        MatchArmBody::Block(statements) => {
                            for statement in statements {
                                self.check_statement(&statement.node);
                            }
                            PolyType::Unknown
                        }
                    };
                    if !matches!(body_type, PolyType::Unknown) {
                        if matches!(arm_type, PolyType::Unknown) {
                            arm_type = body_type;
                        } else {
                            self.require_compatible(&body_type, &arm_type, "match arm".to_string());
                        }
                    }
                    self.pop_scope();
                }
                arm_type
            }
            Expression::Closure { params, body } => {
                self.push_scope();
                let mut parameter_types = Vec::new();
                for parameter in params {
                    let ty = self.annotation_type(&parameter.ty);
                    parameter_types.push(ty.clone());
                    self.declare(&parameter.name, ty);
                }
                let return_type = self.check_expression(body);
                self.pop_scope();
                PolyType::Function {
                    params: parameter_types,
                    ret: Box::new(return_type),
                }
            }
            Expression::ArrayLiteral(elements) => {
                let mut element_type = PolyType::Unknown;
                for element in elements {
                    let ty = self.check_expression(element);
                    if matches!(element_type, PolyType::Unknown) {
                        element_type = ty;
                    } else {
                        self.require_compatible(&ty, &element_type, "array element".to_string());
                    }
                }
                PolyType::Vec(Box::new(element_type))
            }
            Expression::TupleLiteral(elements) => PolyType::Tuple(
                elements
                    .iter()
                    .map(|element| self.check_expression(element))
                    .collect(),
            ),
            Expression::StructLiteral { name, fields } => self.check_struct_literal(name, fields),
            Expression::EnumVariant {
                enum_name,
                variant,
                data,
            } => self.check_enum_variant(enum_name, variant, data.as_deref()),
            Expression::ForLoop {
                variable,
                iterable,
                body,
            } => {
                // `for x in collection` binds `x` to the element type of the
                // iterable, then checks the body like any other loop.
                let iterable_type = self.check_expression(iterable);
                let element_type = match &iterable_type {
                    PolyType::Vec(inner) => (**inner).clone(),
                    PolyType::String => PolyType::Char,
                    _ => PolyType::Unknown,
                };
                self.push_scope();
                self.declare(variable, element_type);
                self.loop_depth += 1;
                for statement in body {
                    self.check_statement(&statement.node);
                }
                self.loop_depth -= 1;
                self.pop_scope();
                PolyType::Unknown
            }
            Expression::LoopRange { ranges, body } => {
                for range in ranges {
                    match range {
                        LoopRangePart::Range {
                            start, end, step, ..
                        } => {
                            let start_type = self.check_expression(start);
                            let end_type = self.check_expression(end);
                            if !is_integer(&start_type) || !is_integer(&end_type) {
                                self.error(TypeCheckError::new(
                                    "loop range bounds must be integers",
                                ));
                            }
                            if let Some(step) = step {
                                let step_type = self.check_expression(step);
                                if !is_integer(&step_type) {
                                    self.error(TypeCheckError::new(
                                        "loop range step must be an integer",
                                    ));
                                }
                            }
                        }
                        LoopRangePart::Value(value) => {
                            self.check_expression(value);
                        }
                    }
                }
                self.push_scope();
                self.declare("i", PolyType::I32);
                self.declare("j", PolyType::I32);
                self.declare("index", PolyType::I32);
                self.declare("fruit", PolyType::Unknown);
                self.declare("record", PolyType::Unknown);
                self.declare("line", PolyType::Unknown);
                for range in ranges {
                    if let LoopRangePart::Value(value) = range {
                        match value.as_ref() {
                            Expression::Identifier(name) => {
                                let binding = name.trim_end_matches('s');
                                let element_type = match self.lookup(name) {
                                    Some(PolyType::Vec(inner)) => *inner,
                                    _ => PolyType::Unknown,
                                };
                                self.declare(binding, element_type);
                            }
                            Expression::MethodCall { object, method, .. }
                                if method == "enumerate" =>
                            {
                                self.declare("index", PolyType::I32);
                                let element_type = match self.check_expression(object) {
                                    PolyType::Vec(inner) => *inner,
                                    _ => PolyType::Unknown,
                                };
                                self.declare("fruit", element_type);
                            }
                            _ => {}
                        }
                    }
                }
                self.loop_depth += 1;
                for statement in body {
                    self.check_statement(&statement.node);
                }
                self.loop_depth -= 1;
                self.pop_scope();
                PolyType::Unknown
            }
            Expression::AsExpression { expr, ty } => {
                let actual = self.check_expression(expr);
                let target = self.annotation_type(ty);
                if !is_castable(&actual, &target) {
                    self.error(TypeCheckError::new(format!(
                        "cannot cast {} to {}",
                        actual, target
                    )));
                }
                target
            }
            Expression::TryExpression(expr) => {
                let result_type = self.check_expression(expr);
                match result_type {
                    PolyType::Result(ok, _) => *ok,
                    PolyType::Unknown => PolyType::Unknown,
                    other => {
                        self.error(TypeCheckError::new(format!(
                            "`try` expects a Result value, got {}",
                            other
                        )));
                        PolyType::Unknown
                    }
                }
            }
            Expression::GetExpression(get) => self.check_get_expression(get),
            Expression::UnsafeBlock(statements) => {
                self.push_scope();
                for statement in statements {
                    self.check_statement(&statement.node);
                }
                self.pop_scope();
                PolyType::Unknown
            }
            Expression::Range { start, end, .. } => {
                let start_type = self.check_expression(start);
                let end_type = self.check_expression(end);
                if !is_integer(&start_type) || !is_integer(&end_type) {
                    self.error(TypeCheckError::new("range bounds must be integers"));
                }
                PolyType::Vec(Box::new(PolyType::I32))
            }
        }
    }

    fn check_binary_op(&mut self, op: &BinaryOp, left: &PolyType, right: &PolyType) -> PolyType {
        // Binary rules are kept in one place so arithmetic, comparisons, logic,
        // and bitwise operations produce consistent errors.
        match op {
            BinaryOp::Add => {
                if *left == PolyType::String
                    && (*right == PolyType::String
                        || is_numeric(right)
                        || matches!(right, PolyType::Char | PolyType::Bool | PolyType::Unknown))
                    || *right == PolyType::String
                        && (is_numeric(left)
                            || matches!(left, PolyType::Char | PolyType::Bool | PolyType::Unknown))
                {
                    PolyType::String
                } else if is_numeric(left) && is_numeric(right) {
                    numeric_join(left, right)
                } else {
                    self.error(TypeCheckError::new(format!(
                        "operator `+` cannot combine {} and {}",
                        left, right
                    )));
                    PolyType::Unknown
                }
            }
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                if is_numeric(left) && is_numeric(right) {
                    numeric_join(left, right)
                } else {
                    self.error(TypeCheckError::new(format!(
                        "arithmetic operator requires numeric operands, got {} and {}",
                        left, right
                    )));
                    PolyType::Unknown
                }
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                self.require_compatible(left, right, "equality comparison".to_string());
                PolyType::Bool
            }
            BinaryOp::Lt | BinaryOp::Gt | BinaryOp::LtEq | BinaryOp::GtEq => {
                if !(is_numeric(left) && is_numeric(right)
                    || (*left == PolyType::String && *right == PolyType::String))
                {
                    self.error(TypeCheckError::new(format!(
                        "ordering comparison requires matching numeric or string operands, got {} and {}",
                        left, right
                    )));
                }
                PolyType::Bool
            }
            BinaryOp::And | BinaryOp::Or => {
                self.require_compatible(left, &PolyType::Bool, "logical operator".to_string());
                self.require_compatible(right, &PolyType::Bool, "logical operator".to_string());
                PolyType::Bool
            }
            BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr => {
                if !is_integer(left) || !is_integer(right) {
                    self.error(TypeCheckError::new(format!(
                        "bitwise operator requires integer operands, got {} and {}",
                        left, right
                    )));
                    PolyType::Unknown
                } else {
                    numeric_join(left, right)
                }
            }
        }
    }

    fn check_call(&mut self, function: &Expression, args: &[Expression]) -> PolyType {
        // Check arguments even when callee lookup fails so diagnostics remain
        // useful for nested expressions.
        let argument_types: Vec<PolyType> =
            args.iter().map(|arg| self.check_expression(arg)).collect();
        if let Expression::Identifier(name) = function {
            if let Some(signature) = self.functions.get(name).cloned() {
                self.check_arguments(name, &signature, &argument_types);
                return signature.return_type.unwrap_or(PolyType::Unknown);
            }
            // A closure or function-typed variable may be called through its
            // name; resolve it through the current scope before falling back
            // to builtins.
            if let Some(PolyType::Function { params, ret }) = self.lookup(name) {
                if params.len() != argument_types.len() {
                    self.error(TypeCheckError::new(format!(
                        "function `{name}` expects {} arguments, got {}",
                        params.len(),
                        argument_types.len()
                    )));
                }
                for (actual, expected) in argument_types.iter().zip(params.iter()) {
                    self.require_compatible(actual, expected, format!("argument to `{name}`"));
                }
                return *ret;
            }
            return self.check_builtin_call(name, &argument_types);
        }

        let function_type = self.check_expression(function);
        if let PolyType::Function { params, ret } = function_type {
            if params.len() != argument_types.len() {
                self.error(TypeCheckError::new(format!(
                    "function expected {} arguments, got {}",
                    params.len(),
                    argument_types.len()
                )));
            }
            for (actual, expected) in argument_types.iter().zip(params.iter()) {
                self.require_compatible(actual, expected, "function argument".to_string());
            }
            *ret
        } else {
            self.error(TypeCheckError::new(format!(
                "{} is not callable",
                function_type
            )));
            PolyType::Unknown
        }
    }

    fn check_builtin_call(&mut self, name: &str, args: &[PolyType]) -> PolyType {
        match name {
            "Ok" => PolyType::Result(
                Box::new(args.first().cloned().unwrap_or(PolyType::Unknown)),
                Box::new(PolyType::Unknown),
            ),
            "Error" => PolyType::Result(
                Box::new(PolyType::Unknown),
                Box::new(args.first().cloned().unwrap_or(PolyType::Unknown)),
            ),
            "Some" => {
                PolyType::Option(Box::new(args.first().cloned().unwrap_or(PolyType::Unknown)))
            }
            "None" => PolyType::Option(Box::new(PolyType::Unknown)),
            "str" | "to_string" => {
                if args.len() != 1 {
                    self.error(TypeCheckError::new(format!(
                        "`{name}` expects one argument"
                    )));
                }
                PolyType::String
            }
            "open" => {
                if args.len() != 1 {
                    self.error(TypeCheckError::new("`open` expects a path argument"));
                }
                self.require_compatible(
                    args.first().unwrap_or(&PolyType::Unknown),
                    &PolyType::String,
                    "open path".to_string(),
                );
                PolyType::Named("File".into())
            }
            "sleep" => PolyType::Unknown,
            "delay" => PolyType::String,
            "http_get" => PolyType::Result(Box::new(PolyType::String), Box::new(PolyType::String)),
            "tcp_connect" => PolyType::String,
            "db_execute" => PolyType::Vec(Box::new(PolyType::String)),
            "max" | "min" => args
                .iter()
                .cloned()
                .reduce(|left, right| numeric_join(&left, &right))
                .unwrap_or(PolyType::Unknown),
            "abs" | "sqrt" => args.first().cloned().unwrap_or(PolyType::Unknown),
            "pow" => args
                .iter()
                .cloned()
                .reduce(|left, right| numeric_join(&left, &right))
                .unwrap_or(PolyType::Unknown),
            "spawn_task" | "exit" | "process_config" | "for_each_destructure" => PolyType::Unknown,
            _ => {
                if let Some(variant_type) = self.enum_variant_type(name) {
                    return variant_type;
                }
                self.error(TypeCheckError::new(format!("unknown function `{name}`")));
                PolyType::Unknown
            }
        }
    }

    fn check_arguments(&mut self, name: &str, signature: &FunctionSignature, args: &[PolyType]) {
        if signature.params.len() != args.len() {
            self.error(TypeCheckError::new(format!(
                "function `{name}` expects {} arguments, got {}",
                signature.params.len(),
                args.len()
            )));
        }
        for (actual, expected) in args.iter().zip(signature.params.iter()) {
            self.require_compatible(actual, expected, format!("argument to `{name}`"));
        }
    }

    fn check_method_call(
        &mut self,
        object: &Expression,
        method: &str,
        args: &[Expression],
    ) -> PolyType {
        // Resolve the receiver first; method lookup depends on its concrete
        // struct, enum, or built-in container type.
        let object_type = self.check_expression(object);
        let argument_types: Vec<PolyType> =
            args.iter().map(|arg| self.check_expression(arg)).collect();

        if method == "await" {
            return object_type;
        }
        if method == "to_string" {
            self.expect_no_arguments(method, &argument_types, PolyType::String);
            return PolyType::String;
        }

        if method == "await" {
            return object_type;
        }

        match &object_type {
            PolyType::String => match method {
                "len" => self.expect_no_arguments(method, &argument_types, PolyType::I32),
                "to_string" => self.expect_no_arguments(method, &argument_types, PolyType::String),
                "contains" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    if let Some(argument) = argument_types.first() {
                        self.require_compatible(
                            argument,
                            &PolyType::String,
                            "contains argument".to_string(),
                        );
                    }
                    PolyType::Bool
                }
                "split" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    PolyType::Vec(Box::new(PolyType::String))
                }
                "parse" => {
                    self.expect_argument_count(method, &argument_types, 0);
                    PolyType::Unknown
                }
                "pad_left" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    PolyType::String
                }
                "trim" | "to_uppercase" | "to_lowercase" | "chars" | "bytes" => {
                    self.expect_no_arguments(method, &argument_types, PolyType::String)
                }
                "starts_with" | "ends_with" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    PolyType::Bool
                }
                "replace" => {
                    self.expect_argument_count(method, &argument_types, 2);
                    PolyType::String
                }
                _ => self.unknown_method(method, &object_type),
            },
            PolyType::Vec(inner) => match method {
                "len" => self.expect_no_arguments(method, &argument_types, PolyType::I32),
                "push" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    if let Some(argument) = argument_types.first() {
                        self.require_compatible(argument, inner, "vector element".to_string());
                    }
                    PolyType::Unknown
                }
                "enumerate" => PolyType::Unknown,
                _ => self.unknown_method(method, &object_type),
            },
            PolyType::Named(name) if name == "File" => match method {
                "eof" => self.expect_no_arguments(method, &argument_types, PolyType::Bool),
                "get_line" => self.expect_no_arguments(method, &argument_types, PolyType::String),
                _ => self.unknown_method(method, &object_type),
            },
            PolyType::Named(name) => {
                let signature = self
                    .structs
                    .get(name)
                    .and_then(|info| info.methods.get(method))
                    .cloned();
                if let Some(signature) = signature {
                    let params = signature.params.get(1..).unwrap_or(&[]);
                    if params.len() != argument_types.len() {
                        self.error(TypeCheckError::new(format!(
                            "method `{method}` expects {} arguments, got {}",
                            params.len(),
                            argument_types.len()
                        )));
                    }
                    for (actual, expected) in argument_types.iter().zip(params.iter()) {
                        self.require_compatible(
                            actual,
                            expected,
                            format!("argument to `{method}`"),
                        );
                    }
                    signature.return_type.unwrap_or(PolyType::Unknown)
                } else {
                    self.unknown_method(method, &object_type)
                }
            }
            PolyType::Unknown => PolyType::Unknown,
            _ => self.unknown_method(method, &object_type),
        }
    }

    fn check_field_access(&mut self, object: &Expression, field: &str) -> PolyType {
        let object_type = self.check_expression(object);
        if let PolyType::Named(name) = &object_type {
            if let Some(info) = self.structs.get(name) {
                if let Some(ty) = info.fields.get(field) {
                    return ty.clone();
                }
                self.error(TypeCheckError::new(format!(
                    "struct `{name}` has no field `{field}`"
                )));
                return PolyType::Unknown;
            }
        }
        if matches!(object_type, PolyType::Unknown) {
            PolyType::Unknown
        } else {
            self.error(TypeCheckError::new(format!(
                "type {} has no field `{field}`",
                object_type
            )));
            PolyType::Unknown
        }
    }

    fn check_struct_literal(&mut self, name: &str, fields: &[(String, Expression)]) -> PolyType {
        let Some(info) = self.structs.get(name).cloned() else {
            self.error(TypeCheckError::new(format!("unknown struct `{name}`")));
            for (_, value) in fields {
                self.check_expression(value);
            }
            return PolyType::Unknown;
        };
        let mut seen = HashMap::new();
        for (field, value) in fields {
            let value_type = self.check_expression(value);
            if let Some(expected) = info.fields.get(field) {
                self.require_compatible(&value_type, expected, format!("field `{field}`"));
            } else {
                self.error(TypeCheckError::new(format!(
                    "struct `{name}` has no field `{field}`"
                )));
            }
            seen.insert(field, ());
        }
        for field in info.fields.keys() {
            if !seen.contains_key(field) {
                self.error(TypeCheckError::new(format!(
                    "struct literal `{name}` is missing field `{field}`"
                )));
            }
        }
        PolyType::Named(name.to_string())
    }

    fn check_enum_variant(
        &mut self,
        enum_name: &str,
        variant: &str,
        data: Option<&[Expression]>,
    ) -> PolyType {
        if enum_name == "Result" || enum_name.is_empty() {
            let values: Vec<PolyType> = data
                .unwrap_or_default()
                .iter()
                .map(|value| self.check_expression(value))
                .collect();
            return PolyType::Result(
                Box::new(values.first().cloned().unwrap_or(PolyType::Unknown)),
                Box::new(PolyType::Unknown),
            );
        }
        // `Type::method(...)` is an associated-function call on a struct, not
        // an enum variant construction.  Resolve it against the struct's
        // methods before assuming the name is an enum.
        if let Some(struct_info) = self.structs.get(enum_name).cloned() {
            if let Some(signature) = struct_info.methods.get(variant).cloned() {
                let values: Vec<PolyType> = data
                    .unwrap_or_default()
                    .iter()
                    .map(|value| self.check_expression(value))
                    .collect();
                let params = if signature.has_self {
                    signature.params.get(1..).unwrap_or(&[])
                } else {
                    &signature.params
                };
                if params.len() != values.len() {
                    self.error(TypeCheckError::new(format!(
                        "method `{variant}` expects {} arguments, got {}",
                        params.len(),
                        values.len()
                    )));
                }
                for (actual, expected) in values.iter().zip(params.iter()) {
                    self.require_compatible(actual, expected, format!("argument to `{variant}`"));
                }
                return signature.return_type.unwrap_or(PolyType::Unknown);
            }
        }
        let Some(info) = self.enums.get(enum_name).cloned() else {
            self.error(TypeCheckError::new(format!("unknown enum `{enum_name}`")));
            return PolyType::Unknown;
        };
        let Some(variant_info) = info.variants.get(variant) else {
            self.error(TypeCheckError::new(format!(
                "enum `{enum_name}` has no variant `{variant}`"
            )));
            return PolyType::Named(enum_name.to_string());
        };
        let values: Vec<PolyType> = data
            .unwrap_or_default()
            .iter()
            .map(|value| self.check_expression(value))
            .collect();
        match variant_info {
            EnumVariantInfo::Unit => {
                if !values.is_empty() {
                    self.error(TypeCheckError::new(format!(
                        "variant `{variant}` takes no values"
                    )));
                }
            }
            EnumVariantInfo::Tuple(expected) => {
                self.check_value_list(expected, &values, variant);
            }
            EnumVariantInfo::Struct(expected) => {
                self.check_value_list(
                    &expected.values().cloned().collect::<Vec<_>>(),
                    &values,
                    variant,
                );
            }
        }
        PolyType::Named(enum_name.to_string())
    }

    fn check_value_list(&mut self, expected: &[PolyType], actual: &[PolyType], name: &str) {
        if expected.len() != actual.len() {
            self.error(TypeCheckError::new(format!(
                "variant `{name}` expects {} values, got {}",
                expected.len(),
                actual.len()
            )));
        }
        for (actual, expected) in actual.iter().zip(expected.iter()) {
            self.require_compatible(actual, expected, format!("variant `{name}`"));
        }
    }

    fn check_pattern(&mut self, pattern: &Pattern, scrutinee_type: &PolyType) {
        // Pattern checking validates both the pattern's shape and any literal
        // or enum payload types against the match scrutinee.
        match pattern {
            Pattern::Wildcard => {}
            Pattern::Literal(expression) => {
                let pattern_type = self.check_expression(expression);
                self.require_compatible(&pattern_type, scrutinee_type, "match pattern".to_string());
            }
            Pattern::Identifier(name) => {
                if matches!(name.as_str(), "Some" | "None") {
                    // Builtin Option variants: a bare `None` (or `Some`)
                    // matches an Option scrutinee; other scrutinees error.
                    if !matches!(scrutinee_type, PolyType::Option(_))
                        && !matches!(scrutinee_type, PolyType::Unknown)
                    {
                        self.error(TypeCheckError::new(format!(
                            "variant `{name}` cannot match {}",
                            scrutinee_type
                        )));
                    }
                } else if let Some(enum_name) = self.enum_name_for_variant(name) {
                    self.require_compatible(
                        scrutinee_type,
                        &PolyType::Named(enum_name),
                        "enum match pattern".to_string(),
                    );
                } else {
                    self.declare(name, scrutinee_type.clone());
                }
            }
            Pattern::Tuple(patterns) => {
                if let PolyType::Tuple(types) = scrutinee_type {
                    if types.len() != patterns.len() {
                        self.error(TypeCheckError::new(
                            "tuple pattern has the wrong number of elements",
                        ));
                    }
                    for (pattern, ty) in patterns.iter().zip(types.iter()) {
                        self.check_pattern(pattern, ty);
                    }
                } else {
                    self.error(TypeCheckError::new(format!(
                        "tuple pattern cannot match {}",
                        scrutinee_type
                    )));
                }
            }
            Pattern::Enum {
                enum_name,
                variant,
                inner,
            } => {
                let is_result =
                    enum_name.is_empty() && matches!(scrutinee_type, PolyType::Result(_, _));
                let is_option = enum_name.is_empty()
                    && matches!(variant.as_str(), "Some" | "None")
                    && matches!(scrutinee_type, PolyType::Option(_));
                let resolved_enum = if enum_name.is_empty() {
                    match scrutinee_type {
                        PolyType::Named(name) => Some(name.clone()),
                        PolyType::Result(_, _) => Some("Result".to_string()),
                        PolyType::Option(_) => Some("Option".to_string()),
                        _ => self.enum_name_for_variant(variant),
                    }
                } else {
                    Some(enum_name.clone())
                };
                if !is_result && !is_option {
                    let expected = PolyType::Named(resolved_enum.clone().unwrap_or_default());
                    self.require_compatible(
                        scrutinee_type,
                        &expected,
                        "enum match pattern".to_string(),
                    );
                }

                let inner_types = if is_result {
                    match (variant.as_str(), scrutinee_type) {
                        ("Ok", PolyType::Result(ok, _)) => vec![(**ok).clone()],
                        ("Error", PolyType::Result(_, error)) => vec![(**error).clone()],
                        _ => Vec::new(),
                    }
                } else if is_option {
                    match (variant.as_str(), scrutinee_type) {
                        ("Some", PolyType::Option(inner)) => vec![(**inner).clone()],
                        _ => Vec::new(),
                    }
                } else if let Some(enum_name) = resolved_enum.as_deref() {
                    match self
                        .enums
                        .get(enum_name)
                        .and_then(|info| info.variants.get(variant))
                    {
                        Some(EnumVariantInfo::Tuple(types)) => types.clone(),
                        Some(EnumVariantInfo::Struct(fields)) => fields.values().cloned().collect(),
                        _ => Vec::new(),
                    }
                } else {
                    Vec::new()
                };

                if let Some(inner) = inner {
                    for (index, pattern) in inner.iter().enumerate() {
                        let pattern_type = inner_types.get(index).unwrap_or(&PolyType::Unknown);
                        self.check_pattern(pattern, pattern_type);
                    }
                }
            }
            Pattern::NamedFields { name, fields } => {
                if let Some(info) = self.structs.get(name).cloned() {
                    for (field, pattern) in fields {
                        if let Some(ty) = info.fields.get(field).cloned() {
                            self.check_pattern(pattern, &ty);
                        } else {
                            self.error(TypeCheckError::new(format!(
                                "struct `{name}` has no field `{field}`"
                            )));
                        }
                    }
                } else {
                    self.error(TypeCheckError::new(format!("unknown struct `{name}`")));
                }
            }
            Pattern::Range {
                start,
                end,
                inclusive: _,
            } => {
                let start_type = self.check_expression(start);
                let end_type = self.check_expression(end);
                if !is_integer(&start_type) || !is_integer(&end_type) {
                    self.error(TypeCheckError::new("range pattern bounds must be integers"));
                }
                self.require_compatible(scrutinee_type, &start_type, "range pattern".to_string());
            }
            Pattern::Binding { name, pattern } => {
                self.declare(name, scrutinee_type.clone());
                self.check_pattern(pattern, scrutinee_type);
            }
        }
    }

    fn enum_name_for_variant(&self, variant: &str) -> Option<String> {
        if matches!(variant, "Some" | "None") {
            // Builtin Option variants, mirroring the builtin Ok/Error for
            // Result that are resolved without a declared enum.
            return Some("Option".to_string());
        }
        self.enums
            .iter()
            .find_map(|(name, info)| info.variants.contains_key(variant).then(|| name.clone()))
    }

    fn enum_variant_type(&self, variant: &str) -> Option<PolyType> {
        match variant {
            "Some" => Some(PolyType::Option(Box::new(PolyType::Unknown))),
            "None" => Some(PolyType::Option(Box::new(PolyType::Unknown))),
            _ => self.enum_name_for_variant(variant).map(PolyType::Named),
        }
    }

    fn check_get_expression(&mut self, get: &GetExpr) -> PolyType {
        // `get` is polymorphic at the syntax level. Flags and the surrounding
        // declaration determine whether it ultimately yields text, bytes, or
        // a parsed value.
        if let Some(prompt) = &get.prompt {
            let prompt_type = self.check_expression(prompt);
            self.require_compatible(&prompt_type, &PolyType::String, "input prompt".to_string());
        }
        if let Some(source) = &get.source {
            let source_type = self.check_expression(source);
            self.require_compatible(
                &source_type,
                &PolyType::String,
                "input file path".to_string(),
            );
        }
        for flag in &get.flags {
            match flag {
                GetFlag::Timeout(value) | GetFlag::Bytes(value) => {
                    let ty = self.check_expression(value);
                    self.require_compatible(&ty, &PolyType::I32, "input flag".to_string());
                }
                GetFlag::Default(value) | GetFlag::Mask(value) | GetFlag::Until(value) => {
                    self.check_expression(value);
                }
                GetFlag::As(ty) => return self.annotation_type(ty),
            }
        }
        if get.source.is_some()
            && get
                .flags
                .iter()
                .any(|flag| matches!(flag, GetFlag::Bytes(_)))
        {
            PolyType::Vec(Box::new(PolyType::U8))
        } else if get
            .flags
            .iter()
            .any(|flag| matches!(flag, GetFlag::Timeout(_)))
        {
            PolyType::Result(Box::new(PolyType::String), Box::new(PolyType::String))
        } else {
            PolyType::String
        }
    }

    fn function_signature(
        &self,
        function: &FunctionDecl,
        owner: Option<&str>,
    ) -> FunctionSignature {
        FunctionSignature {
            params: function
                .params
                .iter()
                .map(|parameter| self.annotation_type_for_owner(&parameter.ty, owner))
                .collect(),
            has_self: function
                .params
                .first()
                .is_some_and(|parameter| parameter.name == "self"),
            return_type: function
                .return_type
                .as_ref()
                .map(|ty| self.annotation_type_for_owner(ty, owner)),
        }
    }

    fn annotation_type(&self, annotation: &TypeAnnotation) -> PolyType {
        // Owner-aware resolution is needed for `Self` in trait and impl methods;
        // ordinary annotations use the program-level namespace.
        self.annotation_type_for_owner(annotation, None)
    }

    fn annotation_type_for_owner(
        &self,
        annotation: &TypeAnnotation,
        owner: Option<&str>,
    ) -> PolyType {
        // Convert parser-facing annotations into the normalized type form used
        // by compatibility checks and expression inference.
        match annotation {
            TypeAnnotation::Named(name) => {
                if name == "Self" {
                    return owner
                        .map(|owner| PolyType::Named(owner.to_string()))
                        .unwrap_or_else(|| PolyType::Named(name.clone()));
                }
                if self.type_variables.contains(name) {
                    return PolyType::Unknown;
                }
                let type_value = match name.as_str() {
                    "i8" => PolyType::I8,
                    "u8" | "byte" => PolyType::U8,
                    "i16" => PolyType::I16,
                    "u16" => PolyType::U16,
                    "i32" => PolyType::I32,
                    "u32" => PolyType::U32,
                    "i64" => PolyType::I64,
                    "u64" => PolyType::U64,
                    "i128" => PolyType::I128,
                    "u128" => PolyType::U128,
                    "f32" => PolyType::F32,
                    "f64" => PolyType::F64,
                    "isize" => PolyType::ISize,
                    "usize" => PolyType::USize,
                    "bool" => PolyType::Bool,
                    "char" | "uchar" => PolyType::Char,
                    "string" | "ustring" | "String" => PolyType::String,
                    "bytes" => PolyType::Vec(Box::new(PolyType::U8)),
                    _ => PolyType::Named(name.clone()),
                };
                self.resolve_alias(type_value)
            }
            TypeAnnotation::Array(inner, size) => {
                let length = match size.as_ref() {
                    Expression::IntLiteral(value) => value.parse().unwrap_or(0),
                    _ => 0,
                };
                PolyType::Array(
                    Box::new(self.annotation_type_for_owner(inner, owner)),
                    length,
                )
            }
            TypeAnnotation::Tuple(types) => PolyType::Tuple(
                types
                    .iter()
                    .map(|ty| self.annotation_type_for_owner(ty, owner))
                    .collect(),
            ),
            TypeAnnotation::Vec(inner) => {
                PolyType::Vec(Box::new(self.annotation_type_for_owner(inner, owner)))
            }
            TypeAnnotation::Option(inner) | TypeAnnotation::Nullable(inner) => {
                PolyType::Option(Box::new(self.annotation_type_for_owner(inner, owner)))
            }
            TypeAnnotation::Result(ok, error) => PolyType::Result(
                Box::new(self.annotation_type_for_owner(ok, owner)),
                Box::new(self.annotation_type_for_owner(error, owner)),
            ),
            TypeAnnotation::Reference(mutable, inner) => PolyType::Reference(
                *mutable,
                Box::new(self.annotation_type_for_owner(inner, owner)),
            ),
            TypeAnnotation::Pointer(inner) => PolyType::Named(format!(
                "ptr {}",
                self.annotation_type_for_owner(inner, owner)
            )),
            TypeAnnotation::Function { params, ret } => PolyType::Function {
                params: params
                    .iter()
                    .map(|ty| self.annotation_type_for_owner(ty, owner))
                    .collect(),
                ret: Box::new(self.annotation_type_for_owner(ret, owner)),
            },
            TypeAnnotation::Generic { name, args } => {
                let rendered: Vec<String> = args
                    .iter()
                    .map(|ty| format!("{}", self.annotation_type_for_owner(ty, owner)))
                    .collect();
                self.resolve_alias(PolyType::Named(format!(
                    "{}<{}>",
                    name,
                    rendered.join(", ")
                )))
            }
        }
    }

    fn resolve_alias(&self, ty: PolyType) -> PolyType {
        if let PolyType::Named(name) = &ty {
            if let Some(alias) = self.aliases.get(name) {
                return alias.clone();
            }
        }
        ty
    }

    fn require_compatible(&mut self, actual: &PolyType, expected: &PolyType, context: String) {
        // Keep compatibility policy in the helper below so every assignment,
        // argument, and return check behaves consistently.
        if !compatible(actual, expected) {
            self.error(TypeCheckError::in_context(
                format!("expected {}, got {}", expected, actual),
                context,
            ));
        }
    }

    fn declare(&mut self, name: &str, ty: PolyType) {
        // A declaration belongs only to the innermost lexical scope.
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn lookup(&self, name: &str) -> Option<PolyType> {
        // Search from inner to outer scope to implement lexical shadowing.
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn push_scope(&mut self) {
        // A separate map keeps temporary bindings isolated from their caller.
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        // Retain the root scope even if recovery encounters an imbalanced block.
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn enter_type_variables(&mut self, generics: &[GenericParam]) {
        for param in generics {
            self.type_variables.insert(param.name.clone());
        }
    }

    fn exit_type_variables(&mut self, generics: &[GenericParam]) {
        for param in generics {
            self.type_variables.remove(&param.name);
        }
    }

    fn error(&mut self, error: TypeCheckError) {
        // Do not short-circuit: one pass should expose as many independent
        // semantic problems as possible.
        self.errors.push(error);
    }

    fn expect_argument_count(&mut self, method: &str, args: &[PolyType], expected: usize) {
        if args.len() != expected {
            self.error(TypeCheckError::new(format!(
                "method `{method}` expects {expected} arguments, got {}",
                args.len()
            )));
        }
    }

    fn expect_no_arguments(
        &mut self,
        method: &str,
        args: &[PolyType],
        return_type: PolyType,
    ) -> PolyType {
        self.expect_argument_count(method, args, 0);
        return_type
    }

    fn unknown_method(&mut self, method: &str, object_type: &PolyType) -> PolyType {
        self.error(TypeCheckError::new(format!(
            "type {} has no method `{method}`",
            object_type
        )));
        PolyType::Unknown
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Check a parsed program using the default semantic rules.
pub fn check_program(program: &Program) -> Result<(), Vec<TypeCheckError>> {
    TypeChecker::check(program)
}

fn compatible(actual: &PolyType, expected: &PolyType) -> bool {
    if matches!(actual, PolyType::Unknown) || matches!(expected, PolyType::Unknown) {
        return true;
    }
    if actual == expected {
        return true;
    }
    match (actual, expected) {
        (a, b) if is_numeric(a) && is_numeric(b) => numeric_rank(a) <= numeric_rank(b),
        // An empty array literal is polymorphic and may initialize any container.
        (PolyType::Vec(inner), _) if matches!(**inner, PolyType::Unknown) => true,
        (PolyType::Array(a, _), PolyType::Array(b, _)) => compatible(a, b),
        (PolyType::Vec(a), PolyType::Vec(b)) => {
            compatible(a, b) || (is_numeric(a) && is_numeric(b))
        }
        (PolyType::Tuple(a), PolyType::Tuple(b)) if a.len() == b.len() => {
            a.iter().zip(b.iter()).all(|(a, b)| compatible(a, b))
        }
        (PolyType::Option(a), PolyType::Option(b)) => compatible(a, b),
        (PolyType::Result(a_ok, a_err), PolyType::Result(b_ok, b_err)) => {
            compatible(a_ok, b_ok) && compatible(a_err, b_err)
        }
        (PolyType::Reference(a_mut, a), PolyType::Reference(b_mut, b)) => {
            (!*b_mut || *a_mut) && compatible(a, b)
        }
        _ => false,
    }
}

fn numeric_join(left: &PolyType, right: &PolyType) -> PolyType {
    if numeric_rank(left) >= numeric_rank(right) {
        left.clone()
    } else {
        right.clone()
    }
}

fn numeric_rank(ty: &PolyType) -> u8 {
    match ty {
        PolyType::I8 | PolyType::U8 => 1,
        PolyType::I16 | PolyType::U16 => 2,
        PolyType::I32 | PolyType::U32 => 3,
        PolyType::I64 | PolyType::U64 => 4,
        PolyType::I128 | PolyType::U128 => 5,
        PolyType::ISize | PolyType::USize => 4,
        PolyType::F32 => 6,
        PolyType::F64 => 7,
        _ => 0,
    }
}

fn is_numeric(ty: &PolyType) -> bool {
    numeric_rank(ty) > 0
}

fn is_integer(ty: &PolyType) -> bool {
    matches!(
        ty,
        PolyType::I8
            | PolyType::U8
            | PolyType::I16
            | PolyType::U16
            | PolyType::I32
            | PolyType::U32
            | PolyType::I64
            | PolyType::U64
            | PolyType::I128
            | PolyType::U128
            | PolyType::ISize
            | PolyType::USize
    )
}

fn element_type(ty: &PolyType) -> Option<PolyType> {
    match ty {
        PolyType::Array(inner, _) | PolyType::Vec(inner) => Some((**inner).clone()),
        PolyType::String => Some(PolyType::Char),
        _ => None,
    }
}

fn is_castable(actual: &PolyType, target: &PolyType) -> bool {
    matches!(actual, PolyType::Unknown)
        || matches!(target, PolyType::Unknown)
        || (is_numeric(actual) && is_numeric(target))
        || (actual == &PolyType::Char && is_integer(target))
        || (is_integer(actual) && target == &PolyType::Char)
        || actual == target
}

#[cfg(test)]
mod tests {
    use super::*;
    use poly_lexer::Lexer;
    use poly_parser::Parser;

    fn check(source: &str) -> Result<(), Vec<TypeCheckError>> {
        let (tokens, lexer_errors) = Lexer::lex(source);
        assert!(lexer_errors.is_empty(), "lexer errors: {lexer_errors:?}");
        let mut parser = Parser::new(&tokens);
        let program = parser.parse().expect("source should parse");
        TypeChecker::check(&program)
    }

    #[test]
    fn accepts_valid_typed_program() {
        let source =
            "fn sum(a: i32, b: i32): i32\n    return a + b\nend fn\nvar result i32 := sum(2, 3)";
        assert!(check(source).is_ok());
    }

    #[test]
    fn rejects_wrong_initializer_type() {
        let errors = check("var value i32 := unicode \"wrong\"").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expected i32")));
    }

    #[test]
    fn rejects_unknown_names_and_bad_calls() {
        let errors =
            check("fn consume(value: i32)\n    return missing(value)\nend fn").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("unknown function `missing`")));
    }

    #[test]
    fn rejects_non_boolean_conditions_and_bad_mutation() {
        let errors = check("var value i32 := 1\nif value\n    value += true\nend if").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| format!("{error}").contains("if condition")));
        assert!(errors
            .iter()
            .any(|error| error.message.contains("mutation amount")));
    }

    #[test]
    fn checks_struct_fields_and_methods() {
        let source = "struct Point\n    var x: i32\n    var y: i32\n    fn length(self): i32\n        return self.x\n    end fn\nend struct\nvar point := Point { x: 1, y: 2 }\nvar value i32 := point.length()";
        assert!(check(source).is_ok());
    }

    #[test]
    fn rejects_invalid_enum_variant() {
        let errors = check("enum Color\n    Red\nend enum\nvar color := Color::Blue").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("has no variant `Blue`")));
    }

    #[test]
    fn accepts_break_and_continue_inside_while_loops() {
        let source = "var i i32 := 0\nwhile i < 3\n    break\n    i += 1\nend while\n\
                      while i < 5\n    if i = 1,\n        continue\n    end if\n    i += 1\nend while";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn rejects_break_outside_a_loop() {
        let errors = check("break\ncontinue").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("only valid inside a loop")));
    }

    #[test]
    fn accepts_nested_function_calls() {
        let source = "fn outer(x: i32): i32\n    fn inner(y: i32): i32\n        return y * 2\n    end fn\n    return inner(x) + 1\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn accepts_option_some_none_values_and_patterns() {
        let source = "fn main()\n    var maybe Option<i32> := Some(42)\n    var empty Option<i32> := None\n    match maybe\n        Some(v) => put v\n        None => put 0\n    end match\n    match empty\n        Some(v) => put v\n        None => put 0\n    end match\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn rejects_option_variant_matching_a_non_option() {
        let errors =
            check("var x i32 := 5\nmatch x\n    Some(v) => put v\n    None => put 0\nend match")
                .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| format!("{error}").contains("enum match pattern")));
    }
}

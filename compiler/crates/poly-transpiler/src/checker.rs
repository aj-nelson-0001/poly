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
    // Generic parameter names (`T` in `fn identity<T>(x: T): T`) declared by
    // this function. `declared_params`/`declared_return` keep those names as
    // `PolyType::Named` markers (instead of lowering them to `Unknown`) so call
    // sites can substitute the concrete argument types into the return type.
    generics: Vec<String>,
    declared_params: Vec<PolyType>,
    declared_return: Option<PolyType>,
    // True for functions supplied by a foreign target block or declaration.
    is_foreign: bool,
    // Explicit `extern <target> fn ...` declarations opt into Poly-side
    // arity/type validation while the native compiler remains authoritative.
    has_explicit_signature: bool,
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
    // Non-fatal diagnostics (e.g. experimental runtime APIs) collected while
    // checking; they do not prevent code generation.
    warnings: Vec<String>,
    // Nested function checks use this stack to validate each return statement.
    return_types: Vec<Option<PolyType>>,
    // Used to reject break/continue outside loop bodies.
    loop_depth: usize,
    // Generic type parameter names currently in scope (e.g. `T` in `fn id<T>`).
    // They lower to an unresolved, polymorphic type.
    type_variables: std::collections::HashSet<String>,
    // While true, `annotation_type_for_owner` keeps type-variable names as
    // `PolyType::Named(name)` markers instead of `Unknown`, so declared
    // function signatures can be unified against concrete argument types.
    keep_type_variable_names: bool,
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
            warnings: Vec::new(),
            return_types: Vec::new(),
            loop_depth: 0,
            type_variables: std::collections::HashSet::new(),
            keep_type_variable_names: false,
        }
    }

    /// Check a complete program and return every semantic error found.
    pub fn check(program: &Program) -> Result<(), Vec<TypeCheckError>> {
        Self::check_with_warnings(program).0
    }

    /// Check a complete program, returning errors and any non-fatal warnings.
    pub fn check_with_warnings(
        program: &Program,
    ) -> (Result<(), Vec<TypeCheckError>>, Vec<String>) {
        let mut checker = Self::new();
        checker.check_program(program);
        let warnings = std::mem::take(&mut checker.warnings);
        if checker.errors.is_empty() {
            (Ok(()), warnings)
        } else {
            (Err(checker.errors), warnings)
        }
    }

    /// Check a complete program with a reusable checker instance.
    pub fn check_program(&mut self, program: &Program) {
        // Register functions defined in #rust blocks so the checker
        // accepts calls to them without type information.
        let statements: Vec<&Statement> = program.statements.iter().map(|s| &s.node).collect();
        self.register_foreign_functions(&statements);
        self.register_extern_functions(&statements);

        // Declaration collection is intentionally a separate pass: expression
        // checking can then resolve forward references and recursive functions.
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

    /// Extract function names from `#rust` blocks and register them as
    /// known functions with `Unknown` types. This allows Poly code to call
    /// functions defined in foreign blocks without type-checking errors.
    fn register_foreign_functions(&mut self, statements: &[&Statement]) {
        for statement in statements {
            if let Statement::ForeignBlock {
                language: _,
                content,
            } = statement
            {
                // Register simple function declarations from any selected
                // foreign backend. Their native signatures remain opaque to
                // Poly; the target compiler validates the call itself.
                for line in content.lines() {
                    let trimmed = line.trim();
                    let name = if let Some(after_fn) = trimmed
                        .strip_prefix("pub fn ")
                        .or_else(|| trimmed.strip_prefix("async fn "))
                        .or_else(|| trimmed.strip_prefix("fn "))
                    {
                        after_fn
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect()
                    } else if trimmed.contains('(')
                        && !trimmed.starts_with("if ")
                        && !trimmed.starts_with("for ")
                        && !trimmed.starts_with("while ")
                        && !trimmed.starts_with("switch ")
                    {
                        trimmed[..trimmed.find('(').unwrap_or(0)]
                            .split_whitespace()
                            .last()
                            .unwrap_or("")
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect()
                    } else {
                        String::new()
                    };
                    if !name.is_empty() && name != "main" {
                        self.functions.insert(
                            name,
                            FunctionSignature {
                                params: vec![],
                                has_self: false,
                                return_type: Some(PolyType::Unknown),
                                generics: vec![],
                                declared_params: vec![],
                                declared_return: Some(PolyType::Unknown),
                                is_foreign: true,
                                has_explicit_signature: false,
                            },
                        );
                    }
                }
            }
        }
    }

    fn register_extern_functions(&mut self, statements: &[&Statement]) {
        for statement in statements {
            if let Statement::ExternFunctionDeclaration(declaration) = statement {
                let signature = FunctionSignature {
                    params: declaration
                        .params
                        .iter()
                        .map(|parameter| self.annotation_type(&parameter.ty))
                        .collect(),
                    has_self: false,
                    return_type: declaration
                        .return_type
                        .as_ref()
                        .map(|ty| self.annotation_type(ty)),
                    generics: Vec::new(),
                    declared_params: declaration
                        .params
                        .iter()
                        .map(|parameter| self.annotation_type(&parameter.ty))
                        .collect(),
                    declared_return: declaration
                        .return_type
                        .as_ref()
                        .map(|ty| self.annotation_type(ty)),
                    is_foreign: true,
                    has_explicit_signature: true,
                };
                match self.functions.get(&declaration.name) {
                    Some(existing) if existing.is_foreign && !existing.has_explicit_signature => {
                        self.functions.insert(declaration.name.clone(), signature);
                    }
                    Some(_) => self.error(TypeCheckError::new(format!(
                        "duplicate foreign function declaration `{}`",
                        declaration.name
                    ))),
                    None => {
                        self.functions.insert(declaration.name.clone(), signature);
                    }
                }
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
                Statement::ExternFunctionDeclaration(_) => {}
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
            Statement::Assignment { target, value } => {
                let target_type = self.check_lvalue(target);
                let value_type = self.check_expression(value);
                self.require_compatible(&value_type, &target_type, "assignment".to_string());
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
            | Statement::TypeDeclaration(_)
            | Statement::ForeignBlock { .. }
            | Statement::ExternFunctionDeclaration(_) => {}
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
            .map(|value| match &expected {
                // Closure literals returned from a function-typed return
                // inherit the declared signature, so `return |x| x + n`
                // checks against `: |x: i32| i32`.
                Some(expected) => self.check_argument(value, expected),
                None => self.check_expression(value),
            })
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
            Expression::Index { object, index } => self.check_index(object, index),
            Expression::TupleIndex { object, index } => self.check_tuple_index(object, *index),
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
            Expression::Index { object, index } => self.check_index(object, index),
            Expression::FieldAccess { object, field } => self.check_field_access(object, field),
            Expression::TupleIndex { object, index } => self.check_tuple_index(object, *index),
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
                    self.check_pattern(&arm.pattern, &scrutinee_type, true);
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
                // `for (a, b) in collection` destructures tuple elements, so
                // each name is declared with the matching component type (the
                // same handling as `loop: (a, b) in ...`).
                let iterable_type = self.check_expression(iterable);
                let element_type = match &iterable_type {
                    PolyType::Vec(inner) => (**inner).clone(),
                    PolyType::Iterator(inner) => (**inner).clone(),
                    PolyType::String => PolyType::Char,
                    _ => PolyType::Unknown,
                };
                self.push_scope();
                let binding_names = split_loop_binding(variable);
                if binding_names.len() == 1 {
                    self.declare(&binding_names[0], element_type);
                } else {
                    match &element_type {
                        PolyType::Tuple(components) => {
                            for (index, name) in binding_names.iter().enumerate() {
                                let component =
                                    components.get(index).cloned().unwrap_or(PolyType::Unknown);
                                self.declare(name, component);
                            }
                        }
                        _ => {
                            for name in &binding_names {
                                self.declare(name, PolyType::Unknown);
                            }
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
            Expression::LoopRange {
                variable,
                ranges,
                body,
            } => {
                let mut variable_type = PolyType::Unknown;
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
                                // A constant zero step would loop forever (or
                                // panic in the generated `step_by(0)`); reject
                                // it up front.
                                if let Expression::IntLiteral(value) = step {
                                    if value.parse::<i64>().ok() == Some(0) {
                                        self.error(TypeCheckError::new(
                                            "loop range step must not be zero",
                                        ));
                                    }
                                }
                            }
                            if matches!(variable_type, PolyType::Unknown) {
                                variable_type = PolyType::I32;
                            }
                        }
                        LoopRangePart::Value(value) => {
                            let value_type = self.check_expression(value);
                            let element_type = match value_type {
                                PolyType::Vec(inner) => *inner,
                                PolyType::Iterator(inner) => *inner,
                                PolyType::String => PolyType::Char,
                                other => other,
                            };
                            if matches!(variable_type, PolyType::Unknown) {
                                variable_type = element_type;
                            }
                        }
                    }
                }
                self.push_scope();
                // `loop: (a, b) in collection` carries a tuple pattern in the
                // variable; declare each name with the matching component type
                // (or `Unknown` when the element type is not yet known, e.g.
                // `collection.enumerate()`).
                let binding_names = split_loop_binding(variable);
                if binding_names.len() == 1 {
                    self.declare(&binding_names[0], variable_type);
                } else {
                    match &variable_type {
                        PolyType::Tuple(components) => {
                            for (index, name) in binding_names.iter().enumerate() {
                                let component =
                                    components.get(index).cloned().unwrap_or(PolyType::Unknown);
                                self.declare(name, component);
                            }
                        }
                        _ => {
                            for name in &binding_names {
                                self.declare(name, PolyType::Unknown);
                            }
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
            Expression::InfiniteLoop(body) => {
                self.push_scope();
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
                } else if matches!(left, PolyType::Unknown) || matches!(right, PolyType::Unknown) {
                    // Unknown operands (e.g. untyped closure parameters) are
                    // permissive: assume the operation is well-typed and let
                    // the expected signature pin the types down later.
                    if is_numeric(right) {
                        right.clone()
                    } else if is_numeric(left) {
                        left.clone()
                    } else {
                        PolyType::Unknown
                    }
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
                } else if matches!(left, PolyType::Unknown) || matches!(right, PolyType::Unknown) {
                    if is_numeric(right) {
                        right.clone()
                    } else if is_numeric(left) {
                        left.clone()
                    } else {
                        PolyType::Unknown
                    }
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
                    || (*left == PolyType::String && *right == PolyType::String)
                    || matches!(left, PolyType::Unknown)
                    || matches!(right, PolyType::Unknown))
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
        // Resolve the callee's declared parameter types up front so that
        // closure-literal arguments can be checked against the expected
        // signature instead of being inferred from the literal alone.
        let expected_params: Option<Vec<PolyType>> = match function {
            Expression::Identifier(name) => {
                if let Some(signature) = self.functions.get(name) {
                    Some(signature.params.clone())
                } else if let Some(PolyType::Function { params, .. }) = self.lookup(name) {
                    Some(params.clone())
                } else {
                    None
                }
            }
            _ => None,
        };
        // Check arguments even when callee lookup fails so diagnostics remain
        // useful for nested expressions.
        let argument_types: Vec<PolyType> = args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                match expected_params
                    .as_ref()
                    .and_then(|params| params.get(index))
                {
                    Some(expected) => self.check_argument(arg, expected),
                    None => self.check_expression(arg),
                }
            })
            .collect();
        if let Expression::Identifier(name) = function {
            if let Some(signature) = self.functions.get(name).cloned() {
                if signature.generics.is_empty() {
                    self.check_arguments(name, &signature, &argument_types);
                    return signature.return_type.unwrap_or(PolyType::Unknown);
                }
                // Generic function: unify the declared parameter types with the
                // concrete argument types, then substitute the bindings into
                // the declared return type so `identity(42)` yields `i32`
                // instead of an opaque `Unknown`.
                let mut bindings: HashMap<String, PolyType> = HashMap::new();
                for (declared, actual) in
                    signature.declared_params.iter().zip(argument_types.iter())
                {
                    unify_declared(declared, actual, &signature.generics, &mut bindings);
                }
                // Opaque foreign functions remain permissive. Explicit extern
                // declarations are checked like Poly signatures.
                if !signature.is_foreign || signature.has_explicit_signature {
                    if signature.declared_params.len() != argument_types.len() {
                        self.error(TypeCheckError::new(format!(
                            "function `{name}` expects {} arguments, got {}",
                            signature.declared_params.len(),
                            argument_types.len()
                        )));
                    }
                    // Check each argument against its substituted declared type.
                    for (declared, actual) in
                        signature.declared_params.iter().zip(argument_types.iter())
                    {
                        let expected = substitute_generic(declared, &signature.generics, &bindings);
                        self.require_compatible(actual, &expected, format!("argument to `{name}`"));
                    }
                }
                return signature
                    .declared_return
                    .as_ref()
                    .map(|ret| substitute_generic(ret, &signature.generics, &bindings))
                    .unwrap_or(PolyType::Unknown);
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
            "delay" => PolyType::Unknown,
            // Network/database builtins: `http_get` and `tcp_connect` are real
            // (minimal) tokio TCP implementations, and `db_execute` runs SQL
            // against a shared in-memory SQLite database (rusqlite), returning
            // one string per row. Programs using it need the rusqlite crate in
            // the generated project.
            "http_get" => PolyType::Result(Box::new(PolyType::String), Box::new(PolyType::String)),
            "tcp_connect" => {
                PolyType::Result(Box::new(PolyType::String), Box::new(PolyType::String))
            }
            "db_execute" => {
                if args.len() != 1 {
                    self.error(TypeCheckError::new(
                        "`db_execute` expects one query argument",
                    ));
                }
                self.require_compatible(
                    args.first().unwrap_or(&PolyType::Unknown),
                    &PolyType::String,
                    "`db_execute` query".to_string(),
                );
                PolyType::Vec(Box::new(PolyType::String))
            }
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
            "assert" => {
                // `assert(cond)` fails the program when the condition is
                // false; the argument must be a boolean expression.
                if args.len() != 1 {
                    self.error(TypeCheckError::new("`assert` expects one boolean argument"));
                }
                self.require_compatible(
                    args.first().unwrap_or(&PolyType::Unknown),
                    &PolyType::Bool,
                    "`assert` condition".to_string(),
                );
                PolyType::Unknown
            }
            "pass" | "fail" => {
                // Mini test-framework helpers: `pass(msg)` prints a PASS
                // marker, `fail(msg)` prints FAIL and exits non-zero.
                if args.len() != 1 {
                    self.error(TypeCheckError::new(format!(
                        "`{name}` expects one message argument"
                    )));
                }
                self.require_compatible(
                    args.first().unwrap_or(&PolyType::Unknown),
                    &PolyType::String,
                    format!("`{name}` message"),
                );
                PolyType::Unknown
            }
            "spawn_task" => {
                // `spawn <expr>` lowers to `tokio::spawn(<expr>)`, which yields
                // a `JoinHandle`; nothing in the language consumes it, so the
                // call site type is opaque.
                PolyType::Unknown
            }
            "exit" => PolyType::Unknown,
            _ => {
                if let Some(variant_type) = self.enum_variant_type(name) {
                    return variant_type;
                }
                self.error(TypeCheckError::new(format!("unknown function `{name}`")));
                PolyType::Unknown
            }
        }
    }

    /// Check an argument expression, using an expected type when available.
    ///
    /// Closure literals passed to function-typed parameters inherit the
    /// parameter types from the expected signature, so `apply(|x| x * 2, 21)`
    /// type-checks against `fn apply(f: |x: i32| i32, ...)` even though the
    /// literal itself is untyped.
    fn check_argument(&mut self, arg: &Expression, expected: &PolyType) -> PolyType {
        if let (
            Expression::Closure { params, body },
            PolyType::Function {
                params: expected_params,
                ..
            },
        ) = (arg, expected)
        {
            self.push_scope();
            let mut parameter_types = Vec::new();
            for (index, parameter) in params.iter().enumerate() {
                let ty = match expected_params.get(index) {
                    // An untyped parameter inherits the declared type.
                    Some(expected_ty) if matches!(&parameter.ty, TypeAnnotation::Named(name) if name == "_") => {
                        expected_ty.clone()
                    }
                    // Parameters beyond the expected list keep their own
                    // annotation; the arity mismatch is reported later when
                    // the inferred closure type is compared with the expected
                    // function type.
                    _ => self.annotation_type(&parameter.ty),
                };
                parameter_types.push(ty.clone());
                self.declare(&parameter.name, ty);
            }
            let return_type = self.check_expression(body);
            self.pop_scope();
            PolyType::Function {
                params: parameter_types,
                ret: Box::new(return_type),
            }
        } else {
            self.check_expression(arg)
        }
    }

    fn check_arguments(&mut self, name: &str, signature: &FunctionSignature, args: &[PolyType]) {
        // Opaque foreign functions remain permissive; explicit extern
        // declarations opt into normal interface checking.
        if signature.is_foreign && !signature.has_explicit_signature {
            return;
        }
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

    /// Check a collection higher-order method call (`map`, `filter`, `reduce`,
    /// `sort_by`) on vectors (element type `element`) or strings (`Char`).
    ///
    /// The callback argument is checked against a signature derived from the
    /// element type (and the initial value for `reduce`), so untyped closure
    /// literals like `xs.map(|x| x * 2)` inherit `x: i32`.
    fn check_hof_method(
        &mut self,
        method: &str,
        element: &PolyType,
        args: &[Expression],
    ) -> PolyType {
        match method {
            "map" => {
                if args.len() != 1 {
                    self.error(TypeCheckError::new("`map` expects one callback argument"));
                }
                let ret = args.first().and_then(|arg| {
                    self.check_hof_callback(
                        method,
                        &PolyType::Function {
                            params: vec![element.clone()],
                            ret: Box::new(PolyType::Unknown),
                        },
                        arg,
                        1,
                    )
                });
                PolyType::Vec(Box::new(ret.unwrap_or(PolyType::Unknown)))
            }
            "filter" => {
                if args.len() != 1 {
                    self.error(TypeCheckError::new(
                        "`filter` expects one callback argument",
                    ));
                }
                let ret = args.first().and_then(|arg| {
                    self.check_hof_callback(
                        method,
                        &PolyType::Function {
                            params: vec![element.clone()],
                            ret: Box::new(PolyType::Bool),
                        },
                        arg,
                        1,
                    )
                });
                if let Some(ret) = &ret {
                    self.require_compatible(ret, &PolyType::Bool, "filter callback".to_string());
                }
                PolyType::Vec(Box::new(element.clone()))
            }
            "reduce" => {
                if args.len() != 2 {
                    self.error(TypeCheckError::new(
                        "`reduce` expects an initial value and a callback",
                    ));
                }
                let init_type = match args.first() {
                    Some(init) => self.check_expression(init),
                    None => PolyType::Unknown,
                };
                let ret = args.get(1).and_then(|arg| {
                    self.check_hof_callback(
                        method,
                        &PolyType::Function {
                            params: vec![init_type.clone(), element.clone()],
                            ret: Box::new(init_type.clone()),
                        },
                        arg,
                        2,
                    )
                });
                if let Some(ret) = &ret {
                    self.require_compatible(ret, &init_type, "reduce accumulator".to_string());
                }
                init_type
            }
            "sort_by" => {
                if args.len() != 1 {
                    self.error(TypeCheckError::new(
                        "`sort_by` expects one comparator argument",
                    ));
                }
                let ret = args.first().and_then(|arg| {
                    self.check_hof_callback(
                        method,
                        &PolyType::Function {
                            params: vec![element.clone(), element.clone()],
                            ret: Box::new(PolyType::Bool),
                        },
                        arg,
                        2,
                    )
                });
                if let Some(ret) = &ret {
                    self.require_compatible(ret, &PolyType::Bool, "sort_by comparator".to_string());
                }
                PolyType::Vec(Box::new(element.clone()))
            }
            _ => unreachable!(),
        }
    }

    /// Validate a `map` callback on an iterator and return its result type.
    fn check_iterator_map(&mut self, element: &PolyType, args: &[Expression]) -> Option<PolyType> {
        if args.len() != 1 {
            self.error(TypeCheckError::new("`map` expects one callback argument"));
            return None;
        }
        args.first().and_then(|arg| {
            self.check_hof_callback(
                "map",
                &PolyType::Function {
                    params: vec![element.clone()],
                    ret: Box::new(PolyType::Unknown),
                },
                arg,
                1,
            )
        })
    }

    /// Validate a `filter` callback on an iterator (must return `bool`).
    fn check_iterator_filter(&mut self, element: &PolyType, args: &[Expression]) {
        if args.len() != 1 {
            self.error(TypeCheckError::new(
                "`filter` expects one callback argument",
            ));
            return;
        }
        if let Some(ret) = args.first().and_then(|arg| {
            self.check_hof_callback(
                "filter",
                &PolyType::Function {
                    params: vec![element.clone()],
                    ret: Box::new(PolyType::Bool),
                },
                arg,
                1,
            )
        }) {
            self.require_compatible(&ret, &PolyType::Bool, "filter callback".to_string());
        }
    }

    /// Check a single callback argument against an expected function type and
    /// return its return type, enforcing the expected parameter count.
    fn check_hof_callback(
        &mut self,
        method: &str,
        expected: &PolyType,
        arg: &Expression,
        expected_param_count: usize,
    ) -> Option<PolyType> {
        let callback_type = self.check_argument(arg, expected);
        match &callback_type {
            PolyType::Function { params, ret } => {
                if params.len() != expected_param_count {
                    let noun = if expected_param_count == 1 {
                        "parameter"
                    } else {
                        "parameters"
                    };
                    self.error(TypeCheckError::new(format!(
                        "`{method}` expects a callback with {expected_param_count} {noun}, got {}",
                        callback_type
                    )));
                }
                Some((**ret).clone())
            }
            PolyType::Unknown => None,
            other => {
                self.error(TypeCheckError::new(format!(
                    "`{method}` expects a function callback, got {}",
                    other
                )));
                None
            }
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
        // Container higher-order methods receive an expected callback
        // signature so untyped closure literals inherit the element (and
        // accumulator) types instead of being inferred from the literal alone.
        // Vectors use their element type; strings iterate over `Char`.
        if matches!(method, "map" | "filter" | "reduce" | "sort_by") {
            match &object_type {
                PolyType::Vec(inner) => return self.check_hof_method(method, inner, args),
                // Iterator map/filter stay lazy iterators (the chain is
                // consumed by a later `.sum()`/`.collect()` or a loop).
                PolyType::Iterator(inner) => match method {
                    "map" => {
                        if args.len() != 1 {
                            self.error(TypeCheckError::new("`map` expects one callback argument"));
                            return PolyType::Unknown;
                        }
                        return self
                            .check_iterator_map(inner, args)
                            .map(|t| PolyType::Iterator(Box::new(t)))
                            .unwrap_or(PolyType::Unknown);
                    }
                    "filter" => {
                        if args.len() != 1 {
                            self.error(TypeCheckError::new(
                                "`filter` expects one callback argument",
                            ));
                            return PolyType::Unknown;
                        }
                        self.check_iterator_filter(inner, args);
                        return PolyType::Iterator(Box::new((**inner).clone()));
                    }
                    _ => {
                        for arg in args {
                            self.check_expression(arg);
                        }
                        return self.unknown_method(method, &object_type);
                    }
                },
                PolyType::String if method != "sort_by" => {
                    return self.check_hof_method(method, &PolyType::Char, args);
                }
                PolyType::Unknown => {
                    for arg in args {
                        self.check_expression(arg);
                    }
                    return PolyType::Unknown;
                }
                _ => {
                    for arg in args {
                        self.check_expression(arg);
                    }
                    return self.unknown_method(method, &object_type);
                }
            }
        }
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
                "repeat" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    if let Some(count) = argument_types.first() {
                        if !is_integer(count) {
                            self.error(TypeCheckError::new(format!(
                                "`repeat` count must be an integer, got {}",
                                count
                            )));
                        }
                    }
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
                // Checked (non-panicking) indexing: `text.get(i)` returns
                // `Option<char>` instead of panicking on a bad index.
                "get" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    if let Some(index) = argument_types.first() {
                        if !is_integer(index) {
                            self.error(TypeCheckError::new(format!(
                                "`get` index must be an integer, got {}",
                                index
                            )));
                        }
                    }
                    PolyType::Option(Box::new(PolyType::Char))
                }
                _ => self.unknown_method(method, &object_type),
            },
            PolyType::Vec(inner) => match method {
                // `xs.iter()` yields an iterator over owned elements (codegen
                // emits `iter().cloned()`), so chained `.map`/`.filter`/
                // `.sum` and `loop: x in xs.iter()` all see the element type.
                "iter" => {
                    self.expect_no_arguments(method, &argument_types, PolyType::Unknown);
                    PolyType::Iterator(Box::new((**inner).clone()))
                }
                "len" => self.expect_no_arguments(method, &argument_types, PolyType::I32),
                "clear" => self.expect_no_arguments(method, &argument_types, PolyType::Unknown),
                "reserve" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    if let Some(argument) = argument_types.first() {
                        if !is_integer(argument) {
                            self.error(TypeCheckError::new(format!(
                                "`reserve` capacity must be an integer, got {}",
                                argument
                            )));
                        }
                    }
                    PolyType::Unknown
                }
                "push" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    if let Some(argument) = argument_types.first() {
                        self.require_compatible(argument, inner, "vector element".to_string());
                    }
                    PolyType::Unknown
                }
                "enumerate" => PolyType::Unknown,
                "join" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    if let Some(separator) = argument_types.first() {
                        self.require_compatible(
                            separator,
                            &PolyType::String,
                            "`join` separator".to_string(),
                        );
                    }
                    PolyType::String
                }
                // Checked (non-panicking) indexing: `xs.get(i)` returns
                // `Option<T>` instead of panicking on an out-of-bounds index.
                "get" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    if let Some(index) = argument_types.first() {
                        if !is_integer(index) {
                            self.error(TypeCheckError::new(format!(
                                "`get` index must be an integer, got {}",
                                index
                            )));
                        }
                    }
                    PolyType::Option(Box::new((**inner).clone()))
                }
                _ => self.unknown_method(method, &object_type),
            },
            PolyType::Iterator(inner) => match method {
                // Iterator chains over owned elements: `.sum()` collapses to
                // the element type, `.collect()` materializes a vector.
                "sum" => {
                    self.expect_no_arguments(method, &argument_types, PolyType::Unknown);
                    if !is_numeric(inner) {
                        self.error(TypeCheckError::new(format!(
                            "`sum` requires a numeric element type, got {}",
                            inner
                        )));
                    }
                    (**inner).clone()
                }
                "collect" => {
                    self.expect_no_arguments(method, &argument_types, PolyType::Unknown);
                    PolyType::Vec(Box::new((**inner).clone()))
                }
                "enumerate" => PolyType::Unknown,
                _ => self.unknown_method(method, &object_type),
            },
            PolyType::Result(ok, _error) => match method {
                // Test-framework accessors on `Result` values.
                "is_ok" => self.expect_no_arguments(method, &argument_types, PolyType::Bool),
                "is_error" => self.expect_no_arguments(method, &argument_types, PolyType::Bool),
                "unwrap" => self.expect_no_arguments(method, &argument_types, (**ok).clone()),
                "unwrap_or" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    if let Some(fallback) = argument_types.first() {
                        self.require_compatible(fallback, ok, "`unwrap_or` fallback".to_string());
                    }
                    (**ok).clone()
                }
                _ => self.unknown_method(method, &object_type),
            },
            PolyType::Map(key_type, value_type) => match method {
                "len" => self.expect_no_arguments(method, &argument_types, PolyType::I32),
                "is_empty" => self.expect_no_arguments(method, &argument_types, PolyType::Bool),
                "insert" => {
                    self.expect_argument_count(method, &argument_types, 2);
                    if argument_types.len() == 2 {
                        self.require_compatible(
                            &argument_types[0],
                            key_type,
                            "map key".to_string(),
                        );
                        self.require_compatible(
                            &argument_types[1],
                            value_type,
                            "map value".to_string(),
                        );
                    }
                    PolyType::Unknown
                }
                "get" | "remove" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    if let Some(key) = argument_types.first() {
                        self.require_compatible(key, key_type, "map key".to_string());
                    }
                    PolyType::Option(Box::new((**value_type).clone()))
                }
                "contains_key" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    if let Some(key) = argument_types.first() {
                        self.require_compatible(key, key_type, "map key".to_string());
                    }
                    PolyType::Bool
                }
                "iter" => PolyType::Unknown,
                _ => self.unknown_method(method, &object_type),
            },
            PolyType::Set(inner) => match method {
                "len" => self.expect_no_arguments(method, &argument_types, PolyType::I32),
                "is_empty" => self.expect_no_arguments(method, &argument_types, PolyType::Bool),
                "insert" | "contains" | "remove" => {
                    self.expect_argument_count(method, &argument_types, 1);
                    if let Some(element) = argument_types.first() {
                        self.require_compatible(element, inner, "set element".to_string());
                    }
                    PolyType::Bool
                }
                "iter" => PolyType::Unknown,
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

    fn check_index(&mut self, object: &Expression, index: &Expression) -> PolyType {
        let object_type = self.check_expression(object);
        let index_type = self.check_expression(index);
        // Map indexing uses the key type rather than an integer index;
        // `m[key]` reads (and `m[key] = value` assigns) through the map.
        if let PolyType::Map(key_type, value_type) = &object_type {
            self.require_compatible(&index_type, key_type, "map key".to_string());
            return (**value_type).clone();
        }
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

    fn check_tuple_index(&mut self, object: &Expression, index: usize) -> PolyType {
        let object_type = self.check_expression(object);
        if let PolyType::Tuple(types) = &object_type {
            if let Some(ty) = types.get(index) {
                return ty.clone();
            }
            self.error(TypeCheckError::new(format!(
                "tuple index {index} out of bounds (has {} elements)",
                types.len()
            )));
            return PolyType::Unknown;
        }
        if matches!(object_type, PolyType::Unknown) {
            PolyType::Unknown
        } else {
            self.error(TypeCheckError::new(format!(
                "type {} is not a tuple and cannot be indexed with `.{index}`",
                object_type
            )));
            PolyType::Unknown
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

    fn check_pattern(&mut self, pattern: &Pattern, scrutinee_type: &PolyType, top_level: bool) {
        // Pattern checking validates both the pattern's shape and any literal
        // or enum payload types against the match scrutinee.
        match pattern {
            Pattern::Wildcard => {}
            Pattern::Literal(expression) => {
                // Range expressions in pattern position (`2..=9`) are range
                // patterns; checking them as ordinary literals infers a
                // vector-ish type and rejects valid matches.
                if let Expression::Range {
                    start,
                    end,
                    inclusive: _,
                } = expression
                {
                    let start_type = self.check_expression(start);
                    let end_type = self.check_expression(end);
                    if !is_integer(&start_type) || !is_integer(&end_type) {
                        self.error(TypeCheckError::new("range pattern bounds must be integers"));
                    }
                    self.require_compatible(
                        scrutinee_type,
                        &start_type,
                        "range pattern".to_string(),
                    );
                    return;
                }
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
                } else if top_level {
                    // An unknown name on an enum/struct/Result scrutinee is
                    // almost certainly a typo'd variant; accepting it as a
                    // binding would silently swallow every other value. Names
                    // nested inside payloads (`Add(l, r)`) stay bindings.
                    match scrutinee_type {
                        PolyType::Named(type_name) if self.enums.contains_key(type_name) => {
                            self.error(TypeCheckError::new(format!(
                                "`{name}` is not a variant of enum `{type_name}`"
                            )));
                        }
                        PolyType::Named(type_name) if self.structs.contains_key(type_name) => {
                            self.error(TypeCheckError::new(format!(
                                "`{name}` cannot match a value of struct type `{type_name}`"
                            )));
                        }
                        PolyType::Result(_, _)
                            if !matches!(name.as_str(), "Ok" | "Error" | "Timeout") =>
                        {
                            self.error(TypeCheckError::new(format!(
                                "`{name}` is not a Result variant (expected Ok, Error, or Timeout)"
                            )));
                        }
                        _ => self.declare(name, scrutinee_type.clone()),
                    }
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
                        self.check_pattern(pattern, ty, false);
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
                    // A bare variant with data (`Gren(x)`) against a declared
                    // enum must name an actual variant, otherwise Rust treats
                    // the unknown name as an irrefutable binding pattern and
                    // the arm silently swallows everything else.
                    if let Some(enum_name) = &resolved_enum {
                        if let Some(info) = self.enums.get(enum_name) {
                            if !info.variants.contains_key(variant) {
                                self.error(TypeCheckError::new(format!(
                                    "enum `{enum_name}` has no variant `{variant}`"
                                )));
                            }
                        }
                    }
                    let expected = PolyType::Named(resolved_enum.clone().unwrap_or_default());
                    self.require_compatible(
                        scrutinee_type,
                        &expected,
                        "enum match pattern".to_string(),
                    );
                } else if is_result && !matches!(variant.as_str(), "Ok" | "Error" | "Timeout") {
                    // Same trap for Result scrutinees: an unknown variant with
                    // payload would lower to an irrefutable binding.
                    self.error(TypeCheckError::new(format!(
                        "`{variant}` is not a Result variant (expected Ok, Error, or Timeout)"
                    )));
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
                        // Names inside a variant payload are bindings even when
                        // the payload type is itself an enum (`Add(l, r)` where
                        // the fields are `Expr`).
                        self.check_pattern(pattern, pattern_type, false);
                    }
                }
            }
            Pattern::NamedFields { name, fields } => {
                if let Some(info) = self.structs.get(name).cloned() {
                    for (field, pattern) in fields {
                        if let Some(ty) = info.fields.get(field).cloned() {
                            self.check_pattern(pattern, &ty, false);
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
                self.check_pattern(pattern, scrutinee_type, false);
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
                GetFlag::Default(value) => {
                    let ty = self.check_expression(value);
                    self.require_compatible(&ty, &PolyType::String, "default value".to_string());
                }
                GetFlag::Mask(value) => {
                    let ty = self.check_expression(value);
                    // The mask character may be a string (e.g. `unicode "*"`)
                    // or a char literal; the generated code suppresses terminal
                    // echo on Unix and falls back to a plain read elsewhere.
                    if !matches!(ty, PolyType::String | PolyType::Char | PolyType::Unknown) {
                        self.error(TypeCheckError::new(format!(
                            "`--mask` expects a character, got {}",
                            ty
                        )));
                    }
                }
                GetFlag::Until(value) => {
                    let ty = self.check_expression(value);
                    self.require_compatible(
                        &ty,
                        &PolyType::String,
                        "`--until` delimiter".to_string(),
                    );
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
        &mut self,
        function: &FunctionDecl,
        owner: Option<&str>,
    ) -> FunctionSignature {
        // The ordinary signature lowers type variables to `Unknown` so generic
        // bodies stay permissive. The declared signature keeps the names as
        // `Named("T")` markers so a call site can substitute the concrete
        // argument types into the return type.
        self.keep_type_variable_names = true;
        let declared_params: Vec<PolyType> = function
            .params
            .iter()
            .map(|parameter| self.annotation_type_for_owner(&parameter.ty, owner))
            .collect();
        let declared_return: Option<PolyType> = function
            .return_type
            .as_ref()
            .map(|ty| self.annotation_type_for_owner(ty, owner));
        self.keep_type_variable_names = false;
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
            generics: function
                .generics
                .iter()
                .map(|generic| generic.name.clone())
                .collect(),
            declared_params,
            declared_return,
            is_foreign: false,
            has_explicit_signature: false,
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
                // `_` is the placeholder annotation for untyped closure
                // parameters; it must behave like an unknown type so the
                // body can be checked before a signature pins it down.
                if name == "_" {
                    return PolyType::Unknown;
                }
                if self.type_variables.contains(name) {
                    // Inside a generic declaration the name is a type
                    // parameter; keep it as a marker when building declared
                    // signatures so call sites can substitute the concrete
                    // argument type, otherwise treat it as an opaque
                    // polymorphic type for body checking.
                    if self.keep_type_variable_names {
                        return PolyType::Named(name.clone());
                    }
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
            TypeAnnotation::Generic { name, args } => match name.as_str() {
                // Structured containers carry their element types so the
                // checker can validate `insert`/`get`/`contains` and friends.
                "Map" if args.len() == 2 => PolyType::Map(
                    Box::new(self.annotation_type_for_owner(&args[0], owner)),
                    Box::new(self.annotation_type_for_owner(&args[1], owner)),
                ),
                "Set" if args.len() == 1 => {
                    PolyType::Set(Box::new(self.annotation_type_for_owner(&args[0], owner)))
                }
                _ => {
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
            },
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

/// Check a parsed program, returning errors and non-fatal warnings.
pub fn check_program_with_warnings(
    program: &Program,
) -> (Result<(), Vec<TypeCheckError>>, Vec<String>) {
    TypeChecker::check_with_warnings(program)
}

/// Unify a declared generic type against a concrete argument type, recording
/// each type-variable binding (`T -> i32`) in `bindings`.
///
/// Only declared types that mention a generic parameter (directly or inside a
/// container like `Vec<T>`) produce bindings; unrelated shapes are ignored so
/// compatibility checking can report the mismatch.
fn unify_declared(
    declared: &PolyType,
    actual: &PolyType,
    generics: &[String],
    bindings: &mut HashMap<String, PolyType>,
) {
    if let PolyType::Named(name) = declared {
        if generics.iter().any(|generic| generic == name) {
            bindings.insert(name.clone(), actual.clone());
            return;
        }
    }
    match (declared, actual) {
        (PolyType::Vec(a), PolyType::Vec(b)) => unify_declared(a, b, generics, bindings),
        (PolyType::Map(ak, av), PolyType::Map(bk, bv)) => {
            unify_declared(ak, bk, generics, bindings);
            unify_declared(av, bv, generics, bindings);
        }
        (PolyType::Set(a), PolyType::Set(b)) => unify_declared(a, b, generics, bindings),
        (PolyType::Option(a), PolyType::Option(b)) => unify_declared(a, b, generics, bindings),
        (PolyType::Result(a_ok, a_err), PolyType::Result(b_ok, b_err)) => {
            unify_declared(a_ok, b_ok, generics, bindings);
            unify_declared(a_err, b_err, generics, bindings);
        }
        (PolyType::Array(a, _), PolyType::Array(b, _)) => unify_declared(a, b, generics, bindings),
        (PolyType::Tuple(a), PolyType::Tuple(b)) if a.len() == b.len() => {
            for (a, b) in a.iter().zip(b.iter()) {
                unify_declared(a, b, generics, bindings);
            }
        }
        (PolyType::Reference(_, a), PolyType::Reference(_, b)) => {
            unify_declared(a, b, generics, bindings)
        }
        (
            PolyType::Function {
                params: a_params,
                ret: a_ret,
            },
            PolyType::Function {
                params: b_params,
                ret: b_ret,
            },
        ) => {
            for (a, b) in a_params.iter().zip(b_params.iter()) {
                unify_declared(a, b, generics, bindings);
            }
            unify_declared(a_ret, b_ret, generics, bindings);
        }
        _ => {}
    }
}

/// Replace type-variable markers in `ty` with the concrete types gathered at a
/// call site. Unbound variables stay as `Named(name)` markers so downstream
/// checks treat them as opaque polymorphic types (same as the `Unknown`
/// lowering used inside generic bodies).
fn substitute_generic(
    ty: &PolyType,
    generics: &[String],
    bindings: &HashMap<String, PolyType>,
) -> PolyType {
    match ty {
        PolyType::Named(name) => {
            if generics.iter().any(|generic| generic == name) {
                bindings.get(name).cloned().unwrap_or_else(|| ty.clone())
            } else {
                ty.clone()
            }
        }
        PolyType::Vec(inner) => {
            PolyType::Vec(Box::new(substitute_generic(inner, generics, bindings)))
        }
        PolyType::Map(key, value) => PolyType::Map(
            Box::new(substitute_generic(key, generics, bindings)),
            Box::new(substitute_generic(value, generics, bindings)),
        ),
        PolyType::Set(inner) => {
            PolyType::Set(Box::new(substitute_generic(inner, generics, bindings)))
        }
        PolyType::Option(inner) => {
            PolyType::Option(Box::new(substitute_generic(inner, generics, bindings)))
        }
        PolyType::Result(ok, err) => PolyType::Result(
            Box::new(substitute_generic(ok, generics, bindings)),
            Box::new(substitute_generic(err, generics, bindings)),
        ),
        PolyType::Array(inner, size) => PolyType::Array(
            Box::new(substitute_generic(inner, generics, bindings)),
            *size,
        ),
        PolyType::Tuple(types) => PolyType::Tuple(
            types
                .iter()
                .map(|inner| substitute_generic(inner, generics, bindings))
                .collect(),
        ),
        PolyType::Reference(mutable, inner) => PolyType::Reference(
            *mutable,
            Box::new(substitute_generic(inner, generics, bindings)),
        ),
        PolyType::Function { params, ret } => PolyType::Function {
            params: params
                .iter()
                .map(|inner| substitute_generic(inner, generics, bindings))
                .collect(),
            ret: Box::new(substitute_generic(ret, generics, bindings)),
        },
        other => other.clone(),
    }
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
        (PolyType::Map(a_key, a_value), PolyType::Map(b_key, b_value)) => {
            compatible(a_key, b_key) && compatible(a_value, b_value)
        }
        (PolyType::Set(a), PolyType::Set(b)) => compatible(a, b),
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
        (
            PolyType::Function {
                params: a_params,
                ret: a_ret,
            },
            PolyType::Function {
                params: b_params,
                ret: b_ret,
            },
        ) => {
            a_params.len() == b_params.len()
                && a_params
                    .iter()
                    .zip(b_params.iter())
                    .all(|(a, b)| compatible(a, b))
                && compatible(a_ret, b_ret)
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
        PolyType::Map(_, value) => Some((**value).clone()),
        PolyType::String => Some(PolyType::Char),
        _ => None,
    }
}

/// Split a loop binding into names. `loop: (a, b) in collection` carries the
/// tuple pattern in the loop variable; every other binding is a plain name.
fn split_loop_binding(variable: &str) -> Vec<String> {
    let trimmed = variable.trim();
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        trimmed[1..trimmed.len() - 1]
            .split(',')
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect()
    } else {
        vec![trimmed.to_string()]
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
    fn accepts_iterator_chains() {
        assert!(check(
            "fn main()\n    var xs := [1, 2, 3]\n    var s i32 := xs.iter().sum()\n    var d := xs.iter().map(|x| x * 2).collect()\n    var e := xs.iter().filter(|x| x % 2 = 0).collect()\n    loop x in xs.iter()\n        put x\n    end loop\nend fn"
        )
        .is_ok());
    }

    #[test]
    fn rejects_sum_over_non_numeric_iterator() {
        let errors =
            check("fn main()\n    var xs := [\"a\", \"b\"]\n    var s := xs.iter().sum()\nend fn")
                .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("sum` requires a numeric")),
            "expected numeric-sum error, got {errors:?}"
        );
    }

    #[test]
    fn rejects_iterator_map_with_non_function_argument() {
        let errors = check(
            "fn main()\n    var xs := [1, 2, 3]\n    var d := xs.iter().map(42).collect()\nend fn",
        )
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("expects a function callback")),
            "expected callback error, got {errors:?}"
        );
    }

    #[test]
    fn accepts_clear_and_reserve_on_vectors() {
        assert!(check(
            "fn main()\n    var list Vec<i32> := []\n    list.reserve(100)\n    list.clear()\nend fn"
        )
        .is_ok());
    }

    #[test]
    fn accepts_explicit_loop_variable() {
        assert!(check(
            "fn main()\n    loop value 1..3, 7\n        put value\n    end loop\nend fn"
        )
        .is_ok());
    }

    #[test]
    fn accepts_tuple_destructured_collection_loop() {
        assert!(check(
            "fn main()\n    var pairs Vec<(i32, ustring)> := []\n    loop (index, fruit) in pairs.enumerate()\n        put index\n        put fruit\n    end loop\nend fn"
        )
        .is_ok());
    }

    #[test]
    fn tuple_destructured_loop_declares_typed_bindings() {
        assert!(check(
            "fn main()\n    var pairs Vec<(i32, ustring)> := []\n    loop (index, fruit) in pairs\n        var n i32 := index\n        var s ustring := fruit\n    end loop\nend fn"
        )
        .is_ok());
    }

    #[test]
    fn tuple_index_access_has_element_type() {
        // `pair.1` must resolve to the tuple's element type, so assigning it
        // to a `String` fails while `pair.0` (an i32) is fine.
        let source = "fn main()\n    var pair := (10, true)\n    var n i32 := pair.0\n    var b bool := pair.1\nend fn";
        assert!(check(source).is_ok());

        let errors =
            check("fn main()\n    var pair := (10, true)\n    var s ustring := pair.1\nend fn")
                .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expected String, got bool")));
    }

    #[test]
    fn for_loop_tuple_destructuring_declares_typed_bindings() {
        // `for (a, b) in vec_of_tuples` must declare `a`/`b` with the element
        // types so the body type-checks against them.
        let source = "fn main()\n    var pairs Vec<(i32, ustring)> := []\n    for (num, label) in pairs\n        var n i32 := num\n        var s ustring := label\n    end for\nend fn";
        assert!(check(source).is_ok());
    }

    #[test]
    fn for_loop_enumerate_single_binding_type_checks() {
        // `for pair in xs.enumerate()` binds a tuple; reading its elements
        // must type-check (the element type is Unknown, so reads are lenient).
        let source = "fn main()\n    var xs := [1, 2]\n    for pair in xs.enumerate()\n        put pair.0\n    end for\nend fn";
        assert!(check(source).is_ok());
    }

    #[test]
    fn tuple_element_assignment_checks_types() {
        // Assigning a compatible value to `pair.0` is fine; a wrong type is
        // rejected.
        let source =
            "fn main()\n    var pair := (10, true)\n    pair.0 := 99\n    pair.1 := false\nend fn";
        assert!(check(source).is_ok());

        let errors =
            check("fn main()\n    var pair := (10, true)\n    pair.0 := unicode \"oops\"\nend fn")
                .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expected i32")));
    }

    #[test]
    fn rejects_out_of_bounds_and_non_tuple_index() {
        let errors =
            check("fn main()\n    var pair := (10, true)\n    put pair.2\nend fn").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("tuple index 2 out of bounds")));

        let errors = check("fn main()\n    var x := 42\n    put x.0\nend fn").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("type i32 is not a tuple")));
    }

    #[test]
    fn nested_tuple_index_access_type_checks() {
        let source = "fn main()\n    var nested := (1, (2, 3))\n    var a i32 := nested.1.0\n    var b i32 := nested.1.1\nend fn";
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
    fn rejects_non_boolean_conditions() {
        let errors = check("var value i32 := 1\nif value\n    put 1\nend if").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| format!("{error}").contains("if condition")));
    }

    #[test]
    fn allows_string_append_mutation() {
        // String concatenation via `:=` works;
        assert!(check("var output ustring := \"\"\noutput := output + \"abc\"\n").is_ok());
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
    fn accepts_infinite_loop_with_break() {
        assert!(check(
            "fn main()\n    var count i32 := 0\n    loop\n        count := count + 1\n        if count = 3,\n            break\n        end if\n    end loop\nend fn"
        )
        .is_ok());
    }

    #[test]
    fn rejects_break_outside_a_loop() {
        let errors = check("break\ncontinue").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("only valid inside a loop")));
    }

    #[test]
    fn accepts_break_and_continue_inside_while_loops() {
        let source = "var i i32 := 0\nwhile i < 3\n    break\n    i := i + 1\nend while\n\
                      while i < 5\n    if i = 1,\n        continue\n    end if\n    i := i + 1\nend while";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn accepts_nested_function_calls() {
        let source = "fn outer(x: i32): i32\n    fn inner(y: i32): i32\n        return y * 2\n    end fn\n    return inner(x) + 1\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn accepts_option_some_none_values_and_patterns() {
        let source = "fn main()\n    var maybe Option<i32> := Some(42)\n    var empty Option<i32> := None\n    match maybe\n        Some(v), put v\n        None, put 0\n    end match\n    match empty\n        Some(v), put v\n        None, put 0\n    end match\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn rejects_option_variant_matching_a_non_option() {
        let errors =
            check("var x i32 := 5\nmatch x\n    Some(v), put v\n    None, put 0\nend match")
                .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| format!("{error}").contains("enum match pattern")));
    }

    #[test]
    fn accepts_closure_type_annotation_and_untyped_literal() {
        // The closure type annotation parses and an untyped closure literal
        // passed as the argument inherits the declared parameter types.
        let source = "fn apply(f: |x: i32| i32, v: i32): i32\n    return f(v)\nend fn\n\
                      fn main()\n    put apply(|x| x * 2, 21)\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn accepts_multi_param_closure_type_with_untyped_literal() {
        let source = "fn combine(f: |a: i32, b: i32| i32, x: i32, y: i32): i32\n    return f(x, y)\nend fn\n\
                      fn main()\n    put combine(|a, b| a + b, 2, 3)\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn accepts_typed_closure_variable_passed_to_function_param() {
        let source = "fn apply(f: |x: i32| i32, v: i32): i32\n    return f(v)\nend fn\n\
                      fn main()\n    var double := |x: i32| x * 2\n    put apply(double, 21)\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn rejects_closure_return_type_mismatch() {
        let errors = check(
            "fn apply(f: |x: i32| i32, v: i32): i32\n    return f(v)\nend fn\n\
             fn main()\n    put apply(|x| str(x), 21)\nend fn",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expected fn(i32) -> i32")));
    }

    #[test]
    fn rejects_closure_arity_mismatch() {
        let errors = check(
            "fn apply(f: |x: i32| i32, v: i32): i32\n    return f(v)\nend fn\n\
             fn main()\n    put apply(|a, b| a + b, 21)\nend fn",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expected fn(i32) -> i32")));
    }

    #[test]
    fn accepts_function_returning_closure() {
        // A function-typed return value: the returned closure literal inherits
        // the declared signature and the result can be stored and called.
        let source = "fn make_adder(n: i32): |x: i32| i32\n    return |x| x + n\nend fn\n\
                      fn main()\n    var add5 := make_adder(5)\n    put add5(10)\n    put make_adder(100)(1)\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn rejects_closure_return_value_mismatch() {
        let errors =
            check("fn make(x: i32): |x: i32| i32\n    return |x| str(x)\nend fn").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("expected fn(i32) -> i32, got fn(i32) -> String")));
    }

    #[test]
    fn accepts_vec_higher_order_methods() {
        let source = "fn main()\n    var xs := [1, 2, 3, 4, 5]\n    put xs.map(|x| x * 2)[0]\n    put xs.filter(|x| x % 2 = 0)\n    put xs.reduce(0, |acc, x| acc + x)\n    put xs.reduce(1, |acc, x| acc * x)\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn accepts_typed_closure_variable_passed_to_hof() {
        let source = "fn main()\n    var xs := [1, 2, 3]\n    var double := |x: i32| x * 2\n    put xs.map(double)[0]\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn rejects_filter_callback_returning_non_bool() {
        let errors =
            check("fn main()\n    var xs := [1, 2, 3]\n    put xs.filter(|x| x + 1)\nend fn")
                .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expected bool, got i32")));
    }

    #[test]
    fn rejects_hof_callback_arity_mismatch() {
        let errors = check("fn main()\n    var xs := [1, 2, 3]\n    put xs.map(|a, b| a)\nend fn")
            .unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("expects a callback with 1 parameter")));
    }
    #[test]
    fn rejects_hof_callback_that_is_not_a_function() {
        let errors =
            check("fn main()\n    var xs := [1, 2, 3]\n    put xs.map(5)\nend fn").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("expects a function callback, got i32")));
    }

    #[test]
    fn accepts_higher_order_methods_on_strings_and_sort_by() {
        let source = "fn main()\n    put \"hello\".map(|c| c)\n    put \"hello\".filter(|c| c != unicode 'l')\n    put \"hello\".reduce(0, |acc, c| acc + (c as i32))\n    var xs := [3, 1, 2]\n    put xs.sort_by(|a, b| a > b)\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn rejects_sort_by_on_a_string() {
        let errors = check("fn main()\n    put \"hi\".sort_by(|a, b| a < b)\nend fn").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("type String has no method `sort_by`")));
    }

    #[test]
    fn rejects_sort_by_comparator_returning_non_bool() {
        let errors =
            check("fn main()\n    var xs := [3, 1, 2]\n    put xs.sort_by(|a, b| a + b)\nend fn")
                .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expected bool, got i32")));
    }

    // ------------------------------------------------------------------
    // Audit fixes: match-pattern validation, range patterns, Map/Set,
    // stored-and-returned closures, loop steps, runtime API warnings.
    // ------------------------------------------------------------------

    #[test]
    fn rejects_misspelled_enum_variant_in_match() {
        let errors = check(
            "enum Color\n    Red\n    Green\n    Blue\nend enum\nfn main()\n    var c := Color::Red\n    match c\n        Red, put 1\n        Gren, put 2\n        Blue, put 3\n    end match\nend fn",
        )
        .unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("`Gren` is not a variant of enum `Color`")));
    }

    #[test]
    fn rejects_misspelled_enum_variant_with_payload_in_match() {
        let errors = check(
            "enum Shape\n    Circle(i32)\nend enum\nfn main()\n    var s := Shape::Circle(1)\n    match s\n        Gren(x), put x\n    end match\nend fn",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("enum `Shape` has no variant `Gren`")));
    }

    #[test]
    fn rejects_unknown_identifier_pattern_on_result() {
        let errors = check(
            "fn main()\n    var r := get --timeout 100\n    match r\n        Bogus(x), put x\n    end match\nend fn",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("`Bogus` is not a Result variant")));
    }

    #[test]
    fn accepts_binding_patterns_on_scalar_scrutinees() {
        // Binding identifiers remain valid for non-enum scrutinees.
        assert!(check(
            "fn main()\n    var n := 42\n    match n\n        x, put x\n    end match\nend fn"
        )
        .is_ok());
    }

    #[test]
    fn accepts_range_patterns_in_match() {
        assert!(check(
            "fn main()\n    var n := 42\n    match n\n        0..=9, put unicode \"small\"\n        10..=99, put unicode \"medium\"\n        _, put unicode \"large\"\n    end match\nend fn"
        )
        .is_ok());
    }

    #[test]
    fn accepts_map_and_set_methods() {
        let source = "fn main()\n    var m Map<ustring, i32> := []\n    m.insert(unicode \"key\", 42)\n    var v := m.get(unicode \"key\")\n    put m.contains_key(unicode \"key\")\n    m.remove(unicode \"key\")\n    put m.len()\n    var s Set<i32> := []\n    s.insert(1)\n    put s.contains(1)\n    s.remove(1)\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn accepts_map_indexing_with_key_type() {
        let source = "fn main()\n    var m Map<ustring, i32> := []\n    m[unicode \"a\"] = 1\n    var v := m[unicode \"a\"]\n    put v\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn rejects_wrong_map_key_type() {
        let errors =
            check("fn main()\n    var m Map<ustring, i32> := []\n    m.insert(7, 1)\nend fn")
                .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expected String, got i32")));
    }

    #[test]
    fn accepts_stored_then_returned_closure() {
        // Untyped closure parameters stored in a variable and then returned
        // must type-check against the declared function-typed return.
        let source =
            "fn make_adder(n: i32): |x: i32| i32\n    var f := |x| x + n\n    return f\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn accepts_typed_closure_stored_in_variable() {
        let source = "fn main()\n    var double := |x: i32| x * 2\n    put double(21)\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn rejects_zero_loop_step() {
        let errors =
            check("fn main()\n    loop i 0..10 step 0\n        put i\n    end loop\nend fn")
                .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("loop range step must not be zero")));
    }

    #[test]
    fn no_warning_for_real_network_builtins() {
        // `http_get`/`tcp_connect`/`spawn` are real tokio implementations and
        // `db_execute` runs SQL against SQLite; none of them should warn.
        let (tokens, lexer_errors) = Lexer::lex(
            "fn main()\n    var data := http_get(unicode \"http://example.com\").await\n    put data\nend fn",
        );
        assert!(lexer_errors.is_empty());
        let mut parser = Parser::new(&tokens);
        let program = parser.parse().expect("source should parse");
        let (result, warnings) = TypeChecker::check_with_warnings(&program);
        assert!(result.is_ok());
        assert!(!warnings
            .iter()
            .any(|warning| warning.contains("`http_get` is an experimental stub")));
    }

    #[test]
    fn db_execute_no_longer_warns() {
        // `db_execute` runs SQL against SQLite now; it must type-check without
        // the old "experimental stub" warning.
        let source = "fn main()\n    var rows := db_execute(unicode \"SELECT 1\").await\n    put rows\nend fn";
        let (tokens, lexer_errors) = Lexer::lex(source);
        assert!(lexer_errors.is_empty());
        let mut parser = Parser::new(&tokens);
        let program = parser.parse().expect("source should parse");
        let (result, warnings) = TypeChecker::check_with_warnings(&program);
        assert!(result.is_ok(), "{:?}", result.err());
        assert!(!warnings
            .iter()
            .any(|warning| warning.contains("`db_execute` is an experimental stub")));
    }

    #[test]
    fn db_execute_requires_string_query() {
        let source = "fn main()\n    var rows := db_execute(42).await\n    put rows\nend fn";
        let errors = check(source).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expected String, got i32")));
    }

    #[test]
    fn explicit_extern_signature_checks_arguments_and_return_type() {
        let valid = "extern c fn double(value: i32): i32\n#c\nint double(int value) { return value * 2; }\n#endc\nfn main()\n    var result i32 := double(21)\n    put result\nend fn";
        assert!(check(valid).is_ok(), "{:?}", check(valid).err());

        let invalid = "extern c fn double(value: i32): i32\n#c\nint double(int value) { return value * 2; }\n#endc\nfn main()\n    var result i32 := double(unicode \"wrong\")\n    put result\nend fn";
        let errors = check(invalid).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expected i32, got String")));
    }

    #[test]
    fn unknown_compatibility_builtin_is_rejected() {
        let source = "fn main()\n    process_config(unicode \"config\")\nend fn";
        let errors = check(source).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("unknown function `process_config`")));
    }

    #[test]
    fn generic_call_infers_return_type_at_call_site() {
        // `identity(42)` must resolve `T = i32` and return `i32`, so assigning
        // the result to a `String` fails instead of silently passing as
        // `Unknown`.
        let source = "fn identity<T>(value: T): T\n    return value\nend fn\n\nfn main()\n    var s ustring := identity(42)\n    put s\nend fn";
        let errors = check(source).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expected String, got i32")));
    }

    #[test]
    fn generic_call_infers_nested_container_types() {
        // `first(xs)` with `xs: Vec<i32>` must bind `T = i32` through the
        // container and return `i32`.
        let source = "fn first<T>(xs: Vec<T>): T\n    return xs[0]\nend fn\n\nfn main()\n    var xs Vec<i32> := [1, 2, 3]\n    var head i32 := first(xs)\n    put head.to_string()\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn generic_call_accepts_correct_argument_type() {
        let source = "fn identity<T>(value: T): T\n    return value\nend fn\n\nfn main()\n    var n i32 := identity(42)\n    put n.to_string()\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn mask_and_until_flags_are_implemented() {
        // `--mask` and `--until` are real now: they must type-check without
        // the old "not implemented" warnings.
        let source = "fn main()\n    var pw := get --mask unicode \"*\"\n    put pw\n    var field := get --until unicode \",\"\n    put field\nend fn";
        let (tokens, lexer_errors) = Lexer::lex(source);
        assert!(lexer_errors.is_empty());
        let mut parser = Parser::new(&tokens);
        let program = parser.parse().expect("source should parse");
        let (result, warnings) = TypeChecker::check_with_warnings(&program);
        assert!(result.is_ok(), "{:?}", result.err());
        assert!(!warnings
            .iter()
            .any(|warning| warning.contains("`--mask` is not implemented")));
        assert!(!warnings
            .iter()
            .any(|warning| warning.contains("`--until` is not implemented")));
    }

    #[test]
    fn assert_and_test_helpers_type_check() {
        let source = "fn main()\n    var x i32 := 4\n    assert(x > 3)\n    pass(unicode \"x is positive\")\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn assert_requires_boolean() {
        let source = "fn main()\n    assert(42)\nend fn";
        let errors = check(source).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expected bool, got i32")));
    }

    #[test]
    fn checked_index_get_returns_option() {
        // `xs.get(i)` must resolve to `Option<T>` and reject non-integer
        // indexes, so the result is usable with `match`/`if` patterns.
        let source = "fn main()\n    var xs Vec<i32> := [1, 2, 3]\n    match xs.get(0)\n        Some(value), put value.to_string()\n        None, put \"empty\"\n    end match\n    match unicode \"abc\".get(1)\n        Some(c), put c\n        None, put \"none\"\n    end match\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn checked_index_get_rejects_non_integer() {
        let source =
            "fn main()\n    var xs Vec<i32> := [1, 2, 3]\n    put xs.get(unicode \"zero\")\nend fn";
        let errors = check(source).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("`get` index must be an integer")));
    }

    #[test]
    fn result_accessors_type_check() {
        let source = "fn main()\n    var result := http_get(unicode \"http://example.com\").await\n    assert(result.is_ok())\n    if result.is_ok(),\n        put result.unwrap()\n    end if\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn string_interpolation_type_checks() {
        let source = "fn main()\n    var name ustring := unicode \"Alice\"\n    var age i32 := 30\n    put \"Name: {name}, Age: {age}\"\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }

    #[test]
    fn interpolation_keeps_literal_braces_when_unparseable() {
        // `{` that does not form a parseable expression stays literal text,
        // so strings containing JSON-style braces still compile.
        let source =
            "fn main()\n    var json ustring := \"{{\\\"a\\\": 1}}\"\n    put json\nend fn";
        assert!(check(source).is_ok(), "{:?}", check(source).err());
    }
}

//! AST → intermediate representation conversion.
//!
//! The generator lowers the parser's AST into the compiler intermediate representation.  Because the intermediate representation
//! intentionally mirrors the AST's statement/expression split, this conversion
//! is mostly structural; the optimizer and intermediate representation codegen benefit from the stable,
//! flat intermediate representation form afterwards.

use poly_parser::ast;

use crate::intermediate_representation::*;

/// Convert a parsed Poly program into intermediate representation.
pub fn generate(program: &ast::Program) -> Result<Program, String> {
    // Keep this pass structural: semantic meaning belongs to the checker and
    // target-specific lowering belongs to codegen, which keeps the IR reusable.
    let mut intermediate_representation = Program::default();
    // Statement byte offsets ride alongside the lowered statements so codegen
    // can emit precise source-map mappings.  Bodies and `main_body` keep a
    // parallel `Vec<Option<SourceLocation>>`; nested blocks (if/match/loop
    // bodies) are covered by the same per-statement offsets of their
    // enclosing list, so only top-level lists need the array.
    let mut main_body_locations: Vec<Option<SourceLocation>> = Vec::new();
    for spanned in &program.statements {
        let statement = &spanned.node;
        match statement {
            ast::Statement::FunctionDeclaration(function) => {
                intermediate_representation
                    .functions
                    .push(gen_function(function)?);
            }
            ast::Statement::StructDeclaration(struct_decl) => {
                intermediate_representation
                    .structs
                    .push(gen_struct(struct_decl)?);
            }
            ast::Statement::EnumDeclaration(enum_decl) => {
                intermediate_representation.enums.push(gen_enum(enum_decl)?);
            }
            ast::Statement::TraitDeclaration(trait_decl) => {
                intermediate_representation
                    .traits
                    .push(gen_trait(trait_decl)?);
            }
            ast::Statement::ImplDeclaration(impl_decl) => {
                intermediate_representation.impls.push(gen_impl(impl_decl)?);
            }
            ast::Statement::ModuleDeclaration(module) => {
                let mut statements = Vec::new();
                for spanned in &module.statements {
                    statements.push(gen_statement(&spanned.node)?);
                }
                intermediate_representation.modules.push(Module {
                    name: module.name.clone(),
                    statements,
                });
            }
            ast::Statement::ConstDeclaration { name, value } => {
                intermediate_representation.constants.push(Constant {
                    name: name.clone(),
                    value: gen_expr(value)?,
                    source_location: None,
                });
            }
            ast::Statement::TypeDeclaration(decl) => {
                intermediate_representation.type_aliases.push(TypeAlias {
                    name: decl.name.clone(),
                    ty: gen_type(&decl.ty)?,
                });
            }
            ast::Statement::UseDeclaration(decl) => {
                let mut path = decl.path.join("::");
                if let Some(alias) = &decl.alias {
                    path.push_str(&format!(" as {}", alias));
                }
                intermediate_representation.uses.push(path);
            }
            ast::Statement::DependencyDeclaration(decl) => {
                intermediate_representation
                    .dependencies
                    .push((decl.name.clone(), decl.version.clone()));
            }
            // Rust blocks are hoisted because Rust items cannot appear inside
            // the generated `main`; target selection has already removed C/C++.
            ast::Statement::ForeignBlock { language, content } if language == "rust" => {
                intermediate_representation
                    .top_level_rust_blocks
                    .push(content.clone());
            }
            ast::Statement::ForeignBlock { .. } => {}
            ast::Statement::ExternFunctionDeclaration(_) => {}
            _ => {
                main_body_locations.push(Some(SourceLocation {
                    byte_offset: spanned.span.start,
                }));
                intermediate_representation
                    .main_body
                    .push(gen_statement(statement)?);
            }
        }
    }
    intermediate_representation.main_body_locations = main_body_locations;
    Ok(intermediate_representation)
}

fn gen_function(function: &ast::FunctionDecl) -> Result<Function, String> {
    let body_locations: Vec<Option<SourceLocation>> = function
        .body
        .as_ref()
        .map(|body| {
            body.iter()
                .map(|spanned| {
                    Some(SourceLocation {
                        byte_offset: spanned.span.start,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Function {
        name: function.name.clone(),
        params: function
            .params
            .iter()
            .map(gen_parameter)
            .collect::<Result<Vec<_>, String>>()?,
        return_type: function.return_type.as_ref().map(gen_type).transpose()?,
        body: function
            .body
            .as_ref()
            .map(|body| {
                body.iter()
                    .map(|spanned| gen_statement(&spanned.node))
                    .collect::<Result<Vec<_>, String>>()
            })
            .transpose()?
            .unwrap_or_default(),
        body_locations,
        is_async: function.is_async,
        generics: function.generics.iter().map(gen_generic_param).collect(),
        source_location: None,
    })
}

fn gen_generic_param(param: &ast::GenericParam) -> GenericParam {
    GenericParam {
        name: param.name.clone(),
        bounds: param.bounds.clone(),
    }
}

fn gen_parameter(parameter: &ast::Parameter) -> Result<Parameter, String> {
    Ok(Parameter {
        name: parameter.name.clone(),
        ty: gen_type(&parameter.ty)?,
        default: parameter.default.as_ref().map(gen_expr).transpose()?,
    })
}

fn gen_struct(struct_decl: &ast::StructDecl) -> Result<Struct, String> {
    Ok(Struct {
        name: struct_decl.name.clone(),
        fields: struct_decl
            .fields
            .iter()
            .map(|field| {
                Ok(Field {
                    name: field.name.clone(),
                    ty: gen_type(&field.ty)?,
                    default: field.default.as_ref().map(gen_expr).transpose()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
        methods: struct_decl
            .methods
            .iter()
            .map(gen_function)
            .collect::<Result<Vec<_>, String>>()?,
        generics: struct_decl.generics.iter().map(gen_generic_param).collect(),
        source_location: None,
    })
}

fn gen_enum(enum_decl: &ast::EnumDecl) -> Result<Enum, String> {
    let variants = enum_decl
        .variants
        .iter()
        .map(|variant| {
            Ok(match variant {
                ast::EnumVariant::Unit(name) => Variant {
                    name: name.clone(),
                    fields: Vec::new(),
                    is_struct: false,
                },
                ast::EnumVariant::Tuple(name, types) => Variant {
                    name: name.clone(),
                    fields: types
                        .iter()
                        .map(|ty| {
                            Ok(Field {
                                name: String::new(),
                                ty: gen_type(ty)?,
                                default: None,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                    is_struct: false,
                },
                ast::EnumVariant::Struct(name, fields) => Variant {
                    name: name.clone(),
                    fields: fields
                        .iter()
                        .map(|(field, ty)| {
                            Ok(Field {
                                name: field.clone(),
                                ty: gen_type(ty)?,
                                default: None,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                    is_struct: true,
                },
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Enum {
        name: enum_decl.name.clone(),
        variants,
        methods: enum_decl
            .methods
            .iter()
            .map(gen_function)
            .collect::<Result<Vec<_>, String>>()?,
        source_location: None,
    })
}

fn gen_trait(trait_decl: &ast::TraitDecl) -> Result<Trait, String> {
    Ok(Trait {
        name: trait_decl.name.clone(),
        methods: trait_decl
            .methods
            .iter()
            .map(gen_function)
            .collect::<Result<Vec<_>, String>>()?,
        source_location: None,
    })
}

fn gen_impl(impl_decl: &ast::ImplDecl) -> Result<Impl, String> {
    Ok(Impl {
        trait_name: impl_decl.trait_name.clone(),
        type_name: impl_decl.type_name.clone(),
        methods: impl_decl
            .methods
            .iter()
            .map(gen_function)
            .collect::<Result<Vec<_>, String>>()?,
        source_location: None,
    })
}

fn gen_statement(statement: &ast::Statement) -> Result<Statement, String> {
    Ok(match statement {
        ast::Statement::VarDeclaration { name, ty, value } => Statement::VarDecl {
            name: name.clone(),
            ty: ty.as_ref().map(gen_type).transpose()?,
            value: value.as_ref().map(gen_expr).transpose()?,
        },
        ast::Statement::LetDeclaration { name, ty, value } => Statement::LetDecl {
            name: name.clone(),
            ty: ty.as_ref().map(gen_type).transpose()?,
            value: gen_expr(value)?,
        },
        // Only program-scope constants are lifted into `IR.constants` by
        // `generate`; a `const` inside a function body (or a module) is a
        // local binding, so lower it to an immutable declaration. Emitting it
        // as `VarDecl` keeps the immutability distinction out of the IR while
        // producing the correct initialized local in every backend.
        ast::Statement::ConstDeclaration { name, value } => Statement::VarDecl {
            name: name.clone(),
            ty: None,
            value: Some(gen_expr(value)?),
        },
        ast::Statement::Assignment { target, value } => Statement::Assignment {
            target: gen_expr(target)?,
            value: gen_expr(value)?,
        },
        ast::Statement::FunctionDeclaration(function) => {
            Statement::NestedFunction(gen_function(function)?)
        }
        // Program-scope declarations are collected by `generate` before any
        // `gen_statement` call, but the parser also accepts declarations
        // nested inside function bodies. Those never reach the collection
        // pass, so treat them as inert here rather than panicking the
        // compiler — the checker reports them as invalid or unused, and
        // codegen emits nothing for them.
        ast::Statement::StructDeclaration(_)
        | ast::Statement::EnumDeclaration(_)
        | ast::Statement::TraitDeclaration(_)
        | ast::Statement::ImplDeclaration(_)
        | ast::Statement::ModuleDeclaration(_)
        | ast::Statement::UseDeclaration(_)
        | ast::Statement::DependencyDeclaration(_)
        | ast::Statement::TypeDeclaration(_) => Statement::Block(Vec::new()),
        ast::Statement::ExpressionStatement(expr) => Statement::Expression(gen_expr(expr)?),
        ast::Statement::ReturnStatement(value) => {
            Statement::Return(value.as_ref().map(gen_expr).transpose()?)
        }
        ast::Statement::BreakStatement => Statement::Break,
        ast::Statement::ContinueStatement => Statement::Continue,
        ast::Statement::PutStatement { expr, redirect } => Statement::Put {
            expr: gen_expr(expr)?,
            redirect: match redirect.as_ref() {
                Some(ast::Redirect::Write(path)) => Some(Redirect::Write(gen_expr(path)?)),
                Some(ast::Redirect::Append(path)) => Some(Redirect::Append(gen_expr(path)?)),
                None => None,
            },
        },
        ast::Statement::ErrorStatement(expr) => Statement::Error(gen_expr(expr)?),
        ast::Statement::WarnStatement(expr) => Statement::Warn(gen_expr(expr)?),
        ast::Statement::InfoStatement(expr) => Statement::Info(gen_expr(expr)?),
        ast::Statement::Block(statements) => Statement::Block(
            statements
                .iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect::<Result<Vec<_>, String>>()?,
        ),
        ast::Statement::ForeignBlock { language, content } => Statement::ForeignBlock {
            language: language.clone(),
            content: content.clone(),
        },
        ast::Statement::ExternFunctionDeclaration(_) => {
            // Top-level externs are skipped in `generate()` before lowering;
            // reaching this arm means the declaration is nested inside a
            // function/module/block body, which the IR cannot represent.
            return Err(
                "extern function declarations may only appear at the top level".to_string(),
            );
        }
    })
}

fn gen_expr(expr: &ast::Expression) -> Result<Expr, String> {
    Ok(match expr {
        ast::Expression::IntLiteral(value) => Expr::Literal(Literal::Int(value.clone())),
        ast::Expression::FloatLiteral(value) => Expr::Literal(Literal::Float(value.clone())),
        ast::Expression::StringLiteral(value) => Expr::Literal(Literal::String(value.clone())),
        ast::Expression::UnicodeStringLiteral(value) => {
            Expr::Literal(Literal::UnicodeString(value.clone()))
        }
        ast::Expression::UnicodeCharLiteral(value) => Expr::Literal(Literal::Char(*value)),
        ast::Expression::BoolLiteral(value) => Expr::Literal(Literal::Bool(*value)),
        ast::Expression::ByteLiteral(bytes) => Expr::Literal(Literal::Bytes(bytes.clone())),
        ast::Expression::Identifier(name) => Expr::Identifier(name.clone()),
        ast::Expression::BinaryOp { op, left, right } => Expr::BinaryOp {
            op: gen_binary_op(op),
            left: Box::new(gen_expr(left)?),
            right: Box::new(gen_expr(right)?),
        },
        ast::Expression::UnaryOp { op, expr } => Expr::UnaryOp {
            op: match op {
                ast::UnaryOp::Neg => UnaryOp::Neg,
                ast::UnaryOp::Not => UnaryOp::Not,
                ast::UnaryOp::BitNot => UnaryOp::BitNot,
                ast::UnaryOp::Deref => UnaryOp::Deref,
            },
            expr: Box::new(gen_expr(expr)?),
        },
        ast::Expression::Call { func, args } => Expr::Call {
            func: Box::new(gen_expr(func)?),
            args: args
                .iter()
                .map(gen_expr)
                .collect::<Result<Vec<_>, String>>()?,
        },
        ast::Expression::MethodCall {
            object,
            method,
            args,
        } => Expr::MethodCall {
            object: Box::new(gen_expr(object)?),
            method: method.clone(),
            args: args
                .iter()
                .map(gen_expr)
                .collect::<Result<Vec<_>, String>>()?,
        },
        ast::Expression::Index { object, index } => Expr::Index {
            object: Box::new(gen_expr(object)?),
            index: Box::new(gen_expr(index)?),
        },
        ast::Expression::FieldAccess { object, field } => Expr::FieldAccess {
            object: Box::new(gen_expr(object)?),
            field: field.clone(),
        },
        ast::Expression::TupleIndex { object, index } => Expr::TupleIndex {
            object: Box::new(gen_expr(object)?),
            index: *index,
        },
        ast::Expression::Parenthesized(expr) => Expr::Parenthesized(Box::new(gen_expr(expr)?)),
        ast::Expression::IfExpression {
            condition,
            then_block,
            else_block,
        } => Expr::If {
            condition: Box::new(gen_expr(condition)?),
            then_block: then_block
                .iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect::<Result<Vec<_>, String>>()?,
            else_block: else_block
                .as_ref()
                .map(|block| {
                    block
                        .iter()
                        .map(|spanned| gen_statement(&spanned.node))
                        .collect::<Result<Vec<_>, String>>()
                })
                .transpose()?,
        },
        ast::Expression::MatchExpression { scrutinee, arms } => Expr::Match {
            scrutinee: Box::new(gen_expr(scrutinee)?),
            arms: arms
                .iter()
                .map(gen_match_arm)
                .collect::<Result<Vec<_>, String>>()?,
        },
        ast::Expression::WhileLoop { condition, body } => Expr::WhileLoop {
            condition: Box::new(gen_expr(condition)?),
            body: body
                .iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect::<Result<Vec<_>, String>>()?,
        },
        ast::Expression::Closure { params, body } => Expr::Closure {
            params: params
                .iter()
                .map(gen_parameter)
                .collect::<Result<Vec<_>, String>>()?,
            body: Box::new(gen_expr(body)?),
        },
        ast::Expression::ArrayLiteral(elements) => Expr::Array(
            elements
                .iter()
                .map(gen_expr)
                .collect::<Result<Vec<_>, String>>()?,
        ),
        ast::Expression::TupleLiteral(elements) => Expr::Tuple(
            elements
                .iter()
                .map(gen_expr)
                .collect::<Result<Vec<_>, String>>()?,
        ),
        ast::Expression::StructLiteral { name, fields } => Expr::Struct {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| Ok((name.clone(), gen_expr(value)?)))
                .collect::<Result<Vec<_>, String>>()?,
        },
        ast::Expression::EnumVariant {
            enum_name,
            variant,
            data,
        } => Expr::Enum {
            enum_name: enum_name.clone(),
            variant: variant.clone(),
            data: data
                .as_ref()
                .map(|data| {
                    data.iter()
                        .map(gen_expr)
                        .collect::<Result<Vec<_>, String>>()
                })
                .transpose()?,
        },
        ast::Expression::LoopRange {
            variable,
            ranges,
            body,
        } => Expr::LoopRange {
            variable: variable.clone(),
            ranges: ranges
                .iter()
                .map(gen_loop_range_part)
                .collect::<Result<Vec<_>, String>>()?,
            body: body
                .iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect::<Result<Vec<_>, String>>()?,
        },
        ast::Expression::ForLoop {
            variable,
            iterable,
            body,
        } => Expr::ForLoop {
            variable: variable.clone(),
            iterable: Box::new(gen_expr(iterable)?),
            body: body
                .iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect::<Result<Vec<_>, String>>()?,
        },
        ast::Expression::InfiniteLoop(body) => Expr::InfiniteLoop(
            body.iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect::<Result<Vec<_>, String>>()?,
        ),
        ast::Expression::AsExpression { expr, ty } => Expr::As {
            expr: Box::new(gen_expr(expr)?),
            ty: gen_type(ty)?,
        },
        ast::Expression::TryExpression(expr) => Expr::Try(Box::new(gen_expr(expr)?)),
        ast::Expression::GetExpression(get) => Expr::Get(Box::new(gen_get_expr(get)?)),
        ast::Expression::UnsafeBlock(statements) => Expr::UnsafeBlock(
            statements
                .iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect::<Result<Vec<_>, String>>()?,
        ),
        ast::Expression::Range {
            start,
            end,
            inclusive,
        } => Expr::Range {
            start: Box::new(gen_expr(start)?),
            end: Box::new(gen_expr(end)?),
            inclusive: *inclusive,
        },
    })
}

fn gen_match_arm(arm: &ast::MatchArm) -> Result<MatchArm, String> {
    Ok(MatchArm {
        pattern: gen_pattern(&arm.pattern)?,
        guard: arm.guard.as_ref().map(gen_expr).transpose()?,
        body: match &arm.body {
            ast::MatchArmBody::Expression(expr) => MatchArmBody::Expression(gen_expr(expr)?),
            ast::MatchArmBody::Block(statements) => MatchArmBody::Block(
                statements
                    .iter()
                    .map(|spanned| gen_statement(&spanned.node))
                    .collect::<Result<Vec<_>, String>>()?,
            ),
        },
    })
}

fn gen_pattern(pattern: &ast::Pattern) -> Result<Pattern, String> {
    Ok(match pattern {
        ast::Pattern::Wildcard => Pattern::Wildcard,
        ast::Pattern::Literal(expr) => Pattern::Literal(gen_expr(expr)?),
        ast::Pattern::Identifier(name) => Pattern::Identifier(name.clone()),
        ast::Pattern::Tuple(patterns) => Pattern::Tuple(
            patterns
                .iter()
                .map(gen_pattern)
                .collect::<Result<Vec<_>, String>>()?,
        ),
        ast::Pattern::Enum {
            enum_name,
            variant,
            inner,
        } => Pattern::Enum {
            enum_name: enum_name.clone(),
            variant: variant.clone(),
            inner: inner
                .as_ref()
                .map(|inner| {
                    inner
                        .iter()
                        .map(gen_pattern)
                        .collect::<Result<Vec<_>, String>>()
                })
                .transpose()?,
        },
        ast::Pattern::NamedFields { name, fields } => Pattern::NamedFields {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|(name, pattern)| Ok((name.clone(), gen_pattern(pattern)?)))
                .collect::<Result<Vec<_>, String>>()?,
        },
        ast::Pattern::Range {
            start,
            end,
            inclusive,
        } => Pattern::Range {
            start: gen_expr(start)?,
            end: gen_expr(end)?,
            inclusive: *inclusive,
        },
        ast::Pattern::Binding { name, pattern } => Pattern::Binding {
            name: name.clone(),
            pattern: Box::new(gen_pattern(pattern)?),
        },
    })
}

fn gen_loop_range_part(part: &ast::LoopRangePart) -> Result<LoopRangePart, String> {
    Ok(match part {
        ast::LoopRangePart::Range {
            start,
            end,
            inclusive,
            step,
        } => LoopRangePart::Range {
            start: Box::new(gen_expr(start)?),
            end: Box::new(gen_expr(end)?),
            inclusive: *inclusive,
            step: step.as_ref().map(gen_expr).transpose()?,
        },
        ast::LoopRangePart::Value(value) => LoopRangePart::Value(Box::new(gen_expr(value)?)),
    })
}

fn gen_get_expr(get: &ast::GetExpr) -> Result<GetExpr, String> {
    Ok(GetExpr {
        prompt: get
            .prompt
            .as_ref()
            .map(|prompt| gen_expr(prompt))
            .transpose()?
            .map(Box::new),
        source: get
            .source
            .as_ref()
            .map(|source| gen_expr(source))
            .transpose()?
            .map(Box::new),
        flags: get
            .flags
            .iter()
            .map(|flag| match flag {
                ast::GetFlag::Timeout(value) => Ok(GetFlag::Timeout(gen_expr(value)?)),
                ast::GetFlag::Default(value) => Ok(GetFlag::Default(gen_expr(value)?)),
                ast::GetFlag::Mask(value) => Ok(GetFlag::Mask(gen_expr(value)?)),
                ast::GetFlag::Until(value) => Ok(GetFlag::Until(gen_expr(value)?)),
                ast::GetFlag::Bytes(value) => Ok(GetFlag::Bytes(gen_expr(value)?)),
                ast::GetFlag::As(ty) => Ok(GetFlag::As(gen_type(ty)?)),
            })
            .collect::<Result<Vec<_>, String>>()?,
        with_clause: get
            .with_clause
            .as_ref()
            .map(|clause| -> Result<WithClause, String> {
                match clause {
                    ast::WithClause::Validate(expr) => Ok(WithClause::Validate(gen_expr(expr)?)),
                    ast::WithClause::Complete(expr) => Ok(WithClause::Complete(gen_expr(expr)?)),
                    ast::WithClause::Encoding(expr) => Ok(WithClause::Encoding(gen_expr(expr)?)),
                }
            })
            .transpose()?,
    })
}

fn gen_type(ty: &ast::TypeAnnotation) -> Result<Type, String> {
    Ok(match ty {
        ast::TypeAnnotation::Named(name) => Type::Named(name.clone()),
        ast::TypeAnnotation::Array(inner, size) => {
            Type::Array(Box::new(gen_type(inner)?), Box::new(gen_expr(size)?))
        }
        ast::TypeAnnotation::Tuple(types) => Type::Tuple(
            types
                .iter()
                .map(gen_type)
                .collect::<Result<Vec<_>, String>>()?,
        ),
        ast::TypeAnnotation::Vec(inner) => Type::Vec(Box::new(gen_type(inner)?)),
        ast::TypeAnnotation::Option(inner) => Type::Option(Box::new(gen_type(inner)?)),
        ast::TypeAnnotation::Result(ok, err) => {
            Type::Result(Box::new(gen_type(ok)?), Box::new(gen_type(err)?))
        }
        ast::TypeAnnotation::Reference(mutable, inner) => {
            Type::Reference(*mutable, Box::new(gen_type(inner)?))
        }
        ast::TypeAnnotation::Pointer(inner) => Type::Pointer(Box::new(gen_type(inner)?)),
        ast::TypeAnnotation::Nullable(inner) => Type::Nullable(Box::new(gen_type(inner)?)),
        ast::TypeAnnotation::Function { params, ret } => Type::Function {
            params: params
                .iter()
                .map(gen_type)
                .collect::<Result<Vec<_>, String>>()?,
            ret: Box::new(gen_type(ret)?),
        },
        ast::TypeAnnotation::Generic { name, args } => Type::Generic {
            name: name.clone(),
            args: args
                .iter()
                .map(gen_type)
                .collect::<Result<Vec<_>, String>>()?,
        },
    })
}

fn gen_binary_op(op: &ast::BinaryOp) -> BinaryOp {
    match op {
        ast::BinaryOp::Add => BinaryOp::Add,
        ast::BinaryOp::Sub => BinaryOp::Sub,
        ast::BinaryOp::Mul => BinaryOp::Mul,
        ast::BinaryOp::Div => BinaryOp::Div,
        ast::BinaryOp::Mod => BinaryOp::Mod,
        ast::BinaryOp::Eq => BinaryOp::Eq,
        ast::BinaryOp::NotEq => BinaryOp::NotEq,
        ast::BinaryOp::Lt => BinaryOp::Lt,
        ast::BinaryOp::Gt => BinaryOp::Gt,
        ast::BinaryOp::LtEq => BinaryOp::LtEq,
        ast::BinaryOp::GtEq => BinaryOp::GtEq,
        ast::BinaryOp::And => BinaryOp::And,
        ast::BinaryOp::Or => BinaryOp::Or,
        ast::BinaryOp::BitAnd => BinaryOp::BitAnd,
        ast::BinaryOp::BitOr => BinaryOp::BitOr,
        ast::BinaryOp::BitXor => BinaryOp::BitXor,
        ast::BinaryOp::Shl => BinaryOp::Shl,
        ast::BinaryOp::Shr => BinaryOp::Shr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poly_lexer::Lexer;
    use poly_parser::Parser;

    fn parse(source: &str) -> ast::Program {
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty(), "lexer errors: {errors:?}");
        let mut parser = Parser::new(&tokens);
        parser
            .parse()
            .unwrap_or_else(|e| panic!("source should parse: {e}"))
    }

    #[test]
    fn generates_functions_and_main_body() -> Result<(), Box<dyn std::error::Error>> {
        let ast =
            parse("fn sum(a: i32, b: i32): i32\n    return a + b\nend fn\nvar x := sum(1, 2)");
        let intermediate_representation = generate(&ast)?;

        assert_eq!(intermediate_representation.functions.len(), 1);
        assert_eq!(intermediate_representation.functions[0].name, "sum");
        assert_eq!(intermediate_representation.functions[0].params.len(), 2);
        assert_eq!(intermediate_representation.main_body.len(), 1);
        assert!(matches!(
            intermediate_representation.main_body[0],
            Statement::VarDecl { .. }
        ));
        Ok(())
    }

    #[test]
    fn generates_structs_enums_and_traits() -> Result<(), Box<dyn std::error::Error>> {
        let ast = parse(
            "struct Point\n    var x: i32\n    var y: i32\nend struct\n\
             enum Color\n    Red\n    Blue(i32)\nend enum\n\
             trait Drawable\n    fn draw(self)\nend trait",
        );
        let intermediate_representation = generate(&ast)?;

        assert_eq!(intermediate_representation.structs.len(), 1);
        assert_eq!(intermediate_representation.structs[0].fields.len(), 2);
        assert_eq!(intermediate_representation.enums.len(), 1);
        assert_eq!(intermediate_representation.enums[0].variants.len(), 2);
        assert_eq!(intermediate_representation.traits.len(), 1);
        Ok(())
    }

    #[test]
    fn constant_folding_input_is_preserved() -> Result<(), Box<dyn std::error::Error>> {
        let ast = parse("var x := 2 + 3");
        let intermediate_representation = generate(&ast)?;
        match &intermediate_representation.main_body[0] {
            Statement::VarDecl {
                value:
                    Some(Expr::BinaryOp {
                        op: BinaryOp::Add, ..
                    }),
                ..
            } => {}
            other => panic!("expected binary add, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn function_local_constants_lower_to_local_declarations(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // A `const` inside a function body is a local binding, not a
        // program-scope constant: it must lower to a `VarDecl` instead of
        // panicking with "constants are collected first".
        let ast = parse(
            "fn helper(): i32\n    const scale := 3\n    return 5 * scale\nend fn\
             \nfn main()\n    const local := 10\n    put local\nend fn",
        );
        let intermediate_representation = generate(&ast)?;
        assert_eq!(intermediate_representation.constants.len(), 0);
        assert_eq!(intermediate_representation.functions.len(), 2);
        for function in &intermediate_representation.functions {
            assert!(
                function
                    .body
                    .iter()
                    .any(|statement| matches!(statement, Statement::VarDecl { .. })),
                "expected `const` in `{}` to lower to VarDecl",
                function.name
            );
        }
        Ok(())
    }

    #[test]
    fn nested_extern_lowers_to_error() -> Result<(), Box<dyn std::error::Error>> {
        let mut program =
            parse("extern c fn double(value: i32): i32\nfn main()\n    put 1\nend fn");
        let nested_extern = program.statements.remove(0);
        match &mut program.statements[0].node {
            ast::Statement::FunctionDeclaration(function) => {
                function
                    .body
                    .get_or_insert_with(Vec::new)
                    .push(nested_extern);
            }
            other => panic!("expected function declaration first, got {other:?}"),
        }
        let Err(message) = generate(&program) else {
            panic!("nested extern must be rejected by IR lowering");
        };
        assert!(message.contains("top level"), "unexpected error: {message}");
        Ok(())
    }
}

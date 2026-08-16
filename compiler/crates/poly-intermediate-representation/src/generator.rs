//! AST → intermediate representation conversion.
//!
//! The generator lowers the parser's AST into the compiler intermediate representation.  Because the intermediate representation
//! intentionally mirrors the AST's statement/expression split, this conversion
//! is mostly structural; the optimizer and intermediate representation codegen benefit from the stable,
//! flat intermediate representation form afterwards.

use poly_parser::ast;

use crate::intermediate_representation::*;

/// Convert a parsed Poly program into intermediate representation.
pub fn generate(program: &ast::Program) -> Program {
    let mut intermediate_representation = Program::default();
    for spanned in &program.statements {
        let statement = &spanned.node;
        match statement {
            ast::Statement::FunctionDeclaration(function) => {
                intermediate_representation
                    .functions
                    .push(gen_function(function));
            }
            ast::Statement::StructDeclaration(struct_decl) => {
                intermediate_representation
                    .structs
                    .push(gen_struct(struct_decl));
            }
            ast::Statement::EnumDeclaration(enum_decl) => {
                intermediate_representation.enums.push(gen_enum(enum_decl));
            }
            ast::Statement::TraitDeclaration(trait_decl) => {
                intermediate_representation
                    .traits
                    .push(gen_trait(trait_decl));
            }
            ast::Statement::ImplDeclaration(impl_decl) => {
                intermediate_representation.impls.push(gen_impl(impl_decl));
            }
            ast::Statement::ModuleDeclaration(module) => {
                let mut statements = Vec::new();
                for spanned in &module.statements {
                    statements.push(gen_statement(&spanned.node));
                }
                intermediate_representation.modules.push(Module {
                    name: module.name.clone(),
                    statements,
                });
            }
            ast::Statement::ConstDeclaration { name, value } => {
                intermediate_representation.constants.push(Constant {
                    name: name.clone(),
                    value: gen_expr(value),
                    source_location: None,
                });
            }
            ast::Statement::TypeDeclaration(decl) => {
                intermediate_representation.type_aliases.push(TypeAlias {
                    name: decl.name.clone(),
                    ty: gen_type(&decl.ty),
                });
            }
            ast::Statement::UseDeclaration(decl) => {
                let mut path = decl.path.join("::");
                if let Some(alias) = &decl.alias {
                    path.push_str(&format!(" as {}", alias));
                }
                intermediate_representation.uses.push(path);
            }
            _ => {
                intermediate_representation
                    .main_body
                    .push(gen_statement(statement));
            }
        }
    }
    intermediate_representation
}

fn gen_function(function: &ast::FunctionDecl) -> Function {
    Function {
        name: function.name.clone(),
        params: function.params.iter().map(gen_parameter).collect(),
        return_type: function.return_type.as_ref().map(gen_type),
        body: function
            .body
            .as_ref()
            .map(|body| {
                body.iter()
                    .map(|spanned| gen_statement(&spanned.node))
                    .collect()
            })
            .unwrap_or_default(),
        is_async: function.is_async,
        generics: function.generics.iter().map(gen_generic_param).collect(),
        source_location: None,
    }
}

fn gen_generic_param(param: &ast::GenericParam) -> GenericParam {
    GenericParam {
        name: param.name.clone(),
        bounds: param.bounds.clone(),
    }
}

fn gen_parameter(parameter: &ast::Parameter) -> Parameter {
    Parameter {
        name: parameter.name.clone(),
        ty: gen_type(&parameter.ty),
        default: parameter.default.as_ref().map(gen_expr),
    }
}

fn gen_struct(struct_decl: &ast::StructDecl) -> Struct {
    Struct {
        name: struct_decl.name.clone(),
        fields: struct_decl
            .fields
            .iter()
            .map(|field| Field {
                name: field.name.clone(),
                ty: gen_type(&field.ty),
                default: field.default.as_ref().map(gen_expr),
            })
            .collect(),
        methods: struct_decl.methods.iter().map(gen_function).collect(),
        generics: struct_decl.generics.iter().map(gen_generic_param).collect(),
        source_location: None,
    }
}

fn gen_enum(enum_decl: &ast::EnumDecl) -> Enum {
    let variants = enum_decl
        .variants
        .iter()
        .map(|variant| match variant {
            ast::EnumVariant::Unit(name) => Variant {
                name: name.clone(),
                fields: Vec::new(),
                is_struct: false,
            },
            ast::EnumVariant::Tuple(name, types) => Variant {
                name: name.clone(),
                fields: types
                    .iter()
                    .map(|ty| Field {
                        name: String::new(),
                        ty: gen_type(ty),
                        default: None,
                    })
                    .collect(),
                is_struct: false,
            },
            ast::EnumVariant::Struct(name, fields) => Variant {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|(field, ty)| Field {
                        name: field.clone(),
                        ty: gen_type(ty),
                        default: None,
                    })
                    .collect(),
                is_struct: true,
            },
        })
        .collect();
    Enum {
        name: enum_decl.name.clone(),
        variants,
        methods: enum_decl.methods.iter().map(gen_function).collect(),
        source_location: None,
    }
}

fn gen_trait(trait_decl: &ast::TraitDecl) -> Trait {
    Trait {
        name: trait_decl.name.clone(),
        methods: trait_decl.methods.iter().map(gen_function).collect(),
        source_location: None,
    }
}

fn gen_impl(impl_decl: &ast::ImplDecl) -> Impl {
    Impl {
        trait_name: impl_decl.trait_name.clone(),
        type_name: impl_decl.type_name.clone(),
        methods: impl_decl.methods.iter().map(gen_function).collect(),
        source_location: None,
    }
}

fn gen_statement(statement: &ast::Statement) -> Statement {
    match statement {
        ast::Statement::VarDeclaration { name, ty, value } => Statement::VarDecl {
            name: name.clone(),
            ty: ty.as_ref().map(gen_type),
            value: value.as_ref().map(gen_expr),
        },
        ast::Statement::LetDeclaration { name, ty, value } => Statement::LetDecl {
            name: name.clone(),
            ty: ty.as_ref().map(gen_type),
            value: gen_expr(value),
        },
        ast::Statement::ConstDeclaration { .. } => {
            unreachable!("constants are collected first")
        }
        ast::Statement::Set { target, value } => Statement::Assignment {
            target: gen_expr(target),
            value: gen_expr(value),
        },
        ast::Statement::Mutation { target, op, value } => Statement::Mutation {
            target: gen_expr(target),
            op: match op {
                ast::MutationOp::Add => MutationOp::Add,
                ast::MutationOp::Sub => MutationOp::Sub,
                ast::MutationOp::Inc => MutationOp::Inc,
                ast::MutationOp::Dec => MutationOp::Dec,
            },
            value: value.as_ref().map(gen_expr),
        },
        ast::Statement::FunctionDeclaration(function) => {
            Statement::NestedFunction(gen_function(function))
        }
        ast::Statement::StructDeclaration(_) => unreachable!("structs are collected first"),
        ast::Statement::EnumDeclaration(_) => unreachable!("enums are collected first"),
        ast::Statement::TraitDeclaration(_) => unreachable!("traits are collected first"),
        ast::Statement::ImplDeclaration(_) => unreachable!("impls are collected first"),
        ast::Statement::ModuleDeclaration(_) => unreachable!("modules are collected first"),
        ast::Statement::UseDeclaration(_) => unreachable!("use declarations are collected first"),
        ast::Statement::TypeDeclaration(_) => unreachable!("type aliases are collected first"),
        ast::Statement::ExpressionStatement(expr) => Statement::Expression(gen_expr(expr)),
        ast::Statement::ReturnStatement(value) => Statement::Return(value.as_ref().map(gen_expr)),
        ast::Statement::BreakStatement => Statement::Break,
        ast::Statement::ContinueStatement => Statement::Continue,
        ast::Statement::PutStatement {
            no_newline,
            expr,
            redirect,
        } => Statement::Put {
            no_newline: *no_newline,
            expr: gen_expr(expr),
            redirect: redirect.as_ref().map(|redirect| match redirect {
                ast::Redirect::Write(path) => Redirect::Write(gen_expr(path)),
                ast::Redirect::Append(path) => Redirect::Append(gen_expr(path)),
            }),
        },
        ast::Statement::ErrorStatement(expr) => Statement::Error(gen_expr(expr)),
        ast::Statement::WarnStatement(expr) => Statement::Warn(gen_expr(expr)),
        ast::Statement::InfoStatement(expr) => Statement::Info(gen_expr(expr)),
        ast::Statement::Block(statements) => Statement::Block(
            statements
                .iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect(),
        ),
    }
}

fn gen_expr(expr: &ast::Expression) -> Expr {
    match expr {
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
            left: Box::new(gen_expr(left)),
            right: Box::new(gen_expr(right)),
        },
        ast::Expression::UnaryOp { op, expr } => Expr::UnaryOp {
            op: match op {
                ast::UnaryOp::Neg => UnaryOp::Neg,
                ast::UnaryOp::Not => UnaryOp::Not,
                ast::UnaryOp::BitNot => UnaryOp::BitNot,
                ast::UnaryOp::Deref => UnaryOp::Deref,
            },
            expr: Box::new(gen_expr(expr)),
        },
        ast::Expression::Call { func, args } => Expr::Call {
            func: Box::new(gen_expr(func)),
            args: args.iter().map(gen_expr).collect(),
        },
        ast::Expression::MethodCall {
            object,
            method,
            args,
        } => Expr::MethodCall {
            object: Box::new(gen_expr(object)),
            method: method.clone(),
            args: args.iter().map(gen_expr).collect(),
        },
        ast::Expression::Index { object, index } => Expr::Index {
            object: Box::new(gen_expr(object)),
            index: Box::new(gen_expr(index)),
        },
        ast::Expression::FieldAccess { object, field } => Expr::FieldAccess {
            object: Box::new(gen_expr(object)),
            field: field.clone(),
        },
        ast::Expression::Parenthesized(expr) => Expr::Parenthesized(Box::new(gen_expr(expr))),
        ast::Expression::IfExpression {
            condition,
            then_block,
            else_block,
        } => Expr::If {
            condition: Box::new(gen_expr(condition)),
            then_block: then_block
                .iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect(),
            else_block: else_block.as_ref().map(|block| {
                block
                    .iter()
                    .map(|spanned| gen_statement(&spanned.node))
                    .collect()
            }),
        },
        ast::Expression::MatchExpression { scrutinee, arms } => Expr::Match {
            scrutinee: Box::new(gen_expr(scrutinee)),
            arms: arms.iter().map(gen_match_arm).collect(),
        },
        ast::Expression::Closure { params, body } => Expr::Closure {
            params: params.iter().map(gen_parameter).collect(),
            body: Box::new(gen_expr(body)),
        },
        ast::Expression::ArrayLiteral(elements) => {
            Expr::Array(elements.iter().map(gen_expr).collect())
        }
        ast::Expression::TupleLiteral(elements) => {
            Expr::Tuple(elements.iter().map(gen_expr).collect())
        }
        ast::Expression::StructLiteral { name, fields } => Expr::Struct {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| (name.clone(), gen_expr(value)))
                .collect(),
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
                .map(|data| data.iter().map(gen_expr).collect()),
        },
        ast::Expression::LoopRange {
            variable,
            ranges,
            body,
        } => Expr::LoopRange {
            variable: variable.clone(),
            ranges: ranges.iter().map(gen_loop_range_part).collect(),
            body: body
                .iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect(),
        },
        ast::Expression::ForLoop {
            variable,
            iterable,
            body,
        } => Expr::ForLoop {
            variable: variable.clone(),
            iterable: Box::new(gen_expr(iterable)),
            body: body
                .iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect(),
        },
        ast::Expression::InfiniteLoop(body) => Expr::InfiniteLoop(
            body.iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect(),
        ),
        ast::Expression::AsExpression { expr, ty } => Expr::As {
            expr: Box::new(gen_expr(expr)),
            ty: gen_type(ty),
        },
        ast::Expression::TryExpression(expr) => Expr::Try(Box::new(gen_expr(expr))),
        ast::Expression::GetExpression(get) => Expr::Get(Box::new(gen_get_expr(get))),
        ast::Expression::UnsafeBlock(statements) => Expr::UnsafeBlock(
            statements
                .iter()
                .map(|spanned| gen_statement(&spanned.node))
                .collect(),
        ),
        ast::Expression::Range {
            start,
            end,
            inclusive,
        } => Expr::Range {
            start: Box::new(gen_expr(start)),
            end: Box::new(gen_expr(end)),
            inclusive: *inclusive,
        },
    }
}

fn gen_match_arm(arm: &ast::MatchArm) -> MatchArm {
    MatchArm {
        pattern: gen_pattern(&arm.pattern),
        guard: arm.guard.as_ref().map(gen_expr),
        body: match &arm.body {
            ast::MatchArmBody::Expression(expr) => MatchArmBody::Expression(gen_expr(expr)),
            ast::MatchArmBody::Block(statements) => MatchArmBody::Block(
                statements
                    .iter()
                    .map(|spanned| gen_statement(&spanned.node))
                    .collect(),
            ),
        },
    }
}

fn gen_pattern(pattern: &ast::Pattern) -> Pattern {
    match pattern {
        ast::Pattern::Wildcard => Pattern::Wildcard,
        ast::Pattern::Literal(expr) => Pattern::Literal(gen_expr(expr)),
        ast::Pattern::Identifier(name) => Pattern::Identifier(name.clone()),
        ast::Pattern::Tuple(patterns) => Pattern::Tuple(patterns.iter().map(gen_pattern).collect()),
        ast::Pattern::Enum {
            enum_name,
            variant,
            inner,
        } => Pattern::Enum {
            enum_name: enum_name.clone(),
            variant: variant.clone(),
            inner: inner
                .as_ref()
                .map(|inner| inner.iter().map(gen_pattern).collect()),
        },
        ast::Pattern::NamedFields { name, fields } => Pattern::NamedFields {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|(name, pattern)| (name.clone(), gen_pattern(pattern)))
                .collect(),
        },
        ast::Pattern::Range {
            start,
            end,
            inclusive,
        } => Pattern::Range {
            start: gen_expr(start),
            end: gen_expr(end),
            inclusive: *inclusive,
        },
        ast::Pattern::Binding { name, pattern } => Pattern::Binding {
            name: name.clone(),
            pattern: Box::new(gen_pattern(pattern)),
        },
    }
}

fn gen_loop_range_part(part: &ast::LoopRangePart) -> LoopRangePart {
    match part {
        ast::LoopRangePart::Range {
            start,
            end,
            inclusive,
            step,
        } => LoopRangePart::Range {
            start: Box::new(gen_expr(start)),
            end: Box::new(gen_expr(end)),
            inclusive: *inclusive,
            step: step.as_ref().map(gen_expr),
        },
        ast::LoopRangePart::Value(value) => LoopRangePart::Value(Box::new(gen_expr(value))),
    }
}

fn gen_get_expr(get: &ast::GetExpr) -> GetExpr {
    GetExpr {
        prompt: get.prompt.as_ref().map(|prompt| Box::new(gen_expr(prompt))),
        source: get.source.as_ref().map(|source| Box::new(gen_expr(source))),
        flags: get
            .flags
            .iter()
            .map(|flag| match flag {
                ast::GetFlag::Timeout(value) => GetFlag::Timeout(gen_expr(value)),
                ast::GetFlag::Default(value) => GetFlag::Default(gen_expr(value)),
                ast::GetFlag::Mask(value) => GetFlag::Mask(gen_expr(value)),
                ast::GetFlag::Until(value) => GetFlag::Until(gen_expr(value)),
                ast::GetFlag::Bytes(value) => GetFlag::Bytes(gen_expr(value)),
                ast::GetFlag::As(ty) => GetFlag::As(gen_type(ty)),
            })
            .collect(),
        with_clause: get.with_clause.as_ref().map(|clause| match clause {
            ast::WithClause::Validate(expr) => WithClause::Validate(gen_expr(expr)),
            ast::WithClause::Complete(expr) => WithClause::Complete(gen_expr(expr)),
            ast::WithClause::Encoding(expr) => WithClause::Encoding(gen_expr(expr)),
        }),
    }
}

fn gen_type(ty: &ast::TypeAnnotation) -> Type {
    match ty {
        ast::TypeAnnotation::Named(name) => Type::Named(name.clone()),
        ast::TypeAnnotation::Array(inner, size) => {
            Type::Array(Box::new(gen_type(inner)), Box::new(gen_expr(size)))
        }
        ast::TypeAnnotation::Tuple(types) => Type::Tuple(types.iter().map(gen_type).collect()),
        ast::TypeAnnotation::Vec(inner) => Type::Vec(Box::new(gen_type(inner))),
        ast::TypeAnnotation::Option(inner) => Type::Option(Box::new(gen_type(inner))),
        ast::TypeAnnotation::Result(ok, err) => {
            Type::Result(Box::new(gen_type(ok)), Box::new(gen_type(err)))
        }
        ast::TypeAnnotation::Reference(mutable, inner) => {
            Type::Reference(*mutable, Box::new(gen_type(inner)))
        }
        ast::TypeAnnotation::Pointer(inner) => Type::Pointer(Box::new(gen_type(inner))),
        ast::TypeAnnotation::Nullable(inner) => Type::Nullable(Box::new(gen_type(inner))),
        ast::TypeAnnotation::Function { params, ret } => Type::Function {
            params: params.iter().map(gen_type).collect(),
            ret: Box::new(gen_type(ret)),
        },
        ast::TypeAnnotation::Generic { name, args } => Type::Generic {
            name: name.clone(),
            args: args.iter().map(gen_type).collect(),
        },
    }
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
        parser.parse().expect("source should parse")
    }

    #[test]
    fn generates_functions_and_main_body() {
        let ast =
            parse("fn sum(a: i32, b: i32): i32\n    return a + b\nend fn\nvar x := sum(1, 2)");
        let intermediate_representation = generate(&ast);

        assert_eq!(intermediate_representation.functions.len(), 1);
        assert_eq!(intermediate_representation.functions[0].name, "sum");
        assert_eq!(intermediate_representation.functions[0].params.len(), 2);
        assert_eq!(intermediate_representation.main_body.len(), 1);
        assert!(matches!(
            intermediate_representation.main_body[0],
            Statement::VarDecl { .. }
        ));
    }

    #[test]
    fn generates_structs_enums_and_traits() {
        let ast = parse(
            "struct Point\n    var x: i32\n    var y: i32\nend struct\n\
             enum Color\n    Red\n    Blue(i32)\nend enum\n\
             trait Drawable\n    fn draw(self)\nend trait",
        );
        let intermediate_representation = generate(&ast);

        assert_eq!(intermediate_representation.structs.len(), 1);
        assert_eq!(intermediate_representation.structs[0].fields.len(), 2);
        assert_eq!(intermediate_representation.enums.len(), 1);
        assert_eq!(intermediate_representation.enums[0].variants.len(), 2);
        assert_eq!(intermediate_representation.traits.len(), 1);
    }

    #[test]
    fn constant_folding_input_is_preserved() {
        let ast = parse("var x := 2 + 3");
        let intermediate_representation = generate(&ast);
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
    }
}

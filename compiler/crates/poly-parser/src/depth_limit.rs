//! Post-parse AST nesting limit.
//!
//! # Why this exists
//!
//! [`MAX_EXPRESSION_DEPTH`](crate::parser::MAX_EXPRESSION_DEPTH) bounds
//! *syntactic* nesting while parsing, but it cannot bound the depth of the tree
//! that parsing produces. Left-associative operator chains have no nested
//! delimiters at all, so they pass the parse-time guard at any length while
//! building a left-nested tree one level deep per operand:
//!
//! ```text
//! var x i32 := 1 + 1 + 1 + ... + 1   // no parens; parses fine forever
//! ```
//!
//! Every later phase then recurses over that tree once per level -- the type
//! checker's `check_expression`, each backend's expression emitter, and Rust's
//! own recursive `Drop` for `Box<Expression>`. A chain of a few hundred
//! thousand operands therefore aborts the process with
//! `fatal runtime error: stack overflow` rather than producing a diagnostic.
//!
//! The parse-time guard is the wrong place to fix this, because the depth only
//! becomes knowable once the whole chain has been folded into a tree. So this
//! module measures the finished tree and rejects programs that are too deep,
//! which bounds every downstream phase at once.
//!
//! # Why an explicit stack
//!
//! [`ast_depth`] walks the tree iteratively with a heap-allocated worklist. A
//! recursive walk would reintroduce exactly the overflow it is meant to
//! prevent: validating a 200k-deep tree by recursion would overflow before it
//! could report the error.

use crate::ast::{
    Block, EnumVariant, Expression, FunctionDecl, GetExpr, GetFlag, LoopRangePart, MatchArmBody,
    Parameter, Pattern, Program, Redirect, Statement, TypeAnnotation, WithClause,
};

/// Maximum AST nesting depth accepted for a parsed program.
///
/// This counts every level -- statements, blocks, and expressions -- whereas
/// [`MAX_OPERAND_CHAIN`](crate::parser::MAX_OPERAND_CHAIN) counts only the
/// operands of one left-associative chain. A chain at the operand cap nests one
/// expression level per operand on top of the few levels its enclosing
/// function and statement add, so the depth limit is the operand cap plus
/// headroom. Deriving it that way keeps the two checks from disagreeing: a
/// chain the parser accepts must never be rejected a moment later by this
/// check for being "too deep".
///
/// The headroom still leaves the whole limit far above anything hand-written,
/// while keeping every recursive walk in the pipeline (the type checker's
/// `check_expression`, each backend, and `Drop`) well inside the stack.
pub const MAX_AST_DEPTH: usize = crate::parser::MAX_OPERAND_CHAIN + 64;

/// Measure the maximum nesting depth of a parsed program.
///
/// Returns `Ok(depth)` when the tree fits within [`MAX_AST_DEPTH`], and
/// `Err(depth)` with the depth actually measured otherwise, so callers can
/// report how far over the limit the input was.
///
/// Iteration is explicit (see the module docs); this function is safe to call
/// on a tree deep enough to overflow a recursive walk.
pub fn ast_depth(program: &Program) -> Result<usize, usize> {
    // The deepest nesting seen so far, and a heap worklist of nodes still to
    // visit (each paired with its depth).
    let mut deepest = 0usize;
    let mut work: Vec<(Work<'_>, usize)> = Vec::new();

    // Start at depth 1 with every top-level statement.
    for statement in &program.statements {
        work.push((Work::Statement(&statement.node), 1));
    }

    // Popping from the end keeps the traversal depth-first without recursion.
    while let Some((item, depth)) = work.pop() {
        if depth > deepest {
            deepest = depth;
            if deepest > MAX_AST_DEPTH {
                // The caller only needs the fact that the limit was exceeded.
                return Err(deepest);
            }
        }
        // Children sit one level deeper than the node that contains them.
        let inner = depth + 1;
        match item {
            Work::Statement(statement) => statement_children(statement, inner, &mut work),
            Work::Expression(expression) => expression_children(expression, inner, &mut work),
            Work::Pattern(pattern) => pattern_children(pattern, inner, &mut work),
            Work::TypeAnnotation(annotation) => {
                // `[T; N]` embeds an expression (the array length) alongside
                // its child type; measure both.
                if let TypeAnnotation::Array(_, length) = annotation {
                    work.push((Work::Expression(length.as_ref()), inner));
                }
                for child in type_children(annotation) {
                    work.push((Work::TypeAnnotation(child), inner));
                }
            }
        }
    }

    Ok(deepest)
}

/// A pending node in the iterative [`ast_depth`] walk.
///
/// Borrows keep the walk allocation-light: the worklist holds pointers into the
/// caller's tree rather than cloning nodes.
enum Work<'a> {
    Statement(&'a Statement),
    Expression(&'a Expression),
    Pattern(&'a Pattern),
    TypeAnnotation(&'a TypeAnnotation),
}

/// Queue every statement in `block` for visiting at `depth`.
fn push_block<'a>(block: &'a Block, depth: usize, work: &mut Vec<(Work<'a>, usize)>) {
    for statement in block {
        work.push((Work::Statement(&statement.node), depth));
    }
}

/// Queue a function's parameters and body for visiting at `depth`.
fn push_function<'a>(function: &'a FunctionDecl, depth: usize, work: &mut Vec<(Work<'a>, usize)>) {
    if let Some(return_type) = &function.return_type {
        work.push((Work::TypeAnnotation(return_type), depth));
    }
    for parameter in &function.params {
        push_parameter(parameter, depth, work);
    }
    if let Some(body) = &function.body {
        push_block(body, depth, work);
    }
}

fn push_parameter<'a>(parameter: &'a Parameter, depth: usize, work: &mut Vec<(Work<'a>, usize)>) {
    work.push((Work::TypeAnnotation(&parameter.ty), depth));
    if let Some(default) = &parameter.default {
        work.push((Work::Expression(default), depth));
    }
}

fn push_methods<'a>(methods: &'a [FunctionDecl], depth: usize, work: &mut Vec<(Work<'a>, usize)>) {
    for method in methods {
        push_function(method, depth, work);
    }
}

/// Queue the child nodes directly contained in `statement`.
///
/// Every statement variant must list its children here; missing one would let
/// a deep tree escape the depth check.
fn statement_children<'a>(
    statement: &'a Statement,
    inner: usize,
    work: &mut Vec<(Work<'a>, usize)>,
) {
    match statement {
        Statement::VarDeclaration { ty, value, .. } => {
            if let Some(ty) = ty {
                work.push((Work::TypeAnnotation(ty), inner));
            }
            if let Some(value) = value {
                work.push((Work::Expression(value), inner));
            }
        }
        Statement::LetDeclaration { ty, value, .. } => {
            if let Some(ty) = ty {
                work.push((Work::TypeAnnotation(ty), inner));
            }
            work.push((Work::Expression(value), inner));
        }
        Statement::ConstDeclaration { value, .. } | Statement::ExpressionStatement(value) => {
            work.push((Work::Expression(value), inner));
        }
        Statement::Assignment { target, value } => {
            work.push((Work::Expression(target), inner));
            work.push((Work::Expression(value), inner));
        }
        Statement::ReturnStatement(value) => {
            if let Some(value) = value {
                work.push((Work::Expression(value), inner));
            }
        }
        Statement::PutStatement { expr, redirect } => {
            work.push((Work::Expression(expr), inner));
            if let Some(Redirect::Write(path) | Redirect::Append(path) | Redirect::Prepend(path)) =
                redirect
            {
                work.push((Work::Expression(path), inner));
            }
        }
        Statement::ErrorStatement(expression)
        | Statement::WarnStatement(expression)
        | Statement::InfoStatement(expression) => {
            work.push((Work::Expression(expression), inner));
        }
        Statement::Block(block) => push_block(block, inner, work),
        Statement::FunctionDeclaration(function) => push_function(function, inner, work),
        Statement::StructDeclaration(decl) => {
            for field in &decl.fields {
                work.push((Work::TypeAnnotation(&field.ty), inner));
                if let Some(default) = &field.default {
                    work.push((Work::Expression(default), inner));
                }
            }
            push_methods(&decl.methods, inner, work);
        }
        Statement::EnumDeclaration(decl) => {
            for variant in &decl.variants {
                match variant {
                    EnumVariant::Unit(_) => {}
                    EnumVariant::Tuple(_, types) => {
                        for ty in types {
                            work.push((Work::TypeAnnotation(ty), inner));
                        }
                    }
                    EnumVariant::Struct(_, fields) => {
                        for (_, ty) in fields {
                            work.push((Work::TypeAnnotation(ty), inner));
                        }
                    }
                }
            }
            push_methods(&decl.methods, inner, work);
        }
        Statement::TraitDeclaration(decl) => push_methods(&decl.methods, inner, work),
        Statement::ImplDeclaration(decl) => push_methods(&decl.methods, inner, work),
        Statement::ModuleDeclaration(module) => push_block(&module.statements, inner, work),
        Statement::ExternFunctionDeclaration(decl) => {
            for parameter in &decl.params {
                push_parameter(parameter, inner, work);
            }
            if let Some(return_type) = &decl.return_type {
                work.push((Work::TypeAnnotation(return_type), inner));
            }
        }
        Statement::TypeDeclaration(decl) => {
            work.push((Work::TypeAnnotation(&decl.ty), inner));
        }
        // No nested nodes.
        Statement::BreakStatement
        | Statement::ContinueStatement
        | Statement::UseDeclaration(_)
        | Statement::DependencyDeclaration(_)
        | Statement::ForeignBlock { .. } => {}
    }
}

/// Queue the child nodes directly contained in `expression`.
fn expression_children<'a>(
    expression: &'a Expression,
    inner: usize,
    work: &mut Vec<(Work<'a>, usize)>,
) {
    match expression {
        Expression::BinaryOp { left, right, .. } => {
            work.push((Work::Expression(left), inner));
            work.push((Work::Expression(right), inner));
        }
        Expression::UnaryOp { expr, .. } => work.push((Work::Expression(expr), inner)),
        Expression::Call { func, args }
        | Expression::MethodCall {
            object: func, args, ..
        } => {
            work.push((Work::Expression(func), inner));
            for argument in args {
                work.push((Work::Expression(argument), inner));
            }
        }
        Expression::Index { object, index } => {
            work.push((Work::Expression(object), inner));
            work.push((Work::Expression(index), inner));
        }
        Expression::FieldAccess { object, .. } | Expression::TupleIndex { object, .. } => {
            work.push((Work::Expression(object), inner));
        }
        Expression::Parenthesized(inner_expr) | Expression::TryExpression(inner_expr) => {
            work.push((Work::Expression(inner_expr), inner));
        }
        Expression::IfExpression {
            condition,
            then_block,
            else_block,
        } => {
            work.push((Work::Expression(condition), inner));
            push_block(then_block, inner, work);
            if let Some(else_block) = else_block {
                push_block(else_block, inner, work);
            }
        }
        Expression::WhileLoop { condition, body } => {
            work.push((Work::Expression(condition), inner));
            push_block(body, inner, work);
        }
        Expression::MatchExpression { scrutinee, arms } => {
            work.push((Work::Expression(scrutinee), inner));
            for arm in arms {
                work.push((Work::Pattern(&arm.pattern), inner));
                if let Some(guard) = &arm.guard {
                    work.push((Work::Expression(guard), inner));
                }
                match &arm.body {
                    MatchArmBody::Expression(expression) => {
                        work.push((Work::Expression(expression), inner));
                    }
                    MatchArmBody::Block(block) => push_block(block, inner, work),
                }
            }
        }
        Expression::Closure { params, body } => {
            for parameter in params {
                push_parameter(parameter, inner, work);
            }
            work.push((Work::Expression(body), inner));
        }
        Expression::ArrayLiteral(elements) | Expression::TupleLiteral(elements) => {
            for element in elements {
                work.push((Work::Expression(element), inner));
            }
        }
        Expression::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                work.push((Work::Expression(value), inner));
            }
        }
        Expression::EnumVariant { data, .. } => {
            if let Some(data) = data {
                for value in data {
                    work.push((Work::Expression(value), inner));
                }
            }
        }
        Expression::LoopRange { ranges, body, .. } => {
            for range in ranges {
                match range {
                    LoopRangePart::Range {
                        start, end, step, ..
                    } => {
                        work.push((Work::Expression(start), inner));
                        work.push((Work::Expression(end), inner));
                        if let Some(step) = step {
                            work.push((Work::Expression(step), inner));
                        }
                    }
                    LoopRangePart::Value(value) => work.push((Work::Expression(value), inner)),
                }
            }
            push_block(body, inner, work);
        }
        Expression::ForLoop { iterable, body, .. } => {
            work.push((Work::Expression(iterable), inner));
            push_block(body, inner, work);
        }
        Expression::InfiniteLoop(body) | Expression::UnsafeBlock(body) => {
            push_block(body, inner, work)
        }
        Expression::AsExpression { expr, ty } => {
            work.push((Work::Expression(expr), inner));
            work.push((Work::TypeAnnotation(ty), inner));
        }
        Expression::GetExpression(get) => push_get_children(get, inner, work),
        Expression::Range { start, end, .. } => {
            work.push((Work::Expression(start), inner));
            work.push((Work::Expression(end), inner));
        }
        // Leaves.
        Expression::IntLiteral(_)
        | Expression::FloatLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::UnicodeStringLiteral(_)
        | Expression::UnicodeCharLiteral(_)
        | Expression::BoolLiteral(_)
        | Expression::ByteLiteral(_)
        | Expression::Identifier(_) => {}
    }
}

/// Queue the child nodes reachable from a `get` expression.
///
/// Prompt/source/flag expressions nest, and the `as` flag embeds a type
/// annotation, so both kinds of child must enter the walk.
fn push_get_children<'a>(get: &'a GetExpr, depth: usize, work: &mut Vec<(Work<'a>, usize)>) {
    if let Some(prompt) = &get.prompt {
        work.push((Work::Expression(prompt.as_ref()), depth));
    }
    if let Some(source) = &get.source {
        work.push((Work::Expression(source.as_ref()), depth));
    }
    for flag in &get.flags {
        match flag {
            GetFlag::Timeout(expression)
            | GetFlag::Default(expression)
            | GetFlag::Mask(expression)
            | GetFlag::Until(expression)
            | GetFlag::Bytes(expression) => {
                work.push((Work::Expression(expression), depth));
            }
            GetFlag::As(annotation) => {
                work.push((Work::TypeAnnotation(annotation), depth));
            }
        }
    }
    if let Some(with_clause) = &get.with_clause {
        match with_clause {
            WithClause::Validate(expression)
            | WithClause::Complete(expression)
            | WithClause::Encoding(expression) => {
                work.push((Work::Expression(expression), depth));
            }
        }
    }
}

/// Queue the child nodes directly contained in a match `pattern`.
fn pattern_children<'a>(pattern: &'a Pattern, inner: usize, work: &mut Vec<(Work<'a>, usize)>) {
    match pattern {
        Pattern::Tuple(patterns) => {
            for child in patterns {
                work.push((Work::Pattern(child), inner));
            }
        }
        Pattern::Enum { inner: nested, .. } => {
            if let Some(nested) = nested {
                for child in nested {
                    work.push((Work::Pattern(child), inner));
                }
            }
        }
        Pattern::NamedFields { fields, .. } => {
            for (_, child) in fields {
                work.push((Work::Pattern(child), inner));
            }
        }
        Pattern::Binding {
            pattern: nested, ..
        } => work.push((Work::Pattern(nested), inner)),
        // Patterns can also embed expressions, which nest too.
        Pattern::Literal(expression) => work.push((Work::Expression(expression), inner)),
        Pattern::Range { start, end, .. } => {
            work.push((Work::Expression(start), inner));
            work.push((Work::Expression(end), inner));
        }
        Pattern::Wildcard | Pattern::Identifier(_) => {}
    }
}

/// Collect the immediate child types of a type annotation.
fn type_children<'a>(annotation: &'a TypeAnnotation) -> Vec<&'a TypeAnnotation> {
    let mut out: Vec<&'a TypeAnnotation> = Vec::new();
    match annotation {
        TypeAnnotation::Array(inner, _) => out.push(inner.as_ref()),
        TypeAnnotation::Tuple(items) => out.extend(items.iter()),
        TypeAnnotation::Vec(inner)
        | TypeAnnotation::Option(inner)
        | TypeAnnotation::Pointer(inner)
        | TypeAnnotation::Nullable(inner)
        | TypeAnnotation::Reference(_, inner) => out.push(inner.as_ref()),
        TypeAnnotation::Result(ok, err) => {
            out.push(ok.as_ref());
            out.push(err.as_ref());
        }
        TypeAnnotation::Function { params, ret } => {
            out.extend(params.iter());
            out.push(ret.as_ref());
        }
        TypeAnnotation::Generic { args, .. } => out.extend(args.iter()),
        TypeAnnotation::Named(_) => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinaryOp, Spanned};
    use crate::parser::Parser;
    use poly_lexer::token::Span;
    use poly_lexer::Lexer;

    fn parse(source: &str) -> Program {
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty(), "lexer errors: {errors:?}");
        let mut parser = Parser::new(&tokens);
        match parser.parse() {
            Ok(program) => program,
            Err(error) => panic!("source should parse: {error}"),
        }
    }

    fn chain(terms: usize) -> String {
        let joined = std::iter::repeat_n("1", terms)
            .collect::<Vec<_>>()
            .join(" + ");
        format!("fn main()\n  var x i32 := {joined}\n  put x\nend fn")
    }

    #[test]
    fn shallow_program_is_within_limit() {
        let program = parse("fn main()\n  put 1\nend fn");
        assert!(ast_depth(&program).is_ok());
    }

    #[test]
    fn moderate_chain_is_within_limit() {
        // Comfortably legal: a 200-term chain must keep compiling.
        let program = parse(&chain(200));
        match ast_depth(&program) {
            Ok(depth) => assert!(depth <= MAX_AST_DEPTH, "depth {depth} over limit"),
            Err(depth) => panic!("a 200-term chain should fit, measured {depth}"),
        }
    }

    #[test]
    fn very_long_chain_is_rejected_without_overflowing() {
        // The regression: this used to abort with a stack overflow. The parser
        // now refuses the chain while folding it, so the deep tree is never
        // built -- which matters, because rejecting a finished tree would still
        // require dropping it recursively.
        let (tokens, lexer_errors) = Lexer::lex(&chain(200_000));
        assert!(lexer_errors.is_empty(), "lexer errors: {lexer_errors:?}");
        let mut parser = Parser::new(&tokens);
        let Err(error) = parser.parse() else {
            panic!("a 200k operand chain must be rejected");
        };
        assert!(
            error.message.contains("operator chain"),
            "unexpected diagnostic: {}",
            error.message
        );
    }

    #[test]
    fn chain_just_under_the_operand_cap_is_accepted() {
        // Boundary check: the cap must not reject legitimate long chains.
        let source = chain(400);
        let (tokens, _) = Lexer::lex(&source);
        let mut parser = Parser::new(&tokens);
        match parser.parse() {
            Ok(program) => assert!(ast_depth(&program).is_ok()),
            Err(error) => panic!("a 400 operand chain is within the cap: {error}"),
        }
    }

    #[test]
    fn chain_exactly_at_the_operand_cap_is_accepted() {
        // The fold cap and the depth cap must not disagree: a chain the parser
        // accepts at exactly the cap must not be rejected a moment later as
        // "too deep" by the post-parse check.
        let source = chain(crate::parser::MAX_OPERAND_CHAIN);
        let (tokens, _) = Lexer::lex(&source);
        let mut parser = Parser::new(&tokens);
        match parser.parse() {
            Ok(program) => assert!(
                ast_depth(&program).is_ok(),
                "a chain at the operand cap must also fit the depth cap"
            ),
            Err(error) => panic!("a chain at the operand cap must be accepted: {error}"),
        }
    }

    #[test]
    fn chain_one_past_the_operand_cap_is_rejected() {
        let source = chain(crate::parser::MAX_OPERAND_CHAIN + 1);
        let (tokens, _) = Lexer::lex(&source);
        let mut parser = Parser::new(&tokens);
        assert!(
            parser.parse().is_err(),
            "one operand past the cap must be rejected"
        );
    }

    #[test]
    fn ast_depth_rejects_hand_built_over_deep_tree() {
        // The post-parse check is defense in depth for trees that arrive from
        // another route (macro expansion), so verify it reports rather than
        // recurses on a tree that is already built.
        let mut nested = Expression::IntLiteral("1".to_string());
        for _ in 0..(MAX_AST_DEPTH + 100) {
            nested = Expression::BinaryOp {
                op: BinaryOp::Add,
                left: Box::new(nested),
                right: Box::new(Expression::IntLiteral("1".to_string())),
            };
        }
        let program = Program {
            statements: vec![Spanned::new(
                Statement::ExpressionStatement(nested),
                Span::new(0, 0),
            )],
        };
        match ast_depth(&program) {
            Ok(depth) => panic!("over-deep tree must be reported, got Ok({depth})"),
            Err(measured) => assert!(measured > MAX_AST_DEPTH),
        }
    }

    #[test]
    fn nested_array_literals_are_measured() {
        // Arrays nest through a different AST variant than operator chains.
        let depth = MAX_AST_DEPTH + 50;
        let source = format!(
            "fn main()\n  var x := {}1{}\nend fn",
            "[".repeat(depth),
            "]".repeat(depth)
        );
        let (tokens, _) = Lexer::lex(&source);
        let mut parser = Parser::new(&tokens);
        if let Ok(program) = parser.parse() {
            assert!(ast_depth(&program).is_err());
        }
    }

    #[test]
    fn hand_built_left_nested_chain_is_measured_as_deep() {
        // Guards against a refactor that stops descending and passes the tests
        // above by accident: same node count, very different depth.
        let mut nested = Expression::IntLiteral("1".to_string());
        for _ in 0..100 {
            nested = Expression::BinaryOp {
                op: BinaryOp::Add,
                left: Box::new(nested),
                right: Box::new(Expression::IntLiteral("1".to_string())),
            };
        }
        let program = Program {
            statements: vec![Spanned::new(
                Statement::ExpressionStatement(nested),
                Span::new(0, 0),
            )],
        };
        match ast_depth(&program) {
            Ok(depth) => assert!(
                depth > 50,
                "left-nested chain should measure deep, got {depth}"
            ),
            Err(depth) => panic!("a 100-deep tree should fit, measured {depth}"),
        }
    }

    /// Wrap a single statement as a whole program.
    fn program_with(statement: Statement) -> Program {
        Program {
            statements: vec![Spanned::new(statement, Span::new(0, 0))],
        }
    }

    /// A type nested `levels` deep: `Option<Option<... i32>>`.
    fn deep_type(levels: usize) -> TypeAnnotation {
        let mut ty = TypeAnnotation::Named("i32".to_string());
        for _ in 0..levels {
            ty = TypeAnnotation::Option(Box::new(ty));
        }
        ty
    }

    /// An expression nested `levels` deep: `(((1 + 1) + 1) + ...)`.
    fn deep_expr(levels: usize) -> Expression {
        let mut expr = Expression::IntLiteral("1".to_string());
        for _ in 0..levels {
            expr = Expression::BinaryOp {
                op: BinaryOp::Add,
                left: Box::new(expr),
                right: Box::new(Expression::IntLiteral("1".to_string())),
            };
        }
        expr
    }

    fn assert_over_deep(program: Program, what: &str) {
        match ast_depth(&program) {
            Ok(depth) => panic!("{what}: deep subtree escaped the depth check (measured {depth})"),
            Err(measured) => assert!(
                measured > MAX_AST_DEPTH,
                "{what}: measured {measured}, expected over {MAX_AST_DEPTH}"
            ),
        }
    }

    #[test]
    fn deep_types_behind_every_declaration_are_measured() {
        // Regression: type annotations were skipped in several positions
        // (type aliases, var/let types, signatures, fields, enum payloads),
        // so a deep type behind them escaped the post-parse check entirely.
        let levels = MAX_AST_DEPTH + 10;

        assert_over_deep(
            program_with(Statement::TypeDeclaration(crate::ast::TypeDecl {
                name: "Deep".into(),
                ty: deep_type(levels),
            })),
            "type alias",
        );
        assert_over_deep(
            program_with(Statement::VarDeclaration {
                name: "x".into(),
                ty: Some(deep_type(levels)),
                value: None,
            }),
            "var type",
        );
        assert_over_deep(
            program_with(Statement::LetDeclaration {
                name: "x".into(),
                ty: Some(deep_type(levels)),
                value: Expression::IntLiteral("1".into()),
            }),
            "let type",
        );
        assert_over_deep(
            program_with(Statement::FunctionDeclaration(FunctionDecl {
                name: "f".into(),
                params: vec![],
                return_type: Some(deep_type(levels)),
                body: None,
                is_async: false,
                generics: vec![],
            })),
            "return type",
        );
        assert_over_deep(
            program_with(Statement::FunctionDeclaration(FunctionDecl {
                name: "f".into(),
                params: vec![Parameter {
                    name: "p".into(),
                    ty: deep_type(levels),
                    default: None,
                }],
                return_type: None,
                body: None,
                is_async: false,
                generics: vec![],
            })),
            "parameter type",
        );
        assert_over_deep(
            program_with(Statement::StructDeclaration(crate::ast::StructDecl {
                name: "S".into(),
                fields: vec![crate::ast::StructField {
                    mutable: false,
                    name: "f".into(),
                    ty: deep_type(levels),
                    default: None,
                }],
                methods: vec![],
                generics: vec![],
            })),
            "struct field type",
        );
        assert_over_deep(
            program_with(Statement::EnumDeclaration(crate::ast::EnumDecl {
                name: "E".into(),
                variants: vec![EnumVariant::Tuple("V".into(), vec![deep_type(levels)])],
                methods: vec![],
            })),
            "enum variant type",
        );
        assert_over_deep(
            program_with(Statement::ExternFunctionDeclaration(
                crate::ast::ExternFunctionDecl {
                    target: "rust".into(),
                    name: "g".into(),
                    params: vec![],
                    return_type: Some(deep_type(levels)),
                },
            )),
            "extern return type",
        );
    }

    #[test]
    fn deep_array_length_expression_is_measured() {
        // `[T; N]` keeps its length as an expression that the type walk alone
        // never returned; without a dispatch arm the subtree went unmeasured.
        let levels = MAX_AST_DEPTH + 10;
        assert_over_deep(
            program_with(Statement::TypeDeclaration(crate::ast::TypeDecl {
                name: "Arr".into(),
                ty: TypeAnnotation::Array(
                    Box::new(TypeAnnotation::Named("i32".into())),
                    Box::new(deep_expr(levels)),
                ),
            })),
            "array length expression",
        );
    }

    #[test]
    fn deep_get_as_flag_type_is_measured() {
        // The `as` flag of a `get` expression embeds a type annotation the
        // old expression-only children list skipped.
        let levels = MAX_AST_DEPTH + 10;
        let get = GetExpr {
            prompt: None,
            source: None,
            flags: vec![GetFlag::As(deep_type(levels))],
            with_clause: None,
        };
        assert_over_deep(
            program_with(Statement::ExpressionStatement(
                Expression::GetExpression(Box::new(get)),
            )),
            "get `as` flag type",
        );
    }
}

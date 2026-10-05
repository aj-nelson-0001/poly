//! Fast, linear-cost AST rendering for `poly --ast`.
//!
//! # Why not `{:#?}`
//!
//! The CLI used to print the AST with `println!("{:#?}", program)`. Rust's
//! alternate `Debug` formatting indents every line by the node's depth, so a
//! tree of `n` nodes nested `d` deep emits `O(n * d)` bytes. For the
//! left-associative chains that dominate real input that is quadratic, and the
//! formatting itself costs more than quadratic on top of that. Measured on a
//! synthetic left-nested tree:
//!
//! ```text
//! n= 500  {:#?} ->  1,507,503 bytes,    1.5 s   |  {:?} ->  5,003 bytes,  61 us
//! n=1000  {:#?} ->  6,015,003 bytes,   13.5 s   |  {:?} -> 10,003 bytes, 170 us
//! n=2000  {:#?} -> 24,030,003 bytes,  137.5 s   |  {:?} -> 20,003 bytes, 399 us
//! ```
//!
//! So `--ast` on a 1000-term expression took ~34 s and produced megabytes of
//! indentation — a resource-exhaustion vector for any editor or CI step that
//! shells out to inspect a file.
//!
//! # What this does instead
//!
//! [`render_program`] walks the tree with an explicit heap worklist and writes
//! straight into a [`String`]. Two properties follow:
//!
//! * **Linear output.** Indentation is capped at [`MAX_INDENT_LEVELS`], so each
//!   line costs at most a bounded prefix regardless of how deep the tree is.
//!   Structure below the cap is still shown; only the padding stops growing.
//! * **No recursion.** The worklist is iterative, so rendering cannot overflow
//!   the stack on a deep tree — the same failure mode the depth limits in
//!   [`crate::depth_limit`] bound elsewhere in the pipeline.
//!
//! The dump is a debugging aid, not a serialization format: nothing parses it,
//! and it is not intended to round-trip.

use crate::ast::*;

/// Maximum nesting levels that are indented.
///
/// Deeply nested input would otherwise pay `depth` spaces on every line, which
/// is what made the old output quadratic. Past this depth the printer keeps
/// emitting structure but stops adding indentation, so output stays linear in
/// the number of nodes. This is deep enough that ordinary code renders exactly
/// as it always has.
pub const MAX_INDENT_LEVELS: usize = 32;

/// Spaces added per indent level.
const INDENT_WIDTH: usize = 2;

/// Render a whole program.
///
/// Returns the complete dump as a string. Prefer [`write_program`] when the
/// output is going straight to a sink, to avoid materializing it.
pub fn render_program(program: &Program) -> String {
    let mut bytes: Vec<u8> = Vec::with_capacity(1024);
    // Writing into a `Vec<u8>` only fails on capacity overflow.
    let _ = write_program(program, &mut bytes);
    // The printer emits UTF-8; every literal it writes came from source text.
    String::from_utf8(bytes).unwrap_or_default()
}

/// Write a whole program to `out`.
///
/// I/O errors from the sink propagate; rendering itself cannot fail.
pub fn write_program<W: std::io::Write>(program: &Program, out: &mut W) -> std::io::Result<()> {
    Printer { out }.program(program)
}

/// One unit of work for the iterative walk.
enum Step<'a> {
    /// Emit literal text (already formatted) at the given depth.
    Raw(usize, String),
    /// Render a statement at the given depth.
    Statement(&'a Statement, usize),
    /// Render a function declaration held by a type declaration.
    Function(&'a FunctionDecl, usize),
    /// Render an expression at the given depth.
    Expression(&'a Expression, usize),
    /// Render a pattern at the given depth.
    Pattern(&'a Pattern, usize),
    /// Render a type annotation at the given depth.
    Type(&'a TypeAnnotation, usize),
    /// Render each statement of a block at `depth + 1`.
    Block(&'a Block, usize),
    /// Close the level opened by the matching header line.
    Close(usize),
}

struct Printer<'a, W: std::io::Write> {
    out: &'a mut W,
}

impl<W: std::io::Write> Printer<'_, W> {
    /// Write the indent prefix for `depth`, bounded by `MAX_INDENT_LEVELS`.
    ///
    /// Capping the indent is what keeps output linear: past the cap the depth
    /// number still grows but the number of leading spaces stops.
    fn indent(&mut self, depth: usize) -> std::io::Result<()> {
        // Never indent more than the cap, no matter how deep the node is.
        let levels = depth.min(MAX_INDENT_LEVELS);
        // A fixed-size buffer of spaces; we write only the prefix we need.
        let buffer = [b' '; MAX_INDENT_LEVELS * INDENT_WIDTH];
        let width = levels * INDENT_WIDTH;
        self.out.write_all(&buffer[..width])
    }

    /// Write one indented line: indent, text, newline.
    fn line(&mut self, depth: usize, text: &str) -> std::io::Result<()> {
        self.indent(depth)?;
        self.out.write_all(text.as_bytes())?;
        self.out.write_all(b"\n")
    }

    /// Open a line of the form `Header {` and queue the matching `}`.
    fn open<'a>(
        &mut self,
        work: &mut Vec<Step<'a>>,
        depth: usize,
        text: String,
    ) -> std::io::Result<()> {
        self.line(depth, &text)?;
        work.push(Step::Close(depth));
        Ok(())
    }

    fn program(&mut self, program: &Program) -> std::io::Result<()> {
        // The top-level header, then the whole program block one level in.
        self.line(0, "Program")?;
        let mut work: Vec<Step<'_>> = Vec::new();
        work.push(Step::Block(&program.statements, 1));
        self.run(&mut work)
    }

    /// Drain the worklist. The only place the traversal loops, which keeps it
    /// iterative no matter how deep the tree is.
    fn run<'a>(&mut self, work: &mut Vec<Step<'a>>) -> std::io::Result<()> {
        while let Some(step) = work.pop() {
            match step {
                Step::Raw(raw_depth, text) => self.line(raw_depth, &text)?,
                Step::Close(depth) => self.line(depth, "}")?,
                Step::Block(block, depth) => {
                    // This path pushes straight onto the worklist rather than
                    // going through `push_reversed`, so it reverses here.
                    for statement in block.iter().rev() {
                        work.push(Step::Statement(&statement.node, depth));
                    }
                }
                Step::Statement(statement, depth) => self.statement(statement, depth, work)?,
                Step::Function(function, depth) => self.function(function, depth, work)?,
                Step::Expression(expression, depth) => self.expression(expression, depth, work)?,
                Step::Pattern(pattern, depth) => self.pattern(pattern, depth, work)?,
                Step::Type(annotation, depth) => self.type_annotation(annotation, depth, work)?,
            }
        }
        Ok(())
    }

    /// Push children in reverse so the worklist pops them in source order.
    ///
    /// `children` borrows from the node being rendered, which lives at least as
    /// long as the worklist entry that produced it, so both share one lifetime.
    fn push_reversed<'a>(&self, work: &mut Vec<Step<'a>>, children: Vec<Step<'a>>) {
        work.extend(children.into_iter().rev());
    }

    fn expression<'a>(
        &mut self,
        expression: &'a Expression,
        depth: usize,
        work: &mut Vec<Step<'a>>,
    ) -> std::io::Result<()> {
        // Children are pushed in reverse; `run` pops them in order.
        let children: Vec<Step<'a>> = match expression {
            Expression::BinaryOp { op, left, right } => vec![
                Step::Raw(depth + 1, format!("op: {op:?}")),
                Step::Expression(left, depth + 1),
                Step::Expression(right, depth + 1),
            ],
            Expression::UnaryOp { op, expr } => {
                vec![
                    Step::Raw(depth + 1, format!("op: {op:?}")),
                    Step::Expression(expr, depth + 1),
                ]
            }
            Expression::Call { func, args } => {
                let mut children = vec![Step::Expression(func, depth + 1)];
                for argument in args.iter() {
                    children.push(Step::Expression(argument, depth + 1));
                }
                children
            }
            Expression::MethodCall {
                object,
                method,
                args,
            } => {
                let mut children = vec![
                    Step::Raw(depth + 1, format!("method: {method}")),
                    Step::Expression(object, depth + 1),
                ];
                for argument in args.iter() {
                    children.push(Step::Expression(argument, depth + 1));
                }
                children
            }
            Expression::Index { object, index } => vec![
                Step::Expression(object, depth + 1),
                Step::Expression(index, depth + 1),
            ],
            Expression::FieldAccess { object, field } => vec![
                Step::Raw(depth + 1, format!("field: {field}")),
                Step::Expression(object, depth + 1),
            ],
            Expression::TupleIndex { object, index } => vec![
                Step::Raw(depth + 1, format!("index: {index}")),
                Step::Expression(object, depth + 1),
            ],
            Expression::Parenthesized(inner) | Expression::TryExpression(inner) => {
                vec![Step::Expression(inner, depth + 1)]
            }
            Expression::IfExpression {
                condition,
                then_block,
                else_block,
            } => {
                let mut children = vec![Step::Raw(depth + 1, "condition:".to_string())];
                children.push(Step::Expression(condition, depth + 2));
                children.push(Step::Raw(depth + 1, "then:".to_string()));
                children.push(Step::Block(then_block, depth + 2));
                if let Some(else_block) = else_block {
                    children.push(Step::Raw(depth + 1, "else:".to_string()));
                    children.push(Step::Block(else_block, depth + 2));
                }
                children
            }
            Expression::WhileLoop { condition, body } => vec![
                Step::Raw(depth + 1, "condition:".to_string()),
                Step::Expression(condition, depth + 2),
                Step::Raw(depth + 1, "body:".to_string()),
                Step::Block(body, depth + 2),
            ],
            Expression::MatchExpression { scrutinee, arms } => {
                let mut children = vec![
                    Step::Raw(depth + 1, "scrutinee:".to_string()),
                    Step::Expression(scrutinee, depth + 2),
                ];
                for arm in arms.iter() {
                    children.push(Step::Raw(depth + 1, "arm:".to_string()));
                    children.push(Step::Pattern(&arm.pattern, depth + 2));
                    if let Some(guard) = &arm.guard {
                        children.push(Step::Raw(depth + 2, "guard:".to_string()));
                        children.push(Step::Expression(guard, depth + 3));
                    }
                    match &arm.body {
                        MatchArmBody::Expression(expression) => {
                            children.push(Step::Raw(depth + 2, "body:".to_string()));
                            children.push(Step::Expression(expression, depth + 3));
                        }
                        MatchArmBody::Block(block) => {
                            children.push(Step::Raw(depth + 2, "body:".to_string()));
                            children.push(Step::Block(block, depth + 3));
                        }
                    }
                }
                children
            }
            Expression::Closure { params, body } => {
                let mut children = Vec::new();
                for parameter in params.iter() {
                    children.push(Step::Raw(depth + 1, format!("param: {:?}", parameter.name)));
                    children.push(Step::Type(&parameter.ty, depth + 2));
                    if let Some(default) = &parameter.default {
                        children.push(Step::Raw(depth + 2, "default:".to_string()));
                        children.push(Step::Expression(default, depth + 3));
                    }
                }
                children.push(Step::Expression(body, depth + 1));
                children
            }
            Expression::ArrayLiteral(elements) | Expression::TupleLiteral(elements) => {
                let mut children = Vec::new();
                for element in elements.iter() {
                    children.push(Step::Expression(element, depth + 1));
                }
                children
            }
            Expression::StructLiteral { name, fields } => {
                let mut children = vec![Step::Raw(depth + 1, format!("struct: {name}"))];
                for (field, value) in fields.iter() {
                    children.push(Step::Raw(depth + 2, format!("field: {field}")));
                    children.push(Step::Expression(value, depth + 3));
                }
                children
            }
            Expression::EnumVariant {
                enum_name,
                variant,
                data,
            } => {
                let mut children = vec![Step::Raw(
                    depth + 1,
                    format!("enum: {enum_name} variant: {variant}"),
                )];
                if let Some(data) = data {
                    for value in data.iter() {
                        children.push(Step::Expression(value, depth + 2));
                    }
                }
                children
            }
            Expression::LoopRange {
                variable,
                ranges,
                body,
            } => {
                let mut children = vec![Step::Raw(depth + 1, format!("variable: {variable}"))];
                for range in ranges.iter() {
                    match range {
                        LoopRangePart::Range {
                            start,
                            end,
                            inclusive,
                            step,
                        } => {
                            children.push(Step::Raw(
                                depth + 1,
                                format!("range: inclusive={inclusive}"),
                            ));
                            children.push(Step::Expression(start, depth + 2));
                            children.push(Step::Expression(end, depth + 2));
                            if let Some(step) = step {
                                children.push(Step::Raw(depth + 2, "step:".to_string()));
                                children.push(Step::Expression(step, depth + 3));
                            }
                        }
                        LoopRangePart::Value(value) => {
                            children.push(Step::Raw(depth + 1, "value:".to_string()));
                            children.push(Step::Expression(value, depth + 2));
                        }
                    }
                }
                children.push(Step::Raw(depth + 1, "body:".to_string()));
                children.push(Step::Block(body, depth + 2));
                children
            }
            Expression::ForLoop {
                variable,
                iterable,
                body,
            } => vec![
                Step::Raw(depth + 1, format!("variable: {variable}")),
                Step::Raw(depth + 1, "iterable:".to_string()),
                Step::Expression(iterable, depth + 2),
                Step::Raw(depth + 1, "body:".to_string()),
                Step::Block(body, depth + 2),
            ],
            Expression::InfiniteLoop(body) | Expression::UnsafeBlock(body) => {
                vec![Step::Block(body, depth + 1)]
            }
            Expression::AsExpression { expr, ty } => {
                vec![Step::Expression(expr, depth + 1), Step::Type(ty, depth + 1)]
            }
            Expression::GetExpression(get) => {
                // The `GetExpr` shape is flat; its expressions are not, so they
                // are pulled out and walked like any other child.
                let mut children = Vec::new();
                for expression in get_expressions(get) {
                    children.push(Step::Expression(expression, depth + 2));
                }
                children.push(Step::Raw(depth + 1, format!("get: {:?}", get)));
                children
            }
            Expression::Range {
                start,
                end,
                inclusive,
            } => vec![
                Step::Raw(depth + 1, format!("inclusive: {inclusive}")),
                Step::Expression(start, depth + 1),
                Step::Expression(end, depth + 1),
            ],
            // Leaves: render inline, no child lines.
            Expression::IntLiteral(value) => return self.leaf(depth, format!("int {value}")),
            Expression::FloatLiteral(value) => return self.leaf(depth, format!("float {value}")),
            Expression::StringLiteral(value) => {
                return self.leaf(depth, format!("string {value:?}"))
            }
            Expression::UnicodeStringLiteral(value) => {
                return self.leaf(depth, format!("unicode string {value:?}"))
            }
            Expression::UnicodeCharLiteral(value) => {
                return self.leaf(depth, format!("unicode char {value:?}"))
            }
            Expression::BoolLiteral(value) => return self.leaf(depth, format!("bool {value}")),
            Expression::ByteLiteral(bytes) => return self.leaf(depth, format!("bytes {bytes:?}")),
            Expression::Identifier(name) => return self.leaf(depth, format!("ident {name}")),
        };

        self.open(work, depth, format!("{} {{", expression_kind(expression)))?;
        self.push_reversed(work, children);
        Ok(())
    }

    fn leaf(&mut self, depth: usize, text: String) -> std::io::Result<()> {
        self.line(depth, &text)
    }

    fn pattern<'a>(
        &mut self,
        pattern: &'a Pattern,
        depth: usize,
        work: &mut Vec<Step<'a>>,
    ) -> std::io::Result<()> {
        let children: Vec<Step<'a>> = match pattern {
            Pattern::Tuple(patterns) => patterns
                .iter()
                .map(|child| Step::Pattern(child, depth + 1))
                .collect(),
            Pattern::Enum {
                enum_name,
                variant,
                inner,
            } => {
                let mut children = vec![Step::Raw(
                    depth + 1,
                    format!("enum: {enum_name} variant: {variant}"),
                )];
                if let Some(inner) = inner {
                    for child in inner.iter() {
                        children.push(Step::Pattern(child, depth + 2));
                    }
                }
                children
            }
            Pattern::NamedFields { name, fields } => {
                let mut children = vec![Step::Raw(depth + 1, format!("name: {name}"))];
                for (field, child) in fields.iter() {
                    children.push(Step::Raw(depth + 2, format!("field: {field}")));
                    children.push(Step::Pattern(child, depth + 3));
                }
                children
            }
            Pattern::Binding { name, pattern } => vec![
                Step::Raw(depth + 1, format!("name: {name}")),
                Step::Pattern(pattern, depth + 1),
            ],
            Pattern::Range {
                start,
                end,
                inclusive,
            } => vec![
                Step::Raw(depth + 1, format!("inclusive: {inclusive}")),
                Step::Expression(start, depth + 1),
                Step::Expression(end, depth + 1),
            ],
            Pattern::Literal(expression) => vec![Step::Expression(expression, depth + 1)],
            Pattern::Wildcard => return self.leaf(depth, "_".to_string()),
            Pattern::Identifier(name) => return self.leaf(depth, format!("ident {name}")),
        };
        self.open(work, depth, "pattern {".to_string())?;
        self.push_reversed(work, children);
        Ok(())
    }

    fn type_annotation<'a>(
        &mut self,
        annotation: &'a TypeAnnotation,
        depth: usize,
        work: &mut Vec<Step<'a>>,
    ) -> std::io::Result<()> {
        let children: Vec<Step<'a>> = match annotation {
            TypeAnnotation::Named(name) => return self.leaf(depth, format!("type {name}")),
            TypeAnnotation::Array(inner, size) => vec![
                Step::Type(inner, depth + 1),
                Step::Raw(depth + 1, "size:".to_string()),
                Step::Expression(size, depth + 2),
            ],
            TypeAnnotation::Tuple(items) => items
                .iter()
                .map(|child| Step::Type(child, depth + 1))
                .collect(),
            TypeAnnotation::Vec(inner)
            | TypeAnnotation::Option(inner)
            | TypeAnnotation::Pointer(inner)
            | TypeAnnotation::Nullable(inner) => vec![Step::Type(inner, depth + 1)],
            TypeAnnotation::Reference(mutable, inner) => vec![
                Step::Raw(depth + 1, format!("mutable: {mutable}")),
                Step::Type(inner, depth + 1),
            ],
            TypeAnnotation::Result(ok, err) => vec![
                Step::Raw(depth + 1, "ok:".to_string()),
                Step::Type(ok, depth + 2),
                Step::Raw(depth + 1, "err:".to_string()),
                Step::Type(err, depth + 2),
            ],
            TypeAnnotation::Function { params, ret } => {
                let mut children = Vec::new();
                for parameter in params.iter() {
                    children.push(Step::Type(parameter, depth + 1));
                }
                children.push(Step::Raw(depth + 1, "returns:".to_string()));
                children.push(Step::Type(ret, depth + 2));
                children
            }
            TypeAnnotation::Generic { name, args } => {
                let mut children = vec![Step::Raw(depth + 1, format!("name: {name}"))];
                for argument in args.iter() {
                    children.push(Step::Type(argument, depth + 2));
                }
                children
            }
        };
        self.open(work, depth, "type {".to_string())?;
        self.push_reversed(work, children);
        Ok(())
    }

    /// Render a function declaration. Shared by top-level functions and by
    /// methods hanging off struct/enum/trait/impl declarations.
    fn function<'a>(
        &mut self,
        function: &'a FunctionDecl,
        depth: usize,
        work: &mut Vec<Step<'a>>,
    ) -> std::io::Result<()> {
        let mut children = Vec::new();
        for parameter in function.params.iter() {
            children.push(Step::Raw(depth + 1, format!("param: {:?}", parameter.name)));
            children.push(Step::Type(&parameter.ty, depth + 2));
            if let Some(default) = &parameter.default {
                children.push(Step::Raw(depth + 2, "default:".to_string()));
                children.push(Step::Expression(default, depth + 3));
            }
        }
        if let Some(return_type) = &function.return_type {
            children.push(Step::Raw(depth + 1, "returns:".to_string()));
            children.push(Step::Type(return_type, depth + 2));
        }
        if let Some(body) = &function.body {
            children.push(Step::Raw(depth + 1, "body:".to_string()));
            children.push(Step::Block(body, depth + 2));
        }
        self.open(work, depth, format!("fn {} {{", function.name))?;
        self.push_reversed(work, children);
        Ok(())
    }

    fn statement<'a>(
        &mut self,
        statement: &'a Statement,
        depth: usize,
        work: &mut Vec<Step<'a>>,
    ) -> std::io::Result<()> {
        let (header, children): (String, Vec<Step<'a>>) = match statement {
            Statement::VarDeclaration { name, ty, value } => {
                let mut children = Vec::new();
                if let Some(ty) = ty {
                    children.push(Step::Raw(depth + 1, "type:".to_string()));
                    children.push(Step::Type(ty, depth + 2));
                }
                if let Some(value) = value {
                    children.push(Step::Raw(depth + 1, "value:".to_string()));
                    children.push(Step::Expression(value, depth + 2));
                }
                (format!("var {name} {{"), children)
            }
            Statement::LetDeclaration { name, ty, value } => {
                let mut children = Vec::new();
                if let Some(ty) = ty {
                    children.push(Step::Raw(depth + 1, "type:".to_string()));
                    children.push(Step::Type(ty, depth + 2));
                }
                children.push(Step::Raw(depth + 1, "value:".to_string()));
                children.push(Step::Expression(value, depth + 2));
                (format!("let {name} {{"), children)
            }
            Statement::ConstDeclaration { name, value } => (
                format!("const {name} {{"),
                vec![
                    Step::Raw(depth + 1, "value:".to_string()),
                    Step::Expression(value, depth + 2),
                ],
            ),
            Statement::Assignment { target, value } => (
                "assign {".to_string(),
                vec![
                    Step::Raw(depth + 1, "target:".to_string()),
                    Step::Expression(target, depth + 2),
                    Step::Raw(depth + 1, "value:".to_string()),
                    Step::Expression(value, depth + 2),
                ],
            ),
            Statement::ExpressionStatement(expression) => {
                return self.expression(expression, depth, work);
            }
            Statement::ReturnStatement(value) => {
                let children = match value {
                    Some(value) => vec![
                        Step::Raw(depth + 1, "value:".to_string()),
                        Step::Expression(value, depth + 2),
                    ],
                    None => Vec::new(),
                };
                ("return {".to_string(), children)
            }
            Statement::PutStatement { expr, redirect } => {
                let mut children = vec![
                    Step::Raw(depth + 1, "value:".to_string()),
                    Step::Expression(expr, depth + 2),
                ];
                if let Some(redirect) = redirect {
                    children.push(Step::Raw(depth + 1, "redirect:".to_string()));
                    match redirect {
                        Redirect::Write(path) | Redirect::Append(path) => {
                            children.push(Step::Expression(path, depth + 2));
                        }
                    }
                }
                ("put {".to_string(), children)
            }
            Statement::ErrorStatement(expression) => (
                "error {".to_string(),
                vec![
                    Step::Raw(depth + 1, "value:".to_string()),
                    Step::Expression(expression, depth + 2),
                ],
            ),
            Statement::WarnStatement(expression) => (
                "warn {".to_string(),
                vec![
                    Step::Raw(depth + 1, "value:".to_string()),
                    Step::Expression(expression, depth + 2),
                ],
            ),
            Statement::InfoStatement(expression) => (
                "info {".to_string(),
                vec![
                    Step::Raw(depth + 1, "value:".to_string()),
                    Step::Expression(expression, depth + 2),
                ],
            ),
            Statement::Block(block) => {
                self.open(work, depth, "block {".to_string())?;
                work.push(Step::Block(block, depth + 1));
                return Ok(());
            }
            Statement::FunctionDeclaration(function) => {
                return self.function(function, depth, work);
            }
            Statement::StructDeclaration(decl) => {
                let mut children = Vec::new();
                for field in decl.fields.iter() {
                    children.push(Step::Raw(
                        depth + 1,
                        format!("field: {} mutable: {}", field.name, field.mutable),
                    ));
                    children.push(Step::Type(&field.ty, depth + 2));
                }
                for method in decl.methods.iter() {
                    children.push(Step::Function(method, depth + 1));
                }
                (format!("struct {} {{", decl.name), children)
            }
            Statement::EnumDeclaration(decl) => {
                let mut children = Vec::new();
                for variant in decl.variants.iter() {
                    children.push(Step::Raw(depth + 1, format!("variant: {variant:?}")));
                }
                for method in decl.methods.iter() {
                    children.push(Step::Function(method, depth + 1));
                }
                (format!("enum {} {{", decl.name), children)
            }
            Statement::TraitDeclaration(decl) => {
                let mut children = Vec::new();
                for method in decl.methods.iter() {
                    children.push(Step::Function(method, depth + 1));
                }
                (format!("trait {} {{", decl.name), children)
            }
            Statement::ImplDeclaration(decl) => {
                let mut children = Vec::new();
                for method in decl.methods.iter() {
                    children.push(Step::Function(method, depth + 1));
                }
                let trait_name = decl.trait_name.clone().unwrap_or_default();
                (
                    format!("impl {} for {} {{", trait_name, decl.type_name),
                    children,
                )
            }
            Statement::ModuleDeclaration(module) => (
                format!("module {} {{", module.name),
                vec![Step::Block(&module.statements, depth + 1)],
            ),
            Statement::UseDeclaration(decl) => {
                return self.leaf(depth, format!("use {}", decl.path.join("::")));
            }
            Statement::DependencyDeclaration(decl) => {
                return self.leaf(depth, format!("dep {} = {}", decl.name, decl.version));
            }
            Statement::TypeDeclaration(decl) => {
                let children = vec![Step::Type(&decl.ty, depth + 1)];
                (format!("type {} {{", decl.name), children)
            }
            Statement::ForeignBlock { language, content } => {
                return self.leaf(
                    depth,
                    format!("#{language} block ({} bytes)", content.len()),
                );
            }
            Statement::ExternFunctionDeclaration(decl) => {
                let mut children = Vec::new();
                for parameter in decl.params.iter() {
                    children.push(Step::Raw(depth + 1, format!("param: {:?}", parameter.name)));
                    children.push(Step::Type(&parameter.ty, depth + 2));
                }
                if let Some(return_type) = &decl.return_type {
                    children.push(Step::Raw(depth + 1, "returns:".to_string()));
                    children.push(Step::Type(return_type, depth + 2));
                }
                (
                    format!("extern {} fn {} {{", decl.target, decl.name),
                    children,
                )
            }
            Statement::BreakStatement => return self.leaf(depth, "break".to_string()),
            Statement::ContinueStatement => return self.leaf(depth, "continue".to_string()),
        };
        self.open(work, depth, header)?;
        self.push_reversed(work, children);
        Ok(())
    }
}

/// Short label for an expression node, used as the header of its block.
fn expression_kind(expression: &Expression) -> &'static str {
    match expression {
        Expression::BinaryOp { .. } => "binary",
        Expression::UnaryOp { .. } => "unary",
        Expression::Call { .. } => "call",
        Expression::MethodCall { .. } => "method call",
        Expression::Index { .. } => "index",
        Expression::FieldAccess { .. } => "field access",
        Expression::TupleIndex { .. } => "tuple index",
        Expression::Parenthesized(_) => "parenthesized",
        Expression::TryExpression(_) => "try",
        Expression::IfExpression { .. } => "if",
        Expression::WhileLoop { .. } => "while",
        Expression::MatchExpression { .. } => "match",
        Expression::Closure { .. } => "closure",
        Expression::ArrayLiteral(_) => "array",
        Expression::TupleLiteral(_) => "tuple",
        Expression::StructLiteral { .. } => "struct literal",
        Expression::EnumVariant { .. } => "enum variant",
        Expression::LoopRange { .. } => "loop range",
        Expression::ForLoop { .. } => "for",
        Expression::InfiniteLoop(_) => "loop",
        Expression::AsExpression { .. } => "as",
        Expression::GetExpression(_) => "get",
        Expression::Range { .. } => "range",
        Expression::UnsafeBlock(_) => "unsafe block",
        // Leaves are rendered inline and never reach a header.
        Expression::IntLiteral(_)
        | Expression::FloatLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::UnicodeStringLiteral(_)
        | Expression::UnicodeCharLiteral(_)
        | Expression::BoolLiteral(_)
        | Expression::ByteLiteral(_)
        | Expression::Identifier(_) => "literal",
    }
}
/// Expressions reachable from a `get`, so they are walked rather than
/// stringified by `Debug` (the plain `{:?}` above is only for the flag
/// shape itself).
fn get_expressions(get: &GetExpr) -> Vec<&Expression> {
    let mut out = Vec::new();
    if let Some(prompt) = &get.prompt {
        out.push(prompt.as_ref());
    }
    if let Some(source) = &get.source {
        out.push(source.as_ref());
    }
    for flag in &get.flags {
        match flag {
            GetFlag::Timeout(expression)
            | GetFlag::Default(expression)
            | GetFlag::Mask(expression)
            | GetFlag::Until(expression)
            | GetFlag::Bytes(expression) => out.push(expression),
            GetFlag::As(_) => {}
        }
    }
    if let Some(with_clause) = &get.with_clause {
        match with_clause {
            WithClause::Validate(expression)
            | WithClause::Complete(expression)
            | WithClause::Encoding(expression) => out.push(expression),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;
    use poly_lexer::Lexer;

    fn dump(source: &str) -> String {
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty(), "lexer errors: {errors:?}");
        let mut parser = Parser::new(&tokens);
        match parser.parse() {
            Ok(program) => render_program(&program),
            Err(error) => panic!("source should parse: {error}"),
        }
    }

    /// Build a left-nested `1 + 1 + ... + 1` chain of `depth` operands.
    ///
    /// Any deeply nested `Box` tree is itself expensive to drop (dropping the
    /// spine recurses), so callers keep `depth` modest and run on a thread
    /// with a large stack.
    fn left_nested_chain(depth: usize) -> Expression {
        let mut nested = Expression::IntLiteral("1".to_string());
        for _ in 0..depth {
            nested = Expression::BinaryOp {
                op: crate::ast::BinaryOp::Add,
                left: Box::new(nested),
                right: Box::new(Expression::IntLiteral("1".to_string())),
            };
        }
        nested
    }

    /// Run `body` on a thread with a large stack.
    ///
    /// Test threads default to 2 MiB, which both building and dropping a deeply
    /// nested AST can exhaust on their own — before the printer under test is
    /// even reached.
    fn on_big_stack(
        body: impl FnOnce() + Send + 'static,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let handle = std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn(body)?;
        handle.join().map_err(|_| "test thread panicked")?;
        Ok(())
    }

    #[test]
    fn renders_a_simple_function() -> Result<(), Box<dyn std::error::Error>> {
        let output = dump("fn main()\n  put 1\nend fn");
        assert!(
            output.contains("fn main {"),
            "missing function header:\n{output}"
        );
        assert!(output.contains("put {"), "missing put:\n{output}");
        assert!(output.contains("int 1"), "missing literal:\n{output}");
        Ok(())
    }

    #[test]
    fn every_expression_kind_appears() -> Result<(), Box<dyn std::error::Error>> {
        // Exercises a spread of node kinds so a future variant added to the
        // AST cannot silently go missing from the dump: the match in
        // `expression_kind` is exhaustive, so a new variant fails to compile.
        let output = dump(
            "fn main()\n  \
             var a i32 := 1\n  \
             var b bool := true\n  \
             var c f64 := 1.5\n  \
             var s string := \"hi\"\n  \
             var arr := [1, 2]\n  \
             var t := (1, 2)\n  \
             if a = 1\n    put a\n  end if\n  \
             while a < 2\n    a := a + 1\n  end while\n  \
             for item in arr\n    put item\n  end for\n  \
             match a\n    1, put 1\n    _, put 2\n  end match\n\
             end fn",
        );
        for marker in [
            "var a {", "var s {", "array {", "tuple {", "if {", "while {", "for {", "match {",
        ] {
            assert!(output.contains(marker), "missing {marker} in:\n{output}");
        }
        Ok(())
    }

    #[test]
    fn output_size_is_linear_in_node_count() -> Result<(), Box<dyn std::error::Error>> {
        // The regression: `{:#?}` indented every line by depth, so a left-nested
        // chain produced quadratic output (and quadratic-plus time). Two chains
        // of very different length must now differ by roughly their node
        // counts, not by the square of them.
        let chain = |n: usize| {
            let joined = std::iter::repeat_n("1", n).collect::<Vec<_>>().join(" + ");
            format!("fn main()\n  var x i32 := {joined}\n  put x\nend fn")
        };
        let small = dump(&chain(100)).len();
        let large = dump(&chain(400)).len();
        // 4x the operands. Quadratic would give ~16x; linear gives ~4x.
        // Allow generous headroom for constant factors while still failing if
        // the output is quadratic again.
        let ratio = large as f64 / small as f64;
        assert!(
            ratio < 8.0,
            "output grew {ratio:.1}x for 4x the operands, which suggests quadratic output \
             (small={small}, large={large})"
        );
        Ok(())
    }

    #[test]
    fn deep_tree_renders_without_overflowing() -> Result<(), Box<dyn std::error::Error>> {
        // The walk is iterative, so it cannot overflow the stack the way a
        // recursive `Debug` would. Depth here is well past what a recursive
        // printer survives on a normal stack.
        on_big_stack(|| {
            let nested = left_nested_chain(200_000);
            let program = Program {
                statements: vec![crate::ast::Spanned::new(
                    Statement::ExpressionStatement(nested),
                    poly_lexer::token::Span::new(0, 0),
                )],
            };
            let output = render_program(&program);
            assert!(output.contains("binary {"), "deep tree should still render");
        })
    }

    #[test]
    fn indentation_is_capped() -> Result<(), Box<dyn std::error::Error>> {
        // Output stays linear because indent is bounded; a very deep tree must
        // not produce a line whose length grows without limit.
        on_big_stack(|| {
            let nested = left_nested_chain(50_000);
            let program = Program {
                statements: vec![crate::ast::Spanned::new(
                    Statement::ExpressionStatement(nested),
                    poly_lexer::token::Span::new(0, 0),
                )],
            };
            let output = render_program(&program);
            let longest = output.lines().map(str::len).max().unwrap_or(0);
            assert!(
                longest <= MAX_INDENT_LEVELS * INDENT_WIDTH + 64,
                "longest line was {longest} bytes; indent should be capped"
            );
        })
    }

    #[test]
    fn writes_to_a_sink() -> Result<(), Box<dyn std::error::Error>> {
        let (tokens, _) = Lexer::lex("fn main()\n  put 1\nend fn");
        let mut parser = Parser::new(&tokens);
        let program = parser.parse()?;
        let mut buffer: Vec<u8> = Vec::new();
        write_program(&program, &mut buffer)?;
        let text = String::from_utf8(buffer)?;
        assert_eq!(text, render_program(&program));
        Ok(())
    }
}

//! Poly to JavaScript code generation.
//!
//! The JS backend lowers the same orchestration subset as the C target to
//! plain ES2020 JavaScript that runs under Node.js and in the browser with no
//! runtime dependencies. Complex declarations stay in `#js` blocks and are
//! emitted verbatim. See `POLY_JS_DESIGN.md` for the target contract.

use std::fmt::Write;

use poly_parser::ast::{
    self, BinaryOp, Expression, FunctionDecl, Statement, TypeAnnotation, UnaryOp,
};

/// Generate a complete JavaScript program from Poly source.
pub fn transpile(program: &ast::Program) -> Result<String, String> {
    let mut generator = JsGenerator::new();
    generator.generate(program)
}

struct JsGenerator {
    /// Accumulated program text; foreign `#js` blocks are preserved verbatim.
    output: String,
    /// Current indentation for generated `main` statements.
    indent: usize,
    /// Pre-rendered Poly declarations emitted before the entry point.
    functions: Vec<String>,
    structs: Vec<String>,
    /// Opaque target-language definitions selected by the transpiler.
    js_blocks: Vec<String>,
    /// Variables known to hold strings (gates `+` string handling in output).
    string_variables: std::cell::RefCell<std::collections::HashSet<String>>,
    main_statements: Vec<ast::Spanned<Statement>>,
}

impl JsGenerator {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            functions: Vec::new(),
            structs: Vec::new(),
            js_blocks: Vec::new(),
            string_variables: std::cell::RefCell::new(std::collections::HashSet::new()),
            main_statements: Vec::new(),
        }
    }

    fn generate(&mut self, program: &ast::Program) -> Result<String, String> {
        // Partition declarations from orchestration statements so `#js` blocks
        // and Poly functions are emitted at file scope while executable
        // statements keep source order inside the generated `main()`.
        for statement in &program.statements {
            match &statement.node {
                Statement::ForeignBlock { language, content } if language == "js" => {
                    self.js_blocks.push(content.clone());
                }
                Statement::ForeignBlock { .. } => {}
                Statement::ExternFunctionDeclaration(_) => {}
                Statement::FunctionDeclaration(function) => {
                    // `fn main` is the program entry: its body joins the
                    // top-level statements inside the generated `main()`,
                    // mirroring the other targets' entry-point semantics.
                    if function.name == "main"
                        && function.return_type.is_none()
                        && function.params.is_empty()
                        && !function.is_async
                        && function.generics.is_empty()
                    {
                        if let Some(body) = &function.body {
                            self.main_statements.extend(body.clone());
                        }
                    } else {
                        self.functions.push(self.function(function)?)
                    }
                }
                Statement::StructDeclaration(structure) => {
                    self.structs.push(self.structure(structure)?)
                }
                Statement::EnumDeclaration(_) => {
                    return Err(
                        "JS backend does not support Poly enum declarations yet; use a #js helper"
                            .to_string(),
                    )
                }
                Statement::ConstDeclaration { .. } => self.main_statements.push(statement.clone()),
                _ => self.main_statements.push(statement.clone()),
            }
        }

        self.output
            .push_str("/* Generated from Poly source code. */\n");
        self.output.push_str("'use strict';\n\n");
        for block in &self.js_blocks {
            self.output.push_str(block.trim_matches('\n'));
            self.output.push_str("\n\n");
        }
        for structure in &self.structs {
            self.output.push_str(structure);
            self.output.push_str("\n\n");
        }
        for function in &self.functions {
            self.output.push_str(function);
            self.output.push_str("\n\n");
        }

        // Top-level statements and `fn main` bodies both live inside one
        // emitted `main()` so the artifact runs whether or not the source
        // declared `fn main`.
        self.output.push_str("function main() {\n");
        self.indent = 1;
        let main_statements = self.main_statements.clone();
        for statement in &main_statements {
            self.statement(&statement.node)?;
        }
        self.indent = 0;
        self.output.push_str("}\n\nmain();\n");
        Ok(std::mem::take(&mut self.output))
    }

    /// Render a struct declaration as a factory-style constructor. Plain
    /// structs only: methods and generics are rejected with guidance.
    fn structure(&self, structure: &ast::StructDecl) -> Result<String, String> {
        if !structure.methods.is_empty() || !structure.generics.is_empty() {
            return Err(format!(
                "JS backend supports plain structs only; `{}` has methods or generics",
                structure.name
            ));
        }
        let mut result = String::new();
        writeln!(result, "function {}(fields = {{}}) {{", structure.name).unwrap();
        for field in &structure.fields {
            writeln!(result, "  this.{} = fields.{};", field.name, field.name).unwrap();
        }
        writeln!(result, "}}").unwrap();
        Ok(result)
    }

    fn function(&self, function: &FunctionDecl) -> Result<String, String> {
        if function.is_async || !function.generics.is_empty() {
            return Err(format!(
                "JS backend does not support async or generic Poly function `{}`; use a #js definition",
                function.name
            ));
        }
        let mut result = String::new();
        let params = function
            .params
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        for parameter in &function.params {
            if self.is_string_type(&parameter.ty) {
                self.string_variables
                    .borrow_mut()
                    .insert(parameter.name.clone());
            }
        }
        writeln!(result, "function {}({}) {{", function.name, params).unwrap();
        for statement in function.body.as_deref().unwrap_or_default() {
            self.statement_into(&mut result, 1, &statement.node)?;
        }
        writeln!(result, "}}").unwrap();
        Ok(result)
    }

    fn statement(&mut self, statement: &Statement) -> Result<(), String> {
        let mut rendered = String::new();
        self.statement_into(&mut rendered, self.indent, statement)?;
        self.output.push_str(&rendered);
        Ok(())
    }

    fn statement_into(
        &self,
        output: &mut String,
        indent: usize,
        statement: &Statement,
    ) -> Result<(), String> {
        let line_prefix = |output: &mut String| {
            for _ in 0..indent {
                output.push_str("  ");
            }
        };
        match statement {
            Statement::VarDeclaration { name, ty, value } => {
                line_prefix(output);
                if ty.as_ref().is_some_and(|ty| self.is_string_type(ty))
                    || value
                        .as_ref()
                        .is_some_and(|value| self.is_string_literal(value))
                {
                    self.string_variables.borrow_mut().insert(name.clone());
                }
                let value = match value {
                    Some(value) => self.expr(value)?,
                    None => "undefined".to_string(),
                };
                writeln!(output, "let {} = {};", name, value).unwrap();
            }
            Statement::LetDeclaration { name, ty, value } => {
                line_prefix(output);
                if ty.as_ref().is_some_and(|ty| self.is_string_type(ty))
                    || self.is_string_literal(value)
                {
                    self.string_variables.borrow_mut().insert(name.clone());
                }
                writeln!(output, "const {} = {};", name, self.expr(value)?).unwrap();
            }
            Statement::ConstDeclaration { name, value } => {
                line_prefix(output);
                if self.is_string_literal(value) {
                    self.string_variables.borrow_mut().insert(name.clone());
                }
                writeln!(output, "const {} = {};", name, self.expr(value)?).unwrap();
            }
            Statement::Assignment { target, value } => {
                line_prefix(output);
                writeln!(output, "{} = {};", self.expr(target)?, self.expr(value)?).unwrap();
            }
            Statement::PutStatement { expr, redirect } => {
                if redirect.is_some() {
                    return Err(
                        "JS backend file redirects are not supported; use a #js helper".to_string(),
                    );
                }
                line_prefix(output);
                writeln!(output, "console.log({});", self.output_argument(expr)?).unwrap();
            }
            Statement::ErrorStatement(expr)
            | Statement::WarnStatement(expr)
            | Statement::InfoStatement(expr) => {
                line_prefix(output);
                let prefix = match statement {
                    Statement::ErrorStatement(_) => "[ERROR] ",
                    Statement::WarnStatement(_) => "[WARN] ",
                    _ => "[INFO] ",
                };
                writeln!(
                    output,
                    "console.error({});",
                    self.output_argument_prefixed(expr, prefix)?
                )
                .unwrap();
            }
            // The parser uses an `if` node without an else block for `while`;
            // the branch below preserves that compatibility representation while
            // ordinary if statements use the explicit else-block branch.
            Statement::ExpressionStatement(expr) => match expr {
                Expression::IfExpression {
                    condition,
                    then_block,
                    else_block: None,
                } => {
                    line_prefix(output);
                    writeln!(output, "while ({}) {{", self.expr(condition)?).unwrap();
                    self.block_into(output, indent + 1, then_block)?;
                    line_prefix(output);
                    output.push_str("}\n");
                }
                Expression::IfExpression {
                    condition,
                    then_block,
                    else_block: Some(else_block),
                } => {
                    line_prefix(output);
                    writeln!(output, "if ({}) {{", self.expr(condition)?).unwrap();
                    self.block_into(output, indent + 1, then_block)?;
                    line_prefix(output);
                    if else_block.is_empty() {
                        output.push_str("}\n");
                    } else {
                        output.push_str("} else {\n");
                        self.block_into(output, indent + 1, else_block)?;
                        line_prefix(output);
                        output.push_str("}\n");
                    }
                }
                Expression::LoopRange {
                    variable,
                    ranges,
                    body,
                } => {
                    if ranges.len() != 1 {
                        return Err("JS backend currently supports one range per loop".to_string());
                    }
                    // `loop x in collection` lowers to `for (const x of collection)`.
                    if let ast::LoopRangePart::Value(collection) = &ranges[0] {
                        line_prefix(output);
                        writeln!(
                            output,
                            "for (const {variable} of {}) {{",
                            self.expr(collection)?
                        )
                        .unwrap();
                        self.block_into(output, indent + 1, body)?;
                        line_prefix(output);
                        output.push_str("}\n");
                        return Ok(());
                    }
                    let ast::LoopRangePart::Range {
                        start,
                        end,
                        inclusive,
                        step,
                    } = &ranges[0]
                    else {
                        unreachable!("range part is either Value or Range");
                    };
                    let comparison = if *inclusive { "<=" } else { "<" };
                    let step_value = step
                        .as_ref()
                        .map(|value| self.expr(value))
                        .transpose()?
                        .unwrap_or_else(|| "1".to_string());
                    let descending = step.as_ref().is_some_and(is_negative_expression);
                    let update = if descending {
                        format!("{} -= {}", variable, step_value)
                    } else {
                        format!("{} += {}", variable, step_value)
                    };
                    line_prefix(output);
                    writeln!(
                        output,
                        "for (let {variable} = {}; {variable} {comparison} {}; {update}) {{",
                        self.expr(start)?,
                        self.expr(end)?
                    )
                    .unwrap();
                    self.block_into(output, indent + 1, body)?;
                    line_prefix(output);
                    output.push_str("}\n");
                }
                Expression::ForLoop {
                    variable,
                    iterable,
                    body,
                } => {
                    line_prefix(output);
                    writeln!(
                        output,
                        "for (const {variable} of {}) {{",
                        self.expr(iterable)?
                    )
                    .unwrap();
                    self.block_into(output, indent + 1, body)?;
                    line_prefix(output);
                    output.push_str("}\n");
                }
                Expression::InfiniteLoop(body) => {
                    line_prefix(output);
                    output.push_str("for (;;) {\n");
                    self.block_into(output, indent + 1, body)?;
                    line_prefix(output);
                    output.push_str("}\n");
                }
                Expression::MatchExpression { scrutinee, arms } => {
                    // Match lowers to a `switch` over the scrutinee. Literal
                    // and enum-variant arms are supported; structured patterns
                    // are rejected with guidance.
                    let scrutinee_str = self.expr(scrutinee)?;
                    line_prefix(output);
                    writeln!(output, "switch ({scrutinee_str}) {{").unwrap();
                    for arm in arms.iter() {
                        line_prefix_for(output, indent + 1);
                        if matches!(arm.pattern, ast::Pattern::Wildcard) {
                            output.push_str("default:\n");
                        } else {
                            match &arm.pattern {
                                ast::Pattern::Literal(literal) => {
                                    writeln!(output, "case {}:", self.expr(literal)?).unwrap();
                                }
                                ast::Pattern::Enum {
                                    enum_name,
                                    variant,
                                    inner,
                                } => {
                                    if inner.is_some() {
                                        return Err(
                                            "JS backend match supports unit enum variants only; use a #js helper for patterns carrying data"
                                                .to_string(),
                                        );
                                    }
                                    writeln!(output, "case {}.{}:", enum_name, variant).unwrap();
                                }
                                other => {
                                    let _ = other;
                                    return Err(
                                        "JS backend match supports literal, enum-variant, and wildcard patterns; use a #js helper for others"
                                            .to_string(),
                                    );
                                }
                            }
                        }
                        if arm.guard.is_some() {
                            return Err(
                                "JS backend match guards are not supported yet; use a #js helper"
                                    .to_string(),
                            );
                        }
                        match &arm.body {
                            ast::MatchArmBody::Expression(expr) => {
                                line_prefix_for(output, indent + 2);
                                writeln!(output, "{};", self.expr(expr)?).unwrap();
                            }
                            ast::MatchArmBody::Block(statements) => {
                                self.block_into(output, indent + 2, statements)?;
                            }
                        }
                        line_prefix_for(output, indent + 2);
                        output.push_str("break;\n");
                    }
                    line_prefix(output);
                    output.push_str("}\n");
                }
                _ => {
                    line_prefix(output);
                    writeln!(output, "{};", self.expr(expr)?).unwrap();
                }
            },
            Statement::ReturnStatement(value) => {
                line_prefix(output);
                if let Some(value) = value {
                    writeln!(output, "return {};", self.expr(value)?).unwrap();
                } else {
                    output.push_str("return;\n");
                }
            }
            Statement::BreakStatement => {
                line_prefix(output);
                output.push_str("break;\n");
            }
            Statement::ContinueStatement => {
                line_prefix(output);
                output.push_str("continue;\n");
            }
            Statement::ForeignBlock { .. } => {
                return Err("foreign blocks must be at program scope".to_string());
            }
            Statement::ExternFunctionDeclaration(_) => {
                return Err("extern declarations must be at program scope".to_string());
            }
            Statement::Block(block) => self.block_into(output, indent, block)?,
            Statement::FunctionDeclaration(_)
            | Statement::StructDeclaration(_)
            | Statement::EnumDeclaration(_)
            | Statement::TraitDeclaration(_)
            | Statement::ImplDeclaration(_)
            | Statement::ModuleDeclaration(_)
            | Statement::UseDeclaration(_)
            | Statement::TypeDeclaration(_) => {
                return Err(
                    "this Poly declaration is not supported inside a JS function".to_string(),
                );
            }
        }
        Ok(())
    }

    fn block_into(
        &self,
        output: &mut String,
        indent: usize,
        block: &ast::Block,
    ) -> Result<(), String> {
        for statement in block {
            self.statement_into(output, indent, &statement.node)?;
        }
        Ok(())
    }

    /// Render only expressions that have an explicit JS representation.
    /// Keeping unsupported variants as errors prevents silently incorrect JS.
    fn expr(&self, expression: &Expression) -> Result<String, String> {
        match expression {
            Expression::IntLiteral(value) => Ok(value.clone()),
            Expression::FloatLiteral(value) => Ok(value.clone()),
            Expression::StringLiteral(value) | Expression::UnicodeStringLiteral(value) => {
                Ok(format!("\"{}\"", js_escape(value)))
            }
            Expression::UnicodeCharLiteral(value) => {
                Ok(format!("\"{}\"", js_escape(&value.to_string())))
            }
            Expression::BoolLiteral(value) => Ok(if *value { "true" } else { "false" }.to_string()),
            Expression::ByteLiteral(bytes) => Ok(format!(
                "[{}]",
                bytes
                    .iter()
                    .map(|byte| byte.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Expression::Identifier(name) => Ok(name.clone()),
            Expression::BinaryOp { op, left, right } => {
                // Integer division/modulo must keep truncation semantics: JS
                // `/` is float division and `%` keeps the dividend's sign.
                match op {
                    BinaryOp::Div if !self.is_string_expression(left) => {
                        return Ok(format!(
                            "Math.trunc(({}) / ({}))",
                            self.expr(left)?,
                            self.expr(right)?
                        ));
                    }
                    _ => {}
                }
                Ok(format!(
                    "(({}) {} ({}))",
                    self.expr(left)?,
                    self.binary_op(op),
                    self.expr(right)?
                ))
            }
            Expression::UnaryOp { op, expr } => {
                Ok(format!("({}{})", self.unary_op(op)?, self.expr(expr)?))
            }
            Expression::Call { func, args } => Ok(format!(
                "{}({})",
                self.expr(func)?,
                args.iter()
                    .map(|arg| self.expr(arg))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            )),
            Expression::MethodCall {
                object,
                method,
                args,
            } => {
                // The one supported method: `.to_string()` mirrors the other
                // targets' string conversion.
                let _ = args;
                match method.as_str() {
                    "to_string" => Ok(format!("String({})", self.expr(object)?)),
                    _ => Err(format!(
                        "JS backend does not support method calls (`.{method}`); use a #js helper"
                    )),
                }
            }
            Expression::FieldAccess { object, field } => {
                Ok(format!("{}.{}", self.expr(object)?, field))
            }
            Expression::TupleIndex { object, index } => {
                // Tuples are arrays in the JS subset: `.N` indexes the array.
                Ok(format!("{}[{}]", self.expr(object)?, index))
            }
            Expression::Index { object, index } => {
                Ok(format!("{}[{}]", self.expr(object)?, self.expr(index)?))
            }
            Expression::Parenthesized(inner) => Ok(format!("({})", self.expr(inner)?)),
            Expression::AsExpression { expr, .. } => self.expr(expr),
            Expression::ArrayLiteral(elements) => Ok(format!(
                "[{}]",
                elements
                    .iter()
                    .map(|element| self.expr(element))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", ")
            )),
            Expression::TupleLiteral(elements) => Ok(format!(
                "[{}]",
                elements
                    .iter()
                    .map(|element| self.expr(element))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", ")
            )),
            Expression::StructLiteral { name, fields } => {
                let initializers = fields
                    .iter()
                    .map(|(field, value)| Ok(format!("{field}: {}", self.expr(value)?)))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", ");
                Ok(format!("new {}({{ {initializers} }})", name))
            }
            Expression::EnumVariant {
                enum_name,
                variant,
                data,
            } => {
                if data.is_some() {
                    return Err(
                        "JS backend supports unit enum variants only; use a #js helper for variants carrying data"
                            .to_string(),
                    );
                }
                // Enums are frozen objects in `#js` blocks; a Poly unit variant
                // references the qualified property name.
                Ok(format!("{enum_name}.{variant}"))
            }
            Expression::Range { .. }
            | Expression::LoopRange { .. }
            | Expression::ForLoop { .. }
            | Expression::InfiniteLoop(_) => {
                Err("loop expressions are only valid as statements in the JS backend".to_string())
            }
            Expression::IfExpression {
                condition,
                then_block,
                else_block,
            } => {
                if else_block.is_none() {
                    return Err(
                        "while expressions are only valid as statements in the JS backend"
                            .to_string(),
                    );
                }
                let _ = (condition, then_block, else_block);
                Err(
                    "if expressions in value position are not supported by the JS backend"
                        .to_string(),
                )
            }
            Expression::GetExpression(get) => {
                let _ = get;
                Err(
                    "JS backend does not support `get` (browser-hostile); use a #js helper for input"
                        .to_string(),
                )
            }
            Expression::MatchExpression { .. }
            | Expression::Closure { .. }
            | Expression::TryExpression(_)
            | Expression::UnsafeBlock(_) => Err(
                "this expression is not supported by the JS backend; use a #js helper".to_string(),
            ),
        }
    }

    /// Render a `put` argument: plain strings print directly; numbers and
    /// bools print through `String(...)` so `put "x = " + 1` concatenates the
    /// same way the other targets render string output.
    fn output_argument(&self, expression: &Expression) -> Result<String, String> {
        // Strings (literals, tracked string variables, and concatenations
        // involving either) print directly; everything else goes through
        // `String(...)` so numbers and bools render like the other targets.
        if self.is_string_expression(expression) {
            self.expr(expression)
        } else {
            Ok(format!("String({})", self.expr(expression)?))
        }
    }

    fn output_argument_prefixed(
        &self,
        expression: &Expression,
        prefix: &str,
    ) -> Result<String, String> {
        Ok(format!(
            "\"{}\" + String({})",
            js_escape(prefix),
            self.expr(expression)?
        ))
    }

    fn is_string_type(&self, annotation: &TypeAnnotation) -> bool {
        matches!(annotation, TypeAnnotation::Named(name) if name == "string" || name == "ustring")
    }

    fn is_string_literal(&self, expression: &Expression) -> bool {
        matches!(
            expression,
            Expression::StringLiteral(_) | Expression::UnicodeStringLiteral(_)
        )
    }

    fn is_string_expression(&self, expression: &Expression) -> bool {
        match expression {
            Expression::StringLiteral(_) | Expression::UnicodeStringLiteral(_) => true,
            Expression::Identifier(name) => self.string_variables.borrow().contains(name),
            Expression::BinaryOp {
                op: BinaryOp::Add,
                left,
                right,
            } => self.is_string_expression(left) || self.is_string_expression(right),
            _ => false,
        }
    }

    fn binary_op(&self, op: &BinaryOp) -> &'static str {
        match op {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            // Poly `%` is truncated modulo; JS `%` is remainder (sign of the
            // dividend). For positive operands they agree; the first cut
            // documents this rather than emitting a helper.
            BinaryOp::Mod => "%",
            BinaryOp::Eq => "===",
            BinaryOp::NotEq => "!==",
            BinaryOp::Lt => "<",
            BinaryOp::Gt => ">",
            BinaryOp::LtEq => "<=",
            BinaryOp::GtEq => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
            BinaryOp::BitAnd => "&",
            BinaryOp::BitOr => "|",
            BinaryOp::BitXor => "^",
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>>",
        }
    }

    fn unary_op(&self, op: &UnaryOp) -> Result<&'static str, String> {
        match op {
            UnaryOp::Neg => Ok("-"),
            UnaryOp::Not => Ok("!"),
            UnaryOp::BitNot => Ok("~"),
            UnaryOp::Deref => {
                Err("pointer dereference has no JS representation; use a #js helper".to_string())
            }
        }
    }
}

fn line_prefix_for(output: &mut String, indent: usize) {
    for _ in 0..indent {
        output.push_str("  ");
    }
}

fn js_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn is_negative_expression(expression: &Expression) -> bool {
    matches!(
        expression,
        Expression::UnaryOp {
            op: UnaryOp::Neg,
            ..
        }
    ) || matches!(expression, Expression::IntLiteral(value) if value.starts_with('-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use poly_lexer::Lexer;
    use poly_parser::Parser;

    fn generate(source: &str) -> String {
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty(), "{errors:?}");
        let mut parser = Parser::new(&tokens);
        let program = parser.parse().unwrap();
        transpile(&program).unwrap()
    }

    #[test]
    fn emits_js_foreign_block_and_main() {
        let output = generate(
            "#js\nfunction doubleValue(x) {\n    return x * 2;\n}\n#endjs\nvar x i32 := doubleValue(21)\nput x",
        );
        assert!(output.contains("function doubleValue(x)"));
        assert!(output.contains("main();"));
    }

    #[test]
    fn lowers_scalars_and_output() {
        let output = generate("var x i32 := 10\nvar name ustring := unicode \"Poly\"\nput x\nput name\nput \"v: \" + x");
        assert!(output.contains("let x = 10;"));
        assert!(output.contains("let name = \"Poly\";"));
        assert!(output.contains("console.log(String(x));"));
        // A bare unicode string variable prints directly (no String() wrap).
        assert!(output.contains("console.log(name);"));
        assert!(output.contains("console.log(((\"v: \") + (x)));"));
    }

    #[test]
    fn truncates_integer_division() {
        // Poly `/` on integers is truncated division; JS `/` is float division.
        let output = generate("var q i32 := 7 / 2\nput q");
        assert!(output.contains("let q = Math.trunc((7) / (2));"));
    }

    #[test]
    fn lowers_control_flow_and_functions() {
        let output = generate(
            "fn add(a: i32, b: i32): i32\n    return a + b\nend fn\nvar i i32 := 0\nwhile i < 3\n    i := i + 1\nend while\nloop j 0..3\n    put j\nend loop\nif 1 > 2,\n    put 1\nelse\n    put 0\nend if\nloop k in [1, 2]\n    put k\nend loop",
        );
        assert!(output.contains("function add(a, b) {"));
        assert!(output.contains("return ((a) + (b));"));
        assert!(output.contains("while (((i) < (3))) {"));
        assert!(output.contains("for (let j = 0; j <= 3; j += 1) {"));
        assert!(output.contains("for (const k of [1, 2]) {"));
    }

    #[test]
    fn lowers_match_to_switch() {
        let output = generate(
            "var x i32 := 3\nmatch x\n    1, put 10\n    3, put 30\n    _, put 99\nend match",
        );
        assert!(output.contains("switch (x) {"));
        assert!(output.contains("case 1:"));
        assert!(output.contains("case 3:"));
        assert!(output.contains("default:"));
        assert!(output.contains("break;"));
    }

    #[test]
    fn lowers_structs_and_literals() {
        let output = generate(
            "struct Point\n    var x: i32\n    var y: i32\nend struct\nvar p := Point { x: 3, y: 4 }\nput p.x",
        );
        assert!(output.contains("function Point(fields = {}) {"));
        assert!(output.contains("this.x = fields.x;"));
        assert!(output.contains("let p = new Point({ x: 3, y: 4 });"));
        assert!(output.contains("String(p.x)"));
    }

    #[test]
    fn rejects_unsupported_constructs_with_guidance() {
        for source in [
            "var t i32 := get",
            "put \"a\" to \"file.txt\"",
            "var addn := |x: i32| x + 1",
            "enum Color\n    Red\nend enum\nvar c := Color::Red",
        ] {
            let (tokens, errors) = Lexer::lex(source);
            assert!(errors.is_empty(), "{errors:?}");
            let mut parser = Parser::new(&tokens);
            let program = parser.parse().unwrap();
            assert!(
                transpile(&program).is_err(),
                "expected rejection for: {source}"
            );
        }
    }
}

//! Poly to C code generation.
//!
//! The C backend intentionally targets the small orchestration subset of Poly.
//! Complex declarations stay in `#c` blocks and are emitted verbatim.

use std::cell::RefCell;
use std::fmt::Write;

use poly_parser::ast::{
    self, BinaryOp, Expression, FunctionDecl, Statement, TypeAnnotation, UnaryOp,
};

/// Generate a complete C translation unit from Poly source.
pub fn transpile(program: &ast::Program) -> Result<String, String> {
    let mut generator = CGenerator::new();
    generator.generate(program)
}

struct CGenerator {
    /// Accumulated translation unit; keeping output as text preserves foreign C
    /// blocks byte-for-byte while Poly statements are rendered incrementally.
    output: String,
    /// Current indentation for generated `main` statements.
    indent: usize,
    /// Pre-rendered Poly declarations emitted before `main`.
    functions: Vec<String>,
    structs: Vec<String>,
    /// Opaque target-language definitions selected by the transpiler.
    c_blocks: Vec<String>,
    string_variables: RefCell<std::collections::HashSet<String>>,
    main_statements: Vec<ast::Spanned<Statement>>,
}

impl CGenerator {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            functions: Vec::new(),
            structs: Vec::new(),
            c_blocks: Vec::new(),
            string_variables: RefCell::new(std::collections::HashSet::new()),
            main_statements: Vec::new(),
        }
    }

    fn generate(&mut self, program: &ast::Program) -> Result<String, String> {
        // Partition first because C declarations must precede main, while Poly
        // executable statements are intentionally retained in source order.
        // First partition declarations from orchestration statements so C items
        // are emitted at file scope and the generated entry point stays valid.
        for statement in &program.statements {
            match &statement.node {
                Statement::ForeignBlock { language, content } if language == "c" => {
                    self.c_blocks.push(content.clone());
                }
                Statement::ForeignBlock { .. } => {}
                Statement::ExternFunctionDeclaration(_) => {}
                Statement::FunctionDeclaration(function) => {
                    // `fn main` is the program entry on the C target: its body
                    // becomes the body of the emitted `int main(void)` (with
                    // the `return 0;` tail), mirroring how the Rust target
                    // wraps top-level statements in a generated `fn main`.
                    // Emitting it as a separate `void main()` duplicated the
                    // C entry point and failed to compile.
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
                Statement::ConstDeclaration { name, value } => {
                    self.main_statements.push(statement.clone());
                    let _ = (name, value);
                }
                _ => self.main_statements.push(statement.clone()),
            }
        }

        self.output
            .push_str("/* Generated from Poly source code. */\n");
        self.output.push_str("#include <stdbool.h>\n");
        self.output.push_str("#include <stdint.h>\n");
        self.output.push_str("#include <stdio.h>\n");
        self.output.push_str("#include <stdlib.h>\n");
        self.output.push_str("#include <string.h>\n\n");
        for block in &self.c_blocks {
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

        self.output.push_str("int main(void) {\n");
        self.indent = 1;
        let main_statements = self.main_statements.clone();
        for statement in &main_statements {
            self.statement(&statement.node)?;
        }
        self.line("return 0;");
        self.output.push_str("}\n");
        Ok(std::mem::take(&mut self.output))
    }

    fn structure(&self, structure: &ast::StructDecl) -> Result<String, String> {
        if !structure.methods.is_empty() || !structure.generics.is_empty() {
            return Err(format!(
                "C backend supports plain structs only; `{}` has methods or generics",
                structure.name
            ));
        }
        let mut result = String::new();
        writeln!(result, "typedef struct {} {{", structure.name).unwrap();
        for field in &structure.fields {
            writeln!(result, "    {} {};", self.ty(&field.ty)?, field.name).unwrap();
        }
        writeln!(result, "}} {};", structure.name).unwrap();
        Ok(result)
    }

    fn function(&self, function: &FunctionDecl) -> Result<String, String> {
        if function.is_async || !function.generics.is_empty() {
            return Err(format!(
                "C backend does not support async or generic Poly function `{}`; use a #c definition",
                function.name
            ));
        }
        let mut result = String::new();
        let return_type = function
            .return_type
            .as_ref()
            .map(|ty| self.ty(ty))
            .transpose()?
            .unwrap_or_else(|| "void".to_string());
        let params = function
            .params
            .iter()
            .map(|parameter| Ok(format!("{} {}", self.ty(&parameter.ty)?, parameter.name)))
            .collect::<Result<Vec<_>, String>>()?
            .join(", ");
        for parameter in &function.params {
            if self.is_string_type(&parameter.ty) {
                self.string_variables
                    .borrow_mut()
                    .insert(parameter.name.clone());
            }
        }
        writeln!(result, "{} {}({}) {{", return_type, function.name, params).unwrap();
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
                output.push_str("    ");
            }
        };
        match statement {
            Statement::VarDeclaration { name, ty, value } => {
                line_prefix(output);
                let type_name = ty
                    .as_ref()
                    .map(|ty| self.ty(ty))
                    .transpose()?
                    .unwrap_or_else(|| self.inferred_type(value.as_ref()));
                if ty.as_ref().is_some_and(|ty| self.is_string_type(ty))
                    || value
                        .as_ref()
                        .is_some_and(|value| self.is_string_literal(value))
                {
                    self.string_variables.borrow_mut().insert(name.clone());
                }
                let value = value
                    .as_ref()
                    .map(|value| self.expr(value))
                    .transpose()?
                    .unwrap_or_else(|| "0".to_string());
                writeln!(output, "{} {} = {};", type_name, name, value).unwrap();
            }
            Statement::LetDeclaration { name, ty, value } => {
                line_prefix(output);
                let type_name = ty
                    .as_ref()
                    .map(|ty| self.ty(ty))
                    .transpose()?
                    .unwrap_or_else(|| self.inferred_type(Some(value)));
                if ty.as_ref().is_some_and(|ty| self.is_string_type(ty))
                    || self.is_string_literal(value)
                {
                    self.string_variables.borrow_mut().insert(name.clone());
                }
                writeln!(output, "{} {} = {};", type_name, name, self.expr(value)?).unwrap();
            }
            Statement::ConstDeclaration { name, value } => {
                line_prefix(output);
                writeln!(
                    output,
                    "const {} {} = {};",
                    self.inferred_type(Some(value)),
                    name,
                    self.expr(value)?
                )
                .unwrap();
            }
            Statement::Assignment { target, value } => {
                line_prefix(output);
                writeln!(output, "{} = {};", self.expr(target)?, self.expr(value)?).unwrap();
            }
            Statement::PutStatement { expr, redirect } => {
                if redirect.is_some() {
                    return Err(
                        "C backend file redirects are not implemented yet; use a #c helper"
                            .to_string(),
                    );
                }
                line_prefix(output);
                let (format, args) = self.printf_parts(expr)?;
                writeln!(output, "printf(\"{}\\n\"{});", format, args).unwrap();
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
                let (format, args) = self.printf_parts(expr)?;
                writeln!(
                    output,
                    "fprintf(stderr, \"{}{}\\n\"{});",
                    prefix, format, args
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
                    writeln!(output, "while {} {{", self.condition_expr(condition)?).unwrap();
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
                    writeln!(output, "if {} {{", self.condition_expr(condition)?).unwrap();
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
                        return Err("C backend currently supports one range per loop".to_string());
                    }
                    let ast::LoopRangePart::Range {
                        start,
                        end,
                        inclusive,
                        step,
                    } = &ranges[0]
                    else {
                        return Err("C backend requires a numeric range loop".to_string());
                    };
                    let descending = step.as_ref().is_some_and(is_negative_expression);
                    let comparison = if descending {
                        if *inclusive {
                            ">="
                        } else {
                            ">"
                        }
                    } else if *inclusive {
                        "<="
                    } else {
                        "<"
                    };
                    let step_value = step
                        .as_ref()
                        .map(|value| self.expr(value))
                        .transpose()?
                        .unwrap_or_else(|| "1".to_string());
                    let update = if descending {
                        format!("{} -= -({})", variable, step_value)
                    } else {
                        format!("{} += {}", variable, step_value)
                    };
                    line_prefix(output);
                    writeln!(
                        output,
                        "for (int32_t {} = {}; {} {} {}; {}) {{",
                        variable,
                        self.expr(start)?,
                        variable,
                        comparison,
                        self.expr(end)?,
                        update
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
                    let iterable = self.expr(iterable)?;
                    line_prefix(output);
                    writeln!(
                        output,
                        "for (size_t __i = 0; __i < sizeof({}) / sizeof(({})[0]); ++__i) {{",
                        iterable, iterable
                    )
                    .unwrap();
                    line_prefix_for(output, indent + 1);
                    writeln!(output, "int32_t {} = ({})[__i];", variable, iterable).unwrap();
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
                return Err("this Poly declaration is not supported in a C function".to_string());
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

    /// Render a condition with exactly one syntactic parenthesis pair.
    /// Binary expressions already carry grouping parentheses; adding another
    /// pair makes Clang's `-Wparentheses-equality` warning fatal under `-Werror`.
    fn condition_expr(&self, expression: &Expression) -> Result<String, String> {
        let rendered = self.expr(expression)?;
        if rendered.starts_with('(') {
            Ok(rendered)
        } else {
            Ok(format!("({rendered})"))
        }
    }

    /// Render only expressions that have an explicit C representation. Keeping
    /// unsupported variants as errors prevents silently incorrect C programs.
    fn expr(&self, expression: &Expression) -> Result<String, String> {
        match expression {
            Expression::IntLiteral(value) => Ok(value.clone()),
            Expression::FloatLiteral(value) => Ok(value.clone()),
            Expression::StringLiteral(value) | Expression::UnicodeStringLiteral(value) => {
                Ok(format!("\"{}\"", c_escape(value)))
            }
            Expression::UnicodeCharLiteral(value) => {
                Ok(format!("'{}'", c_escape(&value.to_string())))
            }
            Expression::BoolLiteral(value) => Ok(if *value { "true" } else { "false" }.to_string()),
            Expression::ByteLiteral(bytes) => Ok(format!(
                "{{{}}}",
                bytes
                    .iter()
                    .map(|byte| byte.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Expression::Identifier(name) => Ok(name.clone()),
            Expression::BinaryOp { op, left, right } => {
                if matches!(op, BinaryOp::Add)
                    && (self.is_string_expression(left) || self.is_string_expression(right))
                {
                    return Err("string concatenation in C requires a #c helper".to_string());
                }
                Ok(format!(
                    "({} {} {})",
                    self.expr(left)?,
                    self.binary_op(op),
                    self.expr(right)?
                ))
            }
            Expression::UnaryOp { op, expr } => {
                Ok(format!("({}{})", self.unary_op(op), self.expr(expr)?))
            }
            Expression::Call { func, args } => Ok(format!(
                "{}({})",
                self.expr(func)?,
                args.iter()
                    .map(|arg| self.expr(arg))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            )),
            Expression::FieldAccess { object, field } => {
                Ok(format!("{}.{}", self.expr(object)?, field))
            }
            Expression::TupleIndex { .. } => {
                Err("tuple index access is not supported by the C backend".to_string())
            }
            Expression::Index { object, index } => {
                Ok(format!("{}[{}]", self.expr(object)?, self.expr(index)?))
            }
            Expression::Parenthesized(inner) => Ok(format!("({})", self.expr(inner)?)),
            Expression::AsExpression { expr, ty } => {
                Ok(format!("({})({})", self.ty(ty)?, self.expr(expr)?))
            }
            Expression::Range { .. }
            | Expression::LoopRange { .. }
            | Expression::ForLoop { .. }
            | Expression::InfiniteLoop(_) => {
                Err("loop expressions are only valid as statements in the C backend".to_string())
            }
            Expression::IfExpression {
                condition,
                then_block,
                else_block,
            } => {
                if else_block.is_none() {
                    return Err(
                        "while expressions are only valid as statements in the C backend"
                            .to_string(),
                    );
                }
                let _ = (condition, then_block, else_block);
                Err(
                    "if expressions in value position are not supported by the C backend"
                        .to_string(),
                )
            }
            Expression::MatchExpression { .. }
            | Expression::Closure { .. }
            | Expression::StructLiteral { .. }
            | Expression::EnumVariant { .. }
            | Expression::TryExpression(_)
            | Expression::GetExpression(_)
            | Expression::UnsafeBlock(_)
            | Expression::MethodCall { .. }
            | Expression::TupleLiteral(_)
            | Expression::ArrayLiteral(_) => Err(
                "this expression is not supported by the C backend; use a #c helper".to_string(),
            ),
        }
    }

    /// Split a `put` expression into a C format string and argument list.
    /// String concatenation is flattened here so literal percent signs are
    /// escaped before they reach `printf`.
    fn printf_parts(&self, expression: &Expression) -> Result<(String, String), String> {
        match expression {
            Expression::StringLiteral(value) | Expression::UnicodeStringLiteral(value) => {
                Ok((c_format_escape(value), String::new()))
            }
            Expression::BinaryOp {
                op: BinaryOp::Add,
                left,
                right,
            } if self.is_string_expression(left) || self.is_string_expression(right) => {
                let (left_format, left_args) = self.printf_parts(left)?;
                let (right_format, right_args) = self.printf_parts(right)?;
                Ok((
                    format!("{}{}", left_format, right_format),
                    format!("{}{}", left_args, right_args),
                ))
            }
            Expression::BinaryOp { .. } => Ok((
                self.format_for_expression(expression),
                format!(", {}", self.expr(expression)?),
            )),
            Expression::IntLiteral(_)
            | Expression::FloatLiteral(_)
            | Expression::BoolLiteral(_)
            | Expression::UnicodeCharLiteral(_)
            | Expression::UnaryOp { .. }
            | Expression::Identifier(_)
            | Expression::Call { .. }
            | Expression::FieldAccess { .. }
            | Expression::Index { .. }
            | Expression::AsExpression { .. } => {
                let format = if self.is_string_expression(expression) {
                    "%s".to_string()
                } else {
                    self.format_for_expression(expression)
                };
                Ok((format, format!(", {}", self.expr(expression)?)))
            }
            _ => Ok(("%s".to_string(), format!(", {}", self.expr(expression)?))),
        }
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
        self.is_string_literal(expression)
            || matches!(expression, Expression::Identifier(name) if self.string_variables.borrow().contains(name))
    }

    fn format_for_expression(&self, expression: &Expression) -> String {
        match expression {
            Expression::FloatLiteral(_) => "%f".to_string(),
            Expression::BoolLiteral(_) => "%d".to_string(),
            Expression::UnicodeCharLiteral(_) => "%c".to_string(),
            _ => "%d".to_string(),
        }
    }

    fn inferred_type(&self, value: Option<&Expression>) -> String {
        match value {
            Some(Expression::FloatLiteral(_)) => "double".to_string(),
            Some(Expression::StringLiteral(_)) | Some(Expression::UnicodeStringLiteral(_)) => {
                "const char *".to_string()
            }
            Some(Expression::BoolLiteral(_)) => "bool".to_string(),
            _ => "int32_t".to_string(),
        }
    }

    fn ty(&self, annotation: &TypeAnnotation) -> Result<String, String> {
        match annotation {
            TypeAnnotation::Named(name) => Ok(match name.as_str() {
                "bool" => "bool",
                "i8" => "int8_t",
                "u8" | "byte" => "uint8_t",
                "i16" => "int16_t",
                "u16" => "uint16_t",
                "i32" => "int32_t",
                "u32" => "uint32_t",
                "i64" => "int64_t",
                "u64" => "uint64_t",
                "f32" => "float",
                "f64" => "double",
                "char" => "char",
                "string" | "ustring" => "const char *",
                "usize" => "size_t",
                other => other,
            }
            .to_string()),
            TypeAnnotation::Vec(inner) => Ok(format!("{} *", self.ty(inner)?)),
            TypeAnnotation::Reference(_, inner) => self.ty(inner),
            TypeAnnotation::Pointer(inner) => Ok(format!("{} *", self.ty(inner)?)),
            TypeAnnotation::Generic { name, .. } if name == "Vec" => Ok("void *".to_string()),
            TypeAnnotation::Generic { name, .. } => Ok(name.clone()),
            TypeAnnotation::Tuple(_)
            | TypeAnnotation::Array(_, _)
            | TypeAnnotation::Option(_)
            | TypeAnnotation::Result(_, _)
            | TypeAnnotation::Nullable(_)
            | TypeAnnotation::Function { .. } => {
                Err("this Poly type is not supported by the C backend".to_string())
            }
        }
    }

    fn binary_op(&self, op: &BinaryOp) -> &'static str {
        match op {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Mod => "%",
            BinaryOp::Eq => "==",
            BinaryOp::NotEq => "!=",
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
            BinaryOp::Shr => ">>",
        }
    }

    fn unary_op(&self, op: &UnaryOp) -> &'static str {
        match op {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
            UnaryOp::BitNot => "~",
            UnaryOp::Deref => "*",
        }
    }

    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        self.output.push_str(text);
        self.output.push('\n');
    }
}

fn line_prefix_for(output: &mut String, indent: usize) {
    for _ in 0..indent {
        output.push_str("    ");
    }
}

fn c_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn c_format_escape(value: &str) -> String {
    c_escape(value).replace('%', "%%")
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

    #[test]
    fn emits_c_foreign_block_and_main() {
        let source = "#c\nint double_value(int x) { return x * 2; }\n#endc\nvar x i32 := double_value(21)\nput x";
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty(), "{errors:?}");
        let mut parser = Parser::new(&tokens);
        let program = parser.parse().unwrap();
        let output = transpile(&program).unwrap();
        assert!(output.contains("double_value"));
        assert!(output.contains("int main(void)"));
    }
}

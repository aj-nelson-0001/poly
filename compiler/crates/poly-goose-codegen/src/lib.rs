//! Poly to Goose code generation.
//!
//! The Goose backend lowers the same orchestration subset as the JS target to
//! Goose source that passes `goose check` and runs under `goose run`, with no
//! runtime dependencies. It emits only syntax I verified in the pinned
//! Goose language reference (github.com/aardappel/goose, `docs/reference.md`):
//! `fn` declarations, `var`/`let`/`const`
//! bindings, `while`/`for-in`/`loop` loops, struct literals, `match` with
//! literal and wildcard arms, `str(...)` generic formatting, `print(...)`
//! (which renders any value), `.len`/`.push` slice/array members, and the
//! `as!` unchecked cast. Complex declarations stay in `#goose` blocks and are
//! emitted verbatim.
//!
//! Anything with a faithful `#goose` escape hatch is rejected with a message
//! that names both the limitation and that escape hatch, so no construct is
//! silently miscompiled.

use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt::Write;

use poly_parser::ast::{
    self, BinaryOp, Expression, FunctionDecl, LoopRangePart, MatchArmBody, Pattern, Statement,
    TypeAnnotation, UnaryOp,
};
use poly_parser::int_literal_text;

/// Generate a complete Goose program from Poly source.
pub fn transpile(program: &ast::Program) -> Result<String, String> {
    let mut generator = GooseGenerator::new();
    generator.generate(program)
}

struct GooseGenerator {
    /// Accumulated program text; foreign `#goose` blocks are preserved verbatim.
    output: String,
    /// Current indentation for generated statements.
    indent: usize,
    /// Pre-rendered Poly declarations emitted before the entry point.
    functions: Vec<String>,
    structs: Vec<String>,
    /// Opaque Goose definitions selected by the transpiler.
    goose_blocks: Vec<String>,
    /// Variables known to hold strings (gates `+` string handling).
    string_variables: RefCell<HashSet<String>>,
    /// Set when an integer `^` was lowered to the helper, so the helper is emitted.
    used_power_helper: RefCell<bool>,
    /// Set when a `%` was lowered to the helper, so the helper is emitted.
    used_mod_helper: RefCell<bool>,
    /// Fresh-name counter for the temp buffers that break self-referential
    /// string assignments (Goose §4.4).
    temp_counter: RefCell<usize>,
    /// Executable statements queued for the generated `main`.
    main_statements: Vec<ast::Spanned<Statement>>,
}

impl GooseGenerator {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            functions: Vec::new(),
            structs: Vec::new(),
            goose_blocks: Vec::new(),
            string_variables: RefCell::new(HashSet::new()),
            used_power_helper: RefCell::new(false),
            used_mod_helper: RefCell::new(false),
            temp_counter: RefCell::new(0),
            main_statements: Vec::new(),
        }
    }

    /// Build a complete Goose program from the parsed program.
    ///
    /// Strategy mirrors the JS backend: first *partition* the program
    /// (foreign blocks, structs, and functions go to file scope; executable
    /// statements are queued for the generated `main`), then emit the pieces
    /// in dependency order.
    fn generate(&mut self, program: &ast::Program) -> Result<String, String> {
        for statement in &program.statements {
            match &statement.node {
                Statement::ForeignBlock { language, content } if language == "goose" => {
                    self.goose_blocks.push(content.clone());
                }
                Statement::ForeignBlock { .. } => {}
                Statement::ExternFunctionDeclaration(_) => {}
                Statement::FunctionDeclaration(function) => {
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
                        let rendered = self.function(function)?;
                        self.functions.push(rendered);
                    }
                }
                Statement::StructDeclaration(structure) => {
                    let rendered = self.structure(structure)?;
                    self.structs.push(rendered);
                }
                Statement::EnumDeclaration(_) => {
                    return Err(
                        "Goose backend does not support Poly enum declarations; declare the enum in a #goose block"
                            .to_string(),
                    )
                }
                Statement::ConstDeclaration { .. } => self.main_statements.push(statement.clone()),
                // Package managers own external dependencies; `dep`
                // declarations are consumed by project generation, not emission.
                Statement::DependencyDeclaration(_) => {}
                _ => self.main_statements.push(statement.clone()),
            }
        }

        self.output
            .push_str("// Generated from Poly source code.\n");
        self.output.push_str("// Target language: Goose (https://github.com/aardappel/goose).\n\n");
        for block in &self.goose_blocks {
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
        self.output.push_str("fn main() {\n");
        self.indent = 1;
        let main_statements = self.main_statements.clone();
        for statement in &main_statements {
            let mut rendered = String::new();
            self.statement_into(&mut rendered, 1, &statement.node)?;
            self.output.push_str(&rendered);
        }
        self.output.push_str("}\n");

        // Helpers are emitted after every statement (including `main`'s body)
        // has been rendered, so the `used_*_helper` flags are final by now.
        if *self.used_power_helper.borrow() {
            self.output.push_str("\n");
            self.output.push_str(&power_helper());
            self.output.push('\n');
        }
        if *self.used_mod_helper.borrow() {
            self.output.push_str("\n");
            // `std.rem` (the truncated partner of `/`) lives in the standard
            // library, so the file must import it. Imports are top-level and
            // order-free, and a duplicate `import std;` is tolerated, so it
            // can sit here next to the helper that needs it (the helper
            // flags are only final after `main` has been rendered).
            self.output.push_str("import std;\n\n");
            self.output.push_str(&mod_helper());
            self.output.push('\n');
        }
        Ok(std::mem::take(&mut self.output))
    }

    /// Render a struct declaration. Plain structs only: methods and generics
    /// are rejected with guidance (both have a `#goose` escape hatch).
    fn structure(&mut self, structure: &ast::StructDecl) -> Result<String, String> {
        if !structure.methods.is_empty() {
            return Err(format!(
                "Goose backend does not support struct methods (`{}`); use a plain function or a #goose block",
                structure.name
            ));
        }
        if !structure.generics.is_empty() {
            return Err(format!(
                "Goose backend does not support generic structs (`{}`); use a #goose block",
                structure.name
            ));
        }
        let mut result = String::new();
        let fields = structure
            .fields
            .iter()
            .map(|field| {
                let mut rendered = format!("{}: {}", field.name, self.type_to_goose(&field.ty)?);
                if let Some(default) = &field.default {
                    rendered.push_str(&format!(" = {}", self.expr(default)?));
                }
                Ok(rendered)
            })
            .collect::<Result<Vec<_>, String>>()?
            .join(", ");
        writeln!(result, "struct {} {{ {} }}", structure.name, fields).map_err(|e| e.to_string())?;
        Ok(result)
    }

    fn function(&mut self, function: &FunctionDecl) -> Result<String, String> {
        if function.is_async {
            return Err(format!(
                "Goose backend does not support async functions (`{}`); move the async logic into a #goose block or use plain sync functions",
                function.name
            ));
        }
        if !function.generics.is_empty() {
            return Err(format!(
                "Goose backend does not support generic functions (`{}`); monomorphize it or use a #goose block",
                function.name
            ));
        }
        let mut result = String::new();
        let params = function
            .params
            .iter()
            .map(|parameter| {
                if parameter.default.is_some() {
                    return Err(format!(
                        "Goose backend does not support parameter defaults (in `{}`); pass the argument explicitly",
                        function.name
                    ));
                }
                Ok(format!(
                    "{}: {}",
                    parameter.name,
                    self.type_to_goose(&parameter.ty)?
                ))
            })
            .collect::<Result<Vec<_>, String>>()?
            .join(", ");
        let return_type = match &function.return_type {
            Some(ty) => format!(" -> {}", self.type_to_goose(ty)?),
            None => String::new(),
        };
        writeln!(result, "fn {}({}){} {{", function.name, params, return_type)
            .map_err(|e| e.to_string())?;
        for statement in function.body.as_deref().unwrap_or_default() {
            self.statement_into(&mut result, 1, &statement.node)?;
        }
        writeln!(result, "}}").map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Render one statement into `output` at the given indentation.
    fn statement_into(
        &mut self,
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
                self.note_string_declaration(name, ty.as_ref(), value.as_ref());
                let value = match (ty, value) {
                    (Some(ty), Some(value)) => {
                        format!(": {} = {}", self.type_to_goose(ty)?, self.expr(value)?)
                    }
                    (Some(ty), None) => format!(": {}", self.type_to_goose(ty)?),
                    (None, Some(value)) => format!(" = {}", self.expr(value)?),
                    (None, None) => {
                        return Err(format!(
                            "Goose backend needs a type or initializer for `var {name}`"
                        ))
                    }
                };
                writeln!(output, "var {name}{value};").map_err(|e| e.to_string())?;
            }
            Statement::LetDeclaration { name, ty, value } => {
                line_prefix(output);
                self.note_string_declaration(name, ty.as_ref(), Some(value));
                let value = match ty {
                    Some(ty) => format!(": {} = {}", self.type_to_goose(ty)?, self.expr(value)?),
                    None => format!(" = {}", self.expr(value)?),
                };
                writeln!(output, "let {name}{value};").map_err(|e| e.to_string())?;
            }
            Statement::ConstDeclaration { name, value } => {
                line_prefix(output);
                if self.is_string_expression(value) {
                    self.string_variables.borrow_mut().insert(name.clone());
                }
                writeln!(output, "const {name} = {};", self.expr(value)?)
                    .map_err(|e| e.to_string())?;
            }
            Statement::Assignment { target, value } => {
                line_prefix(output);
                let rendered_value = self.expr(value)?;
                // Goose §4.4 rejects building a new value over the old
                // contents of the variable being assigned (`s = str(s,
                // "x")` errors with "build it in a variable of its own, and
                // assign copy() of that"). A self-referential string
                // assignment therefore goes through a fresh `u8[>..]` temp
                // plus `copy()`, which the real `goose check` accepts.
                if let Expression::Identifier(name) = target {
                    if self.string_variables.borrow().contains(name)
                        && self.expression_references_identifier(value, name)
                    {
                        let index = {
                            let mut counter = self.temp_counter.borrow_mut();
                            let index = *counter;
                            *counter += 1;
                            index
                        };
                        writeln!(
                            output,
                            "var __poly_str_tmp{index}: u8[>..] = {rendered_value};"
                        )
                        .map_err(|e| e.to_string())?;
                        writeln!(output, "{name} = copy(__poly_str_tmp{index});")
                            .map_err(|e| e.to_string())?;
                        return Ok(());
                    }
                }
                writeln!(output, "{} = {};", self.expr(target)?, rendered_value)
                    .map_err(|e| e.to_string())?;
            }
            Statement::PutStatement { expr, redirect } => {
                if redirect.is_some() {
                    return Err(
                        "Goose backend does not support `put ... to/from` file redirects; use a #goose block for file I/O"
                            .to_string(),
                    );
                }
                line_prefix(output);
                writeln!(output, "print({});", self.expr(expr)?).map_err(|e| e.to_string())?;
            }
            Statement::ErrorStatement(_)
            | Statement::WarnStatement(_)
            | Statement::InfoStatement(_) => {
                return Err(
                    "Goose backend cannot write to stderr (`err`/`warn`/`info`); use `put` or a #goose block"
                        .to_string(),
                )
            }
            Statement::ExpressionStatement(expr) => match expr {
                Expression::WhileLoop { condition, body } => {
                    line_prefix(output);
                    writeln!(output, "while ({}) {{", self.expr(condition)?)
                        .map_err(|e| e.to_string())?;
                    self.block_into(output, indent + 1, body)?;
                    line_prefix(output);
                    output.push_str("}\n");
                }
                Expression::IfExpression {
                    condition,
                    then_block,
                    else_block,
                } => {
                    line_prefix(output);
                    writeln!(output, "if ({}) {{", self.expr(condition)?)
                        .map_err(|e| e.to_string())?;
                    self.block_into(output, indent + 1, then_block)?;
                    line_prefix(output);
                    match else_block {
                        Some(else_block) if !else_block.is_empty() => {
                            output.push_str("} else {\n");
                            self.block_into(output, indent + 1, else_block)?;
                            line_prefix(output);
                            output.push_str("}\n");
                        }
                        _ => output.push_str("}\n"),
                    }
                }
                Expression::LoopRange {
                    variable,
                    ranges,
                    body,
                } => self.loop_range_into(output, indent, variable, ranges, body)?,
                Expression::ForLoop {
                    variable,
                    iterable,
                    body,
                } => {
                    line_prefix(output);
                    writeln!(output, "for {variable}: i32 in {} {{", self.expr(iterable)?)
                        .map_err(|e| e.to_string())?;
                    self.block_into(output, indent + 1, body)?;
                    line_prefix(output);
                    output.push_str("}\n");
                }
                Expression::InfiniteLoop(body) => {
                    line_prefix(output);
                    output.push_str("loop {\n");
                    self.block_into(output, indent + 1, body)?;
                    line_prefix(output);
                    output.push_str("}\n");
                }
                Expression::MatchExpression { scrutinee, arms } => {
                    self.match_into(output, indent, scrutinee, arms, false)?;
                }
                _ => {
                    line_prefix(output);
                    writeln!(output, "{};", self.expr(expr)?).map_err(|e| e.to_string())?;
                }
            },
            Statement::ReturnStatement(value) => {
                line_prefix(output);
                if let Some(value) = value {
                    writeln!(output, "return {};", self.expr(value)?).map_err(|e| e.to_string())?;
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
            Statement::ForeignBlock { language, content } if language == "goose" => {
                line_prefix(output);
                output.push_str(content.trim_matches('\n'));
                output.push('\n');
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
            | Statement::DependencyDeclaration(_)
            | Statement::TypeDeclaration(_) => {
                return Err(
                    "this Poly declaration is not supported inside a Goose function".to_string(),
                );
            }
        }
        Ok(())
    }

    fn block_into(
        &mut self,
        output: &mut String,
        indent: usize,
        block: &ast::Block,
    ) -> Result<(), String> {
        for statement in block {
            self.statement_into(output, indent, &statement.node)?;
        }
        Ok(())
    }

    /// Lower `loop <var> <parts> [step n]`.
    ///
    /// A single value part becomes a `for x in collection`. A range with a
    /// literal positive step (or no step) lowers to Goose's native half-open
    /// `for x in start..end` when that can express it exactly; otherwise a
    /// `while` carries the inclusive bound, sign, or dynamic step. Goose has
    /// no C-style `for` and no stepped range, so the `while` form is the
    /// verified spelling for those. A runtime (non-literal) step picks its
    /// direction per iteration and a zero step terminates the loop, mirroring
    /// the shared semantics of the other backends.
    fn loop_range_into(
        &mut self,
        output: &mut String,
        indent: usize,
        variable: &str,
        ranges: &[LoopRangePart],
        body: &ast::Block,
    ) -> Result<(), String> {
        let line_prefix_for = |output: &mut String, level: usize| {
            for _ in 0..level {
                output.push_str("    ");
            }
        };
        if ranges.len() != 1 {
            return Err("Goose backend currently supports one range per loop".to_string());
        }
        if let LoopRangePart::Value(collection) = &ranges[0] {
            line_prefix_for(output, indent);
            writeln!(output, "for {variable} in {} {{", self.expr(collection)?)
                .map_err(|e| e.to_string())?;
            self.block_into(output, indent + 1, body)?;
            line_prefix_for(output, indent);
            output.push_str("}\n");
            return Ok(());
        }
        let LoopRangePart::Range {
            start,
            end,
            inclusive,
            step,
        } = &ranges[0]
        else {
            return Err("loop range part must be a Range after Value handling".to_string());
        };
        let step_is_literal = step.as_ref().is_none_or(is_literal_expression);
        let step_is_positive = step.as_ref().is_none_or(|value| {
            !is_negative_expression(value)
                && !matches!(
                    value,
                    Expression::IntLiteral(text)
                        if text.trim_start_matches(['-', '+']).parse::<i64>() == Ok(0)
                )
        });
        // A plain ascending literal step can use the native half-open `for`,
        // with `end + 1` when the range is inclusive. This is the only form
        // that can rely on the native range; every other shape falls to `while`.
        if step_is_literal && step_is_positive && !is_negative_expression(start) {
            let step_text = match step {
                None => "1".to_string(),
                Some(value) => self.expr(value)?,
            };
            if step_text == "1" {
                let upper = if *inclusive {
                    format!("(({}) + 1)", self.expr(end)?)
                } else {
                    self.expr(end)?
                };
                line_prefix_for(output, indent);
                writeln!(output, "for {variable}: i32 in {}..{upper} {{", self.expr(start)?)
                    .map_err(|e| e.to_string())?;
                self.block_into(output, indent + 1, body)?;
                line_prefix_for(output, indent);
                output.push_str("}\n");
                return Ok(());
            }
        }
        // `while` form: seed the variable, an optional step/end temp, then
        // test a fully parenthesized direction-aware condition.
        // Each sign branch needs its own comparison. A non-literal step
        // keeps its sign at runtime, so the descending branch must test the
        // bound downwards (`>=`/`>`) even though nothing in the source is
        // statically negative — the same dispatch the C and JS backends use
        // (their `runtime_step_uses_sign_dispatch` regressions).
        let comparison_up = if *inclusive { "<=" } else { "<" };
        let comparison_down = if *inclusive { ">=" } else { ">" };
        let step_value = step
            .as_ref()
            .map(|value| self.expr(value))
            .transpose()?
            .unwrap_or_else(|| "1".to_string());
        let start_value = self.expr(start)?;
        let end_value = self.expr(end)?;
        line_prefix_for(output, indent);
        output.push_str("{\n");
        line_prefix_for(output, indent + 1);
        // Typed like the C backend: the loop variable is `i32` (Poly's
        // integer default), and the step is widened to `i64` so one sign
        // dispatch covers every step expression.
        writeln!(output, "var {variable}: i32 = {start_value};").map_err(|e| e.to_string())?;
        line_prefix_for(output, indent + 1);
        writeln!(output, "var __poly_end: i32 = {end_value};").map_err(|e| e.to_string())?;
        line_prefix_for(output, indent + 1);
        writeln!(output, "var __poly_step: i64 = {step_value};").map_err(|e| e.to_string())?;
        // Dispatch on the step's runtime sign: ascending compares against
        // the upper bound, descending against the lower bound; the whole
        // condition is parenthesized.
        line_prefix_for(output, indent + 1);
        writeln!(
            output,
            "while (__poly_step > 0 && ({variable} {comparison_up} __poly_end)) || (__poly_step < 0 && ({variable} {comparison_down} __poly_end)) {{"
        )
        .map_err(|e| e.to_string())?;
        self.block_into(output, indent + 2, body)?;
        line_prefix_for(output, indent + 1);
        writeln!(output, "{variable} += __poly_step as i32;").map_err(|e| e.to_string())?;
        line_prefix_for(output, indent + 1);
        output.push_str("}\n");
        line_prefix_for(output, indent);
        output.push_str("}\n");
        Ok(())
    }

    /// Emit a `match` statement (or value when `value_position`).
    fn match_into(
        &mut self,
        output: &mut String,
        indent: usize,
        scrutinee: &Expression,
        arms: &[ast::MatchArm],
        value_position: bool,
    ) -> Result<(), String> {
        let line_prefix = |output: &mut String| {
            for _ in 0..indent {
                output.push_str("    ");
            }
        };
        let line_prefix_for = |output: &mut String, level: usize| {
            for _ in 0..level {
                output.push_str("    ");
            }
        };
        line_prefix(output);
        let label = match (value_position, arms.is_empty()) {
            (false, _) => "match",
            (true, false) => "match",
            (true, true) => return Err("match expression has no arms".to_string()),
        };
        writeln!(output, "{label} {} {{", self.expr(scrutinee)?).map_err(|e| e.to_string())?;
        for arm in arms {
            if arm.guard.is_some() {
                return Err(
                    "Goose backend does not support match guards; handle the condition with an if inside a #goose block"
                        .to_string(),
                );
            }
            let pattern = match &arm.pattern {
                Pattern::Wildcard => "_".to_string(),
                Pattern::Literal(literal) => self.expr(literal)?,
                Pattern::Identifier(name) => {
                    return Err(format!(
                        "Goose backend supports literal and wildcard match patterns only (found binding `{name}`); use a #goose block for binding patterns"
                    ))
                }
                other => {
                    let _ = other;
                    return Err(
                        "Goose backend supports literal and wildcard match patterns only; use a #goose block for other patterns"
                            .to_string(),
                    );
                }
            };
            line_prefix_for(output, indent + 1);
            match &arm.body {
                MatchArmBody::Expression(expr) if value_position => {
                    writeln!(output, "{pattern} => {},", self.expr(expr)?)
                        .map_err(|e| e.to_string())?;
                }
                MatchArmBody::Expression(expr) => {
                    writeln!(output, "{pattern} => {},", self.expr(expr)?)
                        .map_err(|e| e.to_string())?;
                }
                MatchArmBody::Block(statements) if value_position => {
                    return Err(
                        "Goose backend match expressions support expression arms only in value position; use a match statement"
                            .to_string(),
                    )
                }
                MatchArmBody::Block(statements) => {
                    // A multi-statement arm is a block after the `=>`, not a
                    // bare block: `name => { ... },` (Goose reference §9).
                    writeln!(output, "{pattern} => {{").map_err(|e| e.to_string())?;
                    self.block_into(output, indent + 2, statements)?;
                    line_prefix_for(output, indent + 1);
                    output.push_str("},\n");
                }
            }
        }
        line_prefix(output);
        output.push_str("}\n");
        Ok(())
    }

    /// Render only expressions that have an explicit Goose representation.
    /// Keeping unsupported variants as errors prevents silently incorrect code.
    fn expr(&mut self, expression: &Expression) -> Result<String, String> {
        match expression {
            // Normalise to plain decimal: Goose understands `0x`, `0b`,
            // and `0o` but not leading-zero octal (`0755`). Decimal is valid
            // everywhere and always means the same number.
            Expression::IntLiteral(value) => Ok(int_literal_text(value)),
            Expression::FloatLiteral(value) => Ok(value.clone()),
            Expression::StringLiteral(value) | Expression::UnicodeStringLiteral(value) => {
                Ok(format!("\"{}\"", goose_escape(value)))
            }
            Expression::UnicodeCharLiteral(value) => Err(format!(
                "Goose backend does not support unicode character literals (`{}`); use a string literal",
                value
            )),
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
                if matches!(op, BinaryOp::Power) {
                    return self.gen_power(left, right);
                }
                // String `+` concatenation: Goose has no operator `+` for
                // text, so every operand of an additive chain involving a
                // string is flattened and formatted with the generic `str(...)`.
                if matches!(op, BinaryOp::Add)
                    && (self.is_string_expression(left) || self.is_string_expression(right))
                {
                    let mut operands = Vec::new();
                    self.collect_add_operands(left, &mut operands)?;
                    self.collect_add_operands(right, &mut operands)?;
                    return Ok(format!("str({})", operands.join(", ")));
                }
                match op {
                    BinaryOp::Div if !self.is_string_expression(left) => {
                        // Integer division must truncate like Rust/C, not floor.
                        return Ok(format!(
                            "(({}) / ({}))",
                            self.expr(left)?,
                            self.expr(right)?
                        ));
                    }
                    _ => {}
                }
                if matches!(op, BinaryOp::Mod) {
                    *self.used_mod_helper.borrow_mut() = true;
                    return Ok(format!(
                        "__poly_mod_trunc({}, {})",
                        self.expr(left)?,
                        self.expr(right)?
                    ));
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
                match method.as_str() {
                    // `x.to_string()` becomes the generic `str(x)`, which
                    // renders any value the way the other targets do.
                    "to_string" if args.is_empty() => {
                        Ok(format!("str({})", self.expr(object)?))
                    }
                    // Slices and arrays expose `.len` as a member, not a method.
                    "len" if args.is_empty() => Ok(format!("({}).len", self.expr(object)?)),
                    "push" if args.len() == 1 => Ok(format!(
                        "({}).push({})",
                        self.expr(object)?,
                        self.expr(&args[0])?
                    )),
                    _ => Err(format!(
                        "Goose backend does not support the method `.{method}`; use `.to_string()`, `.len`, `.push`, or a #goose block"
                    )),
                }
            }
            Expression::FieldAccess { object, field } => {
                Ok(format!("({}).{}", self.expr(object)?, field))
            }
            Expression::TupleIndex { .. } => Err(
                "Goose backend does not support tuple indexing; use a struct or a #goose block"
                    .to_string(),
            ),
            Expression::Index { object, index } => Ok(format!(
                "({})[{}]",
                self.expr(object)?,
                self.expr(index)?
            )),
            Expression::Parenthesized(inner) => Ok(format!("({})", self.expr(inner)?)),
            Expression::AsExpression { expr, ty } => Ok(format!(
                "({}) as! {}",
                self.expr(expr)?,
                self.type_to_goose(ty)?
            )),
            Expression::ArrayLiteral(elements) => Ok(format!(
                "[{}]",
                elements
                    .iter()
                    .map(|element| self.expr(element))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", ")
            )),
            Expression::TupleLiteral(_) => Err(
                "Goose backend does not support tuple literals; use a struct or a #goose block"
                    .to_string(),
            ),
            Expression::StructLiteral { name, fields } => {
                let initializers = fields
                    .iter()
                    .map(|(field, value)| Ok(format!("{field}: {}", self.expr(value)?)))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", ");
                Ok(format!("{name} {{ {initializers} }}"))
            }
            Expression::EnumVariant { enum_name, .. } => Err(format!(
                "Goose backend does not support Poly enum variants (`{enum_name}`); declare the enum in a #goose block"
            )),
            Expression::Range { start, end, inclusive } => {
                if *inclusive {
                    Ok(format!("({})..(({}) + 1)", self.expr(start)?, self.expr(end)?))
                } else {
                    Ok(format!("({})..({})", self.expr(start)?, self.expr(end)?))
                }
            }
            Expression::LoopRange { .. }
            | Expression::ForLoop { .. }
            | Expression::InfiniteLoop(_)
            | Expression::WhileLoop { .. } => Err(
                "loop expressions are only valid as statements in the Goose backend".to_string(),
            ),
            Expression::IfExpression { .. } => Err(
                "if expressions in value position are not supported by the Goose backend; assign with a match or split into statements"
                    .to_string(),
            ),
            Expression::GetExpression(_) => Err(
                "Goose backend does not support `get` input; read the value as a constant or use a #goose block"
                    .to_string(),
            ),
            Expression::MatchExpression { scrutinee, arms } => {
                // Value-position match lowers to Goose's native `match`
                // expression, which requires expression arms.
                let mut rendered = String::new();
                let mut scratch = String::new();
                self.match_expression_into(&mut scratch, scrutinee, arms)?;
                rendered.push_str(&scratch);
                Ok(rendered)
            }
            Expression::Closure { .. } => Err(
                "Goose backend does not support closures; use a named function or a #goose block"
                    .to_string(),
            ),
            Expression::TryExpression(_) => Err(
                "Goose backend does not support `?` try expressions; handle the error explicitly or use a #goose block"
                    .to_string(),
            ),
            Expression::UnsafeBlock(_) => Err(
                "Goose backend does not support unsafe blocks; use a #goose block".to_string(),
            ),
        }
    }

    /// Render a value-position `match` expression (expression arms only).
    fn match_expression_into(
        &mut self,
        output: &mut String,
        scrutinee: &Expression,
        arms: &[ast::MatchArm],
    ) -> Result<(), String> {
        writeln!(output, "match {} {{", self.expr(scrutinee)?).map_err(|e| e.to_string())?;
        for arm in arms {
            if arm.guard.is_some() {
                return Err(
                    "Goose backend does not support match guards; handle the condition with an if inside a #goose block"
                        .to_string(),
                );
            }
            let pattern = match &arm.pattern {
                Pattern::Wildcard => "_".to_string(),
                Pattern::Literal(literal) => self.expr(literal)?,
                Pattern::Identifier(name) => {
                    return Err(format!(
                        "Goose backend supports literal and wildcard match patterns only (found binding `{name}`); use a #goose block for binding patterns"
                    ))
                }
                other => {
                    let _ = other;
                    return Err(
                        "Goose backend supports literal and wildcard match patterns only; use a #goose block for other patterns"
                            .to_string(),
                    );
                }
            };
            match &arm.body {
                MatchArmBody::Expression(expr) => {
                    writeln!(output, "    {pattern} => {},", self.expr(expr)?)
                        .map_err(|e| e.to_string())?;
                }
                MatchArmBody::Block(_) => {
                    return Err(
                        "Goose backend match expressions support expression arms only in value position; use a match statement"
                            .to_string(),
                    )
                }
            }
        }
        writeln!(output, "}}").map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Flatten an additive chain's string-bearing operands into `str(...)` arguments.
    fn collect_add_operands(
        &mut self,
        expression: &Expression,
        operands: &mut Vec<String>,
    ) -> Result<(), String> {
        if let Expression::BinaryOp {
            op: BinaryOp::Add,
            left,
            right,
        } = expression
        {
            if self.is_string_expression(left) || self.is_string_expression(right) {
                self.collect_add_operands(left, operands)?;
                self.collect_add_operands(right, operands)?;
                return Ok(());
            }
        }
        operands.push(self.expr(expression)?);
        Ok(())
    }

    /// Render `a ^ b` (exponentiation). Goose has no power operator, so
    /// integer-class operands lower to the `__poly_pow` helper, which matches
    /// the shared "negative integer exponent yields 0" rule. Float exponents
    /// are rejected with guidance rather than silently miscompiled.
    fn gen_power(&mut self, left: &Expression, right: &Expression) -> Result<String, String> {
        if self.is_float_expression(left) || self.is_float_expression(right) {
            return Err(concat!(
                "Goose backend does not support float exponentiation (`^` on f32/f64); ",
                "cast both sides to an integer or handle the power in a #goose block"
            )
            .to_string());
        }
        *self.used_power_helper.borrow_mut() = true;
        Ok(format!(
            "__poly_pow({}, {})",
            self.expr(left)?,
            self.expr(right)?
        ))
    }

    fn binary_op(&self, op: &BinaryOp) -> &'static str {
        match op {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            // Mod never reaches the infix renderer: the BinaryOp arm routes it
            // to the helper so the result is always the positive remainder.
            BinaryOp::Mod => "%",
            BinaryOp::Power => "^",
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

    fn unary_op(&self, op: &UnaryOp) -> Result<&'static str, String> {
        match op {
            UnaryOp::Neg => Ok("-"),
            UnaryOp::Not => Ok("!"),
            UnaryOp::BitNot => Ok("~"),
            UnaryOp::Deref => Err(
                "Goose backend does not support raw pointer dereferences; use a #goose block"
                    .to_string(),
            ),
        }
    }

    fn note_string_declaration(
        &self,
        name: &str,
        ty: Option<&TypeAnnotation>,
        value: Option<&Expression>,
    ) {
        if ty.is_some_and(|ty| self.is_string_type(ty))
            || value.is_some_and(|value| self.is_string_literal(value))
        {
            self.string_variables.borrow_mut().insert(name.to_string());
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

    /// True when `name` occurs anywhere inside `expression`. Used to spot a
    /// self-referential string assignment (`s := s + "x"`), which Goose
    /// §4.4 forces through a temp buffer plus `copy()`. The match covers
    /// every expression variant that can embed a sub-expression, so the
    /// check never under-approximates (a miss would emit code the real
    /// `goose check` rejects); statement-shaped variants cannot appear in an
    /// assignment's right-hand side and fall through to `false`.
    fn expression_references_identifier(&self, expression: &Expression, name: &str) -> bool {
        match expression {
            Expression::Identifier(found) => found == name,
            Expression::BinaryOp { left, right, .. } => {
                self.expression_references_identifier(left, name)
                    || self.expression_references_identifier(right, name)
            }
            Expression::UnaryOp { expr, .. }
            | Expression::Parenthesized(expr)
            | Expression::TryExpression(expr)
            | Expression::FieldAccess { object: expr, .. }
            | Expression::TupleIndex { object: expr, .. } => {
                self.expression_references_identifier(expr, name)
            }
            Expression::AsExpression { expr, .. } => {
                self.expression_references_identifier(expr, name)
            }
            Expression::Call { func, args } => {
                self.expression_references_identifier(func, name)
                    || args
                        .iter()
                        .any(|arg| self.expression_references_identifier(arg, name))
            }
            Expression::MethodCall { object, args, .. } => {
                self.expression_references_identifier(object, name)
                    || args
                        .iter()
                        .any(|arg| self.expression_references_identifier(arg, name))
            }
            Expression::Index { object, index } => {
                self.expression_references_identifier(object, name)
                    || self.expression_references_identifier(index, name)
            }
            Expression::ArrayLiteral(items) | Expression::TupleLiteral(items) => items
                .iter()
                .any(|item| self.expression_references_identifier(item, name)),
            Expression::StructLiteral { fields, .. } => fields
                .iter()
                .any(|(_, value)| self.expression_references_identifier(value, name)),
            Expression::EnumVariant { data: Some(items), .. } => items
                .iter()
                .any(|item| self.expression_references_identifier(item, name)),
            Expression::EnumVariant { data: None, .. }
            | Expression::StringLiteral(_)
            | Expression::UnicodeStringLiteral(_)
            | Expression::IntLiteral(_)
            | Expression::FloatLiteral(_)
            | Expression::BoolLiteral(_)
            | Expression::GetExpression(_) => false,
            Expression::Range { start, end, .. } => {
                self.expression_references_identifier(start, name)
                    || self.expression_references_identifier(end, name)
            }
            Expression::MatchExpression { scrutinee, arms } => {
                self.expression_references_identifier(scrutinee, name)
                    || arms.iter().any(|arm| {
                        arm.guard.as_ref().is_some_and(|guard| {
                            self.expression_references_identifier(guard, name)
                        }) || match &arm.body {
                            ast::MatchArmBody::Expression(expression) => {
                                self.expression_references_identifier(expression, name)
                            }
                            ast::MatchArmBody::Block(block) => block.iter().any(|statement| {
                                self.statement_references_identifier(&statement.node, name)
                            }),
                        }
                    })
            }
            Expression::Closure { body, .. } => {
                self.expression_references_identifier(body, name)
            }
            Expression::UnsafeBlock(block) => block
                .iter()
                .any(|statement| self.statement_references_identifier(&statement.node, name)),
            Expression::WhileLoop { condition, body } => {
                self.expression_references_identifier(condition, name)
                    || body
                        .iter()
                        .any(|statement| self.statement_references_identifier(&statement.node, name))
            }
            Expression::ForLoop { iterable, body, .. } => {
                self.expression_references_identifier(iterable, name)
                    || body
                        .iter()
                        .any(|statement| self.statement_references_identifier(&statement.node, name))
            }
            Expression::LoopRange { ranges, body, .. } => {
                ranges.iter().any(|part| match part {
                    ast::LoopRangePart::Value(expression) => {
                        self.expression_references_identifier(expression, name)
                    }
                    ast::LoopRangePart::Range { start, end, step, .. } => {
                        self.expression_references_identifier(start, name)
                            || self.expression_references_identifier(end, name)
                            || step.as_ref().is_some_and(|step| {
                                self.expression_references_identifier(step, name)
                            })
                    }
                }) || body
                    .iter()
                    .any(|statement| self.statement_references_identifier(&statement.node, name))
            }
            Expression::InfiniteLoop(body) => body
                .iter()
                .any(|statement| self.statement_references_identifier(&statement.node, name)),
            Expression::IfExpression {
                condition,
                then_block,
                else_block,
            } => {
                self.expression_references_identifier(condition, name)
                    || then_block
                        .iter()
                        .any(|statement| self.statement_references_identifier(&statement.node, name))
                    || else_block.as_ref().is_some_and(|block| {
                        block
                            .iter()
                            .any(|statement| {
                                self.statement_references_identifier(&statement.node, name)
                            })
                    })
            }
            _ => false,
        }
    }

    /// Statement-level counterpart of
    /// [`Self::expression_references_identifier`], covering the expression
    /// variants that embed statement blocks (if expressions) whose bodies
    /// can reference an assignment's own target. The `_` arm tolerates the
    /// loop-shaped expression variants (`while`, `loop`, `for`) the parser
    /// can also produce there.
    fn statement_references_identifier(&self, statement: &Statement, name: &str) -> bool {
        match statement {
            Statement::VarDeclaration { value, .. } => value
                .as_ref()
                .is_some_and(|value| self.expression_references_identifier(value, name)),
            Statement::LetDeclaration { value, .. } | Statement::ConstDeclaration { value, .. } => {
                self.expression_references_identifier(value, name)
            }
            Statement::Assignment { target, value } => {
                self.expression_references_identifier(target, name)
                    || self.expression_references_identifier(value, name)
            }
            _ => false,
        }
    }

    /// True when the expression is plausibly a float (the checker's same-class
    /// rule keeps floats out of integer arithmetic, so a float operand on `^`
    /// means the whole operation is float-class).
    fn is_float_expression(&self, expression: &Expression) -> bool {
        match expression {
            Expression::FloatLiteral(_) => true,
            Expression::Parenthesized(inner) => self.is_float_expression(inner),
            Expression::AsExpression { expr, ty } => {
                matches!(&**ty, TypeAnnotation::Named(name) if name == "f32" || name == "f64")
                    || self.is_float_expression(expr)
            }
            Expression::UnaryOp { expr, .. } => self.is_float_expression(expr),
            Expression::BinaryOp { left, right, .. } => {
                self.is_float_expression(left) || self.is_float_expression(right)
            }
            _ => false,
        }
    }

    /// Map a Poly type to its Goose spelling. Poly strings are UTF-8 bytes,
    /// so they lower to `u8[>..]`; other arrays keep their element type and
    /// slice spelling.
    fn type_to_goose(&mut self, annotation: &TypeAnnotation) -> Result<String, String> {
        match annotation {
            TypeAnnotation::Named(name) => Ok(match name.as_str() {
                "string" | "ustring" | "bytes" => "u8[>..]".to_string(),
                other => other.to_string(),
            }),
            TypeAnnotation::Vec(inner) => Ok(format!("{}[>..]", self.type_to_goose(inner)?)),
            TypeAnnotation::Array(inner, len) => {
                let len_text = match len.as_ref() {
                    Expression::IntLiteral(value) => int_literal_text(value),
                    other => {
                        return Err(format!(
                            "Goose backend needs a literal array length; found `{}`. Use a slice or a #goose block",
                            self.expr(other)?
                        ))
                    }
                };
                Ok(format!("{}[{len_text}]", self.type_to_goose(inner)?))
            }
            TypeAnnotation::Option(inner) => {
                Ok(format!("{}?", self.type_to_goose(inner)?))
            }
            other => {
                let _ = other;
                Err(
                    "Goose backend does not support this Poly type (tuples, references, raw pointers, nullable, function, or generic types); use a #goose block"
                        .to_string(),
                )
            }
        }
    }
}

/// Render a Poly string literal's bytes as a Goose string with only the
/// escapes the language reference documents: `\\`, `\"`, `\n`, `\r`, `\t`,
/// and fixed two-digit `\xNN`. Everything else passes through byte-for-byte,
/// so valid UTF-8 round-trips unchanged.
fn goose_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'\\' => escaped.push_str("\\\\"),
            b'"' => escaped.push_str("\\\""),
            b'\n' => escaped.push_str("\\n"),
            b'\r' => escaped.push_str("\\r"),
            b'\t' => escaped.push_str("\\t"),
            0x20..=0x7E => escaped.push(byte as char),
            _ => {
                let _ = write!(escaped, "\\x{byte:02X}");
            }
        }
    }
    escaped
}

/// The helper lowering `a ^ b` for integer-class operands. Goose has no
/// power operator, so this mirrors the shared "negative integer exponent
/// yields 0" contract via an exponentiation-by-squaring loop.
///
/// The parameters are deliberately untyped: Goose specializes an untyped
/// function per call-site type (verified against the real compiler), so one
/// helper covers i8..u64 operands. A typed `base: i32` helper rejected every
/// non-i32 operand ("narrowing and sign changes require an explicit `as`").
/// Locals are `var` because Goose `let` bindings are immutable, and the loop
/// is iterative because a recursive Goose function must be declared
/// `recursive fn` *and* take fully explicit parameter types — both of which
/// are incompatible with a generic helper.
fn power_helper() -> String {
    "fn __poly_pow(base, exp) {\n    if exp < 0 {\n        return 0;\n    }\n    var result = 1;\n    var b = base;\n    var e = exp;\n    while e > 0 {\n        if e % 2 == 1 {\n            result = result * b;\n        }\n        b = b * b;\n        e = e / 2;\n    }\n    return result;\n}"
        .to_string()
}

/// The helper lowering `%` to the truncated (sign-of-dividend) remainder,
/// matching the frozen rust/c/asm/js contract.
///
/// Goose's native `%` is *Euclidean*, not truncated (reference
/// §Arithmetic: "the result lies in `[0, |b|)` and is never negative"), so
/// `-17 % 5` yields 3 where the contract demands -2. `std.rem(a, b)` is the
/// documented truncated partner of `/` and returns exactly the contract
/// values (verified against the real compiler), so the helper delegates to
/// it; the file-level `import std;` is emitted alongside. The parameters
/// are untyped so the helper specializes per operand width and signedness.
fn mod_helper() -> String {
    "fn __poly_mod_trunc(a, b) {\n    return rem(a, b);\n}".to_string()
}

/// True for integer literals (optionally wrapped in parens), used to decide
/// between the native half-open `for` (literal step 1) and the `while` form.
fn is_literal_expression(expression: &Expression) -> bool {
    match expression {
        Expression::IntLiteral(_) => true,
        Expression::UnaryOp {
            op: UnaryOp::Neg,
            expr,
        } => matches!(expr.as_ref(), Expression::IntLiteral(_)),
        Expression::Parenthesized(inner) => is_literal_expression(inner),
        _ => false,
    }
}

/// True for a negative integer literal, used to detect a descending loop.
fn is_negative_expression(expression: &Expression) -> bool {
    match expression {
        Expression::IntLiteral(value) => value.trim_start().starts_with('-'),
        Expression::UnaryOp {
            op: UnaryOp::Neg,
            expr,
        } => matches!(expr.as_ref(), Expression::IntLiteral(_)),
        Expression::Parenthesized(inner) => is_negative_expression(inner),
        _ => false,
    }
}

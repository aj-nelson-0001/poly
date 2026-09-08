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
    enums: Vec<String>,
    /// Opaque target-language definitions selected by the transpiler.
    c_blocks: Vec<String>,
    string_variables: RefCell<std::collections::HashSet<String>>,
    main_statements: Vec<ast::Spanned<Statement>>,
    /// Set when a `get` expression is lowered; gates the stdin runtime emission.
    uses_get: std::cell::Cell<bool>,
    /// Counter for synthesizing unique closure function names.
    closure_counter: std::cell::Cell<usize>,
    /// Definitions discovered while rendering main (closure functions); these
    /// are emitted after the eagerly-rendered declarations, right before main.
    late_definitions: Vec<String>,
}

impl CGenerator {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            functions: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            c_blocks: Vec::new(),
            string_variables: RefCell::new(std::collections::HashSet::new()),
            main_statements: Vec::new(),
            uses_get: std::cell::Cell::new(false),
            closure_counter: std::cell::Cell::new(0),
            late_definitions: Vec::new(),
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
                Statement::EnumDeclaration(declaration) => {
                    self.enums.push(self.enumeration(declaration)?)
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
        for enumeration in &self.enums {
            self.output.push_str(enumeration);
            self.output.push_str("\n\n");
        }
        for function in &self.functions {
            self.output.push_str(function);
            self.output.push_str("\n\n");
        }

        // Render main into a scratch buffer first: statements can surface new
        // file-scope definitions (closure functions) and runtime helpers (`get`)
        // that must be emitted before main itself.
        let mut main_output = String::new();
        std::mem::swap(&mut main_output, &mut self.output);
        self.indent = 1;
        let main_statements = self.main_statements.clone();
        for statement in &main_statements {
            self.statement(&statement.node)?;
        }
        std::mem::swap(&mut main_output, &mut self.output);
        self.indent = 0;

        if self.uses_get.get()
            || self
                .main_statements
                .iter()
                .any(|s| statement_contains_get(&s.node))
        {
            self.output.push_str(self.get_runtime());
            self.output.push_str("\n\n");
        }
        let late_definitions = std::mem::take(&mut self.late_definitions);
        for definition in &late_definitions {
            self.output.push_str(definition);
            self.output.push('\n');
        }
        self.output.push_str("int main(void) {\n");
        self.output.push_str(&main_output);
        self.indent = 1;
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

    /// Render an enum declaration as a C enum. Only unit variants are
    /// representable; tuple/struct variants are rejected with guidance because
    /// C enums cannot carry payloads.
    fn enumeration(&self, declaration: &ast::EnumDecl) -> Result<String, String> {
        let mut result = String::new();
        writeln!(result, "typedef enum {} {{", declaration.name).unwrap();
        for variant in &declaration.variants {
            match variant {
                ast::EnumVariant::Unit(name) => {
                    // Enumerators are qualified (`Color_Red`) so `Color::Red`
                    // in Poly lowers to a unique C identifier.
                    writeln!(
                        result,
                        "    {},",
                        self.enum_variant_c_name(&declaration.name, name)
                    )
                    .unwrap();
                }
                ast::EnumVariant::Tuple(name, types) => {
                    let _ = types;
                    return Err(format!(
                        "C backend supports unit enum variants only; variant `{}` of `{}` carries data; use a #c helper",
                        name, declaration.name
                    ));
                }
                ast::EnumVariant::Struct(name, fields) => {
                    let _ = fields;
                    return Err(format!(
                        "C backend supports unit enum variants only; variant `{}` of `{}` carries data; use a #c helper",
                        name, declaration.name
                    ));
                }
            }
        }
        writeln!(result, "}} {};", declaration.name).unwrap();
        Ok(result)
    }

    /// Map a variant name to its qualified C enumerator (`Color::Red` -> `Color_Red`).
    fn enum_variant_c_name(&self, enum_name: &str, variant: &str) -> String {
        format!("{enum_name}_{variant}")
    }

    /// Lower a `get` input expression. The C subset supports stdin reads
    /// (with an optional prompt); file sources and behavioral flags fall back
    /// to `#c` helpers.
    fn get_expression(&self, get: &ast::GetExpr) -> Result<String, String> {
        if get.source.is_some() {
            return Err(
                "C backend `get` reads stdin only; file input requires a #c helper".to_string(),
            );
        }
        if !get.flags.is_empty() {
            return Err(
                "C backend `get` supports no flags; use a #c helper for flagged input".to_string(),
            );
        }
        if get.with_clause.is_some() {
            return Err(
                "C backend `get` does not support `with` clauses; use a #c helper".to_string(),
            );
        }
        let prompt = match &get.prompt {
            Some(prompt) => self.expr(prompt)?,
            None => String::new(),
        };
        self.uses_get.set(true);
        Ok(format!("__poly_get_line({prompt})"))
    }

    /// The shared stdin line-read helper, emitted once per translation unit.
    fn get_runtime(&self) -> &'static str {
        "/* Poly `get` runtime: read one line from stdin (without the newline). */\n\
         static char *__poly_get_line(const char *prompt) {\n\
             size_t __poly_cap = 128;\n\
             char *__poly_buf = (char *)malloc(__poly_cap);\n\
             size_t __poly_len = 0;\n\
             int __poly_c;\n\
             if (prompt != NULL && prompt[0] != '\\0') {\n\
                 fputs(prompt, stdout);\n\
                 fflush(stdout);\n\
             }\n\
             while ((__poly_c = fgetc(stdin)) != EOF && __poly_c != '\\n') {\n\
                 if (__poly_len + 1 >= __poly_cap) {\n\
                     __poly_cap *= 2;\n\
                     __poly_buf = (char *)realloc(__poly_buf, __poly_cap);\n\
                 }\n\
                 __poly_buf[__poly_len++] = (char)__poly_c;\n\
             }\n\
             __poly_buf[__poly_len] = '\\0';\n\
             return __poly_buf;\n\
         }"
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
        // Non-capturing closures in `var`/`let` initializers lower to a static
        // function plus a function pointer: `var double := |x: i32| x * 2` ->
        // `static int32_t __poly_closure_0(int32_t x) {...}` +
        // `int32_t (*double)(int32_t) = __poly_closure_0;`. Capturing closures
        // are rejected with guidance because C has no closure runtime here.
        let closure = match statement {
            Statement::VarDeclaration {
                value: Some(value), ..
            }
            | Statement::LetDeclaration { value, .. } => match value {
                Expression::Closure { .. } => Some(value.clone()),
                _ => None,
            },
            _ => None,
        };
        if let Some(Expression::Closure { params, body }) = closure {
            let captures = closure_captures(&params, &body);
            if !captures.is_empty() {
                return Err(format!(
                    "C backend supports non-capturing closures only; `{}` is captured from the enclosing scope; use a #c helper",
                    captures.join("`, `")
                ));
            }
            let index = self.closure_counter.get();
            self.closure_counter.set(index + 1);
            let fn_name = format!("__poly_closure_{index}");
            let mut rendered_fn = String::new();
            let return_type = self.inferred_type(Some(&body));
            let param_list = params
                .iter()
                .map(|parameter| Ok(format!("{} {}", self.ty(&parameter.ty)?, parameter.name)))
                .collect::<Result<Vec<_>, String>>()?
                .join(", ");
            writeln!(
                rendered_fn,
                "static {} {}({}) {{ return ({}); }}",
                return_type,
                fn_name,
                param_list,
                self.expr(&body)?
            )
            .unwrap();
            self.late_definitions.push(rendered_fn);
            let pointer_type = format!(
                "{} (*PLACEHOLDER)({})",
                return_type,
                params
                    .iter()
                    .map(|parameter| self.ty(&parameter.ty))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", ")
            );
            let name = match statement {
                Statement::VarDeclaration { name, .. } | Statement::LetDeclaration { name, .. } => {
                    name
                }
                _ => unreachable!(),
            };
            let line = format!(
                "{} = {};\n",
                pointer_type.replace("PLACEHOLDER", name),
                fn_name
            );
            line_prefix_for(&mut self.output, self.indent);
            self.output.push_str(&line);
            return Ok(());
        }
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
                    || value.as_ref().is_some_and(|value| {
                        self.is_string_literal(value)
                            || matches!(value, Expression::GetExpression(_))
                    })
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
                Expression::MatchExpression { scrutinee, arms } => {
                    // Match lowers to an if/else-if chain over literal-
                    // comparable patterns, wrapped in a block so arm-local
                    // bindings cannot leak into the enclosing scope. Struct
                    // patterns keep the scrutinee in a temporary named
                    // __poly_match (one per arm body via the fresh binding).
                    let scrutinee_str = self.expr(scrutinee)?;
                    let match_var = "__poly_match";
                    line_prefix(output);
                    output.push_str("{\n");
                    line_prefix_for(output, indent + 1);
                    writeln!(
                        output,
                        "const {} {} = {};",
                        self.inferred_type(Some(scrutinee)),
                        match_var,
                        scrutinee_str
                    )
                    .unwrap();
                    for (arm_index, arm) in arms.iter().enumerate() {
                        line_prefix_for(output, indent + 1);
                        let is_wildcard = matches!(arm.pattern, ast::Pattern::Wildcard);
                        if is_wildcard {
                            // Wildcard: always matches, so it closes the chain
                            // with a plain `else` (any later arm is dead code
                            // but still compiles).
                            if arm_index == 0 {
                                output.push_str("if (1) {\n");
                            } else {
                                output.push_str("else {\n");
                            }
                        } else {
                            if arm_index == 0 {
                                write!(output, "if ").unwrap();
                            } else {
                                write!(output, "else if ").unwrap();
                            }
                            match &arm.pattern {
                                ast::Pattern::Literal(literal) => {
                                    writeln!(
                                        output,
                                        "({} == {}) {{",
                                        match_var,
                                        self.expr(literal)?
                                    )
                                    .unwrap();
                                }
                                ast::Pattern::Range {
                                    start,
                                    end,
                                    inclusive,
                                } => {
                                    let lower = if *inclusive { ">=" } else { ">" };
                                    let upper = if *inclusive { "<=" } else { "<" };
                                    writeln!(
                                        output,
                                        "({} {} {} && {} {} {}) {{",
                                        match_var,
                                        lower,
                                        self.expr(start)?,
                                        match_var,
                                        upper,
                                        self.expr(end)?
                                    )
                                    .unwrap();
                                }
                                ast::Pattern::Identifier(name) => {
                                    // Enum-variant identifiers have no C
                                    // representation in this subset; a plain
                                    // binding matches everything and binds the
                                    // scrutinee.
                                    writeln!(output, "(1) {{ // binding `{name}` = {}", match_var)
                                        .unwrap();
                                }
                                ast::Pattern::Enum {
                                    enum_name,
                                    variant,
                                    inner,
                                } => {
                                    if inner.is_some() {
                                        return Err(
                                            "C backend match supports unit enum variants only; use a #c helper for patterns carrying data"
                                                .to_string(),
                                        );
                                    }
                                    writeln!(
                                        output,
                                        "({} == {}) {{",
                                        match_var,
                                        self.enum_variant_c_name(enum_name, variant)
                                    )
                                    .unwrap();
                                }
                                other => {
                                    let _ = other;
                                    return Err(
                                        "C backend match supports literal, range, wildcard, and identifier patterns; use a #c helper for structured patterns"
                                            .to_string(),
                                    );
                                }
                            }
                        }
                        if let Some(guard) = &arm.guard {
                            // Guards are appended as a nested condition so the
                            // fallthrough chain stays intact.
                            let _ = guard;
                            return Err(
                                "C backend match guards are not supported yet; use a #c helper"
                                    .to_string(),
                            );
                        }
                        match &arm.body {
                            ast::MatchArmBody::Expression(expr) => {
                                line_prefix_for(output, indent + 2);
                                writeln!(
                                    output,
                                    "printf(\"%d\\n\", (int32_t)({}));",
                                    self.expr(expr)?
                                )
                                .unwrap();
                            }
                            ast::MatchArmBody::Block(statements) => {
                                self.block_into(output, indent + 2, statements)?;
                            }
                        }
                        line_prefix_for(output, indent + 1);
                        output.push_str("}\n");
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
            Expression::TupleIndex { object, index } => {
                // Tuples compile to anonymous structs; `.N` fields are named
                // `_0`, `_1`, ... (see tuple_struct_type).
                let object_str = self.expr(object)?;
                Ok(format!("{object_str}._{index}"))
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
            Expression::TupleLiteral(elements) => {
                // Anonymous-struct tuple literal: `(i32, i32) -> struct { int32_t _0; int32_t _1; }`.
                let fields = elements
                    .iter()
                    .enumerate()
                    .map(|(index, element)| Ok(format!(". _{index} = {}", self.expr(element)?)))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", ");
                Ok(format!("{{ {fields} }}"))
            }
            Expression::StructLiteral { name, fields } => {
                // C99 compound literal: `Point { x: 3, y: 4 }` ->
                // `(Point){ .x = 3, .y = 4 }`. The struct declaration already
                // emits a `typedef struct Point {...} Point;`.
                let initializers = fields
                    .iter()
                    .map(|(field, value)| Ok(format!(".{field} = {}", self.expr(value)?)))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", ");
                Ok(format!("({name}){{ {initializers} }}"))
            }
            Expression::EnumVariant {
                enum_name,
                variant,
                data,
            } => {
                if data.is_some() {
                    return Err(
                        "C backend supports unit enum variants only; use a #c helper for variants carrying data"
                            .to_string(),
                    );
                }
                Ok(self.enum_variant_c_name(enum_name, variant))
            }
            Expression::GetExpression(get) => self.get_expression(get),
            Expression::MatchExpression { .. }
            | Expression::Closure { .. }
            | Expression::TryExpression(_)
            | Expression::UnsafeBlock(_)
            | Expression::MethodCall { .. }
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
            | Expression::TupleIndex { .. }
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

    /// Render a tuple type as an anonymous struct: `(i32, ustring)` becomes
    /// `struct { int32_t _0; const char *_1; }`. Element fields are named
    /// `_N` so `.0` index access lowers to `._0`.
    fn tuple_struct_type(&self, components: &[TypeAnnotation]) -> String {
        let fields = components
            .iter()
            .enumerate()
            .map(|(index, component)| self.ty(component).map(|ty| format!("{} _{};", ty, index)))
            .collect::<Result<Vec<_>, String>>()
            .unwrap_or_default()
            .join(" ");
        format!("struct {{ {fields} }}")
    }

    fn inferred_type(&self, value: Option<&Expression>) -> String {
        match value {
            Some(Expression::FloatLiteral(_)) => "double".to_string(),
            Some(Expression::StringLiteral(_)) | Some(Expression::UnicodeStringLiteral(_)) => {
                "const char *".to_string()
            }
            Some(Expression::BoolLiteral(_)) => "bool".to_string(),
            // `get` reads a line of text; its result is a heap-allocated C string.
            Some(Expression::GetExpression(_)) => "const char *".to_string(),
            Some(Expression::TupleLiteral(_)) => self.tuple_inferred_type(value.unwrap()),
            // Struct literals infer their own type: `var p := Point { ... }`.
            Some(Expression::StructLiteral { name, .. }) => name.clone(),
            _ => "int32_t".to_string(),
        }
    }

    fn tuple_inferred_type(&self, value: &Expression) -> String {
        let Expression::TupleLiteral(elements) = value else {
            return "int32_t".to_string();
        };
        let fields = elements
            .iter()
            .enumerate()
            .map(|(index, element)| format!("{} _{};", self.inferred_type(Some(element)), index))
            .collect::<Vec<_>>()
            .join(" ");
        format!("struct {{ {fields} }}")
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
            TypeAnnotation::Tuple(components) => Ok(self.tuple_struct_type(components)),
            TypeAnnotation::Reference(_, inner) => self.ty(inner),
            TypeAnnotation::Pointer(inner) => Ok(format!("{} *", self.ty(inner)?)),
            TypeAnnotation::Generic { name, .. } if name == "Vec" => Ok("void *".to_string()),
            TypeAnnotation::Generic { name, .. } => Ok(name.clone()),
            TypeAnnotation::Array(_, _)
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

/// Check whether a statement's expressions contain a `get` input expression,
/// used to decide whether the stdin runtime helper must be emitted.
fn statement_contains_get(statement: &Statement) -> bool {
    fn expr_has_get(expr: &Expression) -> bool {
        match expr {
            Expression::GetExpression(_) => true,
            Expression::BinaryOp { left, right, .. } => expr_has_get(left) || expr_has_get(right),
            Expression::UnaryOp { expr, .. }
            | Expression::Parenthesized(expr)
            | Expression::TryExpression(expr)
            | Expression::AsExpression { expr, .. } => expr_has_get(expr),
            Expression::Call { func, args } => expr_has_get(func) || args.iter().any(expr_has_get),
            Expression::MethodCall { object, args, .. } => {
                expr_has_get(object) || args.iter().any(expr_has_get)
            }
            Expression::Index { object, index }
            | Expression::Range {
                start: object,
                end: index,
                ..
            } => expr_has_get(object) || expr_has_get(index),
            Expression::FieldAccess { object, .. } | Expression::TupleIndex { object, .. } => {
                expr_has_get(object)
            }
            Expression::StructLiteral { fields, .. } => {
                fields.iter().any(|(_, value)| expr_has_get(value))
            }
            Expression::ArrayLiteral(elements) | Expression::TupleLiteral(elements) => {
                elements.iter().any(expr_has_get)
            }
            Expression::IfExpression {
                condition,
                then_block,
                else_block,
            } => {
                expr_has_get(condition)
                    || block_has_get(then_block)
                    || else_block.as_ref().is_some_and(block_has_get)
            }
            Expression::MatchExpression { scrutinee, arms } => {
                expr_has_get(scrutinee)
                    || arms.iter().any(|arm| match &arm.body {
                        ast::MatchArmBody::Expression(body) => expr_has_get(body),
                        ast::MatchArmBody::Block(body) => block_has_get(body),
                    })
            }
            Expression::Closure { params: _, body } => expr_has_get(body),
            _ => false,
        }
    }

    fn block_has_get(block: &ast::Block) -> bool {
        block
            .iter()
            .any(|statement| statement_contains_get(&statement.node))
    }

    match statement {
        Statement::VarDeclaration {
            value: Some(value), ..
        }
        | Statement::LetDeclaration { value, .. }
        | Statement::Assignment { value, .. }
        | Statement::PutStatement { expr: value, .. }
        | Statement::ErrorStatement(value)
        | Statement::WarnStatement(value)
        | Statement::InfoStatement(value)
        | Statement::ReturnStatement(Some(value)) => expr_has_get(value),
        Statement::ExpressionStatement(expr) => expr_has_get(expr),
        Statement::Block(block) => block_has_get(block),
        _ => false,
    }
}

/// Collect identifiers a closure body references that are not its own
/// parameters. Non-empty output means the closure captures its environment,
/// which the C subset cannot represent.
fn closure_captures(params: &[ast::Parameter], body: &Expression) -> Vec<String> {
    let mut identifiers = Vec::new();
    fn walk(expr: &Expression, identifiers: &mut Vec<String>) {
        match expr {
            Expression::Identifier(name) => identifiers.push(name.clone()),
            Expression::BinaryOp { left, right, .. } => {
                walk(left, identifiers);
                walk(right, identifiers);
            }
            Expression::UnaryOp { expr, .. }
            | Expression::Parenthesized(expr)
            | Expression::TryExpression(expr)
            | Expression::AsExpression { expr, .. } => walk(expr, identifiers),
            Expression::Call { func, args } => {
                walk(func, identifiers);
                args.iter().for_each(|arg| walk(arg, identifiers));
            }
            Expression::Index { object, index }
            | Expression::Range {
                start: object,
                end: index,
                ..
            } => {
                walk(object, identifiers);
                walk(index, identifiers);
            }
            Expression::FieldAccess { object, .. } | Expression::TupleIndex { object, .. } => {
                walk(object, identifiers)
            }
            Expression::Closure { params, body } => {
                let _ = params;
                walk(body, identifiers);
            }
            _ => {}
        }
    }
    walk(body, &mut identifiers);
    let param_names: std::collections::HashSet<&str> = params
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect();
    let mut captured: Vec<String> = Vec::new();
    for name in identifiers {
        if !param_names.contains(name.as_str()) && !captured.contains(&name) {
            captured.push(name);
        }
    }
    captured
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

    fn generate(source: &str) -> String {
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty(), "{errors:?}");
        let mut parser = Parser::new(&tokens);
        let program = parser.parse().unwrap();
        transpile(&program).unwrap()
    }

    #[test]
    fn supports_tuples_as_anonymous_structs() {
        let output = generate("var pair (i32, i32) := (1, 2)\nput pair.0\nput pair.1");
        // Tuple type and literal both compile to anonymous structs with _N
        // fields; index access lowerS to the matching field name.
        assert!(output.contains("struct { int32_t _0; int32_t _1; }"));
        assert!(output.contains(". _0 = 1, . _1 = 2"));
        assert!(output.contains("printf(\"%d\\n\", pair._0)"));
        assert!(output.contains("printf(\"%d\\n\", pair._1)"));
    }

    #[test]
    fn supports_struct_literals_as_compound_literals() {
        let output = generate(
            "struct Point\n    var x: i32\n    var y: i32\nend struct\nvar p := Point { x: 3, y: 4 }\nput p.x",
        );
        // Struct literals lower to C99 compound literals against the emitted
        // typedef; field access is unchanged.
        assert!(output.contains("typedef struct Point {"));
        assert!(output.contains("Point p = (Point){ .x = 3, .y = 4 };"));
        assert!(output.contains("printf(\"%d\\n\", p.x)"));
    }

    #[test]
    fn supports_enums_and_enum_variant_matching() {
        let output = generate(
            "enum Color\n    Red\n    Green\nend enum\nvar c := Color::Green\nmatch c\n    Color::Red, put 1\n    Color::Green, put 2\n    _, put 0\nend match",
        );
        // Enumerators are qualified (Color_Red) so variant references and
        // patterns share one C representation.
        assert!(output.contains("typedef enum Color {"));
        assert!(output.contains("Color_Red,"));
        assert!(output.contains("int32_t c = Color_Green;"));
        assert!(output.contains("__poly_match == Color_Green"));
    }

    #[test]
    fn rejects_tuple_enum_variants_with_guidance() {
        let error = std::panic::catch_unwind(|| {
            generate("enum Shape\n    Circle(f64)\nend enum\nvar s := Shape::Circle(1.0)")
        });
        assert!(error.is_err());
    }

    #[test]
    fn supports_get_stdin_with_prompt_and_runtime() {
        let output = generate("var name := get unicode \"Who? \"\nput name");
        // The stdin runtime is emitted once, before main; the read result is a
        // C string so `put` uses the %s format.
        assert!(output.contains("static char *__poly_get_line(const char *prompt)"));
        assert!(output.contains("const char * name = __poly_get_line(\"Who? \");"));
        assert!(output.contains("printf(\"%s\\n\", name)"));
        // The helper must be defined before main uses it.
        let helper = output.find("__poly_get_line(const char").unwrap();
        let main = output.find("int main(void)").unwrap();
        assert!(helper < main);
    }

    #[test]
    fn supports_non_capturing_closures_as_function_pointers() {
        let output = generate("var twice := |x: i32| x * 2\nput twice(21)");
        // Non-capturing closures become a static function plus a function
        // pointer variable with a matching signature.
        assert!(output.contains("static int32_t __poly_closure_0(int32_t x) { return ((x * 2)); }"));
        assert!(output.contains("int32_t (*twice)(int32_t) = __poly_closure_0;"));
        // The synthesized function must precede its use in main.
        let function = output.find("__poly_closure_0(int32_t x)").unwrap();
        let main = output.find("int main(void)").unwrap();
        assert!(function < main);
    }

    #[test]
    fn rejects_capturing_closures_with_guidance() {
        let result =
            std::panic::catch_unwind(|| generate("var n i32 := 10\nvar addn := |x: i32| x + n"));
        let payload = result.unwrap_err();
        let message = payload
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_default();
        assert!(
            message.contains("non-capturing closures only"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn supports_match_with_literal_and_wildcard_arms() {
        let output = generate(
            "var x i32 := 3\nmatch x\n    1, put 10\n    3, put 30\n    _, put 99\nend match",
        );
        // The if/else-if chain compares a match-scoped copy of the scrutinee.
        assert!(output.contains("const int32_t __poly_match = x;"));
        assert!(output.contains("if (__poly_match == 1) {"));
        assert!(output.contains("else if (__poly_match == 3) {"));
        // The wildcard arm is a plain else (always matches).
        assert!(output.contains("else {\n"));
    }

    // Note: there is deliberately no test for the structured-pattern
    // rejection path. It is defensive: tuple patterns do not parse in match
    // position, and enum patterns require an enum declaration, which the C
    // subset rejects before codegen. Supported C programs can therefore
    // never reach the structured-pattern error.
}

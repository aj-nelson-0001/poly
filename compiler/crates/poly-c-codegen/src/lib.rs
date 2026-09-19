//! Poly to C code generation.
//!
//! The C backend intentionally targets the small orchestration subset of Poly.
//! Complex declarations stay in `#c` blocks and are emitted verbatim.
//!
//! ## String support
//!
//! Strings are `const char *` values (NUL-terminated). Two runtime helpers
//! are emitted into the output on demand (definitions double as prototypes,
//! inserted after the include block):
//!
//! - `poly_concat(a, b)` — value-position `+` on strings; exact-size
//!   `malloc` + `memcpy`. `put` never calls it: printf parts flatten concat
//!   chains into format pieces (see `printf_parts`), so printing allocates
//!   nothing.
//! - `poly_int_to_string(long long)` / `poly_float_to_string(double)` —
//!   back `n.to_string()` / `f.to_string()`. Typed by design: an earlier
//!   `void *` + `snprintf` sketch printed the pointer, not the pointee.
//! - `poly_bool_str(int)` — bool rendering as `"true"`/`"false"` (put and
//!   `to_string()`), matching the Rust target's Display output; a bare
//!   `%d` printed 1/0 instead. Backed by tracked bool variables, bool
//!   function return types, and comparison/logical expressions.
//!
//! `.len()` on strings is `strlen` with an `int32_t` cast. Results print
//! through `%d` — `printf_parts` defaults unknown compound leaves to `%d`
//! because passing an `int` to `%s` segfaults printf. `%s` requires
//! `is_string_valued`, which recognizes literals, string variables, concat
//! chains, `to_string()` calls, and parentheses around any of them.
//! Helpers and concat results are never freed (leak-until-exit model).

use std::cell::{Cell, RefCell};
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
    /// Interior-mutable flag: `expr(&self)` requests the helper, but the
    /// definition is written into the final output by `generate` (which has
    /// `&mut self`) after body rendering completes.
    concat_helper_emitted: Cell<bool>,
    to_string_helper_emitted: Cell<bool>,
    /// Declared function name -> declared C return type. Collected from both
    /// program-scope and struct/impl methods so `var q := make(3, 4)` can
    /// infer a struct type from a call instead of defaulting to `int32_t`.
    fn_return_types: std::collections::HashMap<String, String>,
    /// Declared/inferred 64-bit variables, consulted by printf_parts so
    /// int64_t values print with %lld instead of truncating through %d.
    var_widths: RefCell<std::collections::HashMap<String, String>>,
    /// Variables declared/inferred f64; printf_parts renders these with %f
    /// instead of %d (a double passed to %d is undefined behavior and
    /// prints garbage).
    float_variables: RefCell<std::collections::HashSet<String>>,
    /// Variables declared/inferred `bool`; printf_parts renders these with
    /// poly_bool_str ("true"/"false") instead of %d (1/0), matching the
    /// Rust target's Display rendering.
    bool_variables: RefCell<std::collections::HashSet<String>>,
    /// Functions declared to return bool (from the pre-pass), for call-result
    /// bool classification in printf_parts.
    bool_functions: RefCell<std::collections::HashSet<String>>,
    bool_str_emitted: Cell<bool>,
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
            concat_helper_emitted: Cell::new(false),
            to_string_helper_emitted: Cell::new(false),
            fn_return_types: std::collections::HashMap::new(),
            var_widths: RefCell::new(std::collections::HashMap::new()),
            float_variables: RefCell::new(std::collections::HashSet::new()),
            bool_variables: RefCell::new(std::collections::HashSet::new()),
            bool_functions: RefCell::new(std::collections::HashSet::new()),
            bool_str_emitted: Cell::new(false),
        }
    }

    fn generate(&mut self, program: &ast::Program) -> Result<String, String> {
        // Partition first because C declarations must precede main, while Poly
        // executable statements are intentionally retained in source order.
        // First partition declarations from orchestration statements so C items
        // are emitted at file scope and the generated entry point stays valid.

        // Pre-collect declared return types so call-result initializers infer
        // the callee's struct type (`var q := make(3, 4)` -> `Point q`). This
        // runs before rendering because function bodies reference it.
        for statement in &program.statements {
            match &statement.node {
                Statement::FunctionDeclaration(function) => {
                    if let Some(return_type) = &function.return_type {
                        let rendered = self.ty(return_type)?;
                        if rendered == "bool" {
                            self.bool_functions
                                .borrow_mut()
                                .insert(function.name.clone());
                        }
                        self.fn_return_types.insert(function.name.clone(), rendered);
                    }
                }
                Statement::StructDeclaration(decl) => {
                    for method in &decl.methods {
                        if let Some(return_type) = &method.return_type {
                            let rendered = self.ty(return_type)?;
                            self.fn_return_types.insert(method.name.clone(), rendered);
                        }
                    }
                }
                Statement::ImplDeclaration(decl) => {
                    for method in &decl.methods {
                        if let Some(return_type) = &method.return_type {
                            let rendered = self.ty(return_type)?;
                            self.fn_return_types.insert(method.name.clone(), rendered);
                        }
                    }
                }
                _ => {}
            }
        }
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
                // Cargo owns external crates; `dep` declarations are consumed
                // by project generation, not C emission.
                Statement::DependencyDeclaration(_) => {}
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

        self.insert_concat_helper();
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
                // Track 64-bit variables: printf of an int64_t needs %lld
                // (an %d/%lld mismatch is undefined behavior and truncates).
                if type_name == "int64_t" || type_name == "uint64_t" {
                    self.var_widths
                        .borrow_mut()
                        .insert(name.clone(), type_name.clone());
                }
                // Track f64 variables: printf of a double needs %f (a double
                // passed to %d is undefined behavior and prints garbage).
                if type_name == "double" {
                    self.float_variables.borrow_mut().insert(name.clone());
                }
                if type_name == "bool" {
                    self.bool_variables.borrow_mut().insert(name.clone());
                }
                if ty.as_ref().is_some_and(|ty| self.is_string_type(ty))
                    || value.as_ref().is_some_and(|value| {
                        self.is_string_valued(value)
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
                    || self.is_string_valued(value)
                {
                    self.string_variables.borrow_mut().insert(name.clone());
                }
                if type_name == "double" {
                    self.float_variables.borrow_mut().insert(name.clone());
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
                Expression::WhileLoop { condition, body } => {
                    line_prefix(output);
                    writeln!(output, "while {} {{", self.condition_expr(condition)?).unwrap();
                    self.block_into(output, indent + 1, body)?;
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
                    // A literal step's sign is known at compile time, so a
                    // plain `for` with the pre-flipped comparison is exact. A
                    // non-literal step may be negative at runtime, and since
                    // Poly ranges are inclusive its sign also decides which
                    // bound terminates the loop — the comparison direction is
                    // chosen per iteration. A zero step yields zero
                    // iterations (rather than hanging) to keep the loop total.
                    let step_is_literal = step.as_ref().is_some_and(|value| match value {
                        Expression::IntLiteral(_) => true,
                        Expression::UnaryOp {
                            op: UnaryOp::Neg,
                            expr,
                        } => matches!(expr.as_ref(), Expression::IntLiteral(_)),
                        _ => false,
                    });
                    if step.is_none() || step_is_literal {
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
                    } else {
                        let cond_up = if *inclusive { "<=" } else { "<" };
                        let cond_down = if *inclusive { ">=" } else { ">" };
                        let end_value = self.expr(end)?;
                        line_prefix(output);
                        output.push_str("{\n");
                        line_prefix_for(output, indent + 1);
                        writeln!(output, "int64_t __poly_step = (int64_t)({});", step_value)
                            .unwrap();
                        line_prefix_for(output, indent + 1);
                        writeln!(
                            output,
                            "for (int32_t {} = {}; __poly_step > 0 ? {} {} {} : (__poly_step < 0 && {} {} {}); {} += (int32_t)__poly_step) {{",
                            variable,
                            self.expr(start)?,
                            variable,
                            cond_up,
                            end_value,
                            variable,
                            cond_down,
                            end_value,
                            variable
                        )
                        .unwrap();
                        self.block_into(output, indent + 2, body)?;
                        line_prefix_for(output, indent + 1);
                        output.push_str("}\n");
                        line_prefix(output);
                        output.push_str("}\n");
                    }
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
            | Statement::DependencyDeclaration(_)
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
                // is_string_valued (not is_string_expression) so chains like
                // `x + n.to_string()` where the left spine bottoms out in a
                // to_string() call still classify as concat. With the plain
                // literal/var check, `+` between two string-valued subchains
                // emitted C `+` (pointer arithmetic) instead of poly_concat.
                if matches!(op, BinaryOp::Add)
                    && (self.is_string_valued(left) || self.is_string_valued(right))
                {
                    // Value-position string concatenation. `put` flattens
                    // chains into printf pieces instead (see printf_parts);
                    // everything else — assignments, returns, call args —
                    // materializes the result with poly_concat, which
                    // allocates `strlen(a) + strlen(b) + 1` bytes exactly
                    // once. Chains parse left-associative, so `(s+a)+b`
                    // becomes poly_concat(poly_concat(s,a),b): O(n^2) in the
                    // worst case like Rust, but correct. The result type is
                    // `const char *`; the C backend never frees, matching
                    // its existing leak-until-exit model.
                    self.ensure_concat_helper();
                    return Ok(format!(
                        "poly_concat({}, {})",
                        self.expr(left)?,
                        self.expr(right)?
                    ));
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
            Expression::MatchExpression { scrutinee, arms } => {
                // Match in value position lowers to a ternary chain over the
                // same patterns the statement form accepts (literal, range,
                // wildcard, unit enum variants). The scrutinee is emitted
                // once per comparison — keep it a simple expression (a
                // literal, identifier, or field access); side-effecting
                // scrutinees need a statement-local binding first. A wildcard
                // arm is required so the chain always yields a value.
                let chain = self.match_expr_chain(scrutinee, arms)?;
                Ok(chain)
            }
            Expression::MethodCall {
                object,
                method,
                args,
            } => {
                // The C backend supports the small method set the generated
                // code needs; everything else stays a hard error so callers
                // know to use a #c helper instead of silently miscompiling.
                match method.as_str() {
                    "len" if args.is_empty() => {
                        // String length: bytes before the NUL terminator,
                        // matching how this backend stores strings (and how
                        // the Rust target counts `String::len()`). Cast
                        // through the target's int type used by printf.
                        if self.is_string_valued(object) {
                            let obj = self.expr(object)?;
                            return Ok(format!("((int32_t)strlen({obj}))"));
                        }
                        Err(
                            "only string `.len()` is supported by the C backend; vectors need a #c helper".to_string(),
                        )
                    }
                    "to_string" if args.is_empty() => {
                        // Typed scalar helpers: the value is passed by copy so
                        // no address-of games (a void* + snprintf("%lld")
                        // design printed the pointer, not the pointee).
                        // Bools render "true"/"false" via poly_bool_str
                        // (matching the Rust target), ints %lld, floats %g,
                        // and string-typed objects are already `const char *`
                        // — their to_string() is the identity. Results are
                        // heap-allocated like poly_concat output; the C
                        // backend never frees (leak-until-exit model).
                        let obj = self.expr(object)?;
                        if self.is_string_expression(object) {
                            return Ok(obj);
                        }
                        if self.is_bool_valued(object) {
                            // poly_bool_str yields a "true"/"false" literal,
                            // which is a valid string value as-is.
                            self.ensure_bool_str_helper();
                            return Ok(format!("poly_bool_str({obj})"));
                        }
                        self.ensure_to_string_helper();
                        if self.format_for_expression(object) == "%f" {
                            return Ok(format!("poly_float_to_string((double)({obj}))"));
                        }
                        Ok(format!("poly_int_to_string((long long)({obj}))"))
                    }
                    _ => Err(
                        "this expression is not supported by the C backend; use a #c helper"
                            .to_string(),
                    ),
                }
            }
            Expression::Closure { .. }
            | Expression::TryExpression(_)
            | Expression::UnsafeBlock(_)
            | Expression::WhileLoop { .. }
            | Expression::ArrayLiteral(_) => Err(
                "this expression is not supported by the C backend; use a #c helper".to_string(),
            ),
        }
    }

    /// Build a ternary chain for a match expression: arm patterns become
    /// comparisons against the scrutinee text and arm bodies become the
    /// selected values. Supports exactly the pattern set of the statement
    /// form: literal, range, wildcard, and unit enum variants.
    fn match_expr_chain(
        &self,
        scrutinee: &Expression,
        arms: &[ast::MatchArm],
    ) -> Result<String, String> {
        let scrutinee_str = self.expr(scrutinee)?;
        let mut parts: Vec<String> = Vec::new();
        let mut saw_wildcard = false;
        for (index, arm) in arms.iter().enumerate() {
            if arm.guard.is_some() {
                return Err(
                    "C backend match guards are not supported yet; use a #c helper".to_string(),
                );
            }
            let condition = if matches!(arm.pattern, ast::Pattern::Wildcard) {
                // A wildcard always matches, so it can only close the chain.
                saw_wildcard = true;
                if index != arms.len() - 1 {
                    return Err(
                        "C backend match expressions require the wildcard arm to be last in value position"
                            .to_string(),
                    );
                }
                String::new()
            } else {
                match &arm.pattern {
                    ast::Pattern::Literal(literal) => {
                        format!("({} == {}) ? ", scrutinee_str, self.expr(literal)?)
                    }
                    ast::Pattern::Range {
                        start,
                        end,
                        inclusive,
                    } => {
                        let lower = if *inclusive { ">=" } else { ">" };
                        let upper = if *inclusive { "<=" } else { "<" };
                        format!(
                            "({scrutinee_str} {lower} {} && {scrutinee_str} {upper} {}) ? ",
                            self.expr(start)?,
                            self.expr(end)?
                        )
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
                        format!(
                            "({} == {}) ? ",
                            scrutinee_str,
                            self.enum_variant_c_name(enum_name, variant)
                        )
                    }
                    other => {
                        let _ = other;
                        return Err(
                            "C backend match expressions support literal, range, wildcard, and unit-enum patterns in value position; use a #c helper for others"
                                .to_string(),
                        );
                    }
                }
            };
            let value = match &arm.body {
                ast::MatchArmBody::Expression(body) => self.expr(body)?,
                ast::MatchArmBody::Block(_) => {
                    return Err(
                        "C backend match expressions support expression arms only in value position; use a match statement or a #c helper for block arms"
                            .to_string(),
                    );
                }
            };
            parts.push(format!("{condition}{value}"));
        }
        if parts.is_empty() {
            return Err("match expression has no arms".to_string());
        }
        let mut out = String::from("(");
        out.push_str(&parts.join(" : "));
        if !saw_wildcard {
            // Without a wildcard the chain has no value for an unmatched
            // scrutinee; mirror the Rust target's panic-on-unmatched
            // semantics with a runtime abort.
            out.push_str(" : (exit(1), 0)");
        }
        out.push(')');
        Ok(out)
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
            } if self.is_string_valued(left) || self.is_string_valued(right) => {
                let (left_format, left_args) = self.printf_parts(left)?;
                let (right_format, right_args) = self.printf_parts(right)?;
                Ok((
                    format!("{}{}", left_format, right_format),
                    format!("{}{}", left_args, right_args),
                ))
            }
            // Bool-valued expressions render "true"/"false" through the
            // poly_bool_str helper, matching the Rust target's Display
            // output (a bare %d prints 1/0 instead).
            Expression::BinaryOp { .. } if self.is_bool_valued(expression) => {
                self.ensure_bool_str_helper();
                Ok((
                    "%s".to_string(),
                    format!(", {}", self.bool_str_expr(expression)?),
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
                } else if self.is_bool_valued(expression) {
                    self.ensure_bool_str_helper();
                    "%s".to_string()
                } else if let Expression::Identifier(name) = expression {
                    // A 64-bit variable must print %lld: %d reads an int,
                    // truncating and (varargs UB) misreading the argument.
                    match self.var_widths.borrow().get(name).map(String::as_str) {
                        Some("int64_t") => "%lld".to_string(),
                        Some("uint64_t") => "%llu".to_string(),
                        _ => self.format_for_expression(expression),
                    }
                } else {
                    self.format_for_expression(expression)
                };
                if self.is_bool_valued(expression) {
                    return Ok((format, format!(", {}", self.bool_str_expr(expression)?)));
                }
                Ok((format, format!(", {}", self.expr(expression)?)))
            }
            // Compound string-valued leaves (`n.to_string()`, concat
            // chains) print with %s; everything else unknown (method calls
            // like `.len()`, `get` reads) falls through to the int format —
            // NOT %s, which would pass an int to a pointer conversion and
            // segfault.
            Expression::MethodCall { .. } => {
                let format = if self.is_string_valued(expression) {
                    "%s".to_string()
                } else {
                    "%d".to_string()
                };
                Ok((format, format!(", {}", self.expr(expression)?)))
            }
            Expression::GetExpression(_) | Expression::Parenthesized(_) => {
                let format = if self.is_string_valued(expression) {
                    "%s".to_string()
                } else if self.is_bool_valued(expression) {
                    self.ensure_bool_str_helper();
                    "%s".to_string()
                } else {
                    "%d".to_string()
                };
                if !self.is_string_valued(expression) && self.is_bool_valued(expression) {
                    return Ok((format, format!(", {}", self.bool_str_expr(expression)?)));
                }
                Ok((format, format!(", {}", self.expr(expression)?)))
            }
            _ => Ok(("%d".to_string(), format!(", {}", self.expr(expression)?))),
        }
    }

    /// Classify printf-position expressions that carry a bool value. Covers
    /// literals, tracked bool variables, comparisons/logical combinators,
    /// `!x`, and calls to functions declared `fn ..(): bool`.
    fn is_bool_valued(&self, expression: &Expression) -> bool {
        match expression {
            Expression::BoolLiteral(_) => true,
            Expression::Parenthesized(inner) => self.is_bool_valued(inner),
            Expression::Identifier(name) => self.bool_variables.borrow().contains(name),
            Expression::UnaryOp {
                op: UnaryOp::Not, ..
            } => true,
            Expression::BinaryOp {
                op:
                    BinaryOp::Eq
                    | BinaryOp::NotEq
                    | BinaryOp::Lt
                    | BinaryOp::Gt
                    | BinaryOp::LtEq
                    | BinaryOp::GtEq
                    | BinaryOp::And
                    | BinaryOp::Or,
                ..
            } => true,
            Expression::Call { func, .. } => match func.as_ref() {
                Expression::Identifier(name) => self.bool_functions.borrow().contains(name),
                _ => false,
            },
            _ => false,
        }
    }

    /// Render a printf-position bool argument: the expression wrapped in the
    /// poly_bool_str helper (`(... ? "true" : "false")`), which must be
    /// emitted first via ensure_bool_str_helper.
    fn bool_str_expr(&self, expression: &Expression) -> Result<String, String> {
        Ok(format!("poly_bool_str({})", self.expr(expression)?))
    }

    /// Request the poly_bool_str helper (emitted by insert_concat_helper).
    fn ensure_bool_str_helper(&self) {
        self.bool_str_emitted.set(true);
    }

    fn is_string_type(&self, annotation: &TypeAnnotation) -> bool {
        matches!(annotation, TypeAnnotation::Named(name) if name == "string" || name == "ustring")
    }

    /// Emit the `poly_concat` helper once. It is declared before main and
    /// defined after it so every call site sees a prototype.
    fn ensure_concat_helper(&self) {
        self.concat_helper_emitted.set(true);
    }

    /// Request the `poly_to_string` helper (see `insert_concat_helper` for
    /// how the definition reaches the output). `value` points at the scalar
    /// to format; `format` is a printf specifier without conversion flags.
    fn ensure_to_string_helper(&self) {
        self.to_string_helper_emitted.set(true);
    }

    /// Insert the `poly_concat` definition into the finished output when any
    /// value-position concatenation was rendered. Called from `generate`
    /// after all expression rendering, positioned immediately after the
    /// include block so the definition doubles as the prototype for every
    /// call site in user functions and main.
    /// Insert requested runtime helpers into the finished output, directly
    /// after the include block (before every user function and main, so the
    /// definitions double as prototypes for all call sites).
    fn insert_concat_helper(&mut self) {
        if !(self.concat_helper_emitted.get()
            || self.to_string_helper_emitted.get()
            || self.bool_str_emitted.get())
        {
            return;
        }
        let mut helpers = String::new();
        if self.bool_str_emitted.get() {
            // bool -> "true"/"false", matching the Rust target's Display
            // rendering of bools so all targets print the same text.
            helpers.push_str(
                "const char *poly_bool_str(int value) {\n    return value ? \"true\" : \"false\";\n}\n\n",
            );
        }
        if self.to_string_helper_emitted.get() {
            helpers.push_str(
                "char *poly_int_to_string(long long value) {\n    char *out = malloc(32);\n    if (out == NULL) {\n        fputs(\"poly_to_string: out of memory\\n\", stderr);\n        exit(1);\n    }\n    snprintf(out, 32, \"%lld\", value);\n    return out;\n}\n\nchar *poly_float_to_string(double value) {\n    char *out = malloc(64);\n    if (out == NULL) {\n        fputs(\"poly_to_string: out of memory\\n\", stderr);\n        exit(1);\n    }\n    snprintf(out, 64, \"%g\", value);\n    return out;\n}\n\n",
            );
        }
        if self.concat_helper_emitted.get() {
            helpers.push_str(
                "char *poly_concat(const char *a, const char *b) {\n    if (a == NULL) a = \"\";\n    if (b == NULL) b = \"\";\n    size_t len_a = strlen(a);\n    size_t len_b = strlen(b);\n    char *out = malloc(len_a + len_b + 1);\n    if (out == NULL) {\n        fputs(\"poly_concat: out of memory\\n\", stderr);\n        exit(1);\n    }\n    memcpy(out, a, len_a);\n    memcpy(out + len_a, b, len_b);\n    out[len_a + len_b] = '\\0';\n    return out;\n}\n\n",
            );
        }
        const MARKER: &str = "#include <string.h>\n\n";
        if let Some(pos) = self.output.find(MARKER) {
            let insert_at = pos + MARKER.len();
            self.output.insert_str(insert_at, &helpers);
        } else {
            // Includes are always emitted before any user code; fall back to
            // the top just in case that invariant ever changes.
            self.output.insert_str(0, &helpers);
        }
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

    /// Whether an expression evaluates to a string value at runtime. Extends
    /// `is_string_expression` with the string-valued compound forms: concat
    /// chains (either side string-valued) and `to_string()` method calls.
    /// Used by printf flattening and type inference; the plain literal/var
    /// check is deliberately kept separate for contexts that must not treat
    /// compound expressions as strings.
    fn is_string_valued(&self, expression: &Expression) -> bool {
        match expression {
            // Parentheses never change a value's type (`(s + "!").len()`
            // reaches here with a Parenthesized object).
            Expression::Parenthesized(inner) => self.is_string_valued(inner),
            Expression::BinaryOp {
                op: BinaryOp::Add,
                left,
                right,
            } => self.is_string_valued(left) || self.is_string_valued(right),
            Expression::MethodCall { method, .. } if method == "to_string" => true,
            // A call to a function whose declared return type is string-
            // valued (`fn build(..): ustring`) produces a string. Checked
            // through fn_return_types, which the pre-pass populated from
            // declared return types before body rendering began.
            Expression::Call { func, .. } => match func.as_ref() {
                Expression::Identifier(name) => self
                    .fn_return_types
                    .get(name)
                    .is_some_and(|ty| ty == "const char *"),
                _ => false,
            },
            _ => self.is_string_expression(expression),
        }
    }

    fn format_for_expression(&self, expression: &Expression) -> String {
        if self.is_float_valued(expression) {
            return "%f".to_string();
        }
        match expression {
            Expression::BoolLiteral(_) => "%d".to_string(),
            Expression::UnicodeCharLiteral(_) => "%c".to_string(),
            _ => "%d".to_string(),
        }
    }

    /// Classify printf-position expressions that carry a floating-point
    /// value: f64 literals, tracked f64 variables, arithmetic over float
    /// operands, and `as` casts to a float type. A double passed to %d is
    /// undefined behavior, so anything float-valued must print via %f.
    fn is_float_valued(&self, expression: &Expression) -> bool {
        match expression {
            Expression::FloatLiteral(_) => true,
            Expression::Parenthesized(inner) => self.is_float_valued(inner),
            Expression::Identifier(name) => self.float_variables.borrow().contains(name),
            Expression::UnaryOp { expr, .. } => self.is_float_valued(expr),
            Expression::BinaryOp { left, right, .. } => {
                self.is_float_valued(left) || self.is_float_valued(right)
            }
            Expression::AsExpression { ty, .. } => matches!(
                ty.as_ref(),
                TypeAnnotation::Named(name) if name == "f64" || name == "f32"
            ),
            _ => false,
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
            // String-typed expressions infer `const char *` so `var s :=
            // a + b` (concat) and `var t := n.to_string()` declare pointers,
            // not integers. Matches the literal cases above. For a concat
            // chain the left spine is checked recursively because
            // is_string_expression only recognizes literals and known
            // string variables — `n.to_string()` on the left is not one.
            Some(Expression::BinaryOp {
                op: BinaryOp::Add,
                left,
                right,
            }) => {
                if self.is_string_valued(left) || self.is_string_valued(right) {
                    "const char *".to_string()
                } else {
                    "int32_t".to_string()
                }
            }
            Some(Expression::MethodCall { method, .. }) if method == "to_string" => {
                "const char *".to_string()
            }
            // A call infers the callee's declared return type so struct-returning
            // functions declare struct-typed variables instead of `int32_t`.
            Some(Expression::Call { func, .. }) => match func.as_ref() {
                Expression::Identifier(name) => self
                    .fn_return_types
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| "int32_t".to_string()),
                _ => "int32_t".to_string(),
            },
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
    fn float_variables_print_with_f() {
        // Regression: `put f` on an f64 variable emitted printf("%d\n", f);
        // a double passed to %d is undefined behavior and printed garbage.
        let output = generate("fn main()\nvar f f64 := 0.1 + 0.2\nput f\nend fn");
        assert!(
            output.contains("printf(\"%f\\n\", f);"),
            "expected %f for f64 variable: {output}"
        );
    }

    #[test]
    fn float_arithmetic_prints_with_f() {
        let output = generate("fn main()\nvar g f64 := 2.5\nput g * 2.0\nend fn");
        assert!(
            output.contains("printf(\"%f\\n\""),
            "expected %f for float-valued arithmetic: {output}"
        );
    }

    #[test]
    fn descending_range_loop_negates_step() {
        // Regression: the C backend emitted `i -= -((-2))`; keep verifying
        // the comparison flips and the step carries its sign.
        let output = generate("fn main()\nloop i 10..1 step -2\nput i\nend loop\nend fn");
        assert!(output.contains("for (int32_t i = 10; i >= 1; i -= -((-2))) {"));
    }

    #[test]
    fn runtime_step_uses_sign_dispatch() {
        // Regression: a non-literal step used to keep the statically-chosen
        // comparison, so a runtime-negative step yielded zero iterations.
        let output =
            generate("fn main()\nvar s i32 := -1\nloop i 10..1 step s\nput i\nend loop\nend fn");
        assert!(
            output.contains("int64_t __poly_step = (int64_t)(s);"),
            "{output}"
        );
        assert!(
            output.contains("__poly_step > 0 ? i <= 1 : (__poly_step < 0 && i >= 1)"),
            "{output}"
        );
        assert!(output.contains("i += (int32_t)__poly_step) {"), "{output}");
    }

    #[test]
    fn negated_variable_step_goes_through_runtime_dispatch() {
        // `-(s)` is not a literal: its sign depends on `s` at runtime, so it
        // must not take the static descending path.
        let output =
            generate("fn main()\nvar s i32 := -2\nloop i 10..1 step -(s)\nput i\nend loop\nend fn");
        assert!(
            output.contains("int64_t __poly_step = (int64_t)"),
            "{output}"
        );
    }

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
    fn value_position_string_concat_uses_helper() {
        // Regression: value-position concat used to error with "requires a
        // #c helper". Now it lowers to the emitted poly_concat runtime
        // helper (put position still flattens to printf pieces).
        let output = generate(
            "var s ustring := \"ab\"\nvar t ustring := s + \"cd\" + \"ef\"\nput t\nput \"x\" + s + \"!\"",
        );
        assert!(
            output.contains("poly_concat("),
            "expected poly_concat call sites: {output}"
        );
        assert!(
            output.contains("char *poly_concat(const char *a, const char *b)"),
            "expected emitted helper definition: {output}"
        );
        // The definition must appear before the first call site so it also
        // serves as the prototype.
        let def_pos = output
            .find("char *poly_concat(const char *a, const char *b) {")
            .expect("helper definition");
        let call_pos = output.find("poly_concat(s").expect("call site");
        assert!(
            def_pos < call_pos,
            "helper definition must precede call sites"
        );
    }

    #[test]
    fn string_len_uses_strlen() {
        // `.len()` on strings lowers to strlen with an int32_t cast; the
        // result must print through %d, never %s (an int passed to %s is
        // a printf segfault).
        let output =
            generate("var s ustring := \"hello\"\nput s.len()\nvar t := (s + \"!!\").len()\nput t");
        assert!(output.contains("((int32_t)strlen(s))"), "{output}");
        assert!(
            output.contains("strlen((poly_concat(s, \"!!\")))"),
            "{output}"
        );
        assert!(
            !output.contains("printf(\"%s\\n\", ((int32_t)strlen"),
            "strlen results must print as integers: {output}"
        );
    }

    #[test]
    fn to_string_and_mixed_concat_infer_and_print() {
        // Regression: `var chain := "a" + n.to_string() + "b"` used to infer
        // int32_t (printing a pointer through %d) and put of the variable
        // printed %d for the same reason. Both paths now classify
        // to_string()-containing chains as string-valued.
        let output = generate(
            "var n i32 := 42\nvar chain := \"a\" + n.to_string() + \"b\"\nput chain\nvar pure ustring := \"ab\" + \"cd\"\nput pure",
        );
        assert!(
            output.contains("const char * chain = poly_concat"),
            "chain must infer as const char *: {output}"
        );
        assert!(
            output.contains("poly_int_to_string"),
            "expected poly_int_to_string call: {output}"
        );
        assert!(
            output.contains("printf(\"%s\\n\", chain)"),
            "chain put must use %s: {output}"
        );
        assert!(
            output.contains("poly_concat(\"ab\", \"cd\")"),
            "literal concat still materializes: {output}"
        );
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

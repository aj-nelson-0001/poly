//! Rust code generator for the Poly language.

use std::fmt::Write;

use poly_lexer::Lexer;
use poly_parser::ast::*;
use poly_parser::Parser;

use crate::source_map::SourceMap;

/// The Poly-to-Rust transpiler.
pub struct Transpiler {
    #[allow(dead_code)]
    indent: usize,
    #[allow(dead_code)]
    output: String,
    /// Source map for debugging
    source_map: Option<SourceMap>,
}

impl Transpiler {
    /// Create a new transpiler instance.
    pub fn new() -> Self {
        Self {
            indent: 0,
            output: String::new(),
            source_map: None,
        }
    }

    /// Create a new transpiler with source map generation enabled.
    pub fn with_source_map() -> Self {
        Self {
            indent: 0,
            output: String::new(),
            source_map: Some(SourceMap::new("", "")),
        }
    }

    /// Transpile Poly source code to Rust code.
    pub fn transpile(&self, source: &str) -> Result<String, String> {
        let (tokens, errors) = Lexer::lex(source);
        if !errors.is_empty() {
            return Err(format!("Lexer errors: {:?}", errors));
        }

        let mut parser = Parser::new(&tokens);
        let program = parser.parse().map_err(|e| e.to_string())?;

        let mut gen = CodeGen::new();
        Ok(gen.generate(&program))
    }

    /// Transpile Poly source code to Rust code with source map.
    pub fn transpile_with_source_map(&mut self, source: &str) -> Result<(String, SourceMap), String> {
        let (tokens, errors) = Lexer::lex(source);
        if !errors.is_empty() {
            return Err(format!("Lexer errors: {:?}", errors));
        }

        let mut parser = Parser::new(&tokens);
        let program = parser.parse().map_err(|e| e.to_string())?;

        let mut gen = CodeGen::new();
        let rust_code = gen.generate(&program);

        // Create source map
        let mut source_map = SourceMap::new(source, &rust_code);
        
        // Add basic line mappings
        for (i, _) in rust_code.lines().enumerate() {
            source_map.add_mapping(i + 1, 1); // Simplified mapping
        }

        Ok((rust_code, source_map))
    }

    /// Get the source map from the last transpilation.
    pub fn source_map(&self) -> Option<&SourceMap> {
        self.source_map.as_ref()
    }
}

impl Default for Transpiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Code generator that walks the AST and produces Rust code.
struct CodeGen {
    indent: usize,
    output: String,
    /// Track what imports are needed
    needs_io_write: bool,
}

impl CodeGen {
    fn new() -> Self {
        Self {
            indent: 0,
            output: String::new(),
            needs_io_write: false,
        }
    }

    fn indent_str(&self) -> String {
        "    ".repeat(self.indent)
    }

    fn generate(&mut self, program: &Program) -> String {
        // First pass: collect top-level declarations and separate from main body
        let mut top_level = Vec::new();
        let mut main_body = Vec::new();

        for stmt in &program.statements {
            match stmt {
                Statement::FunctionDeclaration(_)
                | Statement::StructDeclaration(_)
                | Statement::EnumDeclaration(_)
                | Statement::TraitDeclaration(_)
                | Statement::ImplDeclaration(_)
                | Statement::TypeDeclaration(_) => {
                    top_level.push(stmt);
                }
                _ => {
                    main_body.push(stmt);
                }
            }
        }

        // Generate header with imports
        self.writeln("// Generated from Poly source code");
        self.writeln("#![allow(unused_variables, unused_mut, unused_imports, dead_code)]");
        self.writeln("");

        // Generate top-level declarations
        for stmt in &top_level {
            self.gen_statement(stmt);
            self.writeln("");
        }

        // Check if there are any async functions or await expressions
        let has_async = program.statements.iter().any(|stmt| {
            matches!(stmt, Statement::FunctionDeclaration(f) if f.is_async)
        });

        // Generate main function with remaining statements
        if !main_body.is_empty() {
            if has_async {
                self.writeln("#[tokio::main]");
            }
            self.writeln("fn main() {");
            self.indent += 1;
            for stmt in &main_body {
                self.gen_statement(stmt);
            }
            self.indent -= 1;
            self.writeln("}");
        }

        // Add needed imports at the top
        let mut result = String::new();
        if self.needs_io_write {
            result.push_str("use std::io::Write;\n");
        }
        result.push_str(&self.output);
        result
    }

    fn writeln(&mut self, s: &str) {
        let indent = self.indent_str();
        writeln!(self.output, "{}{}", indent, s).unwrap();
    }

    #[allow(dead_code)]
    fn write(&mut self, s: &str) {
        write!(self.output, "{}", s).unwrap();
    }

    fn gen_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::VarDeclaration { name, ty, value } => {
                let ty_str = ty.as_ref().map(|t| self.gen_type(t)).unwrap_or_default();
                match value {
                    Some(val) => {
                        let val_str = self.gen_expression(val);
                        if ty.is_some() {
                            self.writeln(&format!("let mut {}: {} = {};", name, ty_str, val_str));
                        } else {
                            self.writeln(&format!("let mut {} = {};", name, val_str));
                        }
                    }
                    None => {
                        if ty.is_some() {
                            self.writeln(&format!("let mut {}: {};", name, ty_str));
                        } else {
                            self.writeln(&format!("let mut {};", name));
                        }
                    }
                }
            }
            Statement::LetDeclaration { name, ty, value } => {
                let ty_str = ty.as_ref().map(|t| self.gen_type(t)).unwrap_or_default();
                let val_str = self.gen_expression(value);
                if ty.is_some() {
                    self.writeln(&format!("let {}: {} = {};", name, ty_str, val_str));
                } else {
                    self.writeln(&format!("let {} = {};", name, val_str));
                }
            }
            Statement::ConstDeclaration { name, value } => {
                let val_str = self.gen_expression(value);
                self.writeln(&format!("const {}: _ = {};", name, val_str));
            }
            Statement::Assignment { target, op, value } => {
                let target_str = self.gen_expression(target);
                let value_str = self.gen_expression(value);
                let op_str = self.gen_assignment_op(op);
                self.writeln(&format!("{} {} {};", target_str, op_str, value_str));
            }
            Statement::FunctionDeclaration(decl) => {
                self.gen_function(decl, false);
            }
            Statement::StructDeclaration(decl) => {
                self.gen_struct(decl);
            }
            Statement::EnumDeclaration(decl) => {
                self.gen_enum(decl);
            }
            Statement::ReturnStatement(value) => match value {
                Some(expr) => {
                    let expr_str = self.gen_expression(expr);
                    self.writeln(&format!("return {};", expr_str));
                }
                None => {
                    self.writeln("return;");
                }
            },
            Statement::BreakStatement => {
                self.writeln("break;");
            }
            Statement::ContinueStatement => {
                self.writeln("continue;");
            }
            Statement::PutStatement {
                no_newline,
                expr,
                redirect,
            } => {
                self.gen_put_statement(*no_newline, expr, redirect);
            }
            Statement::ErrorStatement(expr) => {
                let expr_str = self.gen_expression(expr);
                self.writeln(&format!("eprintln!(\"[ERROR] {}\", {});", "{}", expr_str));
            }
            Statement::WarnStatement(expr) => {
                let expr_str = self.gen_expression(expr);
                self.writeln(&format!("eprintln!(\"[WARN] {}\", {});", "{}", expr_str));
            }
            Statement::InfoStatement(expr) => {
                let expr_str = self.gen_expression(expr);
                self.writeln(&format!("eprintln!(\"[INFO] {}\", {});", "{}", expr_str));
            }
            Statement::ExpressionStatement(expr) => {
                // Check for while loop pattern: IfExpression with None else_block
                // While loops are parsed as: IfExpression { ..., else_block: None }
                // If statements are parsed as: IfExpression { ..., else_block: Some([]) }
                if let Expression::IfExpression {
                    condition,
                    then_block,
                    else_block,
                } = expr
                {
                    if else_block.is_none() {
                        // This is a while loop
                        let cond = self.gen_expression(condition);
                        self.writeln(&format!("while {} {{", cond));
                        self.indent += 1;
                        for stmt in then_block {
                            self.gen_statement(stmt);
                        }
                        self.indent -= 1;
                        self.writeln("}");
                        return;
                    }
                }
                let expr_str = self.gen_expression(expr);
                // Only add semicolon for statements that need it
                match expr {
                    Expression::IfExpression { .. }
                    | Expression::LoopRange { .. }
                    | Expression::MatchExpression { .. } => {
                        self.writeln(&expr_str);
                    }
                    _ => {
                        self.writeln(&format!("{};", expr_str));
                    }
                }
            }
            Statement::ModuleDeclaration(decl) => {
                // Module declarations are not directly transpiled to Rust
                // They're used for organizing code
                self.writeln(&format!("// module {}", decl.name));
                // Transpile module contents
                for stmt in &decl.statements {
                    self.gen_statement(stmt);
                }
            }
            Statement::UseDeclaration(decl) => {
                // Convert Poly use statements to Rust use statements
                let path = decl.path.join("::");
                if let Some(alias) = &decl.alias {
                    self.writeln(&format!("use {} as {};", path, alias));
                } else {
                    self.writeln(&format!("use {};", path));
                }
            }
            Statement::TraitDeclaration(decl) => {
                self.gen_trait(decl);
            }
            Statement::ImplDeclaration(decl) => {
                self.gen_impl(decl);
            }
            Statement::TypeDeclaration(decl) => {
                let ty = self.gen_type(&decl.ty);
                self.writeln(&format!("type {} = {};", decl.name, ty));
            }
            _ => {
                self.writeln("/* unimplemented statement */");
            }
        }
    }

    fn gen_function(&mut self, decl: &FunctionDecl, _is_method: bool) {
        let params: Vec<String> = decl
            .params
            .iter()
            .map(|p| {
                let ty = self.gen_type(&p.ty);
                format!("{}: {}", p.name, ty)
            })
            .collect();

        let ret = decl
            .return_type
            .as_ref()
            .map(|t| format!(" -> {}", self.gen_type(t)))
            .unwrap_or_default();

        let async_prefix = if decl.is_async { "async " } else { "" };
        self.writeln(&format!(
            "{}fn {}({}){} {{",
            async_prefix,
            decl.name,
            params.join(", "),
            ret
        ));
        self.indent += 1;

        if let Some(body) = &decl.body {
            for stmt in body {
                self.gen_statement(stmt);
            }
        }

        self.indent -= 1;
        self.writeln("}");
    }

    fn gen_struct(&mut self, decl: &StructDecl) {
        self.writeln(&format!("struct {} {{", decl.name));
        self.indent += 1;

        for field in &decl.fields {
            let ty = self.gen_type(&field.ty);
            self.writeln(&format!("{}: {},", field.name, ty));
        }

        self.indent -= 1;
        self.writeln("}");

        // Generate impl block for methods
        if !decl.methods.is_empty() {
            self.writeln(&format!("impl {} {{", decl.name));
            self.indent += 1;
            for method in &decl.methods {
                self.gen_function(method, true);
            }
            self.indent -= 1;
            self.writeln("}");
        }
    }

    fn gen_trait(&mut self, decl: &TraitDecl) {
        self.writeln(&format!("trait {} {{", decl.name));
        self.indent += 1;

        for method in &decl.methods {
            // Generate method signature without body
            let params: Vec<String> = method
                .params
                .iter()
                .map(|p| {
                    let ty = self.gen_type(&p.ty);
                    format!("{}: {}", p.name, ty)
                })
                .collect();

            let ret = method
                .return_type
                .as_ref()
                .map(|t| format!(" -> {}", self.gen_type(t)))
                .unwrap_or_default();

            self.writeln(&format!("fn {}({}){};", method.name, params.join(", "), ret));
        }

        self.indent -= 1;
        self.writeln("}");
    }

    fn gen_impl(&mut self, decl: &ImplDecl) {
        if let Some(trait_name) = &decl.trait_name {
            self.writeln(&format!("impl {} for {} {{", trait_name, decl.type_name));
        } else {
            self.writeln(&format!("impl {} {{", decl.type_name));
        }
        self.indent += 1;

        for method in &decl.methods {
            self.gen_function(method, true);
        }

        self.indent -= 1;
        self.writeln("}");
    }

    fn gen_enum(&mut self, decl: &EnumDecl) {
        self.writeln(&format!("enum {} {{", decl.name));
        self.indent += 1;

        for variant in &decl.variants {
            match variant {
                EnumVariant::Unit(name) => {
                    self.writeln(&format!("{},", name));
                }
                EnumVariant::Tuple(name, types) => {
                    let type_strs: Vec<String> = types.iter().map(|t| self.gen_type(t)).collect();
                    self.writeln(&format!("{}({}),", name, type_strs.join(", ")));
                }
                EnumVariant::Struct(name, fields) => {
                    self.writeln(&format!("{} {{", name));
                    self.indent += 1;
                    for (fname, fty) in fields {
                        let ty = self.gen_type(fty);
                        self.writeln(&format!("{}: {},", fname, ty));
                    }
                    self.indent -= 1;
                    self.writeln("},");
                }
            }
        }

        self.indent -= 1;
        self.writeln("}");

        // Generate impl block for methods
        if !decl.methods.is_empty() {
            self.writeln(&format!("impl {} {{", decl.name));
            self.indent += 1;
            for method in &decl.methods {
                self.gen_function(method, true);
            }
            self.indent -= 1;
            self.writeln("}");
        }
    }

    fn gen_put_statement(
        &mut self,
        no_newline: bool,
        expr: &Expression,
        redirect: &Option<Redirect>,
    ) {
        // Check if the expression itself contains a file write operation (e.g., `"content" > "file"`)
        if let Expression::BinaryOp { op, left, right } = expr {
            match op {
                BinaryOp::Gt => {
                    // Write: `put "content" > "file"` -> std::fs::write("file", format!("{}", "content"))
                    let content_str = self.gen_expression(left);
                    let path_str = self.gen_expression(right);
                    self.needs_io_write = true;
                    self.writeln(&format!(
                        "std::fs::write({}, format!(\"{{}}\", {})).unwrap();",
                        path_str, content_str
                    ));
                    return;
                }
                BinaryOp::Shr => {
                    // Append: `put "content" >> "file"` -> std::fs::OpenOptions...
                    let content_str = self.gen_expression(left);
                    let path_str = self.gen_expression(right);
                    self.needs_io_write = true;
                    self.writeln(&format!("{{ let mut f = std::fs::OpenOptions::new().append(true).create(true).open({}).unwrap(); writeln!(f, \"{{}}\", {}).unwrap(); }}", path_str, content_str));
                    return;
                }
                _ => {}
            }
        }

        let expr_str = self.gen_expression(expr);

        match redirect {
            Some(Redirect::Write(path)) => {
                let path_str = self.gen_expression(path);
                self.needs_io_write = true;
                if no_newline {
                    self.writeln(&format!(
                        "std::fs::write({}, format!(\"{{}}\", {})).unwrap();",
                        path_str, expr_str
                    ));
                } else {
                    self.writeln(&format!(
                        "std::fs::write({}, format!(\"{{}}\\n\", {})).unwrap();",
                        path_str, expr_str
                    ));
                }
            }
            Some(Redirect::Append(path)) => {
                let path_str = self.gen_expression(path);
                self.needs_io_write = true;
                self.writeln(&format!("{{ let mut f = std::fs::OpenOptions::new().append(true).create(true).open({}).unwrap(); writeln!(f, \"{{}}\", {}).unwrap(); }}", path_str, expr_str));
            }
            None => {
                if no_newline {
                    self.writeln(&format!("print!(\"{{}}\", {});", expr_str));
                } else {
                    self.writeln(&format!("println!(\"{{}}\", {});", expr_str));
                }
            }
        }
    }

    fn gen_expression(&self, expr: &Expression) -> String {
        match expr {
            Expression::IntLiteral(val) => val.clone(),
            Expression::FloatLiteral(val) => val.clone(),
            Expression::StringLiteral(val) => format!("\"{}\"", val),
            Expression::UnicodeStringLiteral(val) => format!("\"{}\"", val),
            Expression::BoolLiteral(val) => val.to_string(),
            Expression::ByteLiteral(bytes) => {
                let hex: Vec<String> = bytes.iter().map(|b| format!("0x{:02X}", b)).collect();
                format!("vec![{}]", hex.join(", "))
            }
            Expression::Identifier(name) => name.clone(),
            Expression::BinaryOp { op, left, right } => {
                let l = self.gen_expression(left);
                let r = self.gen_expression(right);
                let op_str = self.gen_binary_op(op);
                format!("({} {} {})", l, op_str, r)
            }
            Expression::UnaryOp { op, expr } => {
                let e = self.gen_expression(expr);
                let op_str = self.gen_unary_op(op);
                format!("({}{})", op_str, e)
            }
            Expression::Call { func, args } => {
                let func_str = self.gen_expression(func);
                let args_str: Vec<String> = args.iter().map(|a| self.gen_expression(a)).collect();
                // Special handling for built-in functions
                if func_str == "open" && args.len() == 1 {
                    format!("std::fs::File::open({}).unwrap()", args_str.join(", "))
                } else {
                    format!("{}({})", func_str, args_str.join(", "))
                }
            }
            Expression::MethodCall {
                object,
                method,
                args,
            } => {
                // Check for await first (special case)
                if method == "await" {
                    let obj_str = self.gen_expression(object);
                    return format!("{}.await", obj_str);
                }
                
                let obj_str = self.gen_expression(object);
                let args_str: Vec<String> = args.iter().map(|a| self.gen_expression(a)).collect();
                // Special handling for file methods
                match method.as_str() {
                    "eof" => {
                        // file.eof() -> use a placeholder that compiles
                        format!("false /* eof check - needs BufReader implementation */")
                    }
                    "get_line" => {
                        // file.get_line() -> use a placeholder that compiles
                        format!("String::new() /* get_line - needs BufReader implementation */")
                    }
                    "split" => {
                        let result = format!("{}.{}({})", obj_str, method, args_str.join(", "));
                        format!("{}.collect::<Vec<_>>()", result)
                    }
                    _ => {
                        format!("{}.{}({})", obj_str, method, args_str.join(", "))
                    }
                }
            }
            Expression::Index { object, index } => {
                let obj_str = self.gen_expression(object);
                let idx_str = self.gen_expression(index);
                format!("{}[{}]", obj_str, idx_str)
            }
            Expression::FieldAccess { object, field } => {
                let obj_str = self.gen_expression(object);
                format!("{}.{}", obj_str, field)
            }
            Expression::Parenthesized(expr) => {
                format!("({})", self.gen_expression(expr))
            }
            Expression::IfExpression {
                condition,
                then_block,
                else_block,
            } => {
                let cond = self.gen_expression(condition);
                let mut result = format!("if {} {{\n", cond);
                if !then_block.is_empty() {
                    for stmt in then_block {
                        result.push_str(&format!("    {}\n", self.gen_statement_str(stmt)));
                    }
                }
                if let Some(else_stmts) = else_block {
                    if !else_stmts.is_empty() {
                        // Check if this is an else-if chain
                        if else_stmts.len() == 1 {
                            if let Statement::ExpressionStatement(inner_expr) = &else_stmts[0] {
                                if let Expression::IfExpression { condition: inner_cond, then_block: inner_then, else_block: inner_else } = inner_expr {
                                    // This is an else-if: generate } else if ... {
                                    let cond_str = self.gen_expression(inner_cond);
                                    result.push_str(&format!("}} else if {} {{\n", cond_str));
                                    for stmt in inner_then {
                                        result.push_str(&format!("    {}\n", self.gen_statement_str(stmt)));
                                    }
                                    // Handle nested else-if or else recursively
                                    if let Some(nested_else) = inner_else {
                                        if !nested_else.is_empty() {
                                            // Recursively generate the else part
                                            let else_result = self.gen_else_chain(nested_else);
                                            result.push_str(&else_result);
                                        } else {
                                            result.push_str("}\n");
                                        }
                                    } else {
                                        result.push_str("}\n");
                                    }
                                    return result;
                                }
                            }
                        }
                        result.push_str("} else {\n");
                        for stmt in else_stmts {
                            result.push_str(&format!("    {}\n", self.gen_statement_str(stmt)));
                        }
                        result.push_str("}\n");
                    } else {
                        result.push_str("}\n");
                    }
                } else {
                    result.push_str("}\n");
                }
                result
            }
            Expression::ArrayLiteral(elements) => {
                let elems: Vec<String> = elements.iter().map(|e| self.gen_expression(e)).collect();
                format!("vec![{}]", elems.join(", "))
            }
            Expression::TupleLiteral(elements) => {
                let elems: Vec<String> = elements.iter().map(|e| self.gen_expression(e)).collect();
                format!("({})", elems.join(", "))
            }
            Expression::StructLiteral { name, fields } => {
                let field_strs: Vec<String> = fields
                    .iter()
                    .map(|(n, v)| format!("{}: {}", n, self.gen_expression(v)))
                    .collect();
                format!("{} {{ {} }}", name, field_strs.join(", "))
            }
            Expression::EnumVariant {
                enum_name,
                variant,
                data,
            } => match data {
                Some(args) => {
                    let args_str: Vec<String> =
                        args.iter().map(|a| self.gen_expression(a)).collect();
                    format!("{}::{}({})", enum_name, variant, args_str.join(", "))
                }
                None => format!("{}::{}", enum_name, variant),
            },
            Expression::Range {
                start,
                end,
                inclusive,
            } => {
                let s = self.gen_expression(start);
                let e = self.gen_expression(end);
                if *inclusive {
                    format!("{}..={}", s, e)
                } else {
                    format!("{}..{}", s, e)
                }
            }
            Expression::LoopRange { ranges, body } => {
                let mut result = String::new();
                // Generate for loop(s) for the range parts
                if ranges.len() == 1 {
                    match &ranges[0] {
                        LoopRangePart::Range {
                            start,
                            end,
                            inclusive,
                            step,
                        } => {
                            let s = self.gen_expression(start);
                            let e = self.gen_expression(end);
                            let range = if *inclusive {
                                format!("{}..={}", s, e)
                            } else {
                                format!("{}..{}", s, e)
                            };
                            let step_str = step
                                .as_ref()
                                .map(|s| format!(".step_by({})", self.gen_expression(s)))
                                .unwrap_or_default();
                            result.push_str(&format!("for i in {}{} {{\n", range, step_str));
                        }
                        LoopRangePart::Value(val) => {
                            let v = self.gen_expression(val);
                            // Check if this is a variable (identifier) for collection iteration
                            if let Expression::Identifier(name) = val.as_ref() {
                                result.push_str(&format!(
                                    "for {} in {} {{\n",
                                    name.trim_end_matches('s'),
                                    v
                                ));
                            } else {
                                result.push_str(&format!("for i in [{}] {{\n", v));
                            }
                        }
                    }
                } else {
                    // Multiple ranges
                    let mut range_strs = Vec::new();
                    for part in ranges {
                        match part {
                            LoopRangePart::Range {
                                start,
                                end,
                                inclusive,
                                ..
                            } => {
                                let s = self.gen_expression(start);
                                let e = self.gen_expression(end);
                                let range = if *inclusive {
                                    format!("{}..={}", s, e)
                                } else {
                                    format!("{}..{}", s, e)
                                };
                                range_strs.push(range);
                            }
                            LoopRangePart::Value(val) => {
                                let v = self.gen_expression(val);
                                range_strs.push(v);
                            }
                        }
                    }
                    // Chain ranges using flat_map or chain
                    result.push_str(&format!(
                        "for i in [{}].iter().flat_map(|r| r.clone()) {{\n",
                        range_strs.join(", ")
                    ));
                }
                // Generate body
                for stmt in body {
                    result.push_str(&format!("    {}\n", self.gen_statement_str(stmt)));
                }
                result.push('}');
                result
            }
            Expression::AsExpression { expr, ty } => {
                let e = self.gen_expression(expr);
                let t = self.gen_type(ty);
                format!("{} as {}", e, t)
            }
            Expression::TryExpression(expr) => {
                let e = self.gen_expression(expr);
                format!("{}?", e)
            }
            Expression::GetExpression(get_expr) => self.gen_get_expression(get_expr),
            Expression::MatchExpression { scrutinee, arms } => {
                let scrutinee_str = self.gen_expression(scrutinee);
                let mut result = format!("match {} {{\n", scrutinee_str);
                for arm in arms {
                    let pattern_str = self.gen_pattern(&arm.pattern);
                    let body_str = match &arm.body {
                        MatchArmBody::Expression(expr) => self.gen_expression(expr),
                        MatchArmBody::Block(stmts) => {
                            let mut block = String::new();
                            for stmt in stmts {
                                block.push_str(&format!("    {}\n", self.gen_statement_str(stmt)));
                            }
                            block.trim().to_string()
                        }
                    };
                    // Remove trailing semicolon if present (match arms don't need semicolons)
                    let body_str = body_str.trim_end_matches(';').to_string();
                    if let Some(guard) = &arm.guard {
                        let guard_str = self.gen_expression(guard);
                        result.push_str(&format!(
                            "    {} if {} => {},\n",
                            pattern_str, guard_str, body_str
                        ));
                    } else {
                        result.push_str(&format!("    {} => {},\n", pattern_str, body_str));
                    }
                }
                result.push('}');
                result
            }
            Expression::Closure { params, body } => {
                let params_str: Vec<String> = params
                    .iter()
                    .map(|p| {
                        let ty = self.gen_type(&p.ty);
                        if ty == "_" {
                            p.name.clone()
                        } else {
                            format!("{}: {}", p.name, ty)
                        }
                    })
                    .collect();
                let body_str = self.gen_expression(body);
                format!("|{}| {}", params_str.join(", "), body_str)
            }
            Expression::UnsafeBlock(stmts) => {
                let mut result = "unsafe {\n".to_string();
                for stmt in stmts {
                    result.push_str(&format!("    {}\n", self.gen_statement_str(stmt)));
                }
                result.push('}');
                result
            }
        }
    }

    fn gen_statement_str(&self, stmt: &Statement) -> String {
        match stmt {
            Statement::ExpressionStatement(expr) => self.gen_expression(expr),
            Statement::ReturnStatement(Some(expr)) => {
                format!("return {};", self.gen_expression(expr))
            }
            Statement::ReturnStatement(None) => "return;".to_string(),
            Statement::BreakStatement => "break;".to_string(),
            Statement::ContinueStatement => "continue;".to_string(),
            Statement::PutStatement {
                no_newline, expr, ..
            } => {
                let expr_str = self.gen_expression(expr);
                if *no_newline {
                    format!("print!(\"{{}}\", {});", expr_str)
                } else {
                    format!("println!(\"{{}}\", {});", expr_str)
                }
            }
            Statement::VarDeclaration { name, ty, value } => {
                let ty_str = ty.as_ref().map(|t| self.gen_type(t)).unwrap_or_default();
                match value {
                    Some(val) => {
                        let val_str = self.gen_expression(val);
                        if ty.is_some() {
                            format!("let mut {}: {} = {};", name, ty_str, val_str)
                        } else {
                            format!("let mut {} = {};", name, val_str)
                        }
                    }
                    None => {
                        if ty.is_some() {
                            format!("let mut {}: {};", name, ty_str)
                        } else {
                            format!("let mut {};", name)
                        }
                    }
                }
            }
            Statement::Assignment { target, op, value } => {
                let target_str = self.gen_expression(target);
                let value_str = self.gen_expression(value);
                let op_str = self.gen_assignment_op(op);
                format!("{} {} {};", target_str, op_str, value_str)
            }
            _ => "/* statement */".to_string(),
        }
    }

    fn gen_type(&self, ty: &TypeAnnotation) -> String {
        match ty {
            TypeAnnotation::Named(name) => {
                // Map Poly types to Rust types
                match name.as_str() {
                    "ustring" | "string" => "String".to_string(),
                    "uchar" => "char".to_string(),
                    "byte" => "u8".to_string(),
                    "bytes" => "Vec<u8>".to_string(),
                    _ => name.clone(),
                }
            }
            TypeAnnotation::Array(inner, size) => {
                let inner_str = self.gen_type(inner);
                let size_str = self.gen_expression(size);
                format!("[{}; {}]", inner_str, size_str)
            }
            TypeAnnotation::Tuple(types) => {
                let type_strs: Vec<String> = types.iter().map(|t| self.gen_type(t)).collect();
                format!("({})", type_strs.join(", "))
            }
            TypeAnnotation::Vec(inner) => {
                format!("Vec<{}>", self.gen_type(inner))
            }
            TypeAnnotation::Option(inner) => {
                format!("Option<{}>", self.gen_type(inner))
            }
            TypeAnnotation::Result(ok, err) => {
                format!("Result<{}, {}>", self.gen_type(ok), self.gen_type(err))
            }
            TypeAnnotation::Reference(mutable, inner) => {
                if *mutable {
                    format!("&mut {}", self.gen_type(inner))
                } else {
                    format!("&{}", self.gen_type(inner))
                }
            }
            TypeAnnotation::Pointer(inner) => {
                format!("*const {}", self.gen_type(inner))
            }
            TypeAnnotation::Nullable(inner) => {
                format!("Option<{}>", self.gen_type(inner))
            }
            TypeAnnotation::Function { params, ret } => {
                let param_strs: Vec<String> = params.iter().map(|t| self.gen_type(t)).collect();
                format!("fn({}) -> {}", param_strs.join(", "), self.gen_type(ret))
            }
        }
    }

    fn gen_binary_op(&self, op: &BinaryOp) -> String {
        match op {
            BinaryOp::Add => "+".to_string(),
            BinaryOp::Sub => "-".to_string(),
            BinaryOp::Mul => "*".to_string(),
            BinaryOp::Div => "/".to_string(),
            BinaryOp::Mod => "%".to_string(),
            BinaryOp::Eq => "==".to_string(),
            BinaryOp::NotEq => "!=".to_string(),
            BinaryOp::Lt => "<".to_string(),
            BinaryOp::Gt => ">".to_string(),
            BinaryOp::LtEq => "<=".to_string(),
            BinaryOp::GtEq => ">=".to_string(),
            BinaryOp::And => "&&".to_string(),
            BinaryOp::Or => "||".to_string(),
            BinaryOp::BitAnd => "&".to_string(),
            BinaryOp::BitOr => "|".to_string(),
            BinaryOp::BitXor => "^".to_string(),
            BinaryOp::Shl => "<<".to_string(),
            BinaryOp::Shr => ">>".to_string(),
        }
    }

    fn gen_unary_op(&self, op: &UnaryOp) -> String {
        match op {
            UnaryOp::Neg => "-".to_string(),
            UnaryOp::Not => "!".to_string(),
            UnaryOp::BitNot => "!".to_string(),
            UnaryOp::Deref => "*".to_string(),
        }
    }

    fn gen_assignment_op(&self, op: &AssignmentOp) -> String {
        match op {
            AssignmentOp::Eq => "=".to_string(),
            AssignmentOp::PlusEq => "+=".to_string(),
            AssignmentOp::MinusEq => "-=".to_string(),
            AssignmentOp::StarEq => "*=".to_string(),
            AssignmentOp::SlashEq => "/=".to_string(),
            AssignmentOp::PercentEq => "%=".to_string(),
            AssignmentOp::AmpEq => "&=".to_string(),
            AssignmentOp::PipeEq => "|=".to_string(),
            AssignmentOp::CaretEq => "^=".to_string(),
            AssignmentOp::LtLtEq => "<<=".to_string(),
            AssignmentOp::GtGtEq => ">>=".to_string(),
        }
    }

    fn gen_pattern(&self, pattern: &Pattern) -> String {
        match pattern {
            Pattern::Wildcard => "_".to_string(),
            Pattern::Literal(expr) => self.gen_expression(expr),
            Pattern::Identifier(name) => name.clone(),
            Pattern::Tuple(patterns) => {
                let pats: Vec<String> = patterns.iter().map(|p| self.gen_pattern(p)).collect();
                format!("({})", pats.join(", "))
            }
            Pattern::Enum {
                enum_name,
                variant,
                inner,
            } => match inner {
                Some(args) => {
                    let pats: Vec<String> = args.iter().map(|p| self.gen_pattern(p)).collect();
                    format!("{}::{}({})", enum_name, variant, pats.join(", "))
                }
                None => format!("{}::{}", enum_name, variant),
            },
            Pattern::Range {
                start,
                end,
                inclusive,
            } => {
                let s = self.gen_expression(start);
                let e = self.gen_expression(end);
                if *inclusive {
                    format!("{}..={}", s, e)
                } else {
                    format!("{}..{}", s, e)
                }
            }
            Pattern::Binding { name, pattern } => {
                format!("{} @ {}", name, self.gen_pattern(pattern))
            }
            Pattern::NamedFields { name, fields } => {
                let field_strs: Vec<String> = fields
                    .iter()
                    .map(|(n, p)| {
                        match p {
                            Pattern::Identifier(id) if id == n => n.clone(), // shorthand
                            _ => format!("{}: {}", n, self.gen_pattern(p)),
                        }
                    })
                    .collect();
                format!("{} {{ {} }}", name, field_strs.join(", "))
            }
        }
    }

    /// Generate the else/else-if chain for an if expression
    fn gen_else_chain(&self, else_stmts: &[Statement]) -> String {
        if else_stmts.is_empty() {
            return "}\n".to_string();
        }
        
        if else_stmts.len() == 1 {
            if let Statement::ExpressionStatement(inner_expr) = &else_stmts[0] {
                if let Expression::IfExpression { condition, then_block, else_block } = inner_expr {
                    // This is an else-if: generate } else if ... {
                    let cond_str = self.gen_expression(condition);
                    let mut result = format!("}} else if {} {{\n", cond_str);
                    for stmt in then_block {
                        result.push_str(&format!("    {}\n", self.gen_statement_str(stmt)));
                    }
                    // Handle nested else-if or else
                    if let Some(nested_else) = else_block {
                        if !nested_else.is_empty() {
                            let else_result = self.gen_else_chain(nested_else);
                            result.push_str(&else_result);
                        } else {
                            result.push_str("}\n");
                        }
                    } else {
                        result.push_str("}\n");
                    }
                    return result;
                }
            }
        }
        
        // Regular else block
        let mut result = "} else {\n".to_string();
        for stmt in else_stmts {
            result.push_str(&format!("    {}\n", self.gen_statement_str(stmt)));
        }
        result.push_str("}\n");
        result
    }

    fn gen_get_expression(&self, get_expr: &GetExpr) -> String {
        // Check if we have a source (file redirection)
        if let Some(source) = &get_expr.source {
            let path_str = self.gen_expression(source);

            // Check for --bytes flag
            for flag in &get_expr.flags {
                match flag {
                    GetFlag::Bytes(count) => {
                        let count_str = self.gen_expression(count);
                        return format!(
                            "{{ let bytes = std::fs::read({}).unwrap(); bytes[..{}].to_vec() }}",
                            path_str, count_str
                        );
                    }
                    _ => {}
                }
            }

            // Default: read entire file as string
            return format!("std::fs::read_to_string({}).unwrap()", path_str);
        }

        // No source: read from stdin
        "{ let mut input = String::new(); std::io::stdin().read_line(&mut input).unwrap(); input.trim().to_string() }".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transpile_var_declaration() {
        let t = Transpiler::new();
        let result = t.transpile("var x: i32 = 42").unwrap();
        assert!(result.contains("let mut x: i32 = 42;"));
    }

    #[test]
    fn test_transpile_put_statement() {
        let t = Transpiler::new();
        let result = t.transpile(r#"put "Hello""#).unwrap();
        assert!(result.contains(r#"println!("{}", "Hello");"#));
    }

    #[test]
    fn test_transpile_put_no_newline() {
        let t = Transpiler::new();
        let result = t.transpile(r#"put -n "Hello""#).unwrap();
        assert!(result.contains(r#"print!("{}", "Hello");"#));
    }

    #[test]
    fn test_transpile_function() {
        let t = Transpiler::new();
        let result = t
            .transpile("fn sum(a: i32, b: i32): i32\n    return a + b\nend fn")
            .unwrap();
        assert!(result.contains("fn sum(a: i32, b: i32) -> i32"));
        assert!(result.contains("return (a + b);"));
    }

    #[test]
    fn test_transpile_binary_expression() {
        let t = Transpiler::new();
        let result = t.transpile("var x = a + b * 2").unwrap();
        assert!(result.contains("(a + (b * 2))"));
    }

    #[test]
    fn test_transpile_struct() {
        let t = Transpiler::new();
        let result = t
            .transpile("struct Point\n    var x: f32\n    var y: f32\nend struct")
            .unwrap();
        assert!(result.contains("struct Point"));
        assert!(result.contains("x: f32,"));
        assert!(result.contains("y: f32,"));
    }

    #[test]
    fn test_transpile_enum() {
        let t = Transpiler::new();
        let result = t
            .transpile("enum Direction\n    North\n    South\nend enum")
            .unwrap();
        assert!(result.contains("enum Direction"));
        assert!(result.contains("North,"));
        assert!(result.contains("South,"));
    }

    #[test]
    fn test_transpile_comparison() {
        let t = Transpiler::new();
        let result = t.transpile("var x = a > 0").unwrap();
        assert!(result.contains("(a > 0)"));
    }

    #[test]
    fn test_transpile_method_call() {
        let t = Transpiler::new();
        let result = t.transpile("obj.method(arg)").unwrap();
        assert!(result.contains(".method(arg)"));
    }

    #[test]
    fn test_transpile_array_literal() {
        let t = Transpiler::new();
        let result = t.transpile("var arr = [1, 2, 3]").unwrap();
        assert!(result.contains("vec![1, 2, 3]"));
    }

    #[test]
    fn test_transpile_error_statement() {
        let t = Transpiler::new();
        let result = t.transpile(r#"error "something failed""#).unwrap();
        assert!(result.contains("eprintln!"));
        assert!(result.contains("ERROR"));
    }

    #[test]
    fn test_transpile_top_level_functions() {
        let t = Transpiler::new();
        let result = t
            .transpile("fn sum(a: i32, b: i32): i32\n    return a + b\nend fn\n\nvar x = sum(1, 2)")
            .unwrap();
        // Function should be at top level, not inside main
        assert!(result.contains("fn sum(a: i32, b: i32) -> i32"));
        assert!(result.contains("fn main()"));
        // x should be inside main
        assert!(result.contains("let mut x = sum(1, 2);"));
    }

    #[test]
    fn test_transpile_get_from_file() {
        let t = Transpiler::new();
        let result = t.transpile(r#"var content = get < "test.txt""#).unwrap();
        assert!(result.contains("std::fs::read_to_string"));
    }

    #[test]
    fn test_transpile_get_bytes() {
        let t = Transpiler::new();
        let result = t
            .transpile(r#"var data = get < "test.bin" --bytes 10"#)
            .unwrap();
        assert!(result.contains("std::fs::read"));
        assert!(result.contains("10"));
    }
}

#[test]
fn test_transpile_file_write() {
    let t = Transpiler::new();
    let result = t.transpile(r#"put "Hello" > "test.txt""#).unwrap();
    assert!(result.contains("std::fs::write"));
    assert!(result.contains("test.txt"));
}

#[test]
fn test_transpile_file_append() {
    let t = Transpiler::new();
    let result = t.transpile(r#"put "Hello" >> "test.txt""#).unwrap();
    assert!(result.contains("OpenOptions"));
    assert!(result.contains("append(true)"));
}

#[test]
fn test_transpile_type_mapping() {
    let t = Transpiler::new();
    let result = t.transpile("var x: ustring = \"hello\"").unwrap();
    assert!(result.contains("let mut x: String"));
}

#[test]
fn test_transpile_bytes_type() {
    let t = Transpiler::new();
    let result = t.transpile("var data: bytes = []").unwrap();
    assert!(result.contains("let mut data: Vec<u8>"));
}

#[test]
fn test_transpile_split_with_collect() {
    let t = Transpiler::new();
    let result = t
        .transpile(
            r#"var lines = text.split("
")"#,
        )
        .unwrap();
    assert!(result.contains("split"));
    assert!(result.contains("collect"));
}

// =============================================================================
// Trait and Impl Tests
// =============================================================================

#[test]
fn test_transpile_trait_declaration() {
    let t = Transpiler::new();
    let result = t
        .transpile("trait Drawable\n    fn draw(self)\n    fn area(self): f32\nend trait")
        .unwrap();
    assert!(result.contains("trait Drawable"));
    assert!(result.contains("fn draw(self: Self);"));
    assert!(result.contains("fn area(self: Self) -> f32;"));
}

#[test]
fn test_transpile_impl_declaration() {
    let t = Transpiler::new();
    let result = t
        .transpile("impl Circle\n    fn new(x: f32, y: f32): Circle\n        return Circle { x: x, y: y }\n    end fn\nend impl")
        .unwrap();
    assert!(result.contains("impl Circle"));
    assert!(result.contains("fn new(x: f32, y: f32) -> Circle"));
}

#[test]
fn test_transpile_trait_impl() {
    let t = Transpiler::new();
    let result = t
        .transpile(r#"impl Drawable for Circle
    fn draw(self)
        put "Drawing circle"
    end fn
end impl"#)
        .unwrap();
    assert!(result.contains("impl Drawable for Circle"));
    assert!(result.contains("fn draw(self: Self)"));
}

// =============================================================================
// Async/Await Tests
// =============================================================================

#[test]
fn test_transpile_async_function() {
    let t = Transpiler::new();
    let result = t
        .transpile("async fn fetch_data(url: ustring): ustring\n    return u\"data\"\nend fn")
        .unwrap();
    assert!(result.contains("async fn fetch_data(url: String) -> String"));
}

#[test]
fn test_transpile_async_main() {
    let t = Transpiler::new();
    let result = t
        .transpile("async fn main_task()\n    put u\"Hello\"\nend fn\n\nvar x = 1")
        .unwrap();
    assert!(result.contains("#[tokio::main]"));
    assert!(result.contains("async fn main_task()"));
}

#[test]
fn test_transpile_await_expression() {
    let t = Transpiler::new();
    let result = t.transpile("var x = fetch_data().await").unwrap();
    assert!(result.contains("fetch_data().await"));
}

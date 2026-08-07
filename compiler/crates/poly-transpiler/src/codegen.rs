//! Rust code generator for the Poly language.

use std::fmt::Write;

use poly_parser::ast::*;
use poly_lexer::Lexer;
use poly_parser::Parser;

/// The Poly-to-Rust transpiler.
pub struct Transpiler {
    indent: usize,
    output: String,
}

impl Transpiler {
    /// Create a new transpiler instance.
    pub fn new() -> Self {
        Self {
            indent: 0,
            output: String::new(),
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
}

impl CodeGen {
    fn new() -> Self {
        Self {
            indent: 0,
            output: String::new(),
        }
    }

    fn indent_str(&self) -> String {
        "    ".repeat(self.indent)
    }

    fn generate(&mut self, program: &Program) -> String {
        self.writeln("// Generated from Poly source code");
        self.writeln("#![allow(unused_variables, unused_mut, unused_imports)]");
        self.writeln("");
        self.writeln("fn main() {");

        self.indent += 1;
        for stmt in &program.statements {
            self.gen_statement(stmt);
        }
        self.indent -= 1;

        self.writeln("}");
        self.output.clone()
    }

    fn writeln(&mut self, s: &str) {
        let indent = self.indent_str();
        writeln!(self.output, "{}{}", indent, s).unwrap();
    }

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
            Statement::ReturnStatement(value) => {
                match value {
                    Some(expr) => {
                        let expr_str = self.gen_expression(expr);
                        self.writeln(&format!("return {};", expr_str));
                    }
                    None => {
                        self.writeln("return;");
                    }
                }
            }
            Statement::BreakStatement => {
                self.writeln("break;");
            }
            Statement::ContinueStatement => {
                self.writeln("continue;");
            }
            Statement::PutStatement { no_newline, expr, redirect } => {
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
                let expr_str = self.gen_expression(expr);
                self.writeln(&format!("{};", expr_str));
            }
            _ => {
                self.writeln("// TODO: unimplemented statement");
            }
        }
    }

    fn gen_function(&mut self, decl: &FunctionDecl, is_method: bool) {
        let params: Vec<String> = decl.params.iter().map(|p| {
            let ty = self.gen_type(&p.ty);
            format!("{}: {}", p.name, ty)
        }).collect();

        let ret = decl.return_type.as_ref()
            .map(|t| format!(" -> {}", self.gen_type(t)))
            .unwrap_or_default();

        self.writeln(&format!("fn {}({}){} {{", decl.name, params.join(", "), ret));
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
            if field.mutable {
                self.writeln(&format!("{}: {},", field.name, ty));
            } else {
                self.writeln(&format!("{}: {},", field.name, ty));
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

    fn gen_put_statement(&mut self, no_newline: bool, expr: &Expression, redirect: &Option<Redirect>) {
        let expr_str = self.gen_expression(expr);

        match redirect {
            Some(Redirect::Write(path)) => {
                let path_str = self.gen_expression(path);
                if no_newline {
                    self.writeln(&format!("std::fs::write({}, format!(\"{{}}\", {})).unwrap();", path_str, expr_str));
                } else {
                    self.writeln(&format!("std::fs::write({}, format!(\"{{}}\\n\", {})).unwrap();", path_str, expr_str));
                }
            }
            Some(Redirect::Append(path)) => {
                let path_str = self.gen_expression(path);
                self.writeln(&format!("use std::io::Write; let mut f = std::fs::OpenOptions::new().append(true).open({}).unwrap(); writeln!(f, \"{{}}\", {}).unwrap();", path_str, expr_str));
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
                format!("{}({})", func_str, args_str.join(", "))
            }
            Expression::MethodCall { object, method, args } => {
                let obj_str = self.gen_expression(object);
                let args_str: Vec<String> = args.iter().map(|a| self.gen_expression(a)).collect();
                format!("{}.{}({})", obj_str, method, args_str.join(", "))
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
            Expression::IfExpression { condition, then_block, else_block } => {
                let cond = self.gen_expression(condition);
                let mut result = format!("if {} {{\n", cond);
                for stmt in then_block {
                    result.push_str(&format!("    {}\n", self.gen_statement_str(stmt)));
                }
                if let Some(else_stmts) = else_block {
                    result.push_str("} else {\n");
                    for stmt in else_stmts {
                        result.push_str(&format!("    {}\n", self.gen_statement_str(stmt)));
                    }
                }
                result.push('}');
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
                let field_strs: Vec<String> = fields.iter().map(|(n, v)| {
                    format!("{}: {}", n, self.gen_expression(v))
                }).collect();
                format!("{} {{ {} }}", name, field_strs.join(", "))
            }
            Expression::EnumVariant { enum_name, variant, data } => {
                match data {
                    Some(args) => {
                        let args_str: Vec<String> = args.iter().map(|a| self.gen_expression(a)).collect();
                        format!("{}::{}({})", enum_name, variant, args_str.join(", "))
                    }
                    None => format!("{}::{}", enum_name, variant),
                }
            }
            Expression::Range { start, end, inclusive } => {
                let s = self.gen_expression(start);
                let e = self.gen_expression(end);
                if *inclusive {
                    format!("{}..={}", s, e)
                } else {
                    format!("{}..{}", s, e)
                }
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
            Expression::GetExpression(get_expr) => {
                self.gen_get_expression(get_expr)
            }
            Expression::MatchExpression { scrutinee, arms } => {
                let scrutinee_str = self.gen_expression(scrutinee);
                let mut result = format!("match {} {{\n", scrutinee_str);
                for arm in arms {
                    let pattern_str = self.gen_pattern(&arm.pattern);
                    let body_str = self.gen_expression(&arm.body);
                    if let Some(guard) = &arm.guard {
                        let guard_str = self.gen_expression(guard);
                        result.push_str(&format!("    {} if {} => {},\n", pattern_str, guard_str, body_str));
                    } else {
                        result.push_str(&format!("    {} => {},\n", pattern_str, body_str));
                    }
                }
                result.push('}');
                result
            }
            Expression::Closure { params, body } => {
                let params_str: Vec<String> = params.iter().map(|p| {
                    let ty = self.gen_type(&p.ty);
                    format!("{}: {}", p.name, ty)
                }).collect();
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
            _ => "/* unimplemented expression */".to_string(),
        }
    }

    fn gen_statement_str(&self, stmt: &Statement) -> String {
        // Simple statement to string conversion for inline use
        match stmt {
            Statement::ExpressionStatement(expr) => self.gen_expression(expr),
            Statement::ReturnStatement(Some(expr)) => format!("return {}", self.gen_expression(expr)),
            Statement::ReturnStatement(None) => "return".to_string(),
            Statement::PutStatement { no_newline, expr, .. } => {
                let expr_str = self.gen_expression(expr);
                if *no_newline {
                    format!("print!(\"{{}}\", {})", expr_str)
                } else {
                    format!("println!(\"{{}}\", {})", expr_str)
                }
            }
            _ => "/* statement */".to_string(),
        }
    }

    fn gen_type(&self, ty: &TypeAnnotation) -> String {
        match ty {
            TypeAnnotation::Named(name) => name.clone(),
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
            Pattern::Enum { enum_name, variant, inner } => {
                match inner {
                    Some(args) => {
                        let pats: Vec<String> = args.iter().map(|p| self.gen_pattern(p)).collect();
                        format!("{}::{}({})", enum_name, variant, pats.join(", "))
                    }
                    None => format!("{}::{}", enum_name, variant),
                }
            }
            Pattern::Range { start, end, inclusive } => {
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
        }
    }

    fn gen_get_expression(&self, get_expr: &GetExpr) -> String {
        let mut result = String::new();

        // For now, generate a simple stdin read
        result.push_str("{\n");
        result.push_str("    let mut input = String::new();\n");
        result.push_str("    std::io::stdin().read_line(&mut input).unwrap();\n");
        result.push_str("    input.trim().to_string()\n");
        result.push('}');

        result
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
        let result = t.transpile("fn add(a: i32, b: i32): i32\n    return a + b\nend fn").unwrap();
        assert!(result.contains("fn add(a: i32, b: i32) -> i32"));
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
        let result = t.transpile("struct Point\n    var x: f32\n    var y: f32\nend struct").unwrap();
        assert!(result.contains("struct Point"));
        assert!(result.contains("x: f32,"));
        assert!(result.contains("y: f32,"));
    }

    #[test]
    fn test_transpile_enum() {
        let t = Transpiler::new();
        let result = t.transpile("enum Direction\n    North\n    South\nend enum").unwrap();
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
}

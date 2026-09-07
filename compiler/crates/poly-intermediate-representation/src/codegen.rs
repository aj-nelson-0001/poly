//! intermediate representation → Rust code generation.
//!
//! This generator lowers the intermediate representation to idiomatic Rust.  It intentionally mirrors
//! the direct AST generator's behavior for the core language while keeping the
//! walk driven by the flat intermediate representation (which the optimizer may have rewritten).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use crate::intermediate_representation::*;

/// Generates Rust source from an intermediate representation program.
pub struct IntermediateRepresentationCodeGen {
    indent: usize,
    output: String,
    /// Names whose values have Poly's `string` type (needed for `+` lowering).
    /// Mutated from `&self` inside `gen_statement_str` (loop/match bodies),
    /// hence the cell.
    string_scopes: RefCell<Vec<HashSet<String>>>,
    /// Poly type name of named values.
    type_scopes: RefCell<Vec<HashMap<String, String>>>,
    /// Names whose values are vectors (needed for `{:?}` put formatting).
    vec_scopes: RefCell<Vec<HashSet<String>>>,
    /// Vector names mapped to their element type name (for `.iter().sum()`
    /// turbofish and iterator chains).
    vec_element_scopes: RefCell<Vec<HashMap<String, String>>>,
    /// Functions whose return annotation lowers to Rust `String`.
    string_functions: HashSet<String>,
    /// Methods keyed by receiver type and method name whose return type is `String`.
    string_methods: HashSet<(String, String)>,
    /// String constants emitted as `&str` and converted at use sites.
    string_constants: HashSet<String>,
    /// Unqualified enum variant names -> Rust-qualified names.
    enum_variants: HashMap<String, String>,
    /// Named fields for struct-style enum variants.
    enum_struct_fields: HashMap<(String, String), Vec<String>>,
    /// Recursive enum fields that require `Box` indirection in Rust.
    recursive_enum_fields: HashSet<(String, String, usize)>,
    /// Whether generated code needs `std::io::Write`.
    needs_io_write: RefCell<bool>,
    /// Whether generated code needs `std::io::BufRead` (file line reading).
    /// Set from `&self` expression generation, hence the cell.
    needs_bufread: RefCell<bool>,
    /// Whether generated code needs the shared SQLite connection static
    /// (`db_execute` lowers to rusqlite calls).
    needs_rusqlite: RefCell<bool>,
    /// Match bindings whose values are boxed recursive enum fields.
    boxed_bindings: RefCell<Vec<HashSet<String>>>,
    /// Names extracted from nested boxed patterns and their owned expressions.
    pattern_aliases: RefCell<Vec<HashMap<String, String>>>,
    /// Whether the expression currently being generated is a `return` value;
    /// closures rendered under this flag capture with `move` so returned
    /// closures that capture locals satisfy Rust's lifetime rules.
    closure_move: RefCell<bool>,
    /// Per-function names returned from a `return` statement. Closure
    /// declarations bound to these names must own their captures as well.
    returned_closure_names: RefCell<Vec<HashSet<String>>>,
}

impl IntermediateRepresentationCodeGen {
    /// Create a new intermediate representation code generator.
    pub fn new() -> Self {
        Self {
            indent: 0,
            output: String::with_capacity(4096),
            string_scopes: RefCell::new(vec![HashSet::new()]),
            type_scopes: RefCell::new(vec![HashMap::new()]),
            vec_scopes: RefCell::new(vec![HashSet::new()]),
            vec_element_scopes: RefCell::new(vec![HashMap::new()]),
            string_functions: HashSet::new(),
            string_methods: HashSet::new(),
            string_constants: HashSet::new(),
            enum_variants: HashMap::new(),
            enum_struct_fields: HashMap::new(),
            recursive_enum_fields: HashSet::new(),
            needs_io_write: RefCell::new(false),
            needs_bufread: RefCell::new(false),
            needs_rusqlite: RefCell::new(false),
            boxed_bindings: RefCell::new(Vec::new()),
            pattern_aliases: RefCell::new(Vec::new()),
            closure_move: RefCell::new(false),
            returned_closure_names: RefCell::new(Vec::new()),
        }
    }

    /// Generate Rust source for the intermediate representation program.
    pub fn generate(&mut self, program: &Program) -> String {
        self.collect_metadata(program);

        self.writeln("// Generated from Poly source code");
        self.writeln(
            "#![allow(unused_variables, unused_mut, unused_imports, dead_code, unused_parens, unreachable_patterns)]",
        );
        self.writeln("");

        for use_decl in &program.uses {
            self.writeln(&format!("use {};", use_decl));
        }
        if !program.uses.is_empty() {
            self.writeln("");
        }

        for constant in &program.constants {
            self.gen_constant(constant);
        }
        if !program.constants.is_empty() {
            self.writeln("");
        }
        for alias in &program.type_aliases {
            self.writeln(&format!(
                "type {} = {};",
                alias.name,
                self.gen_type(&alias.ty)
            ));
        }
        if !program.type_aliases.is_empty() {
            self.writeln("");
        }
        for structure in &program.structs {
            self.gen_struct(structure);
            self.writeln("");
        }
        for enumeration in &program.enums {
            self.gen_enum(enumeration);
            self.writeln("");
        }
        for trait_decl in &program.traits {
            self.gen_trait(trait_decl);
            self.writeln("");
        }
        for implementation in &program.impls {
            self.gen_impl(implementation);
            self.writeln("");
        }
        for function in &program.functions {
            self.gen_function(function, false);
            self.writeln("");
        }

        // Emit top-level #rust blocks verbatim (outside fn main)
        for rust_block in &program.top_level_rust_blocks {
            self.warn_if_rust_block_has_executable_code(rust_block);
            for line in rust_block.lines() {
                if line.trim().is_empty() {
                    self.writeln("");
                } else {
                    self.writeln(line);
                }
            }
            self.writeln("");
        }

        let has_async = program.functions.iter().any(|f| f.is_async)
            || program.main_body.iter().any(statement_contains_await);

        if !program.main_body.is_empty() {
            if has_async {
                self.writeln("#[tokio::main]");
            }
            self.writeln(if has_async {
                "async fn main() {"
            } else {
                "fn main() {"
            });
            self.indent += 1;
            self.enter_scope();
            for statement in &program.main_body {
                self.gen_statement(statement);
            }
            self.exit_scope();
            self.indent -= 1;
            self.writeln("}");
        }

        let mut result = String::new();
        result.push_str(&self.output);
        if *self.needs_io_write.borrow() {
            result.push_str("\nuse std::io::Write;\n");
        }
        if *self.needs_bufread.borrow() {
            result.push_str("\nuse std::io::BufRead;\n");
        }
        if *self.needs_rusqlite.borrow() {
            // One shared in-memory SQLite connection for the whole program, so
            // `CREATE TABLE` / `INSERT` / `SELECT` calls across separate
            // `db_execute(...).await` blocks see the same database.
            result.push_str(
                "\nstatic __POLY_DB: std::sync::OnceLock<std::sync::Mutex<rusqlite::Connection>> = std::sync::OnceLock::new();\n",
            );
        }
        result
    }

    fn collect_metadata(&mut self, program: &Program) {
        for function in &program.functions {
            if function
                .return_type
                .as_ref()
                .is_some_and(Self::is_string_type)
            {
                self.string_functions.insert(function.name.clone());
            }
        }
        for structure in &program.structs {
            for method in &structure.methods {
                if method
                    .return_type
                    .as_ref()
                    .is_some_and(Self::is_string_type)
                {
                    self.string_methods
                        .insert((structure.name.clone(), method.name.clone()));
                }
            }
        }
        for enumeration in &program.enums {
            for variant in &enumeration.variants {
                self.enum_variants.insert(
                    variant.name.clone(),
                    format!("{}::{}", enumeration.name, variant.name),
                );
                match (&variant.is_struct, &variant.fields) {
                    (true, fields) => {
                        self.enum_struct_fields.insert(
                            (enumeration.name.clone(), variant.name.clone()),
                            fields.iter().map(|field| field.name.clone()).collect(),
                        );
                        for (index, field) in fields.iter().enumerate() {
                            if matches!(&field.ty, Type::Named(name) if name == &enumeration.name) {
                                self.recursive_enum_fields.insert((
                                    enumeration.name.clone(),
                                    variant.name.clone(),
                                    index,
                                ));
                            }
                        }
                    }
                    (false, fields) => {
                        for (index, field) in fields.iter().enumerate() {
                            if matches!(&field.ty, Type::Named(name) if name == &enumeration.name) {
                                self.recursive_enum_fields.insert((
                                    enumeration.name.clone(),
                                    variant.name.clone(),
                                    index,
                                ));
                            }
                        }
                    }
                }
            }
            for method in &enumeration.methods {
                if method
                    .return_type
                    .as_ref()
                    .is_some_and(Self::is_string_type)
                {
                    self.string_methods
                        .insert((enumeration.name.clone(), method.name.clone()));
                }
            }
        }
        for implementation in &program.impls {
            for method in &implementation.methods {
                if method
                    .return_type
                    .as_ref()
                    .is_some_and(Self::is_string_type)
                {
                    self.string_methods
                        .insert((implementation.type_name.clone(), method.name.clone()));
                }
            }
        }
        for constant in &program.constants {
            if matches!(
                constant.value,
                Expr::Literal(Literal::String(_) | Literal::UnicodeString(_))
            ) {
                self.string_constants.insert(constant.name.clone());
                self.declare_string(constant.name.clone());
            }
        }
    }

    fn gen_constant(&mut self, constant: &Constant) {
        match &constant.value {
            Expr::Literal(Literal::String(value))
            | Expr::Literal(Literal::UnicodeString(value)) => {
                self.declare_string(constant.name.clone());
                self.string_constants.insert(constant.name.clone());
                self.writeln(&format!("const {}: &str = {:?};", constant.name, value));
            }
            value => {
                if self.is_string_expr(value) {
                    self.declare_string(constant.name.clone());
                }
                // Rust allows `const X = ...;` with the type inferred from the
                // initializer; a `: _` placeholder is not allowed on item
                // signatures, so annotate literals explicitly instead.
                let ty = match value {
                    Expr::Literal(Literal::Int(_)) => Some("i32"),
                    Expr::Literal(Literal::Float(_)) => Some("f64"),
                    Expr::Literal(Literal::Bool(_)) => Some("bool"),
                    Expr::Literal(Literal::Char(_)) => Some("char"),
                    _ => None,
                };
                match ty {
                    Some(ty) => self.writeln(&format!(
                        "const {}: {} = {};",
                        constant.name,
                        ty,
                        self.gen_expr(value)
                    )),
                    None => self.writeln(&format!(
                        "const {} = {};",
                        constant.name,
                        self.gen_expr(value)
                    )),
                }
            }
        }
    }

    fn gen_function(&mut self, function: &Function, is_method: bool) {
        self.enter_scope();
        self.returned_closure_names
            .borrow_mut()
            .push(collect_returned_binding_names(&function.body));
        let generics = self.gen_generics(&function.generics);
        let params: Vec<String> = function
            .params
            .iter()
            .map(|p| {
                let ty = self.gen_type(&p.ty);
                if p.name == "self" && is_method {
                    "mut self".to_string()
                } else if p.name == "self" {
                    "self".to_string()
                } else {
                    format!("{}: {}", p.name, ty)
                }
            })
            .collect();
        // A function-typed return annotation lowers to `impl Fn` so the body
        // can return closure literals (which capture) as well as fn pointers.
        let ret = match &function.return_type {
            Some(Type::Function { params, ret }) => {
                let rendered: Vec<String> = params.iter().map(|t| self.gen_type(t)).collect();
                format!(
                    " -> impl Fn({}) -> {}",
                    rendered.join(", "),
                    self.gen_type(ret)
                )
            }
            Some(ty) => format!(" -> {}", self.gen_type(ty)),
            None => String::new(),
        };

        for parameter in &function.params {
            if let Some(type_name) = named_type_name(&parameter.ty) {
                self.declare_type(parameter.name.clone(), type_name);
            }
            if Self::is_string_type(&parameter.ty) {
                self.declare_string(parameter.name.clone());
            }
            if Self::is_vec_type(&parameter.ty) {
                self.declare_vec(parameter.name.clone());
            }
        }

        if function.is_async && function.name == "main" {
            self.writeln("#[tokio::main]");
        }
        let async_prefix = if function.is_async { "async " } else { "" };
        self.writeln_fmt(format_args!(
            "{}fn {}{}({}){} {{",
            async_prefix,
            function.name,
            generics,
            params.join(", "),
            ret
        ));
        self.indent += 1;
        for statement in &function.body {
            self.gen_statement(statement);
        }
        self.indent -= 1;
        self.writeln("}");
        self.returned_closure_names.borrow_mut().pop();
        self.exit_scope();
    }

    fn gen_generics(&self, generics: &[GenericParam]) -> String {
        if generics.is_empty() {
            return String::new();
        }
        let rendered: Vec<String> = generics
            .iter()
            .map(|param| {
                if param.bounds.is_empty() {
                    param.name.clone()
                } else {
                    format!("{}: {}", param.name, param.bounds.join(" + "))
                }
            })
            .collect();
        format!("<{}>", rendered.join(", "))
    }

    fn gen_struct(&mut self, structure: &Struct) {
        let generics = self.gen_generics(&structure.generics);
        self.writeln_fmt(format_args!("struct {}{} {{", structure.name, generics));
        self.indent += 1;
        for field in &structure.fields {
            self.writeln_fmt(format_args!(
                "{}: {},",
                field.name,
                self.gen_type(&field.ty)
            ));
        }
        self.indent -= 1;
        self.writeln("}");

        if !structure.methods.is_empty() {
            self.writeln_fmt(format_args!("impl{generics} {} {{", structure.name));
            self.indent += 1;
            for method in &structure.methods {
                self.gen_function(method, true);
            }
            self.indent -= 1;
            self.writeln("}");
        }
    }

    fn gen_enum(&mut self, enumeration: &Enum) {
        self.writeln("#[derive(Debug, Clone)]");
        self.writeln_fmt(format_args!("enum {} {{", enumeration.name));
        self.indent += 1;
        for variant in &enumeration.variants {
            if variant.is_struct {
                self.writeln_fmt(format_args!("{} {{", variant.name));
                self.indent += 1;
                for field in &variant.fields {
                    let ty = if matches!(&field.ty, Type::Named(name) if name == &enumeration.name)
                    {
                        format!("Box<{}>", self.gen_type(&field.ty))
                    } else {
                        self.gen_type(&field.ty)
                    };
                    self.writeln_fmt(format_args!("{}: {},", field.name, ty));
                }
                self.indent -= 1;
                self.writeln("},");
            } else if variant.fields.is_empty() {
                self.writeln_fmt(format_args!("{},", variant.name));
            } else {
                let types: Vec<String> = variant
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        if self.recursive_enum_fields.contains(&(
                            enumeration.name.clone(),
                            variant.name.clone(),
                            index,
                        )) {
                            format!("Box<{}>", self.gen_type(&field.ty))
                        } else {
                            self.gen_type(&field.ty)
                        }
                    })
                    .collect();
                self.writeln_fmt(format_args!("{}({}),", variant.name, types.join(", ")));
            }
        }
        self.indent -= 1;
        self.writeln("}");

        if !enumeration.methods.is_empty() {
            self.writeln_fmt(format_args!("impl {} {{", enumeration.name));
            self.indent += 1;
            for method in &enumeration.methods {
                self.gen_function(method, true);
            }
            self.indent -= 1;
            self.writeln("}");
        }
    }

    fn gen_trait(&mut self, trait_decl: &Trait) {
        self.writeln(&format!("trait {} {{", trait_decl.name));
        self.indent += 1;
        for method in &trait_decl.methods {
            let params: Vec<String> = method
                .params
                .iter()
                .map(|p| {
                    if p.name == "self" {
                        "self".to_string()
                    } else {
                        format!("{}: {}", p.name, self.gen_type(&p.ty))
                    }
                })
                .collect();
            let ret = method
                .return_type
                .as_ref()
                .map(|ty| format!(" -> {}", self.gen_type(ty)))
                .unwrap_or_default();
            let async_prefix = if method.is_async { "async " } else { "" };
            self.writeln(&format!(
                "{}fn {}({}){};",
                async_prefix,
                method.name,
                params.join(", "),
                ret
            ));
        }
        self.indent -= 1;
        self.writeln("}");
    }

    fn gen_impl(&mut self, implementation: &Impl) {
        if let Some(trait_name) = &implementation.trait_name {
            self.writeln(&format!(
                "impl {} for {} {{",
                trait_name, implementation.type_name
            ));
        } else {
            self.writeln(&format!("impl {} {{", implementation.type_name));
        }
        self.indent += 1;
        for method in &implementation.methods {
            self.gen_function(method, true);
        }
        self.indent -= 1;
        self.writeln("}");
    }

    fn gen_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::VarDecl { name, ty, value } => {
                if let Some(type_name) = ty.as_ref().and_then(named_type_name) {
                    self.declare_type(name.clone(), type_name);
                } else if let Some(Expr::Struct {
                    name: type_name, ..
                }) = value
                {
                    self.declare_type(name.clone(), type_name.clone());
                } else if let Some(type_name) = value.as_ref().and_then(|v| self.expr_enum_type(v))
                {
                    self.declare_type(name.clone(), type_name);
                }
                let value_is_string = ty.is_none()
                    && value
                        .as_ref()
                        .is_some_and(|value| self.is_string_expr(value));
                if ty.as_ref().is_some_and(Self::is_string_type) || value_is_string {
                    self.declare_string(name.clone());
                }
                if ty.as_ref().is_some_and(Self::is_vec_type)
                    || value.as_ref().is_some_and(|value| self.is_vec_expr(value))
                {
                    self.declare_vec(name.clone());
                }
                let element_type = ty
                    .as_ref()
                    .and_then(|ty| self.vec_element_type_of_type(ty))
                    .or_else(|| {
                        value
                            .as_ref()
                            .and_then(|value| self.array_element_type(value))
                    });
                if let Some(element) = element_type {
                    self.declare_vec_element(name.clone(), element);
                }
                let ty_str = ty.as_ref().map(|ty| self.gen_type(ty)).unwrap_or_default();
                match value {
                    Some(value) => {
                        let value_str = self.gen_decl_value(name, value, ty.as_ref());
                        if ty.is_some() {
                            self.writeln_fmt(format_args!(
                                "let mut {}: {} = {};",
                                name, ty_str, value_str
                            ));
                        } else {
                            self.writeln_fmt(format_args!("let mut {} = {};", name, value_str));
                        }
                    }
                    None => {
                        if ty.is_some() {
                            self.writeln_fmt(format_args!("let mut {}: {};", name, ty_str));
                        } else {
                            self.writeln_fmt(format_args!("let mut {};", name));
                        }
                    }
                }
            }
            Statement::LetDecl { name, ty, value } => {
                if let Some(type_name) = ty.as_ref().and_then(named_type_name) {
                    self.declare_type(name.clone(), type_name);
                } else if let Expr::Struct {
                    name: type_name, ..
                } = value
                {
                    self.declare_type(name.clone(), type_name.clone());
                }
                let value_is_string = ty.is_none() && self.is_string_expr(value);
                if ty.as_ref().is_some_and(Self::is_string_type) || value_is_string {
                    self.declare_string(name.clone());
                }
                if ty.as_ref().is_some_and(Self::is_vec_type) || self.is_vec_expr(value) {
                    self.declare_vec(name.clone());
                }
                let element_type = ty
                    .as_ref()
                    .and_then(|ty| self.vec_element_type_of_type(ty))
                    .or_else(|| self.array_element_type(value));
                if let Some(element) = element_type {
                    self.declare_vec_element(name.clone(), element);
                }
                let ty_str = ty.as_ref().map(|ty| self.gen_type(ty)).unwrap_or_default();
                let value_str = self.gen_decl_value(name, value, ty.as_ref());
                if ty.is_some() {
                    self.writeln_fmt(format_args!("let {}: {} = {};", name, ty_str, value_str));
                } else {
                    self.writeln_fmt(format_args!("let {} = {};", name, value_str));
                }
            }
            Statement::Assignment { target, value } => {
                // `m[key] = value` on a Map inserts rather than indexing
                // (`HashMap` has no `IndexMut` for owned keys).
                if let Expr::Index { object, index } = target {
                    if self.is_map_expr(object) {
                        let object_str = self.gen_expr(object);
                        let index_str = self.gen_expr(index);
                        let value_str = self.gen_expr(value);
                        self.writeln(&format!(
                            "{}.insert({}, {});",
                            object_str, index_str, value_str
                        ));
                        return;
                    }
                }
                let target_str = self.gen_expr(target);
                let value_str = self.gen_expr(value);
                self.writeln_fmt(format_args!("{} = {};", target_str, value_str));
            }
            Statement::Mutation { target, op, value } => {
                let target_str = self.gen_expr(target);
                let operator = match op {
                    MutationOp::Add => "+=",
                    MutationOp::Sub => "-=",
                    MutationOp::Inc => "+=",
                    MutationOp::Dec => "-=",
                };
                let amount = value
                    .as_ref()
                    .map(|value| self.gen_expr(value))
                    .unwrap_or_else(|| "1".to_string());
                // String append lowers to `String += &str`; the amount must be
                // borrowed so `+= String::from(...)` compiles. Numeric targets
                // keep the plain amount (loop variables are already `&T`).
                let amount = if matches!(op, MutationOp::Add | MutationOp::Inc)
                    && self.is_string_expr(target)
                {
                    format!("&({})", amount)
                } else {
                    amount
                };
                self.writeln_fmt(format_args!("{} {} {};", target_str, operator, amount));
            }
            Statement::Return(value) => match value {
                Some(value) => {
                    // Closures in return position capture with `move` so the
                    // returned closure owns any locals it references.
                    *self.closure_move.borrow_mut() = true;
                    let rendered = self.gen_expr(value);
                    *self.closure_move.borrow_mut() = false;
                    self.writeln_fmt(format_args!("return {};", rendered));
                }
                None => self.writeln("return;"),
            },
            Statement::Break => self.writeln("break;"),
            Statement::Continue => self.writeln("continue;"),
            Statement::Put { expr, redirect } => self.gen_put(expr, redirect.as_ref()),
            Statement::Error(expr) => {
                let expr_str = self.gen_expr(expr);
                self.writeln_fmt(format_args!("eprintln!(\"[ERROR] {{}}\", {});", expr_str));
            }
            Statement::Warn(expr) => {
                let expr_str = self.gen_expr(expr);
                self.writeln_fmt(format_args!("eprintln!(\"[WARN] {{}}\", {});", expr_str));
            }
            Statement::Info(expr) => {
                let expr_str = self.gen_expr(expr);
                self.writeln_fmt(format_args!("eprintln!(\"[INFO] {{}}\", {});", expr_str));
            }
            Statement::Expression(expr) => {
                // `>` and `>>` were legacy file-redirect spellings; redirects
                // are now exclusively `put ... to "file"`, so a top-level
                // binary expression statement is generated as an ordinary
                // expression.
                if let Expr::If {
                    condition,
                    then_block,
                    else_block: None,
                } = expr
                {
                    // While loop shape (parser encodes while as if-without-else).
                    let cond = self.gen_expr(condition);
                    self.writeln_fmt(format_args!("while {} {{", cond));
                    self.indent += 1;
                    self.enter_scope();
                    for statement in then_block {
                        self.gen_statement(statement);
                    }
                    self.exit_scope();
                    self.indent -= 1;
                    self.writeln("}");
                    return;
                }
                let expr_str = self.gen_expr(expr);
                match expr {
                    Expr::If { .. }
                    | Expr::LoopRange { .. }
                    | Expr::ForLoop { .. }
                    | Expr::InfiniteLoop(_)
                    | Expr::Match { .. } => {
                        self.writeln(&expr_str);
                    }
                    _ => self.writeln_fmt(format_args!("{};", expr_str)),
                }
            }
            Statement::If {
                condition,
                then_block,
                else_block,
                is_while,
            } => {
                if *is_while {
                    let cond = self.gen_expr(condition);
                    self.writeln_fmt(format_args!("while {} {{", cond));
                    self.indent += 1;
                    self.enter_scope();
                    for statement in then_block {
                        self.gen_statement(statement);
                    }
                    self.exit_scope();
                    self.indent -= 1;
                    self.writeln("}");
                    return;
                }
                let cond = self.gen_expr(condition);
                self.writeln_fmt(format_args!("if {} {{", cond));
                self.indent += 1;
                self.enter_scope();
                for statement in then_block {
                    self.gen_statement(statement);
                }
                self.exit_scope();
                self.indent -= 1;
                match else_block {
                    Some(else_block) if !else_block.is_empty() => {
                        self.writeln("} else {");
                        self.indent += 1;
                        self.enter_scope();
                        for statement in else_block {
                            self.gen_statement(statement);
                        }
                        self.exit_scope();
                        self.indent -= 1;
                        self.writeln("}");
                    }
                    _ => self.writeln("}"),
                }
            }
            Statement::Match { scrutinee, arms } => {
                let scrutinee_str = self.gen_expr(scrutinee);
                let scrutinee_str = if self.is_string_expr(scrutinee) {
                    format!("({}).as_str()", scrutinee_str)
                } else {
                    scrutinee_str
                };
                self.writeln_fmt(format_args!("match {} {{", scrutinee_str));
                self.indent += 1;
                for arm in arms {
                    let (pattern_str, pattern_guards) = self.gen_pattern_with_guards(&arm.pattern);
                    let mut boxed_bindings = HashSet::new();
                    self.collect_boxed_pattern_bindings(&arm.pattern, &mut boxed_bindings);
                    self.boxed_bindings.borrow_mut().push(boxed_bindings);
                    let mut aliases = HashMap::new();
                    self.collect_pattern_aliases(&arm.pattern, &mut aliases);
                    self.pattern_aliases.borrow_mut().push(aliases);
                    let body_str = match &arm.body {
                        MatchArmBody::Expression(expr) => self.gen_expr(expr),
                        MatchArmBody::Block(statements) => {
                            let mut block = String::from("{\n");
                            self.indent += 1;
                            self.enter_scope();
                            for statement in statements {
                                let mut temp = String::new();
                                std::mem::swap(&mut temp, &mut self.output);
                                let saved = self.indent;
                                let generated = self.gen_statement_str(statement);
                                self.indent = saved;
                                self.output = temp;
                                // Write indent directly to avoid String allocation
                                for _ in 0..self.indent {
                                    block.push_str("    ");
                                }
                                block.push_str(&generated);
                                block.push('\n');
                            }
                            self.exit_scope();
                            self.indent -= 1;
                            for _ in 0..self.indent {
                                block.push_str("    ");
                            }
                            block.push('}');
                            block
                        }
                    };
                    let body_str = body_str.trim_end_matches(';').to_string();
                    let mut guards = pattern_guards;
                    if let Some(guard) = &arm.guard {
                        guards.push(self.gen_expr(guard));
                    }
                    if guards.is_empty() {
                        self.writeln(&format!("    {} => {},", pattern_str, body_str));
                    } else {
                        self.writeln(&format!(
                            "    {} if {} => {},",
                            pattern_str,
                            guards.join(" && "),
                            body_str
                        ));
                    }
                    self.pattern_aliases.borrow_mut().pop();
                    self.boxed_bindings.borrow_mut().pop();
                }
                self.indent -= 1;
                self.writeln("}");
            }
            Statement::Block(statements) => {
                self.writeln("{");
                self.indent += 1;
                self.enter_scope();
                for statement in statements {
                    self.gen_statement(statement);
                }
                self.exit_scope();
                self.indent -= 1;
                self.writeln("}");
            }
            Statement::NestedFunction(function) => self.gen_function(function, false),
            Statement::ForeignBlock { language, content } if language == "rust" => {
                // Foreign blocks are normally hoisted to module scope. Keep
                // nested Rust blocks opaque if one reaches this path.
                self.warn_if_rust_block_has_executable_code(content);
                for line in content.lines() {
                    if line.trim().is_empty() {
                        self.writeln("");
                    } else {
                        self.writeln(line);
                    }
                }
            }
            Statement::ForeignBlock { .. } => {}
        }
    }

    /// The `println!`/`format!` spec used for a put value: `{:?}` for vectors
    /// (which do not implement `Display`), `{}` otherwise.
    fn put_format_spec(&self, expr: &Expr) -> &'static str {
        // Vector-like values and checked-index `Option` results print with
        // debug formatting (`{:?}`), since neither implements `Display`.
        if self.is_vec_expr(expr)
            || matches!(expr, Expr::MethodCall { method, .. } if method == "get")
        {
            "{:?}"
        } else {
            "{}"
        }
    }

    /// Warn if a `#rust` block looks like it contains executable code at
    /// the top level. Poly foreign blocks should only contain declarations
    /// (fn, struct, enum, const, impl, trait, use, etc.).
    fn warn_if_rust_block_has_executable_code(&self, block: &str) {
        let declaration_prefixes = [
            "fn ",
            "pub fn ",
            "pub(crate) fn ",
            "async fn ",
            "pub async fn ",
            "struct ",
            "pub struct ",
            "enum ",
            "pub enum ",
            "const ",
            "pub const ",
            "static ",
            "pub static ",
            "type ",
            "pub type ",
            "impl ",
            "pub impl ",
            "impl<",
            "trait ",
            "pub trait ",
            "use ",
            "pub use ",
            "mod ",
            "pub mod ",
            "extern ",
            "unsafe fn ",
            "pub unsafe fn ",
            "#[", // attribute like #[derive(...)]
            "//", // comment
            "/*", // block comment
        ];
        let executable_prefixes = [
            "let ",
            "let mut ",
            "println!",
            "eprintln!",
            "print!",
            "eprint!",
            "for ",
            "if ",
            "while ",
            "loop ",
            "match ",
            "return ",
            "return;",
            "break",
            "continue",
            "assert!",
            "assert_eq!",
            "assert_ne!",
            "dbg!",
            "todo!",
            "unimplemented!",
            "unreachable!",
            "panic!",
        ];
        // Track brace depth to only check top-level lines
        let mut depth: i32 = 0;
        for line in block.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Count brace changes to track nesting depth
            for ch in trimmed.chars() {
                match ch {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            // Only check lines at depth 0 (top-level, before entering any block)
            // After the first '{', we're inside a function/struct/enum body
            if depth <= 0 && !trimmed.starts_with('}') {
                let is_declaration = declaration_prefixes
                    .iter()
                    .any(|prefix| trimmed.starts_with(prefix));
                let is_executable = executable_prefixes
                    .iter()
                    .any(|prefix| trimmed.starts_with(prefix));
                if !is_declaration && is_executable {
                    eprintln!(
                        "[WARN] #rust block contains executable code at top level: `{}`",
                        trimmed.chars().take(60).collect::<String>()
                    );
                    eprintln!("       Poly #rust blocks should only contain declarations (fn, struct, enum, etc.)");
                    eprintln!("       Executable code should be in Poly sections.");
                    break; // one warning per block is enough
                }
            }
        }
    }

    fn gen_put(&mut self, expr: &Expr, redirect: Option<&Redirect>) {
        // Legacy `put x > "file"` / `put x >> "file"` handling was removed:
        // redirects are now only `put ... to "file"` (the Redirect field).

        let expr_str = self.gen_expr(expr);
        let spec = self.put_format_spec(expr);
        match redirect {
            Some(Redirect::Write(path)) => {
                let path_str = self.gen_expr(path);
                *self.needs_io_write.borrow_mut() = true;
                self.writeln_fmt(format_args!(
                    "std::fs::write({}, format!(\"{}\\n\", {})).unwrap();",
                    path_str, spec, expr_str
                ));
            }
            Some(Redirect::Append(path)) => {
                let path_str = self.gen_expr(path);
                *self.needs_io_write.borrow_mut() = true;
                self.writeln_fmt(format_args!(
                    "{{ let mut f = std::fs::OpenOptions::new().append(true).create(true).open({}).unwrap(); writeln!(f, \"{}\", {}).unwrap(); }}",
                    path_str, spec, expr_str
                ));
            }
            None => {
                self.writeln_fmt(format_args!("println!(\"{}\", {});", spec, expr_str));
            }
        }
    }

    fn gen_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::Literal(Literal::Int(value)) => value.clone(),
            Expr::Literal(Literal::Float(value)) => value.clone(),
            Expr::Literal(Literal::String(value))
            | Expr::Literal(Literal::UnicodeString(value)) => {
                format!("String::from({:?})", value)
            }
            Expr::Literal(Literal::Char(value)) => format!("{:?}", value),
            Expr::Literal(Literal::Bool(value)) => {
                // Avoid String allocation for boolean literals
                if *value {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Expr::Literal(Literal::Bytes(bytes)) => {
                let hex: Vec<String> = bytes.iter().map(|b| format!("0x{:02X}", b)).collect();
                format!("vec![{}]", hex.join(", "))
            }
            Expr::Identifier(name) => {
                if let Some(alias) = self
                    .pattern_aliases
                    .borrow()
                    .iter()
                    .rev()
                    .find_map(|aliases| aliases.get(name))
                {
                    alias.clone()
                } else if self.is_boxed_binding(name) {
                    format!("*{}", name)
                } else if self.string_constants.contains(name) {
                    format!("{}.to_string()", name)
                } else {
                    // Avoid clone when returning the name as-is (most common case)
                    match self.enum_variants.get(name) {
                        Some(qualified) => qualified.clone(),
                        None => name.clone(),
                    }
                }
            }
            Expr::BinaryOp { op, left, right } => {
                let l = self.gen_expr(left);
                let r = self.gen_expr(right);
                if matches!(op, BinaryOp::Add)
                    && (self.is_string_expr(left) || self.is_string_expr(right))
                {
                    format!("format!(\"{{}}{{}}\", {}, {})", l, r)
                } else {
                    let op_str = self.gen_binary_op(op);
                    format!("({} {} {})", l, op_str, r)
                }
            }
            Expr::UnaryOp { op, expr } => {
                let e = self.gen_expr(expr);
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                    UnaryOp::BitNot => "!",
                    UnaryOp::Deref => "*",
                };
                format!("({}{})", op_str, e)
            }
            Expr::Call { func, args } => {
                let func_str = self.gen_expr(func);
                let args_str: Vec<String> = args
                    .iter()
                    .map(|a| {
                        let rendered = self.gen_expr(a);
                        if let Expr::Identifier(name) = a {
                            if self.is_boxed_binding(name) {
                                return format!("({}).clone()", rendered);
                            }
                            if self.is_string_expr(a) {
                                return format!("{}.clone()", rendered);
                            }
                            if self.expression_type_name(a).is_some_and(|type_name| {
                                self.enum_variants.values().any(|qualified| {
                                    qualified.starts_with(&format!("{}::", type_name))
                                })
                            }) {
                                return format!("{}.clone()", rendered);
                            }
                        }
                        rendered
                    })
                    .collect();
                if let Expr::Identifier(name) = func.as_ref() {
                    if let Some(qualified) = self.enum_variants.get(name) {
                        let (enum_name, variant) = qualified
                            .split_once("::")
                            .map(|(e, v)| (e.to_string(), v.to_string()))
                            .unwrap_or_else(|| (String::new(), name.clone()));
                        let values = args_str
                            .iter()
                            .enumerate()
                            .map(|(index, value)| {
                                if self.recursive_enum_fields.contains(&(
                                    enum_name.clone(),
                                    variant.clone(),
                                    index,
                                )) {
                                    format!("Box::new({})", value)
                                } else {
                                    value.clone()
                                }
                            })
                            .collect::<Vec<_>>();
                        if let Some(fields) = self
                            .enum_struct_fields
                            .get(&(enum_name.clone(), variant.clone()))
                        {
                            let rendered = fields
                                .iter()
                                .zip(values.iter())
                                .map(|(field, value)| format!("{}: {}", field, value))
                                .collect::<Vec<_>>();
                            return format!("{} {{ {} }}", qualified, rendered.join(", "));
                        }
                        return format!("{}({})", qualified, values.join(", "));
                    }
                }
                match func_str.as_str() {
                    "open" if args.len() == 1 => {
                        // Lower to a buffered reader so `get_line`/`eof` can
                        // be implemented on top of `BufRead`.
                        *self.needs_bufread.borrow_mut() = true;
                        format!(
                            "std::io::BufReader::new(std::fs::File::open({}).unwrap())",
                            args_str.join(", ")
                        )
                    }
                    "str" | "to_string" if args.len() == 1 => {
                        format!("format!(\"{}\", {})", "{}", args_str[0])
                    }
                    "Error" if args.len() <= 1 => format!(
                        "Err({})",
                        args_str
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "()".to_string())
                    ),
                    "max" if args.len() == 2 => {
                        format!(
                            "({}).max({})",
                            self.gen_numeric_arg(&args[0]),
                            self.gen_numeric_arg(&args[1])
                        )
                    }
                    "min" if args.len() == 2 => {
                        format!(
                            "({}).min({})",
                            self.gen_numeric_arg(&args[0]),
                            self.gen_numeric_arg(&args[1])
                        )
                    }
                    "abs" if args.len() == 1 => {
                        format!("({}).abs()", self.gen_numeric_arg(&args[0]))
                    }
                    "sqrt" if args.len() == 1 => {
                        format!("({}).sqrt()", self.gen_numeric_arg(&args[0]))
                    }
                    "pow" if args.len() == 2 => {
                        let base = self.gen_numeric_arg(&args[0]);
                        let exponent = self.gen_numeric_arg(&args[1]);
                        // Rust's integer `pow` takes a `u32` exponent and
                        // floats use `powf`, so adapt the Poly signature
                        // `pow(base, exponent)` to the matching method.
                        let base_is_float = match &args[0] {
                            Expr::Literal(Literal::Float(_)) => true,
                            Expr::UnaryOp {
                                op: UnaryOp::Neg,
                                expr,
                            } => matches!(expr.as_ref(), Expr::Literal(Literal::Float(_))),
                            _ => false,
                        };
                        if base_is_float {
                            format!("({}).powf({})", base, exponent)
                        } else {
                            format!("({}).pow({} as u32)", base, exponent)
                        }
                    }
                    "sleep" if args.len() == 1 => format!(
                        "std::thread::sleep(std::time::Duration::from_millis({} as u64))",
                        args_str[0]
                    ),
                    "delay" if args.len() == 1 => format!(
                        "tokio::time::sleep(std::time::Duration::from_millis({} as u64))",
                        args_str[0]
                    ),
                    "exit" if args.len() == 1 => {
                        format!("std::process::exit({})", args_str[0])
                    }
                    // `http_get` and `tcp_connect` are real (if minimal)
                    // tokio TCP implementations: `http_get` speaks a raw
                    // HTTP/1.1 GET and returns the response body, and
                    // `tcp_connect` establishes the connection and returns the
                    // peer address. `db_execute` runs the query against a
                    // shared in-memory SQLite database (rusqlite) and returns
                    // each row as a `|`-joined string of its `{:?}` values;
                    // the database persists across calls within one program.
                    "http_get" if args.len() == 1 => self.gen_http_get(&args_str[0]),
                    "tcp_connect" if args.len() == 1 => self.gen_tcp_connect(&args_str[0]),
                    "db_execute" if args.len() == 1 => self.gen_db_execute(&args_str[0]),
                    // Mini test-framework builtins: `assert(cond)` panics on
                    // failure (matching Rust's `assert!`), `pass(msg)` prints a
                    // PASS marker, and `fail(msg)` prints FAIL and exits
                    // non-zero so CI and scripts can detect the failure.
                    "assert" if args.len() == 1 => {
                        format!("assert!({}, \"assertion failed\")", args_str[0])
                    }
                    "pass" if args.len() == 1 => {
                        format!("{{ println!(\"[PASS] {{}}\", {}); }}", args_str[0])
                    }
                    "fail" if args.len() == 1 => {
                        format!(
                            "{{ println!(\"[FAIL] {{}}\", {}); std::process::exit(1); }}",
                            args_str[0]
                        )
                    }
                    "spawn_task" if args.len() == 1 => {
                        // `spawn <expr>` spawns a tokio task running the
                        // expression (which must be a future, e.g. an async
                        // call, `http_get(url)`, or `delay(ms)`).
                        format!("tokio::spawn({})", args_str[0])
                    }
                    _ => format!("{}({})", func_str, args_str.join(", ")),
                }
            }
            Expr::MethodCall {
                object,
                method,
                args,
            } => {
                if method == "await" {
                    let object_str = self.gen_expr(object);
                    return format!("{}.await", object_str);
                }
                let object_str = self.gen_expr(object);
                let args_str: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                match method.as_str() {
                    "eof" => {
                        // `BufRead::fill_buf` peeks without consuming; an empty
                        // buffer (or a failed read) means end-of-file.
                        *self.needs_bufread.borrow_mut() = true;
                        format!(
                            "{{ {}.fill_buf().map(|buffer| buffer.is_empty()).unwrap_or(true) }}",
                            object_str
                        )
                    }
                    "get_line" => {
                        *self.needs_bufread.borrow_mut() = true;
                        format!(
                            "{{ let mut __poly_line = String::new(); let _ = {}.read_line(&mut __poly_line).unwrap(); __poly_line.trim_end().to_string() }}",
                            object_str
                        )
                    }
                    "len" => format!("({}.len() as i32)", object_str),
                    "clear" if args.is_empty() => format!("{}.clear()", object_str),
                    "reserve" if args.len() == 1 => {
                        format!("{}.reserve({} as usize)", object_str, args_str[0])
                    }
                    // `xs.iter()` yields owned elements (`iter().cloned()`) so
                    // chained `map`/`filter`/`sum` and loop bindings see `T`
                    // instead of `&T`, matching the checker's element type.
                    "iter" if args.is_empty() => format!("{}.iter().cloned()", object_str),
                    // `.sum()` is generic in Rust, so annotate the element type
                    // when it is known (from the collection's declaration or an
                    // array literal); otherwise plain `.sum()` still works when
                    // the surrounding binding/argument pins the type.
                    "sum" if args.is_empty() => match self.vec_element_type_of(object) {
                        Some(element) => format!("{}.sum::<{}>()", object_str, element),
                        None => format!("{}.sum()", object_str),
                    },
                    // `.collect()` is generic over the collection type; `Vec<_>`
                    // pins it and lets the element be inferred from the chain.
                    "collect" if args.is_empty() => format!("{}.collect::<Vec<_>>()", object_str),
                    "contains" if args.len() == 1 && self.is_string_expr(object) => {
                        format!("{}.contains({}.as_str())", object_str, args_str[0])
                    }
                    "contains" if args.len() == 1 && self.is_set_expr(object) => {
                        format!("{}.contains(&{})", object_str, args_str[0])
                    }
                    "to_string" => {
                        if self.is_string_expr(object) {
                            format!("{}.to_string()", object_str)
                        } else {
                            format!("format!(\"{{:?}}\", {})", object_str)
                        }
                    }
                    "pad_left" if args.len() == 1 => format!(
                        "format!(\"{{:>width$}}\", {}, width = {})",
                        object_str, args_str[0]
                    ),
                    "repeat" if args.len() == 1 => format!(
                        "{}.repeat({} as usize)",
                        object_str, args_str[0]
                    ),
                    "split" if args.len() == 1 => format!(
                        "{}.split({}.as_str()).map(|part| part.to_string()).collect::<Vec<String>>()",
                        object_str, args_str[0]
                    ),
                    "starts_with" | "ends_with"
                        if args.len() == 1 && self.is_string_expr(object) =>
                    {
                        format!("{}.{}({}.as_str())", object_str, method, args_str[0])
                    }
                    "replace" if args.len() == 2 => format!(
                        "{}.replace({}.as_str(), {}.as_str())",
                        object_str, args_str[0], args_str[1]
                    ),
                    // Higher-order methods iterate owned elements: `.chars()`
                    // for strings, `iter().cloned()` for vectors, so the
                    // receiver stays usable afterwards. When the receiver is
                    // already an iterator (from `.iter()`), the chain stays
                    // lazy and no prefix or `.collect()` is added.
                    "map" if args.len() == 1 => {
                        if self.is_iterator_expr(object) {
                            format!("{}.map({})", object_str, args_str[0])
                        } else {
                            let iter = if self.is_string_expr(object) {
                                "chars()"
                            } else {
                                "iter().cloned()"
                            };
                            format!(
                                "{}.{}.map({}).collect::<Vec<_>>()",
                                object_str, iter, args_str[0]
                            )
                        }
                    }
                    "reduce" if args.len() == 2 => {
                        let iter = if self.is_string_expr(object) {
                            "chars()"
                        } else {
                            "iter().cloned()"
                        };
                        format!(
                            "{}.{}.fold({}, {})",
                            object_str, iter, args_str[0], args_str[1]
                        )
                    }
                    "filter" if args.len() == 1 => {
                        if self.is_iterator_expr(object) {
                            format!("{}.filter({})", object_str, self.gen_filter_callback(&args[0]))
                        } else {
                            let iter = if self.is_string_expr(object) {
                                "chars()"
                            } else {
                                "iter().cloned()"
                            };
                            format!(
                                "{}.{}.filter({}).collect::<Vec<_>>()",
                                object_str,
                                iter,
                                self.gen_filter_callback(&args[0])
                            )
                        }
                    }
                    // `xs.join(sep)` concatenates string elements with the
                    // separator; Rust's slice `join` works on `Vec<String>`.
                    "join" if args.len() == 1 => format!(
                        "{}.join({}.as_str())",
                        object_str, args_str[0]
                    ),
                    "sort_by" if args.len() == 1 => format!(
                        "{{ let mut v = {}.clone(); v.sort_by({}); v }}",
                        object_str,
                        self.gen_sort_callback(&args[0])
                    ),
                    // Map accessors borrow their keys: `HashMap::get(&key)`;
                    // Set `remove` borrows its element too. These must be
                    // matched before the generic Vec `get` arm, which the
                    // loose `is_vec_expr` heuristic also matches for Map- and
                    // Set-typed identifiers.
                    "get" | "contains_key" | "remove"
                        if args.len() == 1 && self.is_map_expr(object) =>
                    {
                        format!("{}.{}(&{})", object_str, method, args_str[0])
                    }
                    "remove" if args.len() == 1 && self.is_set_expr(object) => {
                        format!("{}.remove(&{})", object_str, args_str[0])
                    }
                    // Checked (non-panicking) indexing: `xs.get(i)` returns
                    // `Option<T>` for vectors and `Option<char>` for strings.
                    "get" if args.len() == 1 && self.is_vec_expr(object) => {
                        format!("{}.get({} as usize).cloned()", object_str, args_str[0])
                    }
                    "get" if args.len() == 1 && self.is_string_expr(object) => {
                        format!("{}.chars().nth({} as usize)", object_str, args_str[0])
                    }
                    // Result test-framework accessors.
                    "is_ok" if args.is_empty() => format!("{}.is_ok()", object_str),
                    "is_error" if args.is_empty() => format!("{}.is_err()", object_str),
                    "unwrap" if args.is_empty() => format!("{}.clone().unwrap()", object_str),
                    "unwrap_or" if args.len() == 1 => {
                        format!("{}.clone().unwrap_or({})", object_str, args_str[0])
                    }
                    // String methods that return `&str` in Rust (`trim`,
                    // `to_uppercase`-style transformations are already owned,
                    // but `trim` is not) are re-owned so the result matches the
                    // checker's `String` type.
                    _ if self.is_string_expr(object)
                        && matches!(method.as_str(), "trim") =>
                    {
                        format!("{}.trim().to_string()", object_str)
                    }
                    _ => format!("{}.{}({})", object_str, method, args_str.join(", ")),
                }
            }
            Expr::Index { object, index } => {
                let object_str = self.gen_expr(object);
                let index_str = self.gen_expr(index);
                if self.is_string_expr(object) {
                    format!(
                        "{}.chars().nth({} as usize).expect(\"string index out of bounds\")",
                        object_str, index_str
                    )
                } else if self.is_map_expr(object) {
                    format!(
                        "{}.get(&{}).map(|v| v.clone()).unwrap_or_default()",
                        object_str, index_str
                    )
                } else {
                    format!("{}[({}) as usize]", object_str, index_str)
                }
            }
            Expr::FieldAccess { object, field } => {
                format!("{}.{}", self.gen_expr(object), field)
            }
            Expr::TupleIndex { object, index } => {
                format!("{}.{}", self.gen_expr(object), index)
            }
            Expr::Parenthesized(expr) => format!("({})", self.gen_expr(expr)),
            Expr::If {
                condition,
                then_block,
                else_block,
            } => {
                let cond = self.gen_expr(condition);
                let mut result = format!("if {} {{\n", cond);
                self.enter_scope();
                for (index, statement) in then_block.iter().enumerate() {
                    let generated = self.gen_statement_str(statement);
                    let generated = if index + 1 == then_block.len()
                        && !matches!(
                            statement,
                            Statement::Expression(Expr::MethodCall { method, .. })
                                if method == "await"
                        )
                        && matches!(statement, Statement::Expression(_))
                    {
                        generated.trim_end_matches(';').to_string()
                    } else {
                        generated
                    };
                    result.push_str(&format!("    {}\n", generated));
                }
                self.exit_scope();
                match else_block {
                    Some(else_block) if !else_block.is_empty() => {
                        result.push_str("} else {\n");
                        self.enter_scope();
                        for (index, statement) in else_block.iter().enumerate() {
                            let generated = self.gen_statement_str(statement);
                            let generated = if index + 1 == else_block.len()
                                && !matches!(
                                    statement,
                                    Statement::Expression(Expr::MethodCall { method, .. })
                                        if method == "await"
                                )
                                && matches!(statement, Statement::Expression(_))
                            {
                                generated.trim_end_matches(';').to_string()
                            } else {
                                generated
                            };
                            result.push_str(&format!("    {}\n", generated));
                        }
                        self.exit_scope();
                        result.push_str("}\n");
                    }
                    _ => result.push_str("}\n"),
                }
                result
            }
            Expr::Match { scrutinee, arms } => {
                let scrutinee_str = self.gen_expr(scrutinee);
                let scrutinee_str = if self.is_string_expr(scrutinee) {
                    format!("({}).as_str()", scrutinee_str)
                } else {
                    scrutinee_str
                };
                let mut result = format!("match {} {{\n", scrutinee_str);
                for arm in arms {
                    let (pattern_str, pattern_guards) = self.gen_pattern_with_guards(&arm.pattern);
                    let mut boxed_bindings = HashSet::new();
                    self.collect_boxed_pattern_bindings(&arm.pattern, &mut boxed_bindings);
                    self.boxed_bindings.borrow_mut().push(boxed_bindings);
                    let mut aliases = HashMap::new();
                    self.collect_pattern_aliases(&arm.pattern, &mut aliases);
                    self.pattern_aliases.borrow_mut().push(aliases);
                    let body_str = match &arm.body {
                        MatchArmBody::Expression(expr) => self.gen_expr(expr),
                        MatchArmBody::Block(statements) => {
                            let mut block = String::from("{\n");
                            self.enter_scope();
                            for (index, statement) in statements.iter().enumerate() {
                                let generated = self.gen_statement_str(statement);
                                let generated = if index + 1 == statements.len()
                                    && matches!(statement, Statement::Expression(_))
                                {
                                    generated.trim_end_matches(';').to_string()
                                } else {
                                    generated
                                };
                                block.push_str(&format!("        {}\n", generated));
                            }
                            self.exit_scope();
                            block.push_str("    }");
                            block
                        }
                    };
                    let body_str = body_str.trim_end_matches(';').to_string();
                    let mut guards = pattern_guards;
                    if let Some(guard) = &arm.guard {
                        guards.push(self.gen_expr(guard));
                    }
                    if guards.is_empty() {
                        result.push_str(&format!("    {} => {},\n", pattern_str, body_str));
                    } else {
                        result.push_str(&format!(
                            "    {} if {} => {},\n",
                            pattern_str,
                            guards.join(" && "),
                            body_str
                        ));
                    }
                    self.pattern_aliases.borrow_mut().pop();
                    self.boxed_bindings.borrow_mut().pop();
                }
                result.push('}');
                result
            }
            Expr::Closure { params, body } => {
                let params_str: Vec<String> = params
                    .iter()
                    .map(|p| {
                        let ty = self.gen_type(&p.ty);
                        if ty == "_" {
                            p.name.clone()
                        } else if p.name == "self" {
                            format!("mut self: {}", ty)
                        } else {
                            format!("{}: {}", p.name, ty)
                        }
                    })
                    .collect();
                let prefix = if *self.closure_move.borrow() {
                    "move "
                } else {
                    ""
                };
                format!(
                    "{}|{}| {}",
                    prefix,
                    params_str.join(", "),
                    self.gen_expr(body)
                )
            }
            Expr::Array(elements) => {
                let elems: Vec<String> = elements.iter().map(|e| self.gen_expr(e)).collect();
                format!("vec![{}]", elems.join(", "))
            }
            Expr::Tuple(elements) => {
                let elems: Vec<String> = elements.iter().map(|e| self.gen_expr(e)).collect();
                format!("({})", elems.join(", "))
            }
            Expr::Struct { name, fields } => {
                let field_strs: Vec<String> = fields
                    .iter()
                    .map(|(name, value)| format!("{}: {}", name, self.gen_expr(value)))
                    .collect();
                format!("{} {{ {} }}", name, field_strs.join(", "))
            }
            Expr::Enum {
                enum_name,
                variant,
                data,
            } => {
                let qualified = if enum_name.is_empty() {
                    match variant.as_str() {
                        "Error" => "Err".to_string(),
                        "Timeout" => "Err".to_string(),
                        _ => self
                            .enum_variants
                            .get(variant)
                            .cloned()
                            .unwrap_or_else(|| variant.clone()),
                    }
                } else {
                    format!("{}::{}", enum_name, variant)
                };
                match data {
                    Some(args) => {
                        let args_str: Vec<String> = args
                            .iter()
                            .enumerate()
                            .map(|(index, arg)| {
                                let value = self.gen_expr(arg);
                                let resolved = if enum_name.is_empty() {
                                    qualified.split_once("::").map(|(name, _)| name.to_string())
                                } else {
                                    Some(enum_name.clone())
                                };
                                if resolved.is_some_and(|name| {
                                    self.recursive_enum_fields.contains(&(
                                        name,
                                        variant.clone(),
                                        index,
                                    ))
                                }) {
                                    format!("Box::new({})", value)
                                } else {
                                    value
                                }
                            })
                            .collect();
                        let resolved = if enum_name.is_empty() {
                            qualified.split_once("::").map(|(name, _)| name.to_string())
                        } else {
                            Some(enum_name.clone())
                        };
                        if let Some(resolved) = resolved {
                            if let Some(fields) = self
                                .enum_struct_fields
                                .get(&(resolved.clone(), variant.clone()))
                            {
                                let values = fields
                                    .iter()
                                    .zip(args_str.iter())
                                    .map(|(field, value)| format!("{}: {}", field, value))
                                    .collect::<Vec<_>>();
                                return format!("{} {{ {} }}", qualified, values.join(", "));
                            }
                        }
                        format!("{}({})", qualified, args_str.join(", "))
                    }
                    None => qualified,
                }
            }
            Expr::Range {
                start,
                end,
                inclusive,
            } => {
                let s = self.gen_expr(start);
                let e = self.gen_expr(end);
                if *inclusive {
                    format!("{}..={}", s, e)
                } else {
                    format!("{}..{}", s, e)
                }
            }
            Expr::LoopRange {
                variable,
                ranges,
                body,
            } => self.gen_loop_range(variable, ranges, body),
            Expr::ForLoop {
                variable,
                iterable,
                body,
            } => self.gen_for_loop(variable, iterable, body),
            Expr::InfiniteLoop(body) => {
                let mut result = "loop {\n".to_string();
                for statement in body {
                    result.push_str(&format!("    {}\n", self.gen_statement_str(statement)));
                }
                result.push('}');
                result
            }
            Expr::Try(expr) => format!("{}?", self.gen_expr(expr)),
            Expr::As { expr, ty } => {
                format!("{} as {}", self.gen_expr(expr), self.gen_type(ty))
            }
            Expr::Get(get) => self.gen_get(get),
            Expr::UnsafeBlock(statements) => {
                let mut result = "unsafe {\n".to_string();
                for statement in statements {
                    result.push_str(&format!("    {}\n", self.gen_statement_str(statement)));
                }
                result.push('}');
                result
            }
        }
    }

    /// Render a numeric argument for `abs`/`sqrt`/`pow`/`min`/`max`. Plain
    /// literals are ambiguous in Rust (`(-42).abs()` is E0689), so annotate
    /// them with their inferred type; other expressions (variables, calls)
    /// already carry a concrete type.
    fn gen_numeric_arg(&self, expr: &Expr) -> String {
        match expr {
            Expr::Literal(Literal::Int(value)) => format!("{value}_i32"),
            Expr::Literal(Literal::Float(value)) => format!("{value}_f64"),
            Expr::UnaryOp {
                op: UnaryOp::Neg,
                expr,
            } => match expr.as_ref() {
                Expr::Literal(Literal::Int(value)) => format!("-{value}_i32"),
                Expr::Literal(Literal::Float(value)) => format!("-{value}_f64"),
                _ => self.gen_expr(expr),
            },
            _ => self.gen_expr(expr),
        }
    }

    /// Absolute value of a loop step expression (`-2` -> `2`), used by the
    /// reversed-step iterator lowering.
    fn gen_step_magnitude(&self, step: &Expr) -> String {
        match step {
            Expr::UnaryOp {
                op: UnaryOp::Neg,
                expr,
            } => self.gen_expr(expr),
            Expr::Literal(Literal::Int(value)) => {
                value.strip_prefix('-').unwrap_or(value).to_string()
            }
            other => self.gen_expr(other),
        }
    }

    /// Render `for variable in iterable { body }` with the loop variable
    /// carrying the element type of the iterable.
    fn gen_for_loop(&self, variable: &str, iterable: &Expr, body: &[Statement]) -> String {
        let mut result = String::new();
        // Iterate by value (`iter().cloned()`) so `for x in collection` binds
        // the element type `T` (matching the checker) while still borrowing
        // the collection instead of moving it (so it stays usable after the
        // loop). Plain ranges such as `0..10` do not support `.iter()`, so
        // leave them untouched. The logic mirrors the collection-loop
        // lowering in `gen_loop_range`: strings iterate via `.chars()`, and
        // temporaries (array literals, method results) are consumed directly.
        let iterator = if self.is_string_expr(iterable) {
            format!("{}.chars()", self.gen_expr(iterable))
        } else {
            match iterable {
                Expr::Range { .. } => self.gen_expr(iterable),
                Expr::MethodCall {
                    object,
                    method,
                    args,
                    ..
                } => {
                    let object_str = self.gen_expr(object);
                    if method == "enumerate" && args.is_empty() {
                        if self.is_string_expr(object) {
                            format!("{}.chars().enumerate()", object_str)
                        } else if self.is_iterator_expr(object) {
                            format!("{}.enumerate()", object_str)
                        } else {
                            format!("{}.iter().cloned().enumerate()", object_str)
                        }
                    } else {
                        // Method results (map/filter/split/...) are owned
                        // temporaries and can be consumed directly.
                        self.gen_expr(iterable)
                    }
                }
                Expr::Identifier(_) => format!("{}.iter().cloned()", self.gen_expr(iterable)),
                Expr::Array(_) => self.gen_expr(iterable),
                _ => format!("[{}]", self.gen_expr(iterable)),
            }
        };
        result.push_str(&format!("for {} in {} {{\n", variable, iterator));
        self.enter_scope();
        for statement in body {
            result.push_str(&format!("    {}\n", self.gen_statement_str(statement)));
        }
        self.exit_scope();
        result.push('}');
        result
    }

    fn gen_loop_range(
        &self,
        variable: &str,
        ranges: &[LoopRangePart],
        body: &[Statement],
    ) -> String {
        let mut result = String::with_capacity(128);
        if ranges.len() == 1 {
            match &ranges[0] {
                LoopRangePart::Range {
                    start,
                    end,
                    inclusive: _,
                    step,
                } => {
                    let s = self.gen_expr(start);
                    let e = self.gen_expr(end);
                    // Poly loop ranges include both endpoints, unlike ordinary
                    // Poly/Rust range expressions.
                    let range = format!("{}..={}", s, e);
                    let iterator = if let Some(step) = step {
                        let is_negative = matches!(
                            step,
                            Expr::UnaryOp {
                                op: UnaryOp::Neg,
                                ..
                            }
                        ) || matches!(step, Expr::Literal(Literal::Int(value)) if value.starts_with('-'));
                        if is_negative {
                            format!(
                                "({}..={}).rev().step_by({} as usize)",
                                e,
                                s,
                                self.gen_step_magnitude(step)
                            )
                        } else {
                            format!("({}).step_by({} as usize)", range, self.gen_expr(step))
                        }
                    } else {
                        range
                    };
                    use std::fmt::Write;
                    writeln!(result, "for {} in {} {{", variable, iterator).unwrap();
                }
                LoopRangePart::Value(value) => {
                    // Collection loops iterate by value (`iter().cloned()`) so
                    // the loop variable has the element type `T` (matching the
                    // checker) while the collection stays usable afterwards:
                    // `loop: item in items` lowers to
                    // `for item in items.iter().cloned()`, and
                    // `loop: (a, b) in items.enumerate()` borrows the receiver
                    // too. Strings iterate via `.chars()`, and temporaries
                    // (array literals, method results such as
                    // `map`/`filter`/`split`) can be consumed directly.
                    let iterator = if self.is_string_expr(value) {
                        format!("{}.chars()", self.gen_expr(value))
                    } else if let Expr::MethodCall {
                        object,
                        method,
                        args,
                        ..
                    } = value.as_ref()
                    {
                        let object_str = self.gen_expr(object);
                        if method == "enumerate" && args.is_empty() {
                            if self.is_string_expr(object) {
                                format!("{}.chars().enumerate()", object_str)
                            } else if self.is_iterator_expr(object) {
                                format!("{}.enumerate()", object_str)
                            } else {
                                format!("{}.iter().cloned().enumerate()", object_str)
                            }
                        } else {
                            self.gen_expr(value)
                        }
                    } else if matches!(value.as_ref(), Expr::Identifier(_)) {
                        format!("{}.iter().cloned()", self.gen_expr(value))
                    } else if matches!(value.as_ref(), Expr::Array(_)) {
                        self.gen_expr(value)
                    } else {
                        format!("[{}]", self.gen_expr(value))
                    };
                    use std::fmt::Write;
                    writeln!(result, "for {} in {} {{", variable, iterator).unwrap();
                }
            }
        } else {
            let mut iterators = Vec::new();
            for part in ranges {
                match part {
                    LoopRangePart::Range {
                        start,
                        end,
                        inclusive: _,
                        step,
                    } => {
                        let s = self.gen_expr(start);
                        let e = self.gen_expr(end);
                        // Poly loop ranges include both endpoints.
                        let range = format!("{}..={}", s, e);
                        let iterator = if let Some(step) = step {
                            let is_negative = matches!(
                                step,
                                Expr::UnaryOp {
                                    op: UnaryOp::Neg,
                                    ..
                                }
                            ) || matches!(
                                step,
                                Expr::Literal(Literal::Int(value)) if value.starts_with('-')
                            );
                            if is_negative {
                                format!(
                                    "({}..={}).rev().step_by({} as usize)",
                                    e,
                                    s,
                                    self.gen_step_magnitude(step)
                                )
                            } else {
                                format!("({}).step_by({} as usize)", range, self.gen_expr(step))
                            }
                        } else {
                            format!("({})", range)
                        };
                        iterators.push(iterator);
                    }
                    LoopRangePart::Value(value) => {
                        iterators.push(format!("std::iter::once({})", self.gen_expr(value)));
                    }
                }
            }
            let chained = iterators
                .into_iter()
                .reduce(|left, right| format!("{}.chain({})", left, right))
                .unwrap_or_else(|| "std::iter::empty()".to_string());
            use std::fmt::Write;
            writeln!(result, "for {} in {} {{", variable, chained).unwrap();
        }
        self.enter_scope();
        for statement in body {
            use std::fmt::Write;
            writeln!(result, "    {}", self.gen_statement_str(statement)).unwrap();
        }
        self.exit_scope();
        result.push('}');
        result
    }

    /// Apply a `--as <type>` conversion to the input variable, or return the
    /// trimmed input unchanged when no conversion was requested.
    ///
    /// `read_line` keeps the trailing newline; `str::parse` rejects
    /// surrounding whitespace, so the input is always trimmed first.
    fn gen_get_as(&self, input_var: &str, as_type: Option<&Type>) -> String {
        let trimmed = format!("{input_var}.trim()");
        let Some(ty) = as_type else {
            return format!("{trimmed}.to_string()");
        };
        if matches!(ty, Type::Named(name) if matches!(name.as_str(), "string" | "ustring" | "String"))
        {
            format!("{trimmed}.to_string()")
        } else {
            let rust_ty = self.gen_type(ty);
            format!("{trimmed}.parse::<{rust_ty}>().unwrap_or_default()")
        }
    }

    /// Generate a real HTTP/1.1 GET over a tokio TCP connection.
    ///
    /// The URL is parsed into host/port/path, a raw request is written, and
    /// the body after the header/body separator is returned as
    /// `Ok(body)`. Connection, write, and read failures surface as `Err`.
    /// The whole block is an `async` block so `.await` works on the result.
    fn gen_http_get(&self, url: &str) -> String {
        format!(
            "async {{\n\tlet __poly_url = {};\n\tlet __poly_rest = __poly_url.strip_prefix(\"http://\").or_else(|| __poly_url.strip_prefix(\"https://\")).unwrap_or(&__poly_url);\n\tlet (__poly_host, __poly_path) = match __poly_rest.find('/') {{\n\t\tSome(i) => (&__poly_rest[..i], &__poly_rest[i..]),\n\t\tNone => (__poly_rest, \"/\"),\n\t}};\n\tlet (__poly_hostname, __poly_port) = match __poly_host.rsplit_once(':') {{\n\t\tSome((h, p)) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => (h, p.parse::<u16>().unwrap_or(80)),\n\t\t_ => (__poly_host, 80u16),\n\t}};\n\tlet mut __poly_stream = match tokio::net::TcpStream::connect((__poly_hostname, __poly_port)).await {{\n\t\tOk(stream) => stream,\n\t\tErr(e) => return Err::<String, String>(format!(\"connection failed: {{}}\", e)),\n\t}};\n\tlet __poly_request = format!(\"GET {{}} HTTP/1.1\\r\\nHost: {{}}\\r\\nConnection: close\\r\\n\\r\\n\", __poly_path, __poly_hostname);\n\tif let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut __poly_stream, __poly_request.as_bytes()).await {{\n\t\treturn Err::<String, String>(format!(\"write failed: {{}}\", e));\n\t}}\n\tlet mut __poly_response = Vec::new();\n\tif let Err(e) = tokio::io::AsyncReadExt::read_to_end(&mut __poly_stream, &mut __poly_response).await {{\n\t\treturn Err::<String, String>(format!(\"read failed: {{}}\", e));\n\t}}\n\tlet __poly_text = String::from_utf8_lossy(&__poly_response);\n\tmatch __poly_text.split_once(\"\\r\\n\\r\\n\") {{\n\t\tSome((_, body)) => Ok::<String, String>(body.to_string()),\n\t\tNone => Ok::<String, String>(__poly_text.to_string()),\n\t}}\n}}",
            url
        )
    }

    /// Execute a SQL query against a shared in-memory SQLite database and
    /// return each row as a `|`-joined string of its `{:?}` values.
    ///
    /// The connection lives in a module-level `OnceLock` (emitted once when
    /// the first `db_execute` is seen) so `CREATE TABLE` / `INSERT` /
    /// `SELECT` calls within one program share the same database. Failures
    /// panic with a descriptive message (like the other I/O builtins).
    /// Requires the `rusqlite` crate in the generated project.
    fn gen_db_execute(&self, query: &str) -> String {
        *self.needs_rusqlite.borrow_mut() = true;
        format!(
            "async {{\n\tlet __poly_conn = __POLY_DB.get_or_init(|| std::sync::Mutex::new(rusqlite::Connection::open_in_memory().expect(\"db_execute: failed to open in-memory database\")));\n\tlet __poly_output: Vec<String> = {{\n\t\tlet __poly_guard = __poly_conn.lock().expect(\"db_execute: database lock poisoned\");\n\t\tlet __poly_query = {};\n\t\tlet mut __poly_stmt = __poly_guard.prepare(__poly_query.as_str()).expect(\"db_execute: failed to prepare query\");\n\t\tlet __poly_cols = __poly_stmt.column_count();\n\t\tlet __poly_result = __poly_stmt.query_map([], |row| {{ \n\t\t\tlet mut __poly_cells: Vec<String> = Vec::new();\n\t\t\tfor __poly_i in 0..__poly_cols {{ \n\t\t\t\tlet __poly_value: rusqlite::types::Value = row.get(__poly_i).unwrap_or(rusqlite::types::Value::Null);\n\t\t\t\t__poly_cells.push(format!(\"{{:?}}\", __poly_value));\n\t\t\t}}\n\t\t\tOk(__poly_cells.join(\" | \"))\n\t\t}});\n\t\tmatch __poly_result {{ \n\t\t\tOk(rows) => rows.filter_map(Result::ok).collect::<Vec<String>>(),\n\t\t\tErr(e) => panic!(\"db_execute: query failed: {{}}\", e),\n\t\t}}\n\t}};\n\t__poly_output\n}}",
            query
        )
    }

    /// Establish a real TCP connection to `host:port` and return the peer
    /// address on success, or an `Err` describing the failure. The whole
    /// block is an `async` block so `.await` works on the result.
    fn gen_tcp_connect(&self, address: &str) -> String {
        format!(
            "async {{\n\tlet __poly_address = {};\n\tmatch tokio::net::TcpStream::connect(__poly_address).await {{\n\t\tOk(stream) => Ok::<String, String>(stream.peer_addr().map(|a| a.to_string()).unwrap_or_default()),\n\t\tErr(e) => Err::<String, String>(format!(\"connection failed: {{}}\", e)),\n\t}}\n}}",
            address
        )
    }

    fn gen_get(&self, get: &GetExpr) -> String {
        // File reads and stdin reads share one intermediate representation node. Check redirection first
        // because file-specific byte flags change the generated return type.
        if let Some(source) = &get.source {
            let path_str = self.gen_expr(source);
            for flag in &get.flags {
                if let GetFlag::Bytes(count) = flag {
                    let count_str = self.gen_expr(count);
                    return format!(
                        "{{ let bytes = std::fs::read({}).unwrap(); bytes.into_iter().take({} as usize).collect::<Vec<u8>>() }}",
                        path_str, count_str
                    );
                }
            }
            return format!("std::fs::read_to_string({}).unwrap()", path_str);
        }

        // No source: optionally display the explicit Unicode prompt, then read stdin.
        let prompt = get
            .prompt
            .as_ref()
            .map(|prompt| {
                format!(
                    "print!(\"{{}}\", {}); std::io::Write::flush(&mut std::io::stdout()).unwrap(); ",
                    self.gen_expr(prompt)
                )
            })
            .unwrap_or_default();
        let timeout = get.flags.iter().find_map(|flag| match flag {
            GetFlag::Timeout(value) => Some(self.gen_expr(value)),
            _ => None,
        });
        let default = get.flags.iter().find_map(|flag| match flag {
            GetFlag::Default(value) => Some(self.gen_expr(value)),
            _ => None,
        });
        let bytes = get.flags.iter().find_map(|flag| match flag {
            GetFlag::Bytes(value) => Some(self.gen_expr(value)),
            _ => None,
        });
        let as_type = get.flags.iter().find_map(|flag| match flag {
            GetFlag::As(ty) => Some(ty),
            _ => None,
        });
        let until = get.flags.iter().find_map(|flag| match flag {
            GetFlag::Until(value) => Some(self.gen_expr(value)),
            _ => None,
        });
        let mask = get.flags.iter().find_map(|flag| match flag {
            GetFlag::Mask(value) => Some(self.gen_expr(value)),
            _ => None,
        });

        // `get --bytes N` on stdin reads up to N raw bytes.
        if let Some(count) = bytes {
            return format!(
                "{{ {}let mut __poly_bytes: Vec<u8> = Vec::new(); let _ = std::io::Read::read_to_end(&mut std::io::Read::take(std::io::stdin(), {} as u64), &mut __poly_bytes).unwrap(); __poly_bytes }}",
                prompt, count
            );
        }

        // The stdin read expression: `--until` accumulates bytes until the
        // delimiter is seen (the delimiter is stripped from the result);
        // `--mask` suppresses terminal echo while reading (falls back to a
        // plain read on non-Unix platforms and non-terminals).
        let read_expr = self.gen_stdin_read_expr(until.as_deref(), mask.as_deref());

        // `get --timeout N` reads stdin on a background thread so a blocked
        // read can actually time out; `--default` supplies the value used on
        // timeout, otherwise the read returns `Err("timeout")` which matches
        // the documented `Timeout` arm.
        if let Some(ms) = timeout {
            let on_ok = self.gen_get_as("input", as_type);
            let timeout_result = match default {
                Some(default_expr) => {
                    format!("Ok::<String, String>({})", default_expr)
                }
                None => "Err::<String, String>(String::from(\"timeout\"))".to_string(),
            };
            return format!(
                "{{ {}let (__poly_tx, __poly_rx) = std::sync::mpsc::channel(); std::thread::spawn(move || {{ let __poly_input = {}; let _ = __poly_tx.send(__poly_input.trim().to_string()); }}); match __poly_rx.recv_timeout(std::time::Duration::from_millis({} as u64)) {{ Ok(input) => Ok::<String, String>({}), Err(_) => {} }} }}",
                prompt, read_expr, ms, on_ok, timeout_result
            );
        }

        let finalized = self.gen_get_as("input", as_type);
        format!("{{ {}let input = {}; {} }}", prompt, read_expr, finalized)
    }

    /// Generate the stdin read expression, honoring `--until` (read until a
    /// delimiter, which is stripped from the result) and `--mask` (suppress
    /// terminal echo on Unix; plain read elsewhere).
    fn gen_stdin_read_expr(&self, until: Option<&str>, mask: Option<&str>) -> String {
        if let Some(delimiter) = until {
            format!(
                "{{ let mut __poly_buf: Vec<u8> = Vec::new(); let mut __poly_byte = [0u8; 1]; let __poly_delim = {}; loop {{ match std::io::Read::read(&mut std::io::stdin(), &mut __poly_byte) {{ Ok(0) | Err(_) => break, Ok(_) => {{ __poly_buf.push(__poly_byte[0]); if String::from_utf8_lossy(&__poly_buf).ends_with(__poly_delim.as_str()) {{ break; }} }} }} }} String::from_utf8_lossy(&__poly_buf).trim_end_matches(__poly_delim.as_str()).to_string() }}",
                delimiter
            )
        } else if mask.is_some() {
            // Best-effort echo suppression via `stty` on Unix terminals; on
            // other platforms (or non-terminal stdin) the command fails
            // silently and the read behaves like plain input.
            "{{ let __poly_tty = std::io::IsTerminal::is_terminal(&std::io::stdin()); if __poly_tty {{ let _ = std::process::Command::new(\"stty\").arg(\"-echo\").status(); }} let mut __poly_input = String::new(); let _ = std::io::stdin().read_line(&mut __poly_input); if __poly_tty {{ let _ = std::process::Command::new(\"stty\").arg(\"echo\").status(); }} __poly_input }}"
                .to_string()
        } else {
            "{ let mut __poly_input = String::new(); std::io::stdin().read_line(&mut __poly_input).unwrap(); __poly_input }"
                .to_string()
        }
    }

    /// Render a nested function declaration as a string for statement-string
    /// contexts (e.g. inside match arms and if-expression blocks).
    fn gen_function_str(&self, function: &Function) -> String {
        let generics = self.gen_generics(&function.generics);
        let params: Vec<String> = function
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, self.gen_type(&p.ty)))
            .collect();
        let ret = match &function.return_type {
            Some(Type::Function { params, ret }) => {
                let rendered: Vec<String> = params.iter().map(|t| self.gen_type(t)).collect();
                format!(
                    " -> impl Fn({}) -> {}",
                    rendered.join(", "),
                    self.gen_type(ret)
                )
            }
            Some(ty) => format!(" -> {}", self.gen_type(ty)),
            None => String::new(),
        };
        let async_prefix = if function.is_async { "async " } else { "" };
        format!(
            "{}fn {}{}({}){} {{\n    /* nested function body */\n}}",
            async_prefix,
            function.name,
            generics,
            params.join(", "),
            ret
        )
    }

    fn gen_statement_str(&self, statement: &Statement) -> String {
        match statement {
            Statement::Expression(expr) => {
                let generated = self.gen_expr(expr);
                match expr {
                    Expr::If { .. }
                    | Expr::LoopRange { .. }
                    | Expr::ForLoop { .. }
                    | Expr::Match { .. } => generated,
                    _ => format!("{};", generated),
                }
            }
            Statement::Return(Some(expr)) => {
                *self.closure_move.borrow_mut() = true;
                let rendered = self.gen_expr(expr);
                *self.closure_move.borrow_mut() = false;
                format!("return {};", rendered)
            }
            Statement::Return(None) => "return;".to_string(),
            Statement::Break => "break;".to_string(),
            Statement::Continue => "continue;".to_string(),
            Statement::Put { expr, redirect } => {
                // Legacy `put x > "file"` / `put x >> "file"` handling was
                // removed: redirects are now only `put ... to "file"` (the
                // Redirect field).
                let expr_str = self.gen_expr(expr);
                match redirect {
                    Some(Redirect::Write(path)) => {
                        let path_str = self.gen_expr(path);
                        if self.is_bytes_expr(expr) {
                            format!("std::fs::write({}, {}).unwrap();", path_str, expr_str)
                        } else {
                            format!(
                                "std::fs::write({}, format!(\"{}\\n\", {})).unwrap();",
                                path_str,
                                self.put_format_spec(expr),
                                expr_str
                            )
                        }
                    }
                    Some(Redirect::Append(path)) => {
                        let path_str = self.gen_expr(path);
                        *self.needs_io_write.borrow_mut() = true;
                        format!(
                            "{{ let mut f = std::fs::OpenOptions::new().append(true).create(true).open({0}).unwrap(); writeln!(f, \"{1}\", {2}).unwrap(); }}",
                            path_str, self.put_format_spec(expr), expr_str
                        )
                    }
                    None => {
                        format!(
                            "println!(\"{}\", {});",
                            self.put_format_spec(expr),
                            expr_str
                        )
                    }
                }
            }
            Statement::Error(expr) => {
                format!("eprintln!(\"[ERROR] {{}}\", {});", self.gen_expr(expr))
            }
            Statement::Warn(expr) => {
                format!("eprintln!(\"[WARN] {{}}\", {});", self.gen_expr(expr))
            }
            Statement::Info(expr) => {
                format!("eprintln!(\"[INFO] {{}}\", {});", self.gen_expr(expr))
            }
            Statement::LetDecl { name, ty, value } => {
                let ty_str = ty.as_ref().map(|ty| self.gen_type(ty)).unwrap_or_default();
                let value_str = self.gen_decl_value(name, value, ty.as_ref());
                if ty.is_some() {
                    format!("let {}: {} = {};", name, ty_str, value_str)
                } else {
                    format!("let {} = {};", name, value_str)
                }
            }
            Statement::VarDecl { name, ty, value } => {
                // Register the name's string/vec/type kind so later statements
                // in the same block (`+=` borrow, `{:?}` put formatting, field
                // access) lower correctly. Mirrors `gen_statement`.
                if let Some(type_name) = ty.as_ref().and_then(named_type_name) {
                    self.declare_type(name.clone(), type_name);
                } else if let Some(Expr::Struct {
                    name: type_name, ..
                }) = value
                {
                    self.declare_type(name.clone(), type_name.clone());
                }
                let value_is_string = ty.is_none()
                    && value
                        .as_ref()
                        .is_some_and(|value| self.is_string_expr(value));
                if ty.as_ref().is_some_and(Self::is_string_type) || value_is_string {
                    self.declare_string(name.clone());
                }
                if ty.as_ref().is_some_and(Self::is_vec_type)
                    || value.as_ref().is_some_and(|value| self.is_vec_expr(value))
                {
                    self.declare_vec(name.clone());
                }
                let element_type = ty
                    .as_ref()
                    .and_then(|ty| self.vec_element_type_of_type(ty))
                    .or_else(|| {
                        value
                            .as_ref()
                            .and_then(|value| self.array_element_type(value))
                    });
                if let Some(element) = element_type {
                    self.declare_vec_element(name.clone(), element);
                }
                let ty_str = ty.as_ref().map(|ty| self.gen_type(ty)).unwrap_or_default();
                match value {
                    Some(value) => {
                        let value_str = self.gen_decl_value(name, value, ty.as_ref());
                        if ty.is_some() {
                            format!("let mut {}: {} = {};", name, ty_str, value_str)
                        } else {
                            format!("let mut {} = {};", name, value_str)
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
            Statement::Assignment { target, value } => {
                format!("{} = {};", self.gen_expr(target), self.gen_expr(value))
            }
            Statement::Mutation { target, op, value } => {
                let operator = match op {
                    MutationOp::Add | MutationOp::Inc => "+=",
                    MutationOp::Sub | MutationOp::Dec => "-=",
                };
                let amount = value
                    .as_ref()
                    .map(|value| self.gen_expr(value))
                    .unwrap_or_else(|| "1".to_string());
                // String append lowers to `String += &str`; the amount must be
                // borrowed so `+= String::from(...)` compiles. Numeric targets
                // keep the plain amount (loop variables are already `&T`).
                let amount = if matches!(op, MutationOp::Add | MutationOp::Inc)
                    && self.is_string_expr(target)
                {
                    format!("&({})", amount)
                } else {
                    amount
                };
                format!("{} {} {};", self.gen_expr(target), operator, amount)
            }
            Statement::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                let cond = self.gen_expr(condition);
                let mut result = format!("if {} {{\n", cond);
                self.enter_scope();
                for (index, statement) in then_block.iter().enumerate() {
                    let generated = self.gen_statement_str(statement);
                    let generated = if index + 1 == then_block.len()
                        && !matches!(
                            statement,
                            Statement::Expression(Expr::MethodCall { method, .. })
                                if method == "await"
                        )
                        && matches!(statement, Statement::Expression(_))
                    {
                        generated.trim_end_matches(';').to_string()
                    } else {
                        generated
                    };
                    result.push_str(&format!("    {}\n", generated));
                }
                self.exit_scope();
                match else_block {
                    Some(else_block) if !else_block.is_empty() => {
                        result.push_str("} else {\n");
                        self.enter_scope();
                        for (index, statement) in else_block.iter().enumerate() {
                            let generated = self.gen_statement_str(statement);
                            let generated = if index + 1 == else_block.len()
                                && !matches!(
                                    statement,
                                    Statement::Expression(Expr::MethodCall { method, .. })
                                        if method == "await"
                                )
                                && matches!(statement, Statement::Expression(_))
                            {
                                generated.trim_end_matches(';').to_string()
                            } else {
                                generated
                            };
                            result.push_str(&format!("    {}\n", generated));
                        }
                        self.exit_scope();
                        result.push_str("}\n");
                    }
                    _ => result.push_str("}\n"),
                }
                result
            }
            Statement::Match { scrutinee, arms } => {
                let scrutinee_str = self.gen_expr(scrutinee);
                let scrutinee_str = if self.is_string_expr(scrutinee) {
                    format!("({}).as_str()", scrutinee_str)
                } else {
                    scrutinee_str
                };
                let mut result = format!("match {} {{\n", scrutinee_str);
                for arm in arms {
                    let (pattern_str, pattern_guards) = self.gen_pattern_with_guards(&arm.pattern);
                    let mut boxed_bindings = HashSet::new();
                    self.collect_boxed_pattern_bindings(&arm.pattern, &mut boxed_bindings);
                    self.boxed_bindings.borrow_mut().push(boxed_bindings);
                    let mut aliases = HashMap::new();
                    self.collect_pattern_aliases(&arm.pattern, &mut aliases);
                    self.pattern_aliases.borrow_mut().push(aliases);
                    let body_str = match &arm.body {
                        MatchArmBody::Expression(expr) => self.gen_expr(expr),
                        MatchArmBody::Block(statements) => {
                            let mut block = String::from("{\n");
                            self.enter_scope();
                            for (index, statement) in statements.iter().enumerate() {
                                let generated = self.gen_statement_str(statement);
                                let generated = if index + 1 == statements.len()
                                    && matches!(statement, Statement::Expression(_))
                                {
                                    generated.trim_end_matches(';').to_string()
                                } else {
                                    generated
                                };
                                block.push_str(&format!("        {}\n", generated));
                            }
                            self.exit_scope();
                            block.push_str("    }");
                            block
                        }
                    };
                    let body_str = body_str.trim_end_matches(';').to_string();
                    let mut guards = pattern_guards;
                    if let Some(guard) = &arm.guard {
                        guards.push(self.gen_expr(guard));
                    }
                    if guards.is_empty() {
                        result.push_str(&format!("    {} => {},\n", pattern_str, body_str));
                    } else {
                        result.push_str(&format!(
                            "    {} if {} => {},\n",
                            pattern_str,
                            guards.join(" && "),
                            body_str
                        ));
                    }
                    self.pattern_aliases.borrow_mut().pop();
                    self.boxed_bindings.borrow_mut().pop();
                }
                result.push('}');
                result
            }
            Statement::Block(statements) => {
                let mut result = String::from("{\n");
                self.enter_scope();
                for statement in statements {
                    result.push_str(&format!("    {}\n", self.gen_statement_str(statement)));
                }
                self.exit_scope();
                result.push('}');
                result
            }
            Statement::NestedFunction(function) => self.gen_function_str(function),
            Statement::ForeignBlock { language, content } if language == "rust" => {
                let mut result = String::new();
                for line in content.lines() {
                    result.push_str(line);
                    result.push('\n');
                }
                result
            }
            Statement::ForeignBlock { .. } => String::new(),
        }
    }

    /// Generate a declaration initializer, forcing a closure binding to own
    /// its captures when that binding is returned from the current function.
    fn gen_decl_value(&self, name: &str, value: &Expr, declared_type: Option<&Type>) -> String {
        let should_move = is_closure_expr(value)
            && self
                .returned_closure_names
                .borrow()
                .last()
                .is_some_and(|names| names.contains(name));
        if should_move {
            let previous = *self.closure_move.borrow();
            *self.closure_move.borrow_mut() = true;
            let rendered = self.gen_value_expr(value, declared_type);
            *self.closure_move.borrow_mut() = previous;
            rendered
        } else {
            self.gen_value_expr(value, declared_type)
        }
    }

    fn gen_value_expr(&self, value: &Expr, declared_type: Option<&Type>) -> String {
        if let Expr::Get(get) = value {
            if let Some(Type::Named(name)) = declared_type {
                if name == "bytes" {
                    if let Some(source) = &get.source {
                        let path = self.gen_expr(source);
                        return format!("std::fs::read({}).unwrap()", path);
                    }
                }
            }
            let requested_type = get.flags.iter().find_map(|flag| match flag {
                GetFlag::As(ty) => Some(ty),
                _ => None,
            });
            // An explicit `--as` is already applied inside `gen_get`; only
            // infer a parse from the declared type when no flag was given.
            if requested_type.is_none() {
                if let Some(ty) = declared_type {
                    let rust_type = self.gen_type(ty);
                    if rust_type != "String" && rust_type != "Vec<u8>" {
                        return format!(
                            "({}).parse::<{}>().expect(\"invalid input\")",
                            self.gen_expr(value),
                            rust_type
                        );
                    }
                }
            }
        }
        // An empty `[]` initializer for a Map/Set declaration must produce a
        // HashMap/HashSet rather than the Vec the array literal would emit.
        if let Expr::Array(elements) = value {
            if elements.is_empty() {
                if let Some(Type::Generic { name, .. }) = declared_type {
                    match name.as_str() {
                        "Map" => return "std::collections::HashMap::new()".to_string(),
                        "Set" => return "std::collections::HashSet::new()".to_string(),
                        _ => {}
                    }
                }
            }
        }
        self.gen_expr(value)
    }

    fn gen_pattern(&self, pattern: &Pattern) -> String {
        match pattern {
            Pattern::Wildcard => "_".to_string(),
            Pattern::Literal(expr) => match expr {
                Expr::Literal(Literal::String(value))
                | Expr::Literal(Literal::UnicodeString(value)) => {
                    format!("{:?}", value)
                }
                _ => self.gen_expr(expr),
            },
            Pattern::Identifier(name) => match name.as_str() {
                "Error" | "Timeout" => "Err(_)".to_string(),
                _ => self
                    .enum_variants
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.clone()),
            },
            Pattern::Tuple(patterns) => {
                let inner: Vec<String> = patterns.iter().map(|p| self.gen_pattern(p)).collect();
                format!("({})", inner.join(", "))
            }
            Pattern::Enum {
                enum_name,
                variant,
                inner,
            } => {
                let qualified = if enum_name.is_empty() {
                    match self.enum_variants.get(variant) {
                        Some(qualified) => qualified.clone(),
                        None if matches!(variant.as_str(), "Error" | "Timeout") => {
                            "Err".to_string()
                        }
                        None => variant.clone(),
                    }
                } else {
                    format!("{}::{}", enum_name, variant)
                };
                match inner {
                    Some(inner) => {
                        let inner: Vec<String> =
                            inner.iter().map(|p| self.gen_pattern(p)).collect();
                        // Resolve the declaring enum for struct-field variants
                        // even when the pattern writes the variant
                        // unqualified, e.g. `Error(TooShort(min))`.
                        let resolved_enum = if enum_name.is_empty() {
                            self.enum_variants.get(variant).and_then(|qualified| {
                                qualified.split_once("::").map(|(name, _)| name.to_string())
                            })
                        } else {
                            Some(enum_name.clone())
                        };
                        if let Some(fields) = resolved_enum.as_ref().and_then(|name| {
                            self.enum_struct_fields
                                .get(&(name.clone(), variant.clone()))
                        }) {
                            let rendered = fields
                                .iter()
                                .zip(inner.iter())
                                .map(|(field, pattern)| format!("{}: {}", field, pattern))
                                .collect::<Vec<_>>();
                            format!("{} {{ {} }}", qualified, rendered.join(", "))
                        } else {
                            format!("{}({})", qualified, inner.join(", "))
                        }
                    }
                    None => qualified,
                }
            }
            Pattern::NamedFields { name, fields } => {
                let rendered: Vec<String> = fields
                    .iter()
                    .map(|(field, pattern)| format!("{}: {}", field, self.gen_pattern(pattern)))
                    .collect();
                format!("{} {{ {} }}", name, rendered.join(", "))
            }
            Pattern::Range {
                start,
                end,
                inclusive,
            } => {
                let s = self.gen_expr(start);
                let e = self.gen_expr(end);
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

    /// Generate a pattern, rewriting nested patterns on boxed recursive fields
    /// into fresh bindings plus `matches!` guards (Rust cannot pattern-match a
    /// `Box<Expr>` against `Expr::Num(0)` directly).
    fn gen_pattern_with_guards(&self, pattern: &Pattern) -> (String, Vec<String>) {
        let Pattern::Enum {
            enum_name,
            variant,
            inner: Some(inner),
        } = pattern
        else {
            return (self.gen_pattern(pattern), Vec::new());
        };

        let qualified = if enum_name.is_empty() {
            match self.enum_variants.get(variant) {
                Some(qualified) => qualified.clone(),
                None if matches!(variant.as_str(), "Error" | "Timeout") => "Err".to_string(),
                None => variant.clone(),
            }
        } else {
            format!("{}::{}", enum_name, variant)
        };
        let resolved_enum = if enum_name.is_empty() {
            qualified.split_once("::").map(|(name, _)| name.to_string())
        } else {
            Some(enum_name.clone())
        };
        let mut patterns = Vec::new();
        let mut guards = Vec::new();
        for (index, child) in inner.iter().enumerate() {
            let recursive = resolved_enum.as_ref().is_some_and(|name| {
                self.recursive_enum_fields
                    .contains(&(name.clone(), variant.clone(), index))
            });
            if recursive
                && matches!(
                    child,
                    Pattern::Enum { .. } | Pattern::Literal(_) | Pattern::NamedFields { .. }
                )
            {
                let binding = format!("__poly_box_{}", index);
                patterns.push(binding.clone());
                guards.push(format!(
                    "matches!(&*{}, {})",
                    binding,
                    self.gen_guard_pattern(child)
                ));
            } else {
                patterns.push(self.gen_pattern(child));
            }
        }
        (format!("{}({})", qualified, patterns.join(", ")), guards)
    }

    /// Render a pattern for use inside a `matches!` guard on a boxed field.
    fn gen_guard_pattern(&self, pattern: &Pattern) -> String {
        match pattern {
            Pattern::Identifier(_) | Pattern::Wildcard => "_".to_string(),
            Pattern::Literal(Expr::Literal(Literal::Int(value)))
                if value == "0" || value == "1" =>
            {
                format!("{}.0", value)
            }
            Pattern::Literal(_) => self.gen_pattern(pattern),
            Pattern::Tuple(patterns) => format!(
                "({})",
                patterns
                    .iter()
                    .map(|pattern| self.gen_guard_pattern(pattern))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Pattern::Enum {
                enum_name,
                variant,
                inner,
            } => {
                let qualified = if enum_name.is_empty() {
                    self.enum_variants
                        .get(variant)
                        .cloned()
                        .unwrap_or_else(|| variant.clone())
                } else {
                    format!("{}::{}", enum_name, variant)
                };
                let fields = inner
                    .as_ref()
                    .map(|patterns| {
                        patterns
                            .iter()
                            .map(|pattern| self.gen_guard_pattern(pattern))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                if inner.is_some() {
                    format!("{}({})", qualified, fields)
                } else {
                    qualified
                }
            }
            _ => self.gen_pattern(pattern),
        }
    }

    /// Collect names that bind to boxed recursive enum fields so references to
    /// them can be dereferenced and cloned in generated bodies.
    fn collect_boxed_pattern_bindings(&self, pattern: &Pattern, bindings: &mut HashSet<String>) {
        match pattern {
            Pattern::Enum {
                enum_name,
                variant,
                inner: Some(inner),
            } => {
                let qualified = if enum_name.is_empty() {
                    self.enum_variants.get(variant).cloned()
                } else {
                    Some(format!("{}::{}", enum_name, variant))
                };
                let resolved_enum = qualified
                    .as_deref()
                    .and_then(|name| name.split_once("::").map(|(name, _)| name.to_string()));
                for (index, child) in inner.iter().enumerate() {
                    if let Some(enum_name) = &resolved_enum {
                        if self.recursive_enum_fields.contains(&(
                            enum_name.clone(),
                            variant.clone(),
                            index,
                        )) {
                            if let Pattern::Identifier(name) = child {
                                bindings.insert(name.clone());
                            }
                        }
                    }
                    self.collect_boxed_pattern_bindings(child, bindings);
                }
            }
            Pattern::Tuple(patterns) => {
                for pattern in patterns {
                    self.collect_boxed_pattern_bindings(pattern, bindings);
                }
            }
            Pattern::Binding { pattern, .. } => {
                self.collect_boxed_pattern_bindings(pattern, bindings);
            }
            Pattern::NamedFields { fields, .. } => {
                for (_, pattern) in fields {
                    self.collect_boxed_pattern_bindings(pattern, bindings);
                }
            }
            _ => {}
        }
    }

    /// For identifiers nested inside boxed recursive patterns, build owned
    /// expressions that reach through the binding (`match &*binding { ... }`).
    fn collect_pattern_aliases(&self, pattern: &Pattern, aliases: &mut HashMap<String, String>) {
        let Pattern::Enum {
            enum_name,
            variant,
            inner: Some(inner),
        } = pattern
        else {
            return;
        };
        let qualified = if enum_name.is_empty() {
            self.enum_variants.get(variant).cloned()
        } else {
            Some(format!("{}::{}", enum_name, variant))
        };
        let Some(qualified) = qualified else { return };
        let Some((root_enum, _)) = qualified.split_once("::") else {
            return;
        };
        for (index, child) in inner.iter().enumerate() {
            if !self.recursive_enum_fields.contains(&(
                root_enum.to_string(),
                variant.clone(),
                index,
            )) {
                continue;
            }
            let Pattern::Enum {
                enum_name: nested_enum,
                variant: nested_variant,
                inner: Some(nested_inner),
            } = child
            else {
                continue;
            };
            let nested_qualified = if nested_enum.is_empty() {
                self.enum_variants
                    .get(nested_variant)
                    .cloned()
                    .unwrap_or_else(|| nested_variant.clone())
            } else {
                format!("{}::{}", nested_enum, nested_variant)
            };
            let nested_enum_name = nested_qualified
                .split_once("::")
                .map(|(name, _)| name.to_string())
                .unwrap_or_default();
            for (nested_index, nested_pattern) in nested_inner.iter().enumerate() {
                let Pattern::Identifier(name) = nested_pattern else {
                    continue;
                };
                let binding = format!("__poly_box_{}", index);
                let value = if self.recursive_enum_fields.contains(&(
                    nested_enum_name.clone(),
                    nested_variant.clone(),
                    nested_index,
                )) {
                    "(**value).clone()".to_string()
                } else {
                    "value.clone()".to_string()
                };
                aliases.insert(
                    name.clone(),
                    format!(
                        "match &*{} {{ {}(value) => {}, _ => unreachable!() }}",
                        binding, nested_qualified, value
                    ),
                );
            }
        }
    }

    fn is_boxed_binding(&self, name: &str) -> bool {
        self.boxed_bindings
            .borrow()
            .iter()
            .rev()
            .any(|scope| scope.contains(name))
    }

    fn gen_type(&self, ty: &Type) -> String {
        match ty {
            Type::Named(name) => match name.as_str() {
                "ustring" | "string" => "String".to_string(),
                "uchar" => "char".to_string(),
                "byte" => "u8".to_string(),
                "bytes" => "Vec<u8>".to_string(),
                _ => name.clone(),
            },
            Type::Array(inner, size) => {
                format!("[{}; {}]", self.gen_type(inner), self.gen_expr(size))
            }
            Type::Tuple(types) => {
                let rendered: Vec<String> = types.iter().map(|t| self.gen_type(t)).collect();
                format!("({})", rendered.join(", "))
            }
            Type::Vec(inner) => format!("Vec<{}>", self.gen_type(inner)),
            Type::Option(inner) => format!("Option<{}>", self.gen_type(inner)),
            Type::Result(ok, err) => {
                format!("Result<{}, {}>", self.gen_type(ok), self.gen_type(err))
            }
            Type::Reference(mutable, inner) => {
                if *mutable {
                    format!("&mut {}", self.gen_type(inner))
                } else {
                    format!("&{}", self.gen_type(inner))
                }
            }
            Type::Pointer(inner) => format!("*const {}", self.gen_type(inner)),
            Type::Nullable(inner) => format!("Option<{}>", self.gen_type(inner)),
            Type::Function { params, ret } => {
                let rendered: Vec<String> = params.iter().map(|t| self.gen_type(t)).collect();
                format!("fn({}) -> {}", rendered.join(", "), self.gen_type(ret))
            }
            Type::Generic { name, args } => {
                let inner: Vec<String> = args.iter().map(|t| self.gen_type(t)).collect();
                let inner_join = inner.join(", ");
                match name.as_str() {
                    "Map" => format!("std::collections::HashMap<{}>", inner_join),
                    "Set" => format!("std::collections::HashSet<{}>", inner_join),
                    "Rc" => format!("std::rc::Rc<{}>", inner_join),
                    "Arc" => format!("std::sync::Arc<{}>", inner_join),
                    // `Box<T>` and user-defined generic types pass through.
                    _ => format!("{}<{}>", name, inner_join),
                }
            }
        }
    }

    /// Render the predicate for `filter`.
    ///
    /// `Iterator::filter` hands the closure a reference to each item, so a
    /// single-parameter closure literal is wrapped with a deref binding to
    /// keep the user's body (`|x| x % 2 == 0`) working on owned values. The
    /// parameter is left unannotated so Rust infers `&T` from the iterator.
    fn gen_filter_callback(&self, arg: &Expr) -> String {
        if let Expr::Closure { params, body } = arg {
            if params.len() == 1 {
                let parameter = &params[0];
                return format!(
                    "|{}| {{ let {} = *{}; {} }}",
                    parameter.name,
                    parameter.name,
                    parameter.name,
                    self.gen_expr(body)
                );
            }
        }
        self.gen_expr(arg)
    }

    /// Render the comparator for `sort_by`.
    ///
    /// The user's closure returns a `bool` (`|a, b| a < b`); it is wrapped so
    /// the references handed out by `sort_by` are deref'd and the boolean is
    /// translated into an `Ordering`. Parameters stay unannotated so Rust
    /// infers `&T` from the sort's signature.
    fn gen_sort_callback(&self, arg: &Expr) -> String {
        if let Expr::Closure { params, body } = arg {
            if params.len() == 2 {
                let first = &params[0].name;
                let second = &params[1].name;
                return format!(
                    "|{}, {}| {{ let {} = *{}; let {} = *{}; if {} {{ std::cmp::Ordering::Less }} else if ({} == {}) {{ std::cmp::Ordering::Equal }} else {{ std::cmp::Ordering::Greater }} }}",
                    first,
                    second,
                    first,
                    first,
                    second,
                    second,
                    self.gen_expr(body),
                    first,
                    second
                );
            }
        }
        self.gen_expr(arg)
    }

    fn gen_binary_op(&self, op: &BinaryOp) -> String {
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
        .to_string()
    }

    fn is_string_type(ty: &Type) -> bool {
        matches!(ty, Type::Named(name) if name == "string" || name == "ustring")
    }

    fn is_string_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Literal(Literal::String(_) | Literal::UnicodeString(_)) => true,
            Expr::Identifier(name) => self.is_string_name(name),
            Expr::Call { func, .. } => match func.as_ref() {
                Expr::Identifier(name) => self.string_functions.contains(name),
                _ => false,
            },
            Expr::MethodCall { object, method, .. } => {
                self.expression_type_name(object)
                    .map(|type_name| self.string_methods.contains(&(type_name, method.clone())))
                    .unwrap_or(false)
                    || matches!(method.as_str(), "get_line" | "pad_left" | "to_string")
            }
            Expr::BinaryOp {
                op: BinaryOp::Add,
                left,
                right,
            } => self.is_string_expr(left) || self.is_string_expr(right),
            Expr::Parenthesized(inner) => self.is_string_expr(inner),
            Expr::Get(get) => !get.flags.iter().any(|flag| {
                matches!(
                    flag,
                    GetFlag::Bytes(_) | GetFlag::As(_) | GetFlag::Timeout(_)
                )
            }),
            _ => false,
        }
    }

    fn is_bytes_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Literal(Literal::Bytes(_)) => true,
            Expr::Identifier(_) => self
                .expression_type_name(expr)
                .is_some_and(|type_name| type_name == "bytes"),
            Expr::Get(get) => get
                .flags
                .iter()
                .any(|flag| matches!(flag, GetFlag::Bytes(_))),
            Expr::Parenthesized(inner) => self.is_bytes_expr(inner),
            _ => false,
        }
    }

    fn expr_enum_type(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Identifier(name) => self
                .enum_variants
                .get(name)
                .and_then(|qualified| qualified.split_once("::"))
                .map(|(enum_name, _)| enum_name.to_string()),
            Expr::Enum { enum_name, .. } if !enum_name.is_empty() => Some(enum_name.clone()),
            Expr::Call { func, .. } => match func.as_ref() {
                Expr::Identifier(name) => self
                    .enum_variants
                    .get(name)
                    .and_then(|qualified| qualified.split_once("::"))
                    .map(|(enum_name, _)| enum_name.to_string()),
                _ => None,
            },
            _ => None,
        }
    }

    fn expression_type_name(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Identifier(name) => self
                .type_scopes
                .borrow()
                .iter()
                .rev()
                .find_map(|scope| scope.get(name).cloned()),
            Expr::Struct { name, .. } => Some(name.clone()),
            Expr::Parenthesized(inner) => self.expression_type_name(inner),
            _ => None,
        }
    }

    fn is_string_name(&self, name: &str) -> bool {
        self.string_scopes
            .borrow()
            .iter()
            .rev()
            .any(|scope| scope.contains(name))
    }

    fn declare_string(&self, name: String) {
        if let Some(scope) = self.string_scopes.borrow_mut().last_mut() {
            scope.insert(name);
        }
    }

    fn declare_type(&self, name: String, type_name: String) {
        if let Some(scope) = self.type_scopes.borrow_mut().last_mut() {
            scope.insert(name, type_name);
        }
    }

    fn enter_scope(&self) {
        self.string_scopes.borrow_mut().push(HashSet::new());
        self.type_scopes.borrow_mut().push(HashMap::new());
        self.vec_scopes.borrow_mut().push(HashSet::new());
        self.vec_element_scopes.borrow_mut().push(HashMap::new());
    }

    fn exit_scope(&self) {
        if self.string_scopes.borrow().len() > 1 {
            self.string_scopes.borrow_mut().pop();
            self.type_scopes.borrow_mut().pop();
            self.vec_scopes.borrow_mut().pop();
            self.vec_element_scopes.borrow_mut().pop();
        }
    }

    fn is_vec_name(&self, name: &str) -> bool {
        self.vec_scopes
            .borrow()
            .iter()
            .rev()
            .any(|scope| scope.contains(name))
    }

    fn declare_vec(&self, name: String) {
        if let Some(scope) = self.vec_scopes.borrow_mut().last_mut() {
            scope.insert(name);
        }
    }

    fn declare_vec_element(&self, name: String, element: String) {
        if let Some(scope) = self.vec_element_scopes.borrow_mut().last_mut() {
            scope.insert(name, element);
        }
    }

    /// Whether a type annotation denotes a vector-like value (Vec, array,
    /// bytes, or a Map/Set collection).
    fn is_vec_type(ty: &Type) -> bool {
        matches!(ty, Type::Vec(_) | Type::Array(_, _))
            || matches!(ty, Type::Named(name) if name == "bytes")
            || matches!(
                ty,
                Type::Generic { name, .. } if matches!(name.as_str(), "Vec" | "Set" | "Map")
            )
    }

    /// Whether an expression produces a vector-like value (for `{:?}` put
    /// formatting): array literals, vector-typed names, and the methods and
    /// builtins that return vectors.
    fn is_vec_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Array(_) => true,
            Expr::Identifier(name) => {
                self.is_vec_name(name)
                    || self.expression_type_name(expr).is_some_and(|type_name| {
                        type_name.starts_with("Map<") || type_name.starts_with("Set<")
                    })
            }
            Expr::Call { func, .. } => {
                matches!(func.as_ref(), Expr::Identifier(name) if name == "db_execute")
            }
            Expr::MethodCall { object, method, .. } => {
                matches!(
                    method.as_str(),
                    "map" | "filter" | "split" | "sort_by" | "collect"
                ) || (method == "await" && self.is_vec_expr(object))
            }
            Expr::Get(get) => get
                .flags
                .iter()
                .any(|flag| matches!(flag, GetFlag::Bytes(_))),
            Expr::Parenthesized(inner) => self.is_vec_expr(inner),
            _ => false,
        }
    }

    /// Element type name of a vector type annotation (`Vec<T>`/`[T; n]`).
    fn vec_element_type_of_type(&self, ty: &Type) -> Option<String> {
        match ty {
            Type::Vec(inner) | Type::Array(inner, _) => Some(self.gen_type(inner)),
            Type::Generic { name, args } if name == "Vec" => {
                args.first().map(|inner| self.gen_type(inner))
            }
            _ => None,
        }
    }

    /// Element type name inferred from an array literal's first element.
    fn array_element_type(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Array(elements) => elements.first().and_then(|e| self.expr_element_type(e)),
            Expr::Parenthesized(inner) => self.array_element_type(inner),
            _ => None,
        }
    }

    fn expr_element_type(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Literal(Literal::Int(_)) => Some("i32".to_string()),
            Expr::Literal(Literal::Float(_)) => Some("f64".to_string()),
            Expr::Literal(Literal::String(_) | Literal::UnicodeString(_)) => {
                Some("String".to_string())
            }
            Expr::Literal(Literal::Char(_)) => Some("char".to_string()),
            Expr::Literal(Literal::Bool(_)) => Some("bool".to_string()),
            Expr::UnaryOp {
                op: UnaryOp::Neg,
                expr: inner,
            } => self.expr_element_type(inner),
            Expr::Identifier(_) => self.expression_type_name(expr),
            _ => None,
        }
    }

    /// Element type name of a vector expression, walking through `.iter()`/
    /// `.filter()`/`.map()` chains to the underlying collection (for `map`,
    /// the callback's return type).
    fn vec_element_type_of(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Identifier(name) => self
                .vec_element_scopes
                .borrow()
                .iter()
                .rev()
                .find_map(|scope| scope.get(name).cloned()),
            Expr::MethodCall { object, method, .. }
                if matches!(method.as_str(), "iter" | "filter") =>
            {
                self.vec_element_type_of(object)
            }
            Expr::MethodCall {
                object: _,
                method,
                args,
                ..
            } if method == "map" => args.first().and_then(|arg| self.closure_result_type(arg)),
            Expr::Array(_) => self.array_element_type(expr),
            Expr::Parenthesized(inner) => self.vec_element_type_of(inner),
            _ => None,
        }
    }

    /// Best-effort guess of a closure's return type, for `.map(f).sum()`
    /// turbofish inference. Covers literals, arithmetic with literals, and
    /// `to_string()`.
    fn closure_result_type(&self, expr: &Expr) -> Option<String> {
        let body = match expr {
            Expr::Closure { body, .. } => body.as_ref(),
            Expr::Parenthesized(inner) => match inner.as_ref() {
                Expr::Closure { body, .. } => body.as_ref(),
                other => other,
            },
            _ => return None,
        };
        match body {
            Expr::Literal(Literal::Int(_)) => Some("i32".to_string()),
            Expr::Literal(Literal::Float(_)) => Some("f64".to_string()),
            Expr::Literal(Literal::String(_) | Literal::UnicodeString(_)) => {
                Some("String".to_string())
            }
            Expr::Literal(Literal::Bool(_)) => Some("bool".to_string()),
            Expr::BinaryOp {
                op:
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod
                    | BinaryOp::BitAnd
                    | BinaryOp::BitOr
                    | BinaryOp::BitXor,
                left,
                right,
            } => self
                .expr_element_type(left)
                .or_else(|| self.expr_element_type(right)),
            Expr::MethodCall { method, .. } if method == "to_string" => Some("String".to_string()),
            Expr::Identifier(name) => self
                .string_scopes
                .borrow()
                .iter()
                .rev()
                .any(|scope| scope.contains(name))
                .then(|| "String".to_string()),
            _ => None,
        }
    }

    /// Whether an expression is (or chains from) `xs.iter()` — i.e. it is
    /// already an owned-element iterator, so higher-order methods must not
    /// re-add `.iter().cloned()` or eagerly `.collect()`.
    fn is_iterator_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::MethodCall { method, .. } if method == "iter" => true,
            Expr::MethodCall { object, method, .. }
                if matches!(method.as_str(), "map" | "filter") =>
            {
                self.is_iterator_expr(object)
            }
            Expr::Parenthesized(inner) => self.is_iterator_expr(inner),
            _ => false,
        }
    }

    /// Whether an expression has a `Map<...>` type (for `get`/`insert`
    /// lowering that needs to borrow keys).
    fn is_map_expr(&self, expr: &Expr) -> bool {
        self.expression_type_name(expr)
            .is_some_and(|name| name.starts_with("Map<"))
    }

    /// Whether an expression has a `Set<...>` type (for `contains`/`remove`
    /// which borrow their element).
    fn is_set_expr(&self, expr: &Expr) -> bool {
        self.expression_type_name(expr)
            .is_some_and(|name| name.starts_with("Set<"))
    }

    /// Write a line with indent. Writes indent characters directly to avoid
    /// allocating a String via `repeat()` on every line.
    fn writeln(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        self.output.push_str(s);
        self.output.push('\n');
    }

    /// Write a formatted line directly to the output buffer, avoiding the
    /// intermediate String allocation from `format!`.
    fn writeln_fmt(&mut self, args: std::fmt::Arguments<'_>) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        use std::fmt::Write;
        self.output.write_fmt(args).unwrap();
        self.output.push('\n');
    }
}

impl Default for IntermediateRepresentationCodeGen {
    fn default() -> Self {
        Self::new()
    }
}

fn named_type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Named(name) if name != "string" && name != "ustring" && name != "Self" => {
            Some(name.clone())
        }
        // Structured containers register under their rendered name so
        // `expression_type_name` can detect `Map<...>` receivers.
        Type::Generic { name, args } if matches!(name.as_str(), "Map" | "Set" | "Vec") => {
            let rendered: Vec<String> = args
                .iter()
                .map(|arg| match arg {
                    Type::Named(n) if matches!(n.as_str(), "string" | "ustring" | "String") => {
                        "String".to_string()
                    }
                    Type::Named(n) if n == "uchar" => "char".to_string(),
                    Type::Named(n) if n == "byte" => "u8".to_string(),
                    Type::Named(n) => n.clone(),
                    other => format!("{other:?}"),
                })
                .collect();
            Some(format!("{}<{}>", name, rendered.join(", ")))
        }
        _ => None,
    }
}

fn is_closure_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Closure { .. } => true,
        Expr::Parenthesized(inner) => is_closure_expr(inner),
        _ => false,
    }
}

fn collect_returned_binding_names(statements: &[Statement]) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_returned_binding_names_from_statements(statements, &mut names);
    names
}

fn collect_returned_binding_names_from_statements(
    statements: &[Statement],
    names: &mut HashSet<String>,
) {
    for statement in statements {
        match statement {
            Statement::Return(Some(expr)) => {
                if let Some(name) = returned_binding_name(expr) {
                    names.insert(name.to_string());
                }
                collect_returned_binding_names_from_expr(expr, names);
            }
            Statement::If {
                then_block,
                else_block,
                ..
            } => {
                collect_returned_binding_names_from_statements(then_block, names);
                if let Some(else_block) = else_block {
                    collect_returned_binding_names_from_statements(else_block, names);
                }
            }
            Statement::Block(statements) => {
                collect_returned_binding_names_from_statements(statements, names);
            }
            Statement::Match { arms, .. } => {
                for arm in arms {
                    match &arm.body {
                        MatchArmBody::Expression(expr) => {
                            collect_returned_binding_names_from_expr(expr, names);
                        }
                        MatchArmBody::Block(statements) => {
                            collect_returned_binding_names_from_statements(statements, names);
                        }
                    }
                }
            }
            Statement::Expression(expr) => {
                collect_returned_binding_names_from_expr(expr, names);
            }
            // Nested functions have their own return-binding scope.
            Statement::NestedFunction(_)
            | Statement::ForeignBlock { .. }
            | Statement::Return(None)
            | Statement::Break
            | Statement::Continue
            | Statement::VarDecl { .. }
            | Statement::LetDecl { .. }
            | Statement::Assignment { .. }
            | Statement::Mutation { .. }
            | Statement::Put { .. }
            | Statement::Error(_)
            | Statement::Warn(_)
            | Statement::Info(_) => {}
        }
    }
}

fn collect_returned_binding_names_from_expr(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        // Do not inspect closure bodies: returns inside a closure are not
        // returns from the enclosing function.
        Expr::Closure { .. } => {}
        Expr::If {
            then_block,
            else_block,
            ..
        } => {
            collect_returned_binding_names_from_statements(then_block, names);
            if let Some(else_block) = else_block {
                collect_returned_binding_names_from_statements(else_block, names);
            }
        }
        Expr::Match { arms, .. } => {
            for arm in arms {
                match &arm.body {
                    MatchArmBody::Expression(expr) => {
                        collect_returned_binding_names_from_expr(expr, names);
                    }
                    MatchArmBody::Block(statements) => {
                        collect_returned_binding_names_from_statements(statements, names);
                    }
                }
            }
        }
        Expr::LoopRange { body, .. }
        | Expr::ForLoop { body, .. }
        | Expr::InfiniteLoop(body)
        | Expr::UnsafeBlock(body) => {
            collect_returned_binding_names_from_statements(body, names);
        }
        _ => {}
    }
}

fn returned_binding_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Identifier(name) => Some(name),
        Expr::Parenthesized(inner) => returned_binding_name(inner),
        _ => None,
    }
}

fn statement_contains_await(statement: &Statement) -> bool {
    match statement {
        Statement::Expression(expr) => expr_contains_await(expr),
        Statement::Return(Some(expr)) => expr_contains_await(expr),
        Statement::Put { expr, .. } => expr_contains_await(expr),
        Statement::If {
            then_block,
            else_block,
            ..
        } => {
            then_block.iter().any(statement_contains_await)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block.iter().any(statement_contains_await))
        }
        Statement::Block(statements) => statements.iter().any(statement_contains_await),
        Statement::Match { scrutinee, arms } => {
            expr_contains_await(scrutinee)
                || arms.iter().any(|arm| match &arm.body {
                    MatchArmBody::Expression(expr) => expr_contains_await(expr),
                    MatchArmBody::Block(statements) => {
                        statements.iter().any(statement_contains_await)
                    }
                })
        }
        _ => false,
    }
}

fn expr_contains_await(expr: &Expr) -> bool {
    match expr {
        Expr::MethodCall { method, .. } if method == "await" => true,
        Expr::Call { func, args } => {
            expr_contains_await(func) || args.iter().any(expr_contains_await)
        }
        Expr::MethodCall { object, args, .. } => {
            expr_contains_await(object) || args.iter().any(expr_contains_await)
        }
        Expr::BinaryOp { left, right, .. } => {
            expr_contains_await(left) || expr_contains_await(right)
        }
        Expr::UnaryOp { expr, .. } => expr_contains_await(expr),
        Expr::Index { object, index } => {
            expr_contains_await(object) || expr_contains_await(index)
        }
        Expr::Parenthesized(inner) => expr_contains_await(inner),
        Expr::If { condition, .. } => expr_contains_await(condition),
        Expr::Match { scrutinee, arms } => {
            expr_contains_await(scrutinee)
                || arms.iter().any(|arm| {
                    matches!(&arm.body, MatchArmBody::Expression(expr) if expr_contains_await(expr))
                })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    /// Transpile Poly source through the full intermediate representation pipeline.
    fn transpile(source: &str) -> String {
        crate::transpile(source).expect("transpile should succeed")
    }

    #[test]
    fn append_redirect_inside_loop_emits_io_write_import() {
        // Append redirects inside a loop body go through the inline
        // statement generator, which must still request the `io::Write`
        // import that `writeln!` needs.
        let rust = transpile(
            "fn main()\n    loop i 0..3\n        put \"Line \" + i.to_string() to \"out.txt\" -append\n    end loop\nend fn",
        );
        assert!(
            rust.contains("use std::io::Write;"),
            "missing io::Write import: {rust}"
        );
    }

    #[test]
    fn string_append_emits_borrowed_amount() {
        let rust = transpile(
            "fn main()\n    var output ustring := \"\"\n    output := output + \"abc\"\n    output := output + \"def\"\nend fn",
        );
        assert!(rust.contains("output = format!(\"{}{}\", output, String::from(\"abc\"));"));
        assert!(rust.contains("output = format!(\"{}{}\", output, String::from(\"def\"));"));
    }

    #[test]
    fn string_repeat_and_vec_join_codegen() {
        let rust = transpile(
            "fn main()\n    put \"#\".repeat(5)\n    var parts Vec<ustring> := [unicode \"a\"]\n    put parts.join(\", \")\nend fn",
        );
        assert!(rust.contains("String::from(\"#\").repeat(5 as usize)"));
        assert!(rust.contains("parts.join(String::from(\", \").as_str())"));
    }

    #[test]
    fn empty_array_initializers_for_map_and_set_emit_collections() {
        let rust = transpile(
            "fn main()\n    var m Map<string, i32> := []\n    var s Set<i32> := []\n    var v Vec<i32> := []\nend fn",
        );
        assert!(
            rust.contains("std::collections::HashMap::new()"),
            "missing HashMap::new(): {rust}"
        );
        assert!(
            rust.contains("std::collections::HashSet::new()"),
            "missing HashSet::new(): {rust}"
        );
        assert!(
            rust.contains("let mut v: Vec<i32> = vec![];"),
            "missing vec![]: {rust}"
        );
    }

    #[test]
    fn non_empty_arrays_are_unchanged() {
        let rust = transpile("fn main()\n    var v Vec<i32> := [1, 2, 3]\nend fn");
        assert!(rust.contains("vec![1, 2, 3]"), "{rust}");
    }

    #[test]
    fn string_append_inside_loop_body_borrows_amount() {
        // `add row, ...` on a string declared inside a loop body (rendered by
        // `gen_statement_str`) must lower to `row += &(...)` so the generated
        // Rust compiles (`String` has no `AddAssign<String>`).
        let rust = transpile(
            "fn main()\n    loop i 1..2\n        var row ustring := \"\"\n        row := row + i.to_string()\n        put row\n    end loop\nend fn",
        );
        assert!(
            rust.contains("row = format!(\"{}{}\", row, format!(\"{:?}\", i));"),
            "string append in loop body must borrow the amount: {rust}"
        );
    }

    #[test]
    fn put_vec_declared_in_loop_body_uses_debug_formatting() {
        // A vector declared inside a loop body must register as a vector so
        // `put buf` lowers to `println!(\"{:?}\", buf)` instead of `{}`.
        let rust = transpile(
            "fn main()\n    loop i 1..2\n        var buf Vec<i32> := []\n        put buf\n    end loop\nend fn",
        );
        assert!(
            rust.contains("println!(\"{:?}\", buf);"),
            "vector put in loop body must use debug formatting: {rust}"
        );
    }

    #[test]
    fn function_returning_closure_emits_impl_fn_and_move() {
        let rust = transpile("fn make_adder(n: i32): |x: i32| i32\n    return |x| x + n\nend fn");
        assert!(
            rust.contains("fn make_adder(n: i32) -> impl Fn(i32) -> i32 {"),
            "missing impl Fn return: {rust}"
        );
        assert!(
            rust.contains("return move |x| (x + n);"),
            "missing move closure: {rust}"
        );
    }

    #[test]
    fn closure_binding_returned_from_function_emits_move() {
        let rust = transpile(
            "fn make_adder(n: i32): |x: i32| i32\n    var f := |x| x + n\n    return f\nend fn",
        );
        assert!(
            rust.contains("let mut f = move |x| (x + n);"),
            "returned closure binding must move captures: {rust}"
        );
    }

    #[test]
    fn infinite_loop_emits_rust_loop() {
        let rust = transpile(
            "fn main()\n    var count i32 := 0\n    loop\n        count := count + 1\n        if count = 3,\n            break\n        end if\n    end loop\nend fn",
        );
        assert!(rust.contains("loop {"), "missing loop block: {rust}");
        assert!(
            rust.contains("count = (count + 1);"),
            "missing body: {rust}"
        );
        assert!(rust.contains("break;"), "missing break: {rust}");
    }

    #[test]
    fn multiple_get_flags_parse_and_lower() {
        let rust = transpile(
            "fn main()\n    match get --timeout 5000 --default unicode \"0\"\n        Ok(input), put input\n        Error(e), error e\n    end match\nend fn",
        );
        assert!(
            rust.contains("Ok::<String, String>"),
            "missing Result-returning get: {rust}"
        );
    }

    #[test]
    fn collection_loops_borrow_and_destructure_enumerate() {
        let rust = transpile(
            "fn main()\n    var fruits Vec<ustring> := []\n    loop fruit in fruits\n        put fruit\n    end loop\n    loop (index, fruit) in fruits.enumerate()\n        put index\n        put fruit\n    end loop\nend fn",
        );
        assert!(
            rust.contains("for fruit in fruits.iter().cloned() {"),
            "collection loop must borrow (not move) and bind owned values: {rust}"
        );
        assert!(
            rust.contains("for (index, fruit) in fruits.iter().cloned().enumerate() {"),
            "enumerate loop must destructure and borrow: {rust}"
        );
    }

    #[test]
    fn for_loop_enumerate_borrows_receiver() {
        // `for (idx, val) in items.enumerate()` must lower to
        // `items.iter().enumerate()` (borrowing the receiver) rather than the
        // broken `items.enumerate().iter()`.
        let rust = transpile(
            "fn main()\n    var items := [5, 6]\n    for (idx, val) in items.enumerate()\n        put idx\n        put val\n    end for\nend fn",
        );
        assert!(
            rust.contains("for (idx, val) in items.iter().cloned().enumerate() {"),
            "for-loop enumerate must borrow the receiver: {rust}"
        );
        assert!(
            !rust.contains("enumerate().iter()"),
            "must not call .iter() on the enumerate result: {rust}"
        );
    }

    #[test]
    fn tuple_element_assignment_emits_rust_assignment() {
        let rust = transpile(
            "fn main()\n    var pair := (10, true)\n    pair.0 := 99\n    pair.1 := false\nend fn",
        );
        assert!(rust.contains("pair.0 = 99;"), "{rust}");
        assert!(rust.contains("pair.1 = false;"), "{rust}");
    }

    #[test]
    fn loop_range_uses_declared_variable() {
        let rust = transpile(
            "fn main()\n    loop value 1..3, 7, 19..20\n        put value\n    end loop\nend fn",
        );
        assert!(
            rust.contains("for value in (1..=3).chain(std::iter::once(7)).chain((19..=20))"),
            "{rust}"
        );
        assert!(rust.contains("println!(\"{}\", value);"), "{rust}");
        assert!(
            !rust.contains("for i in"),
            "implicit loop variable leaked: {rust}"
        );
    }

    #[test]
    fn vec_hof_methods_emit_iterator_chains() {
        let rust = transpile(
            "fn main()\n    var xs := [1, 2, 3]\n    put xs.map(|x| x * 2)[0]\n    put xs.filter(|x| x mod 2 = 0)\n    put xs.reduce(0, |acc, x| acc + x)\nend fn",
        );
        assert!(
            rust.contains("xs.iter().cloned().map(|x| (x * 2)).collect::<Vec<_>>()[(0) as usize]"),
            "missing map lowering: {rust}"
        );
        assert!(
            rust.contains("let x = *x;"),
            "missing filter deref binding: {rust}"
        );
        assert!(
            rust.contains("xs.iter().cloned().fold(0, |acc, x| (acc + x))"),
            "missing reduce lowering: {rust}"
        );
    }

    #[test]
    fn put_of_vectors_uses_debug_formatting() {
        let rust = transpile(
            "fn main()\n    var xs := [1, 2, 3]\n    var typed Vec<i32> := [4, 5]\n    put xs\n    put typed\n    put [9, 8]\n    put xs.map(|x| x * 2)\nend fn",
        );
        assert!(
            rust.contains("println!(\"{:?}\", xs);"),
            "missing {{:?}}: {rust}"
        );
        assert!(
            rust.contains("println!(\"{:?}\", typed);"),
            "missing {{:?}}: {rust}"
        );
        assert!(
            rust.contains("println!(\"{:?}\", vec![9, 8]);"),
            "missing {{:?}}: {rust}"
        );
        assert!(
            rust.contains(
                "println!(\"{:?}\", xs.iter().cloned().map(|x| (x * 2)).collect::<Vec<_>>());"
            ),
            "missing {{:?}} on map result: {rust}"
        );
    }

    #[test]
    fn string_hof_methods_emit_chars() {
        let rust = transpile(
            "fn main()\n    put \"hello\".map(|c| c)\n    put \"hello\".filter(|c| c != unicode 'l')\n    put \"hello\".reduce(0, |acc, c| acc + (c as i32))\nend fn",
        );
        assert!(
            rust.contains("String::from(\"hello\").chars().map(|c| c).collect::<Vec<_>>()"),
            "missing string map: {rust}"
        );
        assert!(
            rust.contains("String::from(\"hello\").chars().filter(|c| { let c = *c; (c != 'l') }).collect::<Vec<_>>()"),
            "missing string filter: {rust}"
        );
        assert!(
            rust.contains("String::from(\"hello\").chars().fold(0, |acc, c| (acc + (c as i32)))"),
            "missing string reduce: {rust}"
        );
    }

    #[test]
    fn sort_by_emits_ordering_wrapper() {
        let rust = transpile(
            "fn main()\n    var xs := [3, 1, 2]\n    put xs.sort_by(|a, b| a > b)\nend fn",
        );
        assert!(
            rust.contains("v.sort_by(|a, b| { let a = *a; let b = *b; if (a > b) { std::cmp::Ordering::Less } else if (a == b) { std::cmp::Ordering::Equal } else { std::cmp::Ordering::Greater } })"),
            "missing sort wrapper: {rust}"
        );
        assert!(
            rust.contains("let mut v = xs.clone();"),
            "missing clone: {rust}"
        );
    }

    #[test]
    fn map_methods_borrow_keys() {
        let rust = transpile(
            "fn main()\n    var m Map<ustring, i32> := []\n    m.insert(unicode \"key\", 42)\n    put m.get(unicode \"key\")\n    put m.contains_key(unicode \"key\")\n    m.remove(unicode \"key\")\n    m[unicode \"other\"] := 7\n    var v := m[unicode \"other\"]\n    put v\nend fn",
        );
        assert!(
            rust.contains("m.insert(String::from(\"key\"), 42);"),
            "missing insert: {rust}"
        );
        assert!(
            rust.contains("m.get(&String::from(\"key\"))"),
            "get must borrow the key: {rust}"
        );
        assert!(
            rust.contains("m.contains_key(&String::from(\"key\"))"),
            "contains_key must borrow the key: {rust}"
        );
        assert!(
            rust.contains("m.remove(&String::from(\"key\"))"),
            "remove must borrow the key: {rust}"
        );
        assert!(
            rust.contains("m.insert(String::from(\"other\"), 7);"),
            "map index assignment must lower to insert: {rust}"
        );
        assert!(
            rust.contains("m.get(&String::from(\"other\")).map(|v| v.clone()).unwrap_or_default()"),
            "map index read must lower to get: {rust}"
        );
    }

    #[test]
    fn set_methods_borrow_elements() {
        let rust = transpile(
            "fn main()\n    var s Set<i32> := []\n    s.insert(1)\n    put s.contains(1)\n    s.remove(1)\nend fn",
        );
        assert!(
            rust.contains("s.contains(&1)"),
            "set contains must borrow: {rust}"
        );
        assert!(
            rust.contains("s.remove(&1)"),
            "set remove must borrow: {rust}"
        );
    }

    #[test]
    fn negative_loop_steps_keep_magnitude() {
        let rust = transpile(
            "fn main()\n    loop i 10..1 step -2\n        put i\n    end loop\n    loop j 10..1 step 2\n        put j\n    end loop\nend fn",
        );
        assert!(
            rust.contains("for i in (1..=10).rev().step_by(2 as usize)"),
            "negative step magnitude dropped: {rust}"
        );
        assert!(
            rust.contains("for j in (10..=1).step_by(2 as usize)"),
            "descending positive step should be empty: {rust}"
        );
    }

    #[test]
    fn get_timeout_emits_real_timeout() {
        let rust = transpile(
            "fn main()\n    match get --timeout 2000 --default unicode \"fallback\"\n        Ok(input), put input\n        Timeout, put unicode \"too slow\"\n    end match\nend fn",
        );
        assert!(
            rust.contains("std::sync::mpsc::channel()"),
            "timeout must use a channel: {rust}"
        );
        assert!(
            rust.contains("recv_timeout(std::time::Duration::from_millis(2000 as u64))"),
            "missing recv_timeout: {rust}"
        );
        assert!(
            rust.contains("Ok::<String, String>(String::from(\"fallback\"))"),
            "missing default on timeout: {rust}"
        );
    }

    #[test]
    fn file_read_methods_use_buffered_reader() {
        let rust = transpile(
            "fn main()\n    var f := open(unicode \"data.txt\")\n    while not f.eof()\n        var line := f.get_line()\n        put line\n    end while\nend fn",
        );
        assert!(
            rust.contains(
                "std::io::BufReader::new(std::fs::File::open(String::from(\"data.txt\")).unwrap())"
            ),
            "open must lower to a BufReader: {rust}"
        );
        assert!(
            rust.contains("f.fill_buf().map(|buffer| buffer.is_empty()).unwrap_or(true)"),
            "eof must use fill_buf: {rust}"
        );
        assert!(
            rust.contains("f.read_line(&mut __poly_line).unwrap()"),
            "get_line must use read_line: {rust}"
        );
        assert!(
            rust.contains("use std::io::BufRead;"),
            "missing BufRead import: {rust}"
        );
    }

    #[test]
    fn sleep_delay_and_exit_lower_to_real_calls() {
        let rust = transpile("fn main()\n    sleep(50)\n    exit(0)\nend fn");
        assert!(
            rust.contains("std::thread::sleep(std::time::Duration::from_millis(50 as u64))"),
            "missing thread sleep: {rust}"
        );
        assert!(
            rust.contains("std::process::exit(0)"),
            "missing exit: {rust}"
        );

        let async_rust = transpile("async fn main()\n    delay(50).await\nend fn");
        assert!(
            async_rust.contains("tokio::time::sleep(std::time::Duration::from_millis(50 as u64))"),
            "missing tokio sleep: {async_rust}"
        );
    }

    #[test]
    fn const_declarations_infer_literal_types() {
        // `const PI := 3.14159` must not emit `const PI: _ = ...` (Rust rejects
        // placeholders on item signatures); literals get an explicit type.
        let rust = transpile(
            "const PI := 3.14159\nconst MAX_SIZE := 1024\nconst ENABLED := true\nfn main()\n    put PI\nend fn",
        );
        assert!(rust.contains("const PI: f64 = 3.14159;"), "{rust}");
        assert!(rust.contains("const MAX_SIZE: i32 = 1024;"), "{rust}");
        assert!(rust.contains("const ENABLED: bool = true;"), "{rust}");
        assert!(
            !rust.contains(": _ ="),
            "placeholder const type leaked: {rust}"
        );
    }

    #[test]
    fn numeric_builtins_annotate_literal_arguments() {
        // `abs(-42)` / `pow(3, 2)` must lower to type-annotated literals so
        // Rust can resolve the method instead of E0689, and float `pow` uses
        // `powf` (i32::pow takes u32, f64 uses powf).
        let rust = transpile(
            "fn main()\n    var magnitude i32 := abs(-42)\n    var squared i32 := pow(3, 2)\n    var cubed f64 := pow(2.0, 3.0)\nend fn",
        );
        assert!(rust.contains("(-42_i32).abs()"), "{rust}");
        assert!(rust.contains("(3_i32).pow(2_i32 as u32)"), "{rust}");
        assert!(rust.contains("(2.0_f64).powf(3.0_f64)"), "{rust}");
    }

    #[test]
    fn unqualified_struct_variant_pattern_renders_named_fields() {
        // `Error(TooShort(min))` on an enum whose variant has named fields must
        // render `ValidationError::TooShort { min: min }`, not the tuple form
        // `TooShort(min)` (E0164).
        let rust = transpile(
            "enum ValidationError\n    EmptyInput\n    TooShort(min: i32)\nend enum\n\nfn main()\n    var result Result<ustring, ValidationError> := Error(ValidationError::TooShort(2))\n    match result\n        Error(TooShort(min)), put min\n        Error(_), put 0\n    end match\nend fn",
        );
        assert!(
            rust.contains("ValidationError::TooShort { min: min }"),
            "struct-variant pattern must render named fields: {rust}"
        );
    }
}

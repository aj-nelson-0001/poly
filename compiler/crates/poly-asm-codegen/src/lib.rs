//! Poly to x86-64 assembly code generation (Linux syscalls, AT&T syntax).
//!
//! The assembly backend targets the small orchestration subset of Poly.
//! Complex declarations stay in `#asm` blocks and emitted verbatim.
//!
//! All I/O is done via Linux syscalls (no libc dependency):
//!   - sys_write (syscall 1) for printing strings and integers
//!   - sys_exit  (syscall 60) for termination

use std::fmt::Write;

use poly_parser::ast::{
    self, BinaryOp, Expression, FunctionDecl, Statement, TypeAnnotation, UnaryOp,
};

/// Generate a complete x86-64 assembly translation unit from a Poly program.
pub fn transpile(program: &ast::Program) -> Result<String, String> {
    let mut generator = AsmGenerator::new();
    generator.generate(program)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Location {
    Stack(u32),
    Reg(&'static str),
}

struct StackFrame {
    next_offset: u32,
    locals: std::collections::HashMap<String, Location>,
    /// Variable name -> declared struct type name (if any).
    var_types: std::collections::HashMap<String, String>,
}

impl StackFrame {
    fn new() -> Self {
        Self {
            next_offset: 8,
            locals: std::collections::HashMap::new(),
            var_types: std::collections::HashMap::new(),
        }
    }

    fn allocate(&mut self, name: &str) -> Location {
        if let Some(loc) = self.locals.get(name) {
            return *loc;
        }
        let loc = Location::Stack(self.next_offset);
        self.locals.insert(name.to_string(), loc);
        self.next_offset += 8;
        loc
    }

    /// Allocate space for a struct variable (multiple 8-byte slots).
    fn allocate_struct(&mut self, name: &str, total_size: u32) -> Location {
        if let Some(loc) = self.locals.get(name) {
            return *loc;
        }
        let loc = Location::Stack(self.next_offset);
        self.locals.insert(name.to_string(), loc);
        // Round up to next multiple of 8
        let slots = (total_size + 7) / 8;
        self.next_offset += slots * 8;
        loc
    }

    fn get(&self, name: &str) -> Option<Location> {
        self.locals.get(name).copied()
    }

    fn frame_size(&self) -> u32 {
        let raw = self.next_offset;
        (raw + 15) & !15
    }
}

/// Tracks the labels of an enclosing loop for break/continue.
struct LoopContext {
    /// Label to jump to for `continue` (re-checks condition).
    continue_label: String,
    /// Label to jump to for `break` (exits the loop).
    break_label: String,
}

#[derive(Clone)]
struct FieldInfo {
    offset: u32,
    size: u32,
}

#[derive(Clone)]
struct EnumInfo {
    tags: std::collections::HashMap<String, i64>,
}

struct AsmGenerator {
    output: String,
    label_counter: usize,
    string_data: Vec<(String, String)>,
    loop_stack: Vec<LoopContext>,
    /// Struct name -> (field name -> FieldInfo)
    structs: std::collections::HashMap<String, std::collections::HashMap<String, FieldInfo>>,
    /// Enum name -> EnumInfo
    enums: std::collections::HashMap<String, EnumInfo>,
}

impl AsmGenerator {
    fn new() -> Self {
        Self {
            output: String::new(),
            label_counter: 0,
            string_data: Vec::new(),
            loop_stack: Vec::new(),
            structs: std::collections::HashMap::new(),
            enums: std::collections::HashMap::new(),
        }
    }

    fn fresh_label(&mut self, prefix: &str) -> String {
        let id = self.label_counter;
        self.label_counter += 1;
        format!("{prefix}_{id}")
    }

    fn intern_string(&mut self, value: &str) -> String {
        let label = self.fresh_label("str");
        self.string_data.push((label.clone(), value.to_string()));
        label
    }

    fn emit_string_data(&mut self) {
        if self.string_data.is_empty() {
            return;
        }
        self.output.push_str("\n# String literals\n");
        for (label, content) in &self.string_data {
            let escaped = asm_escape(content);
            let _ = writeln!(self.output, "{label}: .asciz \"{escaped}\"");
        }
    }

    // -------------------------------------------------------------------
    // Top-level generation
    // -------------------------------------------------------------------

    fn generate(&mut self, program: &ast::Program) -> Result<String, String> {
        let mut asm_blocks: Vec<String> = Vec::new();
        let mut functions: Vec<FunctionDecl> = Vec::new();
        let mut top_level: Vec<ast::Spanned<Statement>> = Vec::new();

        for statement in &program.statements {
            match &statement.node {
                Statement::ForeignBlock { language, content } if language == "asm" => {
                    asm_blocks.push(content.clone());
                }
                Statement::ForeignBlock { .. } => {}
                Statement::ExternFunctionDeclaration(_) => {}
                Statement::FunctionDeclaration(func) => {
                    functions.push(func.clone());
                }
                _ => top_level.push(statement.clone()),
            }
        }

        // Pass 1a: emit user-defined functions (must come before _start).
        for func in &functions {
            self.emit_function(func)?;
        }
        let funcs_buf = std::mem::take(&mut self.output);

        // Pass 1b: emit _start body so strings are collected.
        self.output.push_str(".section .text\n");
        self.output.push_str(".globl _start\n\n");
        self.output.push_str("_start:\n");
        // _start is entered by the kernel with rsp 16-byte aligned.
        // pushq %rbp makes it 8 mod 16; subq must be 8 mod 16.
        self.output.push_str("    pushq %rbp\n");
        self.output.push_str("    movq %rsp, %rbp\n");
        self.output.push_str("    subq $4088, %rsp\n");
        let mut frame = StackFrame::new();
        for stmt in &top_level {
            self.emit_statement(&stmt.node, &mut frame)?;
        }
        self.emit_exit();
        let start_body = std::mem::take(&mut self.output);

        // Pass 2: compose the full translation unit.
        self.output.push_str("# Generated from Poly source code.\n");
        self.output
            .push_str("# Target: x86-64 Linux (AT&T syntax, syscalls)\n\n");
        self.output.push_str(".section .bss\n");
        self.output.push_str(".lcomm _itoa_buf, 32\n\n");
        self.output.push_str(".section .data\n");
        self.output.push_str("_newline: .byte 10\n");
        self.output.push_str("_minus:   .byte 45\n\n");
        for block in &asm_blocks {
            self.output.push_str(block.trim_matches('\n'));
            self.output.push('\n');
        }
        self.emit_string_data();
        // Functions in .text, then _start in .text
        self.output.push_str(".section .text\n\n");
        self.output.push_str(&funcs_buf);
        self.output.push('\n');
        self.output.push_str(&start_body);

        // Runtime helper: _print_int — convert integer in %rdi to decimal
        // and write via sys_write (syscall 1). Also _print_int_nobuf (no newline).
        self.output.push_str("\n# Runtime helpers\n");
        self.output.push_str("_print_int:\n");
        self.output.push_str("    pushq %rbp\n");
        self.output.push_str("    movq %rsp, %rbp\n");
        self.output.push_str("    subq $32, %rsp\n");
        // Handle negative
        self.output.push_str("    movq %rdi, %rax\n");
        self.output.push_str("    testq %rax, %rax\n");
        let digits_label = self.fresh_label("digits");
        let _ = writeln!(self.output, "    jge {digits_label}");
        // Print minus sign
        self.output.push_str("    movq $1, %rax\n");
        self.output.push_str("    movq $1, %rdi\n");
        self.output.push_str("    leaq _minus(%rip), %rsi\n");
        self.output.push_str("    movq $1, %rdx\n");
        self.output.push_str("    syscall\n");
        self.output.push_str("    negq %rax\n");
        let _ = writeln!(self.output, "{digits_label}:");
        // Convert digits: rax = absolute value, fill buffer from end
        self.output.push_str("    leaq _itoa_buf+31(%rip), %rcx\n");
        self.output.push_str("    movb $10, (%rcx)\n"); // newline at end
        self.output.push_str("    decq %rcx\n");
        self.output.push_str("    movq $1, %r8\n"); // length counter
        let loop_label = self.fresh_label("itoa");
        let done_label = self.fresh_label("itoa_done");
        let _ = writeln!(self.output, "{loop_label}:");
        self.output.push_str("    xorq %rdx, %rdx\n");
        self.output.push_str("    movq $10, %r9\n");
        self.output.push_str("    divq %r9\n"); // rax=quotient, rdx=remainder
        self.output.push_str("    addb $48, %dl\n"); // to ASCII
        self.output.push_str("    movb %dl, (%rcx)\n");
        self.output.push_str("    decq %rcx\n");
        self.output.push_str("    incq %r8\n");
        self.output.push_str("    testq %rax, %rax\n");
        let _ = writeln!(self.output, "    jnz {loop_label}");
        self.output.push_str("    incq %rcx\n"); // point to first digit
        let _ = writeln!(self.output, "{done_label}:");
        // r8 already counts digits + newline
        // sys_write(1, rcx, r8)
        self.output.push_str("    movq $1, %rax\n");
        self.output.push_str("    movq $1, %rdi\n");
        self.output.push_str("    movq %rcx, %rsi\n");
        self.output.push_str("    movq %r8, %rdx\n");
        self.output.push_str("    syscall\n");
        self.output.push_str("    movq %rbp, %rsp\n");
        self.output.push_str("    popq %rbp\n");
        self.output.push_str("    retq\n\n");

        Ok(std::mem::take(&mut self.output))
    }

    fn emit_exit(&mut self) {
        self.output.push_str("    # sys_exit(0)\n");
        self.output.push_str("    movq $60, %rax\n");
        self.output.push_str("    xorq %rdi, %rdi\n");
        self.output.push_str("    syscall\n");
    }

    // -------------------------------------------------------------------
    // Functions
    // -------------------------------------------------------------------

    fn emit_function(&mut self, func: &ast::FunctionDecl) -> Result<(), String> {
        if func.is_async || !func.generics.is_empty() {
            return Err(format!(
                "Assembly backend does not support async or generic Poly function `{}`; use a #asm definition",
                func.name
            ));
        }

        let mut frame = StackFrame::new();
        let param_regs: &[&str] = &["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];
        let mut param_moves = String::new();
        for (i, param) in func.params.iter().enumerate() {
            let loc = frame.allocate(&param.name);
            if let Location::Stack(offset) = loc {
                let reg = param_regs.get(i).copied().unwrap_or("%rdi");
                let _ = writeln!(param_moves, "    movq {reg}, -{offset}(%rbp)");
            }
        }
        // Pre-reserve one slot per statement so the frame size is final before
        // `subq` is emitted; emit_statement only appends to the frame afterwards.
        if let Some(body) = &func.body {
            for _ in body {
                frame.allocate(&format!("__reserve_{}", frame.next_offset));
            }
        }

        let _ = writeln!(self.output, ".globl {name}", name = func.name);
        let _ = writeln!(self.output, "{name}:", name = func.name);
        self.output.push_str("    pushq %rbp\n");
        self.output.push_str("    movq %rsp, %rbp\n");

        let frame_size = frame.frame_size();
        if frame_size > 0 {
            let _ = writeln!(self.output, "    subq ${frame_size}, %rsp");
        }
        self.output.push_str(&param_moves);

        if let Some(body) = &func.body {
            for stmt in body {
                self.emit_statement(&stmt.node, &mut frame)?;
            }
        }

        self.output.push_str("    movq $0, %rax\n");
        self.output.push_str("    movq %rbp, %rsp\n");
        self.output.push_str("    popq %rbp\n");
        self.output.push_str("    retq\n\n");

        Ok(())
    }

    // -------------------------------------------------------------------
    // Statements
    // -------------------------------------------------------------------

    fn emit_statement(&mut self, stmt: &Statement, frame: &mut StackFrame) -> Result<(), String> {
        match stmt {
            Statement::VarDeclaration { name, ty, value } => {
                // Allocate with proper size for struct/enum types.
                let loc = if let Some(TypeAnnotation::Named(type_name)) = ty {
                    if let Some(layout) = self.structs.get(type_name) {
                        let total: u32 = layout
                            .values()
                            .map(|f| {
                                // Round each field up to 8-byte slot
                                ((f.size + 7) / 8) * 8
                            })
                            .sum();
                        frame.allocate_struct(name, total)
                    } else if self.enums.contains_key(type_name) {
                        frame.allocate_struct(name, 40) // enum tag (8) + up to 4 payload slots (32)
                    } else {
                        frame.allocate(name)
                    }
                } else {
                    frame.allocate(name)
                };
                if let Some(ty) = ty {
                    if !self.is_integer_type(ty)
                        && !self.is_bool_type(ty)
                        && !self.is_string_type(ty)
                        && !self.is_struct_type(ty)
                        && !self.is_enum_type(ty)
                    {
                        return Err(format!(
                            "Assembly backend does not support type `{ty:?}` for `{name}`; use integer, bool, string, struct, or enum"
                        ));
                    }
                    // Record struct/enum/string types for typed access (fields,
                    // strlen-based `put` of string variables).
                    if let TypeAnnotation::Named(type_name) = ty {
                        if self.structs.contains_key(type_name)
                            || self.enums.contains_key(type_name)
                            || type_name == "string"
                            || type_name == "ustring"
                        {
                            frame.var_types.insert(name.clone(), type_name.clone());
                        }
                    }
                }
                if let Some(val) = value {
                    let expr_loc = self.emit_expr(val, frame)?;
                    // For struct types, copy field-by-field.
                    if let Some(TypeAnnotation::Named(type_name)) = ty {
                        if let Some(layout) = self.structs.get(type_name) {
                            // Struct: copy field-by-field
                            if let (Location::Stack(dest_off), Location::Stack(src_off)) =
                                (loc, expr_loc)
                            {
                                let mut fields: Vec<&FieldInfo> = layout.values().collect();
                                fields.sort_by_key(|f| f.offset);
                                for field_info in fields {
                                    let _ = writeln!(
                                        self.output,
                                        "    movq -{}(%rbp), %rax",
                                        src_off + field_info.offset
                                    );
                                    let _ = writeln!(
                                        self.output,
                                        "    movq %rax, -{}(%rbp)",
                                        dest_off + field_info.offset
                                    );
                                }
                            }
                        } else if self.enums.contains_key(type_name) {
                            // Enum: copy tag + all 5 qword slots
                            if let (Location::Stack(dest_off), Location::Stack(src_off)) =
                                (loc, expr_loc)
                            {
                                for slot in 0..5u32 {
                                    let _ = writeln!(
                                        self.output,
                                        "    movq -{}(%rbp), %rax",
                                        src_off + slot * 8
                                    );
                                    let _ = writeln!(
                                        self.output,
                                        "    movq %rax, -{}(%rbp)",
                                        dest_off + slot * 8
                                    );
                                }
                            }
                        } else {
                            self.move_to_loc(expr_loc, loc, frame)?;
                        }
                    } else {
                        self.move_to_loc(expr_loc, loc, frame)?;
                    }
                } else {
                    self.emit_store_const(loc, 0);
                }
            }
            Statement::LetDeclaration { name, ty: _, value } => {
                let loc = frame.allocate(name);
                let expr_loc = self.emit_expr(value, frame)?;
                self.move_to_loc(expr_loc, loc, frame)?;
            }
            Statement::ConstDeclaration { name, value } => {
                let loc = frame.allocate(name);
                let expr_loc = self.emit_expr(value, frame)?;
                self.move_to_loc(expr_loc, loc, frame)?;
            }
            Statement::Assignment { target, value } => {
                let target_name = match target {
                    Expression::Identifier(n) => n.clone(),
                    _ => {
                        return Err(
                            "Assembly backend only supports assignment to variables".to_string()
                        )
                    }
                };
                let dest = frame
                    .get(&target_name)
                    .ok_or_else(|| format!("Undefined variable `{target_name}`"))?;
                let expr_loc = self.emit_expr(value, frame)?;
                // For struct types, copy field-by-field.
                if let Some(type_name) = frame.var_types.get(&target_name).cloned() {
                    if let Some(layout) = self.structs.get(&type_name) {
                        if let (Location::Stack(dest_off), Location::Stack(src_off)) =
                            (dest, expr_loc)
                        {
                            let mut fields: Vec<&FieldInfo> = layout.values().collect();
                            fields.sort_by_key(|f| f.offset);
                            for field_info in fields {
                                let _ = writeln!(
                                    self.output,
                                    "    movq -{}(%rbp), %rax",
                                    src_off + field_info.offset
                                );
                                let _ = writeln!(
                                    self.output,
                                    "    movq %rax, -{}(%rbp)",
                                    dest_off + field_info.offset
                                );
                            }
                        }
                    } else {
                        self.move_to_loc(expr_loc, dest, frame)?;
                    }
                } else {
                    self.move_to_loc(expr_loc, dest, frame)?;
                }
            }
            Statement::PutStatement { expr, redirect } => {
                if redirect.is_some() {
                    return Err(
                        "Assembly backend file redirects are not implemented; use a #asm helper"
                            .to_string(),
                    );
                }
                self.emit_put(expr, frame)?;
            }
            Statement::ErrorStatement(expr) => {
                self.emit_put_with_prefix("[ERROR] ", expr, frame)?;
            }
            Statement::WarnStatement(expr) => {
                self.emit_put_with_prefix("[WARN] ", expr, frame)?;
            }
            Statement::InfoStatement(expr) => {
                self.emit_put_with_prefix("[INFO] ", expr, frame)?;
            }
            Statement::ReturnStatement(value) => {
                if let Some(val) = value {
                    let loc = self.emit_expr(val, frame)?;
                    self.move_to_loc(loc, Location::Reg("%rax"), frame)?;
                } else {
                    self.output.push_str("    movq $0, %rax\n");
                }
                self.output.push_str("    movq %rbp, %rsp\n");
                self.output.push_str("    popq %rbp\n");
                self.output.push_str("    retq\n");
            }
            Statement::BreakStatement => {
                let ctx = self.loop_stack.last().ok_or("break outside of loop")?;
                let _ = writeln!(self.output, "    jmp {}", ctx.break_label);
            }
            Statement::ContinueStatement => {
                let ctx = self.loop_stack.last().ok_or("continue outside of loop")?;
                let _ = writeln!(self.output, "    jmp {}", ctx.continue_label);
            }
            Statement::ExpressionStatement(expr) => match expr {
                Expression::IfExpression {
                    condition,
                    then_block,
                    else_block: None,
                } => {
                    let loop_label = self.fresh_label("while");
                    let body_label = self.fresh_label("while_body");
                    let end_label = self.fresh_label("while_end");
                    self.loop_stack.push(LoopContext {
                        continue_label: loop_label.clone(),
                        break_label: end_label.clone(),
                    });
                    let _ = writeln!(self.output, "    jmp {loop_label}");
                    let _ = writeln!(self.output, "{body_label}:");
                    for stmt in then_block {
                        self.emit_statement(&stmt.node, frame)?;
                    }
                    let _ = writeln!(self.output, "{loop_label}:");
                    let cond_loc = self.emit_expr(condition, frame)?;
                    self.load_to_reg(cond_loc, "%rax", frame)?;
                    self.output.push_str("    testq %rax, %rax\n");
                    let _ = writeln!(self.output, "    jnz {body_label}");
                    let _ = writeln!(self.output, "{end_label}:");
                    self.loop_stack.pop();
                }
                Expression::IfExpression {
                    condition,
                    then_block,
                    else_block: Some(else_block),
                } => {
                    let else_label = self.fresh_label("else");
                    let end_label = self.fresh_label("if_end");
                    let cond_loc = self.emit_expr(condition, frame)?;
                    self.load_to_reg(cond_loc, "%rax", frame)?;
                    self.output.push_str("    testq %rax, %rax\n");
                    let _ = writeln!(self.output, "    jz {else_label}");
                    for stmt in then_block {
                        self.emit_statement(&stmt.node, frame)?;
                    }
                    let _ = writeln!(self.output, "    jmp {end_label}");
                    let _ = writeln!(self.output, "{else_label}:");
                    for stmt in else_block {
                        self.emit_statement(&stmt.node, frame)?;
                    }
                    let _ = writeln!(self.output, "{end_label}:");
                }
                Expression::LoopRange {
                    variable,
                    ranges,
                    body,
                } => {
                    if ranges.len() != 1 {
                        return Err(
                            "Assembly backend currently supports one range per loop".to_string()
                        );
                    }
                    let ast::LoopRangePart::Range {
                        start,
                        end,
                        inclusive,
                        step,
                    } = &ranges[0]
                    else {
                        return Err("Assembly backend requires a numeric range loop".to_string());
                    };
                    let descending = step.as_ref().is_some_and(is_negative_expression);
                    let var_loc = frame.allocate(variable);
                    let start_loc = self.emit_expr(start, frame)?;
                    self.move_to_loc(start_loc, var_loc, frame)?;
                    let end_loc = self.emit_expr(end, frame)?;
                    // Save the previous loop's end bound (%r12) before overwriting.
                    let saved_r12_name = self.fresh_label("saved_r12");
                    let saved_r12 = frame.allocate(&saved_r12_name);
                    self.output.push_str("    movq %r12, %rax\n");
                    self.move_to_loc(Location::Reg("%rax"), saved_r12, frame)?;
                    self.move_to_loc(end_loc, Location::Reg("%r12"), frame)?;
                    let step_val: i64 = if let Some(s) = step {
                        if let Expression::IntLiteral(v) = s {
                            v.parse().unwrap_or(1)
                        } else {
                            1
                        }
                    } else {
                        1
                    };

                    let check_label = self.fresh_label("for_check");
                    let body_label = self.fresh_label("for_body");
                    let incr_label = self.fresh_label("for_incr");
                    let end_label = self.fresh_label("for_end");
                    self.loop_stack.push(LoopContext {
                        continue_label: incr_label.clone(),
                        break_label: end_label.clone(),
                    });
                    let _ = writeln!(self.output, "    jmp {check_label}");
                    let _ = writeln!(self.output, "{body_label}:");
                    for stmt in body {
                        self.emit_statement(&stmt.node, frame)?;
                    }
                    let _ = writeln!(self.output, "{incr_label}:");
                    self.load_to_reg(var_loc, "%rax", frame)?;
                    let _ = writeln!(self.output, "    addq ${step_val}, %rax");
                    self.move_to_loc(Location::Reg("%rax"), var_loc, frame)?;
                    let _ = writeln!(self.output, "{check_label}:");
                    self.load_to_reg(var_loc, "%rax", frame)?;
                    self.output.push_str("    cmpq %r12, %rax\n");
                    let jmp = if descending {
                        if *inclusive {
                            "jge"
                        } else {
                            "jg"
                        }
                    } else if *inclusive {
                        "jle"
                    } else {
                        "jl"
                    };
                    let _ = writeln!(self.output, "    {jmp} {body_label}");
                    let _ = writeln!(self.output, "{end_label}:");
                    // Restore the previous loop's end bound.
                    self.load_to_reg(saved_r12, "%r12", frame)?;
                    self.loop_stack.pop();
                }
                Expression::ForLoop { .. } => {
                    return Err("Assembly backend does not support for-in loops yet".to_string());
                }
                Expression::InfiniteLoop(body) => {
                    let loop_label = self.fresh_label("loop");
                    let end_label = self.fresh_label("loop_end");
                    self.loop_stack.push(LoopContext {
                        continue_label: loop_label.clone(),
                        break_label: end_label.clone(),
                    });
                    let _ = writeln!(self.output, "{loop_label}:");
                    for stmt in body {
                        self.emit_statement(&stmt.node, frame)?;
                    }
                    let _ = writeln!(self.output, "    jmp {loop_label}");
                    let _ = writeln!(self.output, "{end_label}:");
                    self.loop_stack.pop();
                }
                _ => {
                    self.emit_expr(expr, frame)?;
                }
            },
            Statement::ForeignBlock { .. } => {
                return Err("Foreign blocks must be at program scope".to_string());
            }
            Statement::ExternFunctionDeclaration(_) => {
                return Err("Extern declarations must be at program scope".to_string());
            }
            Statement::Block(block) => {
                for stmt in block {
                    self.emit_statement(&stmt.node, frame)?;
                }
            }
            Statement::StructDeclaration(decl) => {
                // Record the struct layout for field access.
                let layout = self.compute_struct_layout(&decl.fields);
                self.structs.insert(decl.name.clone(), layout);
                // Emit no code for the declaration itself.
            }
            Statement::EnumDeclaration(decl) => {
                // Assign integer tags to each variant.
                let mut tags = std::collections::HashMap::new();
                for (i, variant) in decl.variants.iter().enumerate() {
                    let name = match variant {
                        ast::EnumVariant::Unit(name) => name.clone(),
                        ast::EnumVariant::Tuple(name, _) => name.clone(),
                        ast::EnumVariant::Struct(name, _) => name.clone(),
                    };
                    tags.insert(name, i as i64);
                }
                self.enums.insert(decl.name.clone(), EnumInfo { tags });
            }
            Statement::FunctionDeclaration(_)
            | Statement::TraitDeclaration(_)
            | Statement::ImplDeclaration(_)
            | Statement::ModuleDeclaration(_)
            | Statement::UseDeclaration(_)
            | Statement::TypeDeclaration(_) => {
                return Err(
                    "This Poly declaration is not supported in an assembly function".to_string(),
                );
            }
        }
        Ok(())
    }

    // -------------------------------------------------------------------
    // Expressions
    // -------------------------------------------------------------------

    fn emit_expr(&mut self, expr: &Expression, frame: &mut StackFrame) -> Result<Location, String> {
        match expr {
            Expression::IntLiteral(value) => {
                let v: i64 = value
                    .parse()
                    .map_err(|_| format!("Invalid integer literal: {value}"))?;
                let loc = frame.allocate(&format!("__imm_{v}"));
                self.emit_store_const(loc, v);
                Ok(loc)
            }
            Expression::BoolLiteral(value) => {
                let v: i64 = if *value { 1 } else { 0 };
                let loc = frame.allocate(&format!("__bool_{v}"));
                self.emit_store_const(loc, v);
                Ok(loc)
            }
            Expression::StringLiteral(value) | Expression::UnicodeStringLiteral(value) => {
                let label = self.intern_string(value);
                let loc = frame.allocate("__str");
                self.emit_string_addr(&label, loc);
                Ok(loc)
            }
            Expression::Identifier(name) => frame
                .get(name)
                .ok_or_else(|| format!("Undefined variable `{name}`")),
            Expression::BinaryOp { op, left, right } => {
                // String concatenation: do NOT emit here (no side effects in
                // emit_expr). The caller (emit_put) handles it.
                if matches!(op, BinaryOp::Add)
                    && (self.is_string_expr(left) || self.is_string_expr(right))
                {
                    let loc = frame.allocate("__concat");
                    self.emit_store_const(loc, 0);
                    return Ok(loc);
                }

                let left_loc = self.emit_expr(left, frame)?;
                let right_loc = self.emit_expr(right, frame)?;

                self.load_to_reg(left_loc, "%rax", frame)?;
                self.load_to_reg(right_loc, "%rcx", frame)?;
                match op {
                    BinaryOp::Add => self.output.push_str("    addq %rcx, %rax\n"),
                    BinaryOp::Sub => self.output.push_str("    subq %rcx, %rax\n"),
                    BinaryOp::Mul => self.output.push_str("    imulq %rcx, %rax\n"),
                    BinaryOp::Div => {
                        self.output.push_str("    cqo\n");
                        self.output.push_str("    idivq %rcx\n");
                    }
                    BinaryOp::Mod => {
                        self.output.push_str("    cqo\n");
                        self.output.push_str("    idivq %rcx\n");
                        self.output.push_str("    movq %rdx, %rax\n");
                    }
                    BinaryOp::Eq => {
                        self.output.push_str("    cmpq %rcx, %rax\n");
                        self.output.push_str("    sete %al\n");
                        self.output.push_str("    movzbq %al, %rax\n");
                    }
                    BinaryOp::NotEq => {
                        self.output.push_str("    cmpq %rcx, %rax\n");
                        self.output.push_str("    setne %al\n");
                        self.output.push_str("    movzbq %al, %rax\n");
                    }
                    BinaryOp::Lt => {
                        self.output.push_str("    cmpq %rcx, %rax\n");
                        self.output.push_str("    setl %al\n");
                        self.output.push_str("    movzbq %al, %rax\n");
                    }
                    BinaryOp::Gt => {
                        self.output.push_str("    cmpq %rcx, %rax\n");
                        self.output.push_str("    setg %al\n");
                        self.output.push_str("    movzbq %al, %rax\n");
                    }
                    BinaryOp::LtEq => {
                        self.output.push_str("    cmpq %rcx, %rax\n");
                        self.output.push_str("    setle %al\n");
                        self.output.push_str("    movzbq %al, %rax\n");
                    }
                    BinaryOp::GtEq => {
                        self.output.push_str("    cmpq %rcx, %rax\n");
                        self.output.push_str("    setge %al\n");
                        self.output.push_str("    movzbq %al, %rax\n");
                    }
                    BinaryOp::And => {
                        self.output.push_str("    testq %rcx, %rcx\n");
                        self.output.push_str("    setnz %cl\n");
                        self.output.push_str("    testq %rax, %rax\n");
                        self.output.push_str("    setnz %al\n");
                        self.output.push_str("    andb %cl, %al\n");
                        self.output.push_str("    movzbq %al, %rax\n");
                    }
                    BinaryOp::Or => {
                        self.output.push_str("    testq %rcx, %rcx\n");
                        self.output.push_str("    setnz %cl\n");
                        self.output.push_str("    testq %rax, %rax\n");
                        self.output.push_str("    setnz %al\n");
                        self.output.push_str("    orb %cl, %al\n");
                        self.output.push_str("    movzbq %al, %rax\n");
                    }
                    BinaryOp::BitAnd => self.output.push_str("    andq %rcx, %rax\n"),
                    BinaryOp::BitOr => self.output.push_str("    orq %rcx, %rax\n"),
                    BinaryOp::BitXor => self.output.push_str("    xorq %rcx, %rax\n"),
                    BinaryOp::Shl => self.output.push_str("    salq %cl, %rax\n"),
                    BinaryOp::Shr => self.output.push_str("    sarq %cl, %rax\n"),
                }
                let loc = frame.allocate("__binop");
                self.move_to_loc(Location::Reg("%rax"), loc, frame)?;
                Ok(loc)
            }
            Expression::UnaryOp { op, expr } => {
                let inner = self.emit_expr(expr, frame)?;
                self.load_to_reg(inner, "%rax", frame)?;
                match op {
                    UnaryOp::Neg => self.output.push_str("    negq %rax\n"),
                    UnaryOp::Not => {
                        self.output.push_str("    testq %rax, %rax\n");
                        self.output.push_str("    setz %al\n");
                        self.output.push_str("    movzbq %al, %rax\n");
                    }
                    UnaryOp::BitNot => self.output.push_str("    notq %rax\n"),
                    UnaryOp::Deref => {
                        return Err(
                            "Assembly backend does not support dereferencing yet".to_string()
                        );
                    }
                }
                let loc = frame.allocate("__unary");
                self.move_to_loc(Location::Reg("%rax"), loc, frame)?;
                Ok(loc)
            }
            Expression::Call { func, args } => {
                let func_name = match func.as_ref() {
                    Expression::Identifier(n) => n.clone(),
                    _ => {
                        return Err(
                            "Assembly backend only supports direct function calls".to_string()
                        )
                    }
                };
                let param_regs: &[&str] = &["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];
                let mut arg_locs = Vec::new();
                for arg in args {
                    arg_locs.push(self.emit_expr(arg, frame)?);
                }
                // Save callee-saved registers and align stack
                self.output.push_str("    pushq %rbx\n");
                self.output.push_str("    subq $8, %rsp\n"); // maintain 16-byte alignment
                for (i, loc) in arg_locs.iter().enumerate() {
                    let reg = param_regs.get(i).copied().unwrap_or("%rdi");
                    self.load_to_reg(*loc, reg, frame)?;
                }
                let _ = writeln!(self.output, "    callq {func_name}");
                self.output.push_str("    addq $8, %rsp\n");
                self.output.push_str("    popq %rbx\n");
                let loc = frame.allocate(&format!("__call_{func_name}"));
                self.move_to_loc(Location::Reg("%rax"), loc, frame)?;
                Ok(loc)
            }
            Expression::Parenthesized(inner) => self.emit_expr(inner, frame),
            Expression::AsExpression { expr, ty } => {
                if self.is_integer_type(ty) {
                    self.emit_expr(expr, frame)
                } else {
                    Err("Assembly backend only supports integer casts".to_string())
                }
            }
            Expression::StructLiteral { name, fields } => {
                let layout_init = self
                    .structs
                    .get(name)
                    .ok_or_else(|| format!("Unknown struct `{name}`"))?
                    .clone();
                let total: u32 = layout_init.values().map(|f| ((f.size + 7) / 8) * 8).sum();
                let base = frame.allocate_struct(&format!("__struct_{name}"), total);
                // Collect (offset, expr) pairs to avoid borrow conflicts.
                let field_offsets: Vec<(u32, Expression)> = {
                    let layout = self
                        .structs
                        .get(name)
                        .ok_or_else(|| format!("Unknown struct `{name}`"))?
                        .clone();
                    let mut result = Vec::new();
                    for (field_name, expr) in fields {
                        let offset = layout
                            .get(field_name.as_str())
                            .ok_or_else(|| format!("Unknown field `{field_name}` on `{name}"))?
                            .offset;
                        result.push((offset, expr.clone()));
                    }
                    result
                };
                for (offset, expr) in field_offsets {
                    let val_loc = self.emit_expr(&expr, frame)?;
                    if let Location::Stack(base_offset) = base {
                        self.load_to_reg(val_loc, "%rax", frame)?;
                        let _ = writeln!(
                            self.output,
                            "    movq %rax, -{}(%rbp)",
                            base_offset + offset
                        );
                    }
                }
                Ok(base)
            }
            Expression::FieldAccess { object, field } => {
                let obj_loc = self.emit_expr(object, frame)?;
                // Determine the struct type from the variable's declared type.
                let struct_name = match object.as_ref() {
                    Expression::Identifier(var_name) => frame.var_types.get(var_name).cloned(),
                    _ => None,
                };
                if let (Location::Stack(base_offset), Some(sname)) = (obj_loc, &struct_name) {
                    let layout = self
                        .structs
                        .get(sname)
                        .ok_or_else(|| format!("Unknown struct `{sname}"))?
                        .clone();
                    let field_info = layout
                        .get(field)
                        .ok_or_else(|| format!("Unknown field `{field}` on `{sname}"))?;
                    let loc = frame.allocate(&format!("__field_{field}"));
                    let _ = writeln!(
                        self.output,
                        "    movq -{}(%rbp), %rax",
                        base_offset + field_info.offset
                    );
                    self.move_to_loc(Location::Reg("%rax"), loc, frame)?;
                    Ok(loc)
                } else {
                    Err(format!(
                        "Cannot access field `{field}`: struct type unknown at codegen time"
                    ))
                }
            }
            Expression::EnumVariant {
                enum_name,
                variant,
                data,
            } => {
                let enum_info = self
                    .enums
                    .get(enum_name)
                    .ok_or_else(|| format!("Unknown enum `{enum_name}"))?
                    .clone();
                let tag = enum_info
                    .tags
                    .get(variant)
                    .ok_or_else(|| format!("Unknown variant `{variant}` on `{enum_name}"))?;
                // Allocate tag (8 bytes) + up to 4 payload slots (32 bytes)
                let loc = frame.allocate_struct(&format!("__enum_{enum_name}_{variant}"), 40);
                self.emit_store_const(loc, *tag);
                // For tuple/struct variant data, store each payload value after the tag.
                if let Some(payload) = data {
                    if let Location::Stack(base_offset) = loc {
                        for (i, expr) in payload.iter().enumerate() {
                            let val_loc = self.emit_expr(expr, frame)?;
                            self.load_to_reg(val_loc, "%rax", frame)?;
                            let _ = writeln!(
                                self.output,
                                "    movq %rax, -{}(%rbp)",
                                base_offset + 8 + (i as u32) * 8
                            );
                        }
                    }
                }
                Ok(loc)
            }
            Expression::MatchExpression { scrutinee, arms } => {
                let scrut_loc = self.emit_expr(scrutinee, frame)?;
                let end_label = self.fresh_label("match_end");
                let mut arm_labels: Vec<String> = Vec::new();
                for i in 0..arms.len() {
                    arm_labels.push(self.fresh_label(&format!("arm_{i}")));
                }
                // Emit pattern tests: each arm falls through to the next
                // on failure, except the last arm which always matches.
                let next_arm_labels: Vec<String> = (0..arms.len())
                    .map(|i| self.fresh_label(&format!("next_arm_{i}")))
                    .collect();
                for (i, arm) in arms.iter().enumerate() {
                    // Emit the fallthrough label for this arm's test block.
                    let _ = writeln!(self.output, "{}:", next_arm_labels[i]);
                    let is_last = i == arms.len() - 1;
                    match &arm.pattern {
                        ast::Pattern::Wildcard => {
                            // Always matches - jump to body.
                            let _ = writeln!(self.output, "    jmp {}", arm_labels[i]);
                        }
                        ast::Pattern::Identifier(_) => {
                            // Binding: always matches, bind in body.
                            let _ = writeln!(self.output, "    jmp {}", arm_labels[i]);
                        }
                        ast::Pattern::Literal(expr) => {
                            let pat_loc = self.emit_expr(expr, frame)?;
                            self.load_to_reg(scrut_loc, "%rax", frame)?;
                            self.load_to_reg(pat_loc, "%rcx", frame)?;
                            self.output.push_str("    cmpq %rcx, %rax\n");
                            if is_last {
                                let _ = writeln!(self.output, "    jmp {}", arm_labels[i]);
                            } else {
                                let _ = writeln!(self.output, "    je {}", arm_labels[i]);
                                let _ = writeln!(self.output, "    jmp {}", next_arm_labels[i + 1]);
                            }
                        }
                        ast::Pattern::Enum {
                            enum_name,
                            variant,
                            inner: _,
                        } => {
                            // Load tag from scrutinee (offset 0) and compare.
                            if let Location::Stack(base) = scrut_loc {
                                let _ = writeln!(self.output, "    movq -{base}(%rbp), %rax");
                                let tag = self
                                    .enums
                                    .get(enum_name)
                                    .and_then(|e| e.tags.get(variant))
                                    .copied()
                                    .unwrap_or(0);
                                let _ = writeln!(self.output, "    cmpq ${tag}, %rax");
                                if is_last {
                                    let _ = writeln!(self.output, "    jmp {}", arm_labels[i]);
                                } else {
                                    let _ = writeln!(self.output, "    je {}", arm_labels[i]);
                                    let _ =
                                        writeln!(self.output, "    jmp {}", next_arm_labels[i + 1]);
                                }
                            } else {
                                let _ = writeln!(self.output, "    jmp {}", arm_labels[i]);
                            }
                        }
                        ast::Pattern::Tuple(_patterns) => {
                            // Tuple pattern: just jump (partial support).
                            let _ = writeln!(self.output, "    jmp {}", arm_labels[i]);
                        }
                        _ => {
                            // Other patterns: treat as wildcard for now.
                            let _ = writeln!(self.output, "    jmp {}", arm_labels[i]);
                        }
                    }
                }

                // Emit arm bodies.
                let result_loc = frame.allocate("__match_result");
                for (i, arm) in arms.iter().enumerate() {
                    let _ = writeln!(self.output, "{}:", arm_labels[i]);
                    // Bind pattern variables before executing the body.
                    self.bind_pattern_vars(&arm.pattern, scrut_loc, frame)?;
                    match &arm.body {
                        ast::MatchArmBody::Expression(expr) => {
                            let loc = self.emit_expr(expr, frame)?;
                            self.move_to_loc(loc, result_loc, frame)?;
                        }
                        ast::MatchArmBody::Block(block) => {
                            for stmt in block {
                                self.emit_statement(&stmt.node, frame)?;
                            }
                        }
                    }
                    let _ = writeln!(self.output, "    jmp {end_label}");
                }
                let _ = writeln!(self.output, "{end_label}:");
                Ok(result_loc)
            }
            Expression::TupleIndex { .. }
            | Expression::Index { .. }
            | Expression::Range { .. }
            | Expression::LoopRange { .. }
            | Expression::ForLoop { .. }
            | Expression::InfiniteLoop(_)
            | Expression::IfExpression { .. }
            | Expression::Closure { .. }
            | Expression::TryExpression(_)
            | Expression::GetExpression(_)
            | Expression::UnsafeBlock(_)
            | Expression::MethodCall { .. }
            | Expression::TupleLiteral(_)
            | Expression::ArrayLiteral(_)
            | Expression::UnicodeCharLiteral(_)
            | Expression::FloatLiteral(_)
            | Expression::ByteLiteral(_) => Err(
                "Expression not supported by the assembly backend; use a #asm helper".to_string(),
            ),
        }
    }

    // -------------------------------------------------------------------
    // `put` via sys_write (syscall 1): fd=1 (stdout), buf=%rsi, len=%rdx
    // -------------------------------------------------------------------

    /// Bind variables introduced by a match pattern.
    fn bind_pattern_vars(
        &mut self,
        pattern: &ast::Pattern,
        scrut_loc: Location,
        frame: &mut StackFrame,
    ) -> Result<(), String> {
        match pattern {
            ast::Pattern::Identifier(name) => {
                let loc = frame.allocate(name);
                self.move_to_loc(scrut_loc, loc, frame)?;
            }
            ast::Pattern::Enum { inner, .. } => {
                if let Some(inner_patterns) = inner {
                    if let Location::Stack(base) = scrut_loc {
                        // Each payload value is at base + 8 + i * 8
                        for (i, pat) in inner_patterns.iter().enumerate() {
                            let data_loc = Location::Stack(base + 8 + (i as u32) * 8);
                            self.bind_pattern_vars(pat, data_loc, frame)?;
                        }
                    }
                }
            }
            ast::Pattern::Tuple(patterns) => {
                if let Location::Stack(base) = scrut_loc {
                    for (j, pat) in patterns.iter().enumerate() {
                        let elem_loc = Location::Stack(base + (j as u32 + 1) * 8);
                        self.bind_pattern_vars(pat, elem_loc, frame)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn emit_put(&mut self, expr: &Expression, frame: &mut StackFrame) -> Result<(), String> {
        // String-typed expressions: emit the value via sys_write, then a newline.
        if self.is_string_typed_expr(expr) {
            self.emit_string_typed_put(expr, frame)?;
            return Ok(());
        }
        // A bare string-typed variable holds a pointer, not an integer: printing
        // it through _print_int would emit the address. Route it through the
        // strlen-based writer instead.
        if let Expression::Identifier(name) = expr {
            if let Some(type_name) = frame.var_types.get(name).cloned() {
                if type_name == "string" || type_name == "ustring" {
                    self.emit_string_concat(expr, frame)?;
                    self.emit_newline();
                    return Ok(());
                }
            }
        }
        // Integer / boolean expressions: convert to decimal and write.
        let loc = self.emit_expr(expr, frame)?;
        self.load_to_reg(loc, "%rdi", frame)?;
        self.output.push_str("    # put (integer)\n");
        self.output.push_str("    callq _print_int\n");
        Ok(())
    }

    fn emit_newline(&mut self) {
        self.output.push_str("    movq $1, %rax\n");
        self.output.push_str("    movq $1, %rdi\n");
        self.output.push_str("    leaq _newline(%rip), %rsi\n");
        self.output.push_str("    movq $1, %rdx\n");
        self.output.push_str("    syscall\n");
    }

    /// Emit a `put` for a string-typed expression: write value then newline.
    fn emit_string_typed_put(
        &mut self,
        expr: &Expression,
        frame: &mut StackFrame,
    ) -> Result<(), String> {
        match expr {
            Expression::StringLiteral(value) | Expression::UnicodeStringLiteral(value) => {
                let label = self.intern_string(value);
                let len = asm_escape(value).len() as i64;
                self.output.push_str("    # put (string literal)\n");
                let _ = writeln!(self.output, "    movq $1, %rax");
                self.output.push_str("    movq $1, %rdi\n");
                let _ = writeln!(self.output, "    leaq {label}(%rip), %rsi");
                let _ = writeln!(self.output, "    movq ${len}, %rdx");
                self.output.push_str("    syscall\n");
            }
            Expression::BinaryOp {
                op: BinaryOp::Add,
                left,
                right,
            } => {
                self.output.push_str("    # put (string concat)\n");
                self.emit_string_concat(left, frame)?;
                self.emit_string_concat(right, frame)?;
            }
            _ => {
                // Fallback: treat as integer
                let loc = self.emit_expr(expr, frame)?;
                self.load_to_reg(loc, "%rdi", frame)?;
                self.output.push_str("    # put (integer)\n");
                self.output.push_str("    callq _print_int\n");
                return Ok(());
            }
        }
        // Trailing newline
        self.emit_newline();
        Ok(())
    }

    fn emit_put_with_prefix(
        &mut self,
        prefix: &str,
        expr: &Expression,
        frame: &mut StackFrame,
    ) -> Result<(), String> {
        // Write prefix string
        let label = self.intern_string(prefix);
        let len = asm_escape(prefix).len() as i64;
        let _ = writeln!(self.output, "    movq $1, %rax");
        self.output.push_str("    movq $1, %rdi\n");
        let _ = writeln!(self.output, "    leaq {label}(%rip), %rsi");
        let _ = writeln!(self.output, "    movq ${len}, %rdx");
        self.output.push_str("    syscall\n");
        // Write value
        self.emit_put(expr, frame)
    }

    /// Emit a string piece for concatenation (no newline).
    fn emit_string_concat(
        &mut self,
        expr: &Expression,
        frame: &mut StackFrame,
    ) -> Result<(), String> {
        match expr {
            Expression::StringLiteral(value) | Expression::UnicodeStringLiteral(value) => {
                let label = self.intern_string(value);
                let len = asm_escape(value).len() as i64;
                self.output.push_str("    movq $1, %rax\n");
                self.output.push_str("    movq $1, %rdi\n");
                let _ = writeln!(self.output, "    leaq {label}(%rip), %rsi");
                let _ = writeln!(self.output, "    movq ${len}, %rdx");
                self.output.push_str("    syscall\n");
                Ok(())
            }
            Expression::Identifier(name) => {
                // String variable: we need strlen. For simplicity, use sys_write
                // with a generous max length; the buffer is NUL-terminated.
                let loc = frame
                    .get(name)
                    .ok_or_else(|| format!("Undefined variable `{name}`"))?;
                self.load_to_reg(loc, "%r8", frame)?; // r8 = string ptr
                                                      // strlen loop
                let loop_label = self.fresh_label("strlen");
                let end_label = self.fresh_label("strlen_end");
                self.output.push_str("    xorq %rdx, %rdx\n"); // length counter
                let _ = writeln!(self.output, "{loop_label}:");
                self.output.push_str("    movb (%r8,%rdx), %al\n");
                self.output.push_str("    testb %al, %al\n");
                let _ = writeln!(self.output, "    jz {end_label}");
                self.output.push_str("    incq %rdx\n");
                let _ = writeln!(self.output, "    jmp {loop_label}");
                let _ = writeln!(self.output, "{end_label}:");
                // sys_write
                self.output.push_str("    movq $1, %rax\n");
                self.output.push_str("    movq $1, %rdi\n");
                self.output.push_str("    movq %r8, %rsi\n");
                // rdx already has length
                self.output.push_str("    syscall\n");
                Ok(())
            }
            Expression::BinaryOp {
                op: BinaryOp::Add,
                left,
                right,
            } => {
                self.emit_string_concat(left, frame)?;
                self.emit_string_concat(right, frame)?;
                Ok(())
            }
            _ => {
                // Fall back: treat as integer
                let loc = self.emit_expr(expr, frame)?;
                self.load_to_reg(loc, "%rdi", frame)?;
                self.output.push_str("    callq _print_int_nobuf\n");
                Ok(())
            }
        }
    }

    fn is_string_expr(&self, expr: &Expression) -> bool {
        matches!(
            expr,
            Expression::StringLiteral(_) | Expression::UnicodeStringLiteral(_)
        )
    }

    /// Check whether an expression evaluates to a string-typed value.
    fn is_string_typed_expr(&self, expr: &Expression) -> bool {
        match expr {
            Expression::StringLiteral(_) | Expression::UnicodeStringLiteral(_) => true,
            Expression::BinaryOp {
                op: BinaryOp::Add,
                left,
                right,
            } => self.is_string_typed_expr(left) || self.is_string_typed_expr(right),
            _ => false,
        }
    }

    // -------------------------------------------------------------------
    // Register / memory helpers
    // -------------------------------------------------------------------

    fn emit_store_const(&mut self, loc: Location, value: i64) {
        match loc {
            Location::Stack(offset) => {
                let _ = writeln!(self.output, "    movq ${value}, -{offset}(%rbp)");
            }
            Location::Reg(reg) => {
                let _ = writeln!(self.output, "    movq ${value}, {reg}");
            }
        }
    }

    fn emit_string_addr(&mut self, label: &str, loc: Location) {
        match loc {
            Location::Stack(offset) => {
                let _ = writeln!(self.output, "    leaq {label}(%rip), %rax");
                let _ = writeln!(self.output, "    movq %rax, -{offset}(%rbp)");
            }
            Location::Reg(reg) => {
                let _ = writeln!(self.output, "    leaq {label}(%rip), {reg}");
            }
        }
    }

    fn move_to_loc(
        &mut self,
        src: Location,
        dst: Location,
        _frame: &StackFrame,
    ) -> Result<(), String> {
        match (src, dst) {
            (Location::Stack(s), Location::Stack(d)) => {
                let _ = writeln!(self.output, "    movq -{s}(%rbp), %rax");
                let _ = writeln!(self.output, "    movq %rax, -{d}(%rbp)");
            }
            (Location::Stack(s), Location::Reg(d)) => {
                let _ = writeln!(self.output, "    movq -{s}(%rbp), {d}");
            }
            (Location::Reg(s), Location::Stack(d)) => {
                let _ = writeln!(self.output, "    movq {s}, -{d}(%rbp)");
            }
            (Location::Reg(s), Location::Reg(d)) => {
                if s != d {
                    let _ = writeln!(self.output, "    movq {s}, {d}");
                }
            }
        }
        Ok(())
    }

    fn load_to_reg(&mut self, loc: Location, reg: &str, _frame: &StackFrame) -> Result<(), String> {
        match loc {
            Location::Stack(offset) => {
                let _ = writeln!(self.output, "    movq -{offset}(%rbp), {reg}");
            }
            Location::Reg(src) => {
                if src != reg {
                    let _ = writeln!(self.output, "    movq {src}, {reg}");
                }
            }
        }
        Ok(())
    }

    // -------------------------------------------------------------------
    // Type helpers
    // -------------------------------------------------------------------

    fn is_integer_type(&self, ty: &TypeAnnotation) -> bool {
        matches!(ty, TypeAnnotation::Named(name) if matches!(
            name.as_str(),
            "i8" | "u8" | "i16" | "u16" | "i32" | "u32" | "i64" | "u64" | "byte" | "char" | "usize"
        ))
    }

    fn is_bool_type(&self, ty: &TypeAnnotation) -> bool {
        matches!(ty, TypeAnnotation::Named(name) if name == "bool")
    }

    fn is_string_type(&self, ty: &TypeAnnotation) -> bool {
        matches!(ty, TypeAnnotation::Named(name) if name == "string" || name == "ustring")
    }

    fn is_struct_type(&self, ty: &TypeAnnotation) -> bool {
        matches!(ty, TypeAnnotation::Named(name) if self.structs.contains_key(name))
    }

    fn is_enum_type(&self, ty: &TypeAnnotation) -> bool {
        matches!(ty, TypeAnnotation::Named(name) if self.enums.contains_key(name))
    }

    /// Return the size in bytes for a simple type.
    fn type_size(&self, ty: &TypeAnnotation) -> u32 {
        match ty {
            TypeAnnotation::Named(name) => match name.as_str() {
                "i8" | "u8" | "byte" | "char" | "bool" => 1,
                "i16" | "u16" => 2,
                "i32" | "u32" | "f32" => 4,
                "i64" | "u64" | "f64" | "usize" | "isize" => 8,
                "string" | "ustring" => 8, // pointer
                _ => {
                    // Struct type: sum of field sizes
                    self.structs
                        .get(name)
                        .map_or(8, |fields| fields.values().map(|f| f.size).sum::<u32>())
                }
            },
            _ => 8, // pointers
        }
    }

    /// Return the layout for a struct type, computing field offsets.
    fn compute_struct_layout(
        &self,
        fields: &[ast::StructField],
    ) -> std::collections::HashMap<String, FieldInfo> {
        let mut layout = std::collections::HashMap::new();
        let mut offset: u32 = 0;
        for field in fields {
            let size = self.type_size(&field.ty);
            // Each field occupies a full 8-byte slot (movq granularity)
            layout.insert(field.name.clone(), FieldInfo { offset, size });
            offset += 8;
        }
        layout
    }
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

fn asm_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use poly_lexer::Lexer;
    use poly_parser::Parser;

    fn transpile_source(source: &str) -> String {
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty(), "{errors:?}");
        let mut parser = Parser::new(&tokens);
        let program = parser.parse().unwrap();
        transpile(&program).unwrap()
    }

    #[test]
    fn emits_asm_header_and_exit() {
        let output = transpile_source("put 42");
        assert!(output.contains(".globl _start"));
        assert!(output.contains("movq $60")); // sys_exit
        assert!(output.contains("callq _print_int"));
    }

    #[test]
    fn emits_asm_foreign_block() {
        let output = transpile_source("#asm\nmy_func:\n    retq\n#endasm\nvar x i32 := 1\nput x");
        assert!(output.contains("my_func:"));
    }

    #[test]
    fn supports_integer_arithmetic() {
        let output = transpile_source("var x i32 := 10 + 20\nput x");
        assert!(output.contains("addq"));
    }

    #[test]
    fn supports_string_put() {
        let output = transpile_source(r#"put "hello world""#);
        assert!(output.contains("sys_write") || output.contains("movq $1, %rax"));
        assert!(output.contains(".asciz"));
    }

    #[test]
    fn supports_string_variable() {
        let output = transpile_source(
            r#"var name string := "Poly"
put name"#,
        );
        // String variables hold pointers: put must use the strlen-based
        // sys_write path, never _print_int (which would print the address).
        assert!(output.contains("leaq str_"));
        assert!(!output.contains("callq _print_int"));
    }

    #[test]
    fn supports_if_else() {
        let output =
            transpile_source("var x i32 := 10\nif x > 5,\n    put 1\nelse\n    put 0\nend if");
        assert!(output.contains("jz"));
        assert!(output.contains("jmp"));
    }

    #[test]
    fn supports_while_loop() {
        let output = transpile_source("var i i32 := 0\nwhile i < 5\n    i := i + 1\nend while");
        assert!(output.contains("while"));
        assert!(output.contains("cmpq"));
    }

    #[test]
    fn supports_loop_range() {
        let output = transpile_source("loop i 0..10\n    put i\nend loop");
        assert!(output.contains("for"));
        assert!(output.contains("addq"));
    }

    #[test]
    fn supports_boolean_expressions() {
        let output = transpile_source(
            "var a bool := true\nvar b bool := false\nvar c bool := a != b\nput c",
        );
        assert!(output.contains("setne"));
        assert!(output.contains("movzbq"));
    }

    #[test]
    fn supports_string_concatenation_put() {
        let output = transpile_source(r#"put "hello " + "world""#);
        // Should emit multiple sys_write calls for each piece
        assert!(output.contains("_newline"));
    }

    #[test]
    fn supports_error_put() {
        let output = transpile_source(r#"error "something failed""#);
        assert!(output.contains("[ERROR]"));
    }

    #[test]
    fn rejects_unsupported_types() {
        let err = transpile_source_err(r#"var x f64 := 3.14"#);
        assert!(err.contains("does not support type"));
    }

    #[test]
    fn uses_syscalls_not_libc() {
        let output = transpile_source("put 42");
        // Should use syscall, not printf
        assert!(output.contains("syscall"));
        assert!(!output.contains("callq printf"));
    }

    #[test]
    fn function_emission() {
        let output =
            transpile_source("fn double(v: i32): i32\n    return v * 2\nend fn\nput double(21)");
        assert!(output.contains(".globl double"));
        assert!(output.contains("double:"));
        assert!(output.contains("retq"));
    }

    fn transpile_source_err(source: &str) -> String {
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty(), "{errors:?}");
        let mut parser = Parser::new(&tokens);
        let program = parser.parse().unwrap();
        transpile(&program).unwrap_err()
    }
}

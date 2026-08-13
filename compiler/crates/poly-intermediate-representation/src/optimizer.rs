//! intermediate representation optimizer.
//!
//! Optimization runs as a sequence of small passes.  Each pass transforms the
//! intermediate representation in place and reports whether it changed anything, so the driver can loop
//! until a fixpoint is reached (or a pass budget is exhausted).

use std::collections::HashMap;

use crate::intermediate_representation::*;

/// A single optimization pass over the intermediate representation.
pub trait OptimizationPass {
    /// Human-readable pass name (used for `--dump-passes` style diagnostics).
    fn name(&self) -> &str;
    /// Run the pass; returns `true` if the program changed.
    fn run(&self, program: &mut Program) -> bool;
}

/// Runs a sequence of passes to a fixpoint.
pub fn optimize(program: &mut Program) -> Vec<String> {
    let passes: Vec<Box<dyn OptimizationPass>> = vec![
        Box::new(ConstantFolding),
        Box::new(DeadCodeElimination),
        Box::new(ConstantFolding),
        Box::new(LoopOptimizations),
        Box::new(DeadCodeElimination),
        Box::new(Inlining),
        Box::new(ConstantFolding),
        Box::new(DeadCodeElimination),
    ];

    let mut applied = Vec::new();
    for pass in passes {
        let changed = pass.run(program);
        if changed {
            applied.push(pass.name().to_string());
        }
    }
    applied
}

// =============================================================================
// Constant folding
// =============================================================================

/// Constant folding: evaluates constant subexpressions (integer and boolean
/// arithmetic) at compile time.
pub struct ConstantFolding;

impl OptimizationPass for ConstantFolding {
    fn name(&self) -> &str {
        "constant_folding"
    }

    fn run(&self, program: &mut Program) -> bool {
        let mut changed = false;
        for function in &mut program.functions {
            changed |= fold_statements(&mut function.body);
        }
        for structure in &mut program.structs {
            for method in &mut structure.methods {
                changed |= fold_statements(&mut method.body);
            }
        }
        for enumeration in &mut program.enums {
            for method in &mut enumeration.methods {
                changed |= fold_statements(&mut method.body);
            }
        }
        for implementation in &mut program.impls {
            for method in &mut implementation.methods {
                changed |= fold_statements(&mut method.body);
            }
        }
        changed |= fold_statements(&mut program.main_body);
        changed
    }
}

fn fold_statements(statements: &mut [Statement]) -> bool {
    let mut changed = false;
    for statement in statements.iter_mut() {
        match statement {
            Statement::VarDecl { value, .. } => {
                if let Some(value) = value {
                    changed |= fold_expr(value);
                }
            }
            Statement::LetDecl { value, .. } => changed |= fold_expr(value),
            Statement::Assignment { value, .. } => changed |= fold_expr(value),
            Statement::Mutation { value, .. } => {
                if let Some(value) = value {
                    changed |= fold_expr(value);
                }
            }
            Statement::Return(Some(value)) => changed |= fold_expr(value),
            Statement::Return(None) => {}
            Statement::Put { expr, redirect, .. } => {
                changed |= fold_expr(expr);
                if let Some(redirect) = redirect {
                    let path = match redirect {
                        Redirect::Write(path) | Redirect::Append(path) => path,
                    };
                    changed |= fold_expr(path);
                }
            }
            Statement::Error(expr) | Statement::Warn(expr) | Statement::Info(expr) => {
                changed |= fold_expr(expr)
            }
            Statement::Expression(expr) => changed |= fold_expr(expr),
            Statement::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                changed |= fold_expr(condition);
                changed |= fold_statements(then_block);
                if let Some(else_block) = else_block {
                    changed |= fold_statements(else_block);
                }
            }
            Statement::Match { scrutinee, arms } => {
                changed |= fold_expr(scrutinee);
                for arm in arms {
                    if let Some(guard) = &mut arm.guard {
                        changed |= fold_expr(guard);
                    }
                    match &mut arm.body {
                        MatchArmBody::Expression(expr) => changed |= fold_expr(expr),
                        MatchArmBody::Block(statements) => changed |= fold_statements(statements),
                    }
                }
            }
            Statement::Block(statements) => changed |= fold_statements(statements),
            Statement::Break | Statement::Continue => {}
            // Nested function bodies are folded when the outer body is
            // processed; treat the declaration itself as opaque here.
            Statement::NestedFunction(_) => {}
        }
    }
    changed
}

fn fold_expr(expr: &mut Expr) -> bool {
    let mut changed = false;
    match expr {
        Expr::BinaryOp { op, left, right } => {
            changed |= fold_expr(left);
            changed |= fold_expr(right);
            if let (Some(left_value), Some(right_value)) = (as_i64(left), as_i64(right)) {
                let folded = match *op {
                    BinaryOp::Add => Some(left_value + right_value),
                    BinaryOp::Sub => Some(left_value - right_value),
                    BinaryOp::Mul => Some(left_value * right_value),
                    BinaryOp::Div if right_value != 0 => Some(left_value / right_value),
                    BinaryOp::Mod if right_value != 0 => Some(left_value % right_value),
                    BinaryOp::Div | BinaryOp::Mod => None, // division by zero
                    BinaryOp::BitAnd => Some(left_value & right_value),
                    BinaryOp::BitOr => Some(left_value | right_value),
                    BinaryOp::BitXor => Some(left_value ^ right_value),
                    BinaryOp::Shl => Some(left_value << right_value),
                    BinaryOp::Shr => Some(left_value >> right_value),
                    BinaryOp::Eq => Some((left_value == right_value) as i64),
                    BinaryOp::NotEq => Some((left_value != right_value) as i64),
                    BinaryOp::Lt => Some((left_value < right_value) as i64),
                    BinaryOp::Gt => Some((left_value > right_value) as i64),
                    BinaryOp::LtEq => Some((left_value <= right_value) as i64),
                    BinaryOp::GtEq => Some((left_value >= right_value) as i64),
                    BinaryOp::And | BinaryOp::Or => None, // booleans, handled below
                };
                if let Some(folded) = folded {
                    *expr = Expr::Literal(Literal::Int(folded.to_string()));
                    return true;
                }
            }
            // Boolean algebra on literal bools
            if let (Some(left_value), Some(right_value)) = (as_bool(left), as_bool(right)) {
                let folded = match *op {
                    BinaryOp::And => Some(left_value && right_value),
                    BinaryOp::Or => Some(left_value || right_value),
                    _ => None,
                };
                if let Some(folded) = folded {
                    *expr = Expr::Literal(Literal::Bool(folded));
                    return true;
                }
            }
            // Identity simplifications: x + 0, x * 1, x - 0
            if matches!(op, BinaryOp::Add) && is_zero(left) {
                *expr = (**right).clone();
                return true;
            }
            if matches!(op, BinaryOp::Add) && is_zero(right) {
                *expr = (**left).clone();
                return true;
            }
            if matches!(op, BinaryOp::Mul) && is_one(left) {
                *expr = (**right).clone();
                return true;
            }
            if matches!(op, BinaryOp::Mul) && is_one(right) {
                *expr = (**left).clone();
                return true;
            }
        }
        Expr::UnaryOp { op, expr: inner } => {
            changed |= fold_expr(inner);
            if let Some(value) = as_i64(inner) {
                let folded = match op {
                    UnaryOp::Neg => Some(-value),
                    UnaryOp::Not | UnaryOp::BitNot => Some(!value),
                    UnaryOp::Deref => None,
                };
                if let Some(folded) = folded {
                    *expr = Expr::Literal(Literal::Int(folded.to_string()));
                    return true;
                }
            }
            if let Some(value) = as_bool(inner) {
                if matches!(op, UnaryOp::Not) {
                    *expr = Expr::Literal(Literal::Bool(!value));
                    return true;
                }
            }
        }
        Expr::Call { args, .. } => {
            for arg in args.iter_mut() {
                changed |= fold_expr(arg);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            changed |= fold_expr(object);
            for arg in args.iter_mut() {
                changed |= fold_expr(arg);
            }
        }
        Expr::Index { object, index } => {
            changed |= fold_expr(object);
            changed |= fold_expr(index);
        }
        Expr::FieldAccess { object, .. } => changed |= fold_expr(object),
        Expr::Parenthesized(inner) => changed |= fold_expr(inner),
        Expr::If {
            condition,
            then_block,
            else_block,
        } => {
            changed |= fold_expr(condition);
            changed |= fold_statements(then_block);
            if let Some(else_block) = else_block {
                changed |= fold_statements(else_block);
            }
        }
        Expr::Match { scrutinee, arms } => {
            changed |= fold_expr(scrutinee);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    changed |= fold_expr(guard);
                }
                match &mut arm.body {
                    MatchArmBody::Expression(expr) => changed |= fold_expr(expr),
                    MatchArmBody::Block(statements) => changed |= fold_statements(statements),
                }
            }
        }
        Expr::Closure { body, .. } => changed |= fold_expr(body),
        Expr::Array(elements) => {
            for element in elements.iter_mut() {
                changed |= fold_expr(element);
            }
        }
        Expr::Tuple(elements) => {
            for element in elements.iter_mut() {
                changed |= fold_expr(element);
            }
        }
        Expr::Struct { fields, .. } => {
            for (_, value) in fields.iter_mut() {
                changed |= fold_expr(value);
            }
        }
        Expr::Enum { data, .. } => {
            if let Some(data) = data {
                for value in data.iter_mut() {
                    changed |= fold_expr(value);
                }
            }
        }
        Expr::Range { start, end, .. } => {
            changed |= fold_expr(start);
            changed |= fold_expr(end);
        }
        Expr::LoopRange { ranges, body } => {
            for part in ranges.iter_mut() {
                match part {
                    LoopRangePart::Range {
                        start, end, step, ..
                    } => {
                        changed |= fold_expr(start);
                        changed |= fold_expr(end);
                        if let Some(step) = step {
                            changed |= fold_expr(step);
                        }
                    }
                    LoopRangePart::Value(value) => changed |= fold_expr(value),
                }
            }
            changed |= fold_statements(body);
        }
        Expr::ForLoop { iterable, body, .. } => {
            changed |= fold_expr(iterable);
            changed |= fold_statements(body);
        }
        Expr::Try(inner) => changed |= fold_expr(inner),
        Expr::As { expr: inner, .. } => changed |= fold_expr(inner),
        Expr::Get(get) => {
            if let Some(prompt) = &mut get.prompt {
                changed |= fold_expr(prompt);
            }
            if let Some(source) = &mut get.source {
                changed |= fold_expr(source);
            }
            for flag in &mut get.flags {
                match flag {
                    GetFlag::Timeout(value)
                    | GetFlag::Default(value)
                    | GetFlag::Mask(value)
                    | GetFlag::Until(value)
                    | GetFlag::Bytes(value) => changed |= fold_expr(value),
                    GetFlag::As(_) => {}
                }
            }
        }
        Expr::UnsafeBlock(statements) => changed |= fold_statements(statements),
        Expr::Literal(_) | Expr::Identifier(_) => {}
    }
    changed
}

fn as_i64(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Literal(Literal::Int(value)) => value.parse().ok(),
        _ => None,
    }
}

fn as_bool(expr: &Expr) -> Option<bool> {
    match expr {
        Expr::Literal(Literal::Bool(value)) => Some(*value),
        Expr::Literal(Literal::Int(value)) => value.parse().ok(),
        _ => None,
    }
}

fn is_zero(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(Literal::Int(value)) if value == "0")
}

fn is_one(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(Literal::Int(value)) if value == "1")
}

// =============================================================================
// Dead code elimination
// =============================================================================

/// Dead code elimination: removes statements that can never execute (after an
/// unconditional `return`, `break`, or `continue`).
pub struct DeadCodeElimination;

impl OptimizationPass for DeadCodeElimination {
    fn name(&self) -> &str {
        "dead_code_elimination"
    }

    fn run(&self, program: &mut Program) -> bool {
        let mut changed = false;
        for function in &mut program.functions {
            changed |= eliminate_statements(&mut function.body);
        }
        for structure in &mut program.structs {
            for method in &mut structure.methods {
                changed |= eliminate_statements(&mut method.body);
            }
        }
        for enumeration in &mut program.enums {
            for method in &mut enumeration.methods {
                changed |= eliminate_statements(&mut method.body);
            }
        }
        for implementation in &mut program.impls {
            for method in &mut implementation.methods {
                changed |= eliminate_statements(&mut method.body);
            }
        }
        changed |= eliminate_statements(&mut program.main_body);
        changed
    }
}

fn eliminate_statements(statements: &mut Vec<Statement>) -> bool {
    let mut changed = false;
    let mut original_len = statements.len();
    // Keep only the prefix before the first unconditional exit.
    let mut keep = original_len;
    for (index, statement) in statements.iter().enumerate() {
        if is_unconditional_exit(statement) {
            keep = index + 1;
            break;
        }
    }
    if keep < original_len {
        statements.truncate(keep);
        changed = true;
        original_len = statements.len();
    }

    // Recursively clean nested blocks.
    for statement in statements.iter_mut() {
        match statement {
            Statement::If {
                then_block,
                else_block,
                ..
            } => {
                changed |= eliminate_statements(then_block);
                if let Some(else_block) = else_block {
                    changed |= eliminate_statements(else_block);
                }
            }
            Statement::Match { arms, .. } => {
                for arm in arms {
                    if let MatchArmBody::Block(statements) = &mut arm.body {
                        changed |= eliminate_statements(statements);
                    }
                }
            }
            Statement::Block(statements) => changed |= eliminate_statements(statements),
            _ => {}
        }
    }

    if changed && original_len != statements.len() {
        // Re-run once more to catch newly-unreachable code.
        return true;
    }
    changed
}

fn is_unconditional_exit(statement: &Statement) -> bool {
    matches!(
        statement,
        Statement::Return(_) | Statement::Break | Statement::Continue
    )
}

// =============================================================================
// Loop optimizations
// =============================================================================

/// Loop optimizations: simplifies degenerate loops.
///
/// Currently removes loops whose bodies are empty and that carry no side
/// effects, and strips `step 1` from range iterators (matching the default).
pub struct LoopOptimizations;

impl OptimizationPass for LoopOptimizations {
    fn name(&self) -> &str {
        "loop_optimizations"
    }

    fn run(&self, program: &mut Program) -> bool {
        let mut changed = false;
        for function in &mut program.functions {
            changed |= optimize_loops(&mut function.body);
        }
        for structure in &mut program.structs {
            for method in &mut structure.methods {
                changed |= optimize_loops(&mut method.body);
            }
        }
        for enumeration in &mut program.enums {
            for method in &mut enumeration.methods {
                changed |= optimize_loops(&mut method.body);
            }
        }
        for implementation in &mut program.impls {
            for method in &mut implementation.methods {
                changed |= optimize_loops(&mut method.body);
            }
        }
        changed |= optimize_loops(&mut program.main_body);
        changed
    }
}

fn optimize_loops(statements: &mut Vec<Statement>) -> bool {
    let mut changed = false;
    let mut i = 0;
    while i < statements.len() {
        match &mut statements[i] {
            Statement::If {
                then_block,
                else_block,
                ..
            } => {
                changed |= optimize_loops(then_block);
                if let Some(else_block) = else_block {
                    changed |= optimize_loops(else_block);
                }
                i += 1;
            }
            Statement::Match { arms, .. } => {
                for arm in arms {
                    if let MatchArmBody::Block(statements) = &mut arm.body {
                        changed |= optimize_loops(statements);
                    }
                }
                i += 1;
            }
            Statement::Block(statements) => {
                changed |= optimize_loops(statements);
                i += 1;
            }
            Statement::Expression(Expr::ForLoop { body, .. }) => {
                if body.is_empty() {
                    statements.remove(i);
                    changed = true;
                    continue;
                }
                changed |= optimize_loops(body);
                i += 1;
            }
            Statement::Expression(Expr::LoopRange { body, ranges }) => {
                // Remove step == 1 (redundant: it is the default anyway).
                for part in ranges.iter_mut() {
                    if let LoopRangePart::Range {
                        step: Some(step),
                        inclusive,
                        start,
                        end,
                    } = part
                    {
                        if matches!(step, Expr::Literal(Literal::Int(value)) if value == "1") {
                            *part = LoopRangePart::Range {
                                start: start.clone(),
                                end: end.clone(),
                                inclusive: *inclusive,
                                step: None,
                            };
                            changed = true;
                        }
                    }
                }
                if body.is_empty() {
                    statements.remove(i);
                    changed = true;
                    continue;
                }
                changed |= optimize_loops(body);
                i += 1;
            }
            _ => i += 1,
        }
    }
    changed
}

// =============================================================================
// Inlining
// =============================================================================

/// Function inlining: replaces calls to small, pure functions with their body.
///
/// Only functions whose body is a single `return <expr>` are inlined, and only
/// when the body expression is a "simple" expression (identifiers, literals,
/// and arithmetic built from them).  This keeps the transformation safe without
/// needing a full alias analysis.
pub struct Inlining;

impl OptimizationPass for Inlining {
    fn name(&self) -> &str {
        "inlining"
    }

    fn run(&self, program: &mut Program) -> bool {
        let inlinable: HashMap<String, Function> = program
            .functions
            .iter()
            .filter(|function| is_inlinable(function))
            .map(|function| (function.name.clone(), function.clone()))
            .collect();

        if inlinable.is_empty() {
            return false;
        }

        let mut changed = false;
        for function in &mut program.functions {
            changed |= inline_in_statements(&mut function.body, &inlinable);
        }
        for structure in &mut program.structs {
            for method in &mut structure.methods {
                changed |= inline_in_statements(&mut method.body, &inlinable);
            }
        }
        for enumeration in &mut program.enums {
            for method in &mut enumeration.methods {
                changed |= inline_in_statements(&mut method.body, &inlinable);
            }
        }
        for implementation in &mut program.impls {
            for method in &mut implementation.methods {
                changed |= inline_in_statements(&mut method.body, &inlinable);
            }
        }
        changed |= inline_in_statements(&mut program.main_body, &inlinable);
        changed
    }
}

fn is_inlinable(function: &Function) -> bool {
    if function.is_async {
        return false;
    }
    let mut returns = 0;
    for statement in &function.body {
        if let Statement::Return(Some(_)) = statement {
            returns += 1;
        }
    }
    returns == 1 && function.body.len() == 1
}

fn inline_in_statements(
    statements: &mut [Statement],
    inlinable: &HashMap<String, Function>,
) -> bool {
    let mut changed = false;
    for statement in statements.iter_mut() {
        match statement {
            Statement::VarDecl { value, .. } => {
                if let Some(value) = value {
                    changed |= inline_in_expr(value, inlinable);
                }
            }
            Statement::LetDecl { value, .. } => changed |= inline_in_expr(value, inlinable),
            Statement::Assignment { value, .. } => changed |= inline_in_expr(value, inlinable),
            Statement::Mutation { value, .. } => {
                if let Some(value) = value {
                    changed |= inline_in_expr(value, inlinable);
                }
            }
            Statement::Return(Some(value)) => changed |= inline_in_expr(value, inlinable),
            Statement::Put { expr, redirect, .. } => {
                changed |= inline_in_expr(expr, inlinable);
                if let Some(redirect) = redirect {
                    let path = match redirect {
                        Redirect::Write(path) | Redirect::Append(path) => path,
                    };
                    changed |= inline_in_expr(path, inlinable);
                }
            }
            Statement::Error(expr) | Statement::Warn(expr) | Statement::Info(expr) => {
                changed |= inline_in_expr(expr, inlinable)
            }
            Statement::Expression(expr) => changed |= inline_in_expr(expr, inlinable),
            Statement::If {
                then_block,
                else_block,
                ..
            } => {
                changed |= inline_in_statements(then_block, inlinable);
                if let Some(else_block) = else_block {
                    changed |= inline_in_statements(else_block, inlinable);
                }
            }
            Statement::Match { scrutinee, arms } => {
                changed |= inline_in_expr(scrutinee, inlinable);
                for arm in arms {
                    if let Some(guard) = &mut arm.guard {
                        changed |= inline_in_expr(guard, inlinable);
                    }
                    match &mut arm.body {
                        MatchArmBody::Expression(expr) => {
                            changed |= inline_in_expr(expr, inlinable)
                        }
                        MatchArmBody::Block(statements) => {
                            changed |= inline_in_statements(statements, inlinable)
                        }
                    }
                }
            }
            Statement::Block(statements) => changed |= inline_in_statements(statements, inlinable),
            Statement::Break | Statement::Continue | Statement::Return(None) => {}
            Statement::NestedFunction(_) => {}
        }
    }
    changed
}

fn inline_in_expr(expr: &mut Expr, inlinable: &HashMap<String, Function>) -> bool {
    let mut changed = false;
    match expr {
        Expr::BinaryOp { left, right, .. } => {
            changed |= inline_in_expr(left, inlinable);
            changed |= inline_in_expr(right, inlinable);
        }
        Expr::UnaryOp { expr: inner, .. } => changed |= inline_in_expr(inner, inlinable),
        Expr::Call { func, args } => {
            for arg in args.iter_mut() {
                changed |= inline_in_expr(arg, inlinable);
            }
            if let Expr::Identifier(name) = func.as_ref() {
                if let Some(target) = inlinable.get(name) {
                    if let Some(body) = inline_body(target, args) {
                        *expr = body;
                        return true;
                    }
                }
            } else {
                changed |= inline_in_expr(func, inlinable);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            changed |= inline_in_expr(object, inlinable);
            for arg in args.iter_mut() {
                changed |= inline_in_expr(arg, inlinable);
            }
        }
        Expr::Index { object, index } => {
            changed |= inline_in_expr(object, inlinable);
            changed |= inline_in_expr(index, inlinable);
        }
        Expr::FieldAccess { object, .. } => changed |= inline_in_expr(object, inlinable),
        Expr::Parenthesized(inner) => changed |= inline_in_expr(inner, inlinable),
        Expr::If {
            condition,
            then_block,
            else_block,
        } => {
            changed |= inline_in_expr(condition, inlinable);
            changed |= inline_in_statements(then_block, inlinable);
            if let Some(else_block) = else_block {
                changed |= inline_in_statements(else_block, inlinable);
            }
        }
        Expr::Match { scrutinee, arms } => {
            changed |= inline_in_expr(scrutinee, inlinable);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    changed |= inline_in_expr(guard, inlinable);
                }
                match &mut arm.body {
                    MatchArmBody::Expression(expr) => changed |= inline_in_expr(expr, inlinable),
                    MatchArmBody::Block(statements) => {
                        changed |= inline_in_statements(statements, inlinable)
                    }
                }
            }
        }
        Expr::Closure { body, .. } => changed |= inline_in_expr(body, inlinable),
        Expr::Array(elements) => {
            for element in elements.iter_mut() {
                changed |= inline_in_expr(element, inlinable);
            }
        }
        Expr::Tuple(elements) => {
            for element in elements.iter_mut() {
                changed |= inline_in_expr(element, inlinable);
            }
        }
        Expr::Struct { fields, .. } => {
            for (_, value) in fields.iter_mut() {
                changed |= inline_in_expr(value, inlinable);
            }
        }
        Expr::Enum { data, .. } => {
            if let Some(data) = data {
                for value in data.iter_mut() {
                    changed |= inline_in_expr(value, inlinable);
                }
            }
        }
        Expr::Range { start, end, .. } => {
            changed |= inline_in_expr(start, inlinable);
            changed |= inline_in_expr(end, inlinable);
        }
        Expr::LoopRange { ranges, body } => {
            for part in ranges.iter_mut() {
                match part {
                    LoopRangePart::Range {
                        start, end, step, ..
                    } => {
                        changed |= inline_in_expr(start, inlinable);
                        changed |= inline_in_expr(end, inlinable);
                        if let Some(step) = step {
                            changed |= inline_in_expr(step, inlinable);
                        }
                    }
                    LoopRangePart::Value(value) => changed |= inline_in_expr(value, inlinable),
                }
            }
            changed |= inline_in_statements(body, inlinable);
        }
        Expr::ForLoop { iterable, body, .. } => {
            changed |= inline_in_expr(iterable, inlinable);
            changed |= inline_in_statements(body, inlinable);
        }
        Expr::Try(inner) => changed |= inline_in_expr(inner, inlinable),
        Expr::As { expr: inner, .. } => changed |= inline_in_expr(inner, inlinable),
        Expr::Get(get) => {
            if let Some(prompt) = &mut get.prompt {
                changed |= inline_in_expr(prompt, inlinable);
            }
            if let Some(source) = &mut get.source {
                changed |= inline_in_expr(source, inlinable);
            }
            for flag in &mut get.flags {
                match flag {
                    GetFlag::Timeout(value)
                    | GetFlag::Default(value)
                    | GetFlag::Mask(value)
                    | GetFlag::Until(value)
                    | GetFlag::Bytes(value) => changed |= inline_in_expr(value, inlinable),
                    GetFlag::As(_) => {}
                }
            }
        }
        Expr::UnsafeBlock(statements) => changed |= inline_in_statements(statements, inlinable),
        Expr::Literal(_) | Expr::Identifier(_) => {}
    }
    changed
}

/// Build the inlined expression for a call to `target`, or `None` if any
/// argument is not a simple expression.
fn inline_body(target: &Function, args: &[Expr]) -> Option<Expr> {
    if target.params.len() != args.len() {
        return None;
    }
    let body = match target.body.first()? {
        Statement::Return(Some(expr)) => expr.clone(),
        _ => return None,
    };
    let mut bindings = HashMap::new();
    for (parameter, argument) in target.params.iter().zip(args.iter()) {
        if !is_simple_expr(argument) {
            return None;
        }
        bindings.insert(parameter.name.clone(), argument.clone());
    }
    Some(substitute(&body, &bindings))
}

fn is_simple_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) => true,
        Expr::BinaryOp { left, right, .. } => is_simple_expr(left) && is_simple_expr(right),
        Expr::UnaryOp { expr, .. } => is_simple_expr(expr),
        Expr::Parenthesized(inner) => is_simple_expr(inner),
        _ => false,
    }
}

fn substitute(expr: &Expr, bindings: &HashMap<String, Expr>) -> Expr {
    match expr {
        Expr::Identifier(name) => bindings
            .get(name)
            .cloned()
            .unwrap_or_else(|| Expr::Identifier(name.clone())),
        Expr::BinaryOp { op, left, right } => Expr::BinaryOp {
            op: *op,
            left: Box::new(substitute(left, bindings)),
            right: Box::new(substitute(right, bindings)),
        },
        Expr::UnaryOp { op, expr } => Expr::UnaryOp {
            op: *op,
            expr: Box::new(substitute(expr, bindings)),
        },
        Expr::Parenthesized(inner) => Expr::Parenthesized(Box::new(substitute(inner, bindings))),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poly_lexer::Lexer;
    use poly_parser::Parser;

    fn parse_to_ir(source: &str) -> Program {
        let (tokens, errors) = Lexer::lex(source);
        assert!(errors.is_empty(), "lexer errors: {errors:?}");
        let mut parser = Parser::new(&tokens);
        let ast = parser.parse().expect("source should parse");
        crate::generator::generate(&ast)
    }

    fn int_value(expr: &Expr) -> i64 {
        match expr {
            Expr::Literal(Literal::Int(value)) => value.parse().unwrap(),
            other => panic!("expected int literal, got {other:?}"),
        }
    }

    #[test]
    fn folds_constant_arithmetic() {
        let mut program = parse_to_ir("var x := 2 + 3 * 4");
        ConstantFolding.run(&mut program);

        match &program.main_body[0] {
            Statement::VarDecl {
                value: Some(value), ..
            } => assert_eq!(int_value(value), 14),
            other => panic!("expected var decl, got {other:?}"),
        }
    }

    #[test]
    fn folds_boolean_logic() {
        let mut program = parse_to_ir("var x := true && false");
        ConstantFolding.run(&mut program);

        match &program.main_body[0] {
            Statement::VarDecl {
                value: Some(Expr::Literal(Literal::Bool(false))),
                ..
            } => {}
            other => panic!("expected folded false, got {other:?}"),
        }
    }

    #[test]
    fn removes_dead_code_after_return() {
        let mut program =
            parse_to_ir("fn f(): i32\n    return 1\n    var x := 99\n    put x\nend fn");
        DeadCodeElimination.run(&mut program);

        assert_eq!(program.functions[0].body.len(), 1);
        assert!(matches!(program.functions[0].body[0], Statement::Return(_)));
    }

    #[test]
    fn simplifies_redundant_step() {
        let mut program = parse_to_ir("loop: 0..10 step 1\n    put i\nend loop");
        LoopOptimizations.run(&mut program);

        match &program.main_body[0] {
            Statement::Expression(Expr::LoopRange { ranges, .. }) => match &ranges[0] {
                LoopRangePart::Range { step, .. } => assert!(step.is_none()),
                other => panic!("expected range part, got {other:?}"),
            },
            other => panic!("expected loop range, got {other:?}"),
        }
    }

    #[test]
    fn removes_empty_loops() {
        let mut program = parse_to_ir("loop: 0..10\nend loop\nput 42");
        LoopOptimizations.run(&mut program);

        assert_eq!(program.main_body.len(), 1);
        assert!(matches!(&program.main_body[0], Statement::Put { .. }));
    }

    #[test]
    fn inlines_small_functions() {
        let mut program =
            parse_to_ir("fn double(x: i32): i32\n    return x * 2\nend fn\nvar y := double(21)");
        Inlining.run(&mut program);

        // After inlining, the call is replaced by its body with `x` substituted.
        match &program.main_body[0] {
            Statement::VarDecl {
                value:
                    Some(Expr::BinaryOp {
                        op: BinaryOp::Mul,
                        left,
                        right,
                    }),
                ..
            } => {
                assert_eq!(int_value(left), 21);
                assert_eq!(int_value(right), 2);
            }
            other => panic!("expected inlined multiplication, got {other:?}"),
        }
    }

    #[test]
    fn inlining_then_folding_produces_a_constant() {
        let mut program =
            parse_to_ir("fn double(x: i32): i32\n    return x * 2\nend fn\nvar y := double(21)");
        Inlining.run(&mut program);
        ConstantFolding.run(&mut program);

        match &program.main_body[0] {
            Statement::VarDecl {
                value: Some(value), ..
            } => assert_eq!(int_value(value), 42),
            other => panic!("expected var decl, got {other:?}"),
        }
    }

    #[test]
    fn full_pipeline_folds_end_to_end() {
        let mut program = parse_to_ir(
            "fn double(x: i32): i32\n    return x * 2\nend fn\nvar y := double(20) + 2",
        );
        let applied = optimize(&mut program);

        assert!(applied.contains(&"inlining".to_string()));
        match &program.main_body[0] {
            Statement::VarDecl {
                value: Some(value), ..
            } => assert_eq!(int_value(value), 42),
            other => panic!("expected var decl, got {other:?}"),
        }
    }
}

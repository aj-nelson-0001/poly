# Transpiler IR Refactoring Plan

## Overview

This document outlines the plan to refactor the Poly transpiler to use a more modular Intermediate Representation (IR), improving maintainability, extensibility, and enabling future optimizations.

## Current State

The current transpiler directly walks the Poly AST and generates Rust code:

```
Poly Source → Lexer → Tokens → Parser → AST → CodeGen → Rust Code
```

### Issues with Current Approach

1. **Tight Coupling**: AST and code generation are tightly coupled
2. **Hard to Optimize**: No intermediate representation for optimizations
3. **Difficult to Extend**: Adding new features requires modifying the entire pipeline
4. **No Source Maps**: No mapping between generated code and source
5. **Testing Challenges**: Hard to test individual phases independently

## Proposed Architecture

```
Poly Source → Lexer → Tokens → Parser → AST → IR → Optimizer → CodeGen → Rust Code
                                      ↓
                              Source Map Generation
```

### New Components

1. **IR Module** (`poly-ir`): Defines the intermediate representation
2. **IR Generator** (`ast_to_ir`): Converts AST to IR
3. **Optimizer** (`optimizer`): Performs IR transformations
4. **Enhanced CodeGen**: Generates Rust from optimized IR

## IR Design

### Core IR Types

```rust
/// The Poly intermediate representation
pub mod ir {
    /// IR Program - top level container
    pub struct Program {
        pub functions: Vec<Function>,
        pub structs: Vec<Struct>,
        pub enums: Vec<Enum>,
        pub traits: Vec<Trait>,
        pub impls: Vec<Impl>,
        pub modules: Vec<Module>,
        pub main_body: Vec<Statement>,
    }

    /// IR Function
    pub struct Function {
        pub name: String,
        pub params: Vec<Parameter>,
        pub return_type: Option<Type>,
        pub body: Vec<Statement>,
        pub source_location: Option<SourceLocation>,
    }

    /// IR Statement
    pub enum Statement {
        VarDecl(VarDecl),
        LetDecl(LetDecl),
        Assignment(Assignment),
        Return(Option<Expr>),
        Break,
        Continue,
        Put(PutStmt),
        Error(ErrorStmt),
        Warn(WarnStmt),
        Info(InfoStmt),
        Expression(Expr),
        If(IfStmt),
        While(WhileStmt),
        For(ForStmt),
        Match(MatchStmt),
        Block(Vec<Statement>),
    }

    /// IR Expression
    pub enum Expr {
        Literal(Literal),
        Identifier(String),
        BinaryOp(BinaryOp),
        UnaryOp(UnaryOp),
        Call(CallExpr),
        MethodCall(MethodCallExpr),
        Index(IndexExpr),
        FieldAccess(FieldAccessExpr),
        If(IfExpr),
        Match(MatchExpr),
        Closure(ClosureExpr),
        Array(ArrayExpr),
        Tuple(TupleExpr),
        Struct(StructExpr),
        Enum(EnumExpr),
        Range(RangeExpr),
        Try(TryExpr),
        As(AsExpr),
    }

    /// IR Type
    pub enum Type {
        Named(String),
        Array(Box<Type>, usize),
        Tuple(Vec<Type>),
        Vec(Box<Type>),
        Option(Box<Type>),
        Result(Box<Type>, Box<Type>),
        Reference(bool, Box<Type>),
        Pointer(Box<Type>),
        Function(Vec<Type>, Box<Type>),
    }

    // ... more types
}
```

### IR Generator

```rust
/// Converts Poly AST to IR
pub struct IRGenerator {
    source_map: SourceMap,
}

impl IRGenerator {
    pub fn new() -> Self {
        Self {
            source_map: SourceMap::new(),
        }
    }

    pub fn generate(&mut self, ast: &Program) -> ir::Program {
        let mut ir_program = ir::Program::new();

        for stmt in &ast.statements {
            match stmt {
                Statement::FunctionDeclaration(func) => {
                    ir_program.functions.push(self.gen_function(func));
                }
                Statement::StructDeclaration(struct_decl) => {
                    ir_program.structs.push(self.gen_struct(struct_decl));
                }
                Statement::EnumDeclaration(enum_decl) => {
                    ir_program.enums.push(self.gen_enum(enum_decl));
                }
                _ => {
                    ir_program.main_body.push(self.gen_statement(stmt));
                }
            }
        }

        ir_program
    }

    fn gen_function(&mut self, func: &FunctionDecl) -> ir::Function {
        // Convert function declaration to IR
    }

    fn gen_statement(&mut self, stmt: &Statement) -> ir::Statement {
        // Convert statement to IR
    }

    fn gen_expression(&mut self, expr: &Expression) -> ir::Expr {
        // Convert expression to IR
    }
}
```

### Optimizer

```rust
/// IR Optimizer - performs transformations on IR
pub struct Optimizer {
    passes: Vec<Box<dyn OptimizationPass>>,
}

pub trait OptimizationPass {
    fn name(&self) -> &str;
    fn run(&self, program: &mut ir::Program) -> bool;
}

/// Constant folding optimization
pub struct ConstantFolding;

impl OptimizationPass for ConstantFolding {
    fn name(&self) -> &str {
        "constant_folding"
    }

    fn run(&self, program: &mut ir::Program) -> bool {
        let mut changed = false;
        for func in &mut program.functions {
            changed |= self.fold_constants(&mut func.body);
        }
        changed
    }
}

/// Dead code elimination
pub struct DeadCodeElimination;

impl OptimizationPass for DeadCodeElimination {
    fn name(&self) -> &str {
        "dead_code_elimination"
    }

    fn run(&self, program: &mut ir::Program) -> bool {
        // Remove unreachable code
    }
}

/// Inlining
pub struct Inlining {
    max_inline_size: usize,
}

impl OptimizationPass for Inlining {
    fn name(&self) -> &str {
        "inlining"
    }

    fn run(&self, program: &mut ir::Program) -> bool {
        // Inline small functions
    }
}
```

### Enhanced CodeGen

```rust
/// Enhanced code generator using IR
pub struct EnhancedCodeGen {
    source_map: SourceMap,
    indentation: usize,
}

impl EnhancedCodeGen {
    pub fn new() -> Self {
        Self {
            source_map: SourceMap::new(),
            indentation: 0,
        }
    }

    pub fn generate(&mut self, ir_program: &ir::Program) -> (String, SourceMap) {
        let mut output = String::new();

        // Generate header
        output.push_str("// Generated from Poly source code\n");
        output.push_str("#![allow(unused_variables, unused_mut, unused_imports, dead_code)]\n\n");

        // Generate functions
        for func in &ir_program.functions {
            output.push_str(&self.gen_function(func));
            output.push('\n');
        }

        // Generate structs
        for struct_decl in &ir_program.structs {
            output.push_str(&self.gen_struct(struct_decl));
            output.push('\n');
        }

        // Generate enums
        for enum_decl in &ir_program.enums {
            output.push_str(&self.gen_enum(enum_decl));
            output.push('\n');
        }

        // Generate main function
        if !ir_program.main_body.is_empty() {
            output.push_str("fn main() {\n");
            self.indentation += 1;
            for stmt in &ir_program.main_body {
                output.push_str(&self.gen_statement(stmt));
            }
            self.indentation -= 1;
            output.push_str("}\n");
        }

        (output, self.source_map.clone())
    }

    fn gen_function(&mut self, func: &ir::Function) -> String {
        // Generate Rust function from IR
    }

    fn gen_statement(&mut self, stmt: &ir::Statement) -> String {
        // Generate Rust statement from IR
    }

    fn gen_expression(&self, expr: &ir::Expr) -> String {
        // Generate Rust expression from IR
    }
}
```

## Migration Strategy

### Phase 1: Create IR Module (Week 1-2)

1. Create `poly-ir` crate with IR types
2. Implement IR generator from AST
3. Add source map tracking
4. Write unit tests for IR generation

### Phase 2: Implement Basic Optimizer (Week 3-4)

1. Create optimizer framework
2. Implement constant folding
3. Implement dead code elimination
4. Add optimization tests

### Phase 3: Enhanced CodeGen (Week 5-6)

1. Refactor CodeGen to use IR
2. Add source map generation
3. Improve error reporting
4. Update integration tests

### Phase 4: Advanced Optimizations (Week 7-8)

1. Implement inlining
2. Add loop optimizations
3. Implement tail call optimization
4. Performance benchmarking

### Phase 5: Polish and Documentation (Week 9-10)

1. Update documentation
2. Add examples
3. Performance comparison
4. Migration guide

## Benefits

### 1. Modularity

- Each phase is independent and testable
- Easy to add new optimizations
- Clear separation of concerns

### 2. Optimizations

- Constant folding
- Dead code elimination
- Function inlining
- Loop optimizations
- Tail call optimization

### 3. Better Debugging

- Source maps for debugging
- Better error messages
- Intermediate representation inspection

### 4. Extensibility

- Easy to add new language features
- Support for multiple backends
- Plugin system for custom optimizations

### 5. Testing

- Unit tests for each phase
- Property-based testing
- Fuzzing support

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_ir_generation() {
    let source = "var x: i32 = 42";
    let ast = parse(source);
    let ir = generate_ir(&ast);
    
    assert_eq!(ir.statements.len(), 1);
    assert!(matches!(ir.statements[0], ir::Statement::VarDecl(_)));
}

#[test]
fn test_constant_folding() {
    let source = "var x = 2 + 3";
    let ast = parse(source);
    let mut ir = generate_ir(&ast);
    
    let optimizer = ConstantFolding;
    optimizer.run(&mut ir);
    
    // Should be folded to 5
    assert_eq!(ir.statements[0].value, ir::Expr::Literal(ir::Literal::Int(5)));
}
```

### Integration Tests

```rust
#[test]
fn test_end_to_end_optimization() {
    let source = r#"
    fn add(a: i32, b: i32): i32
        return a + b
    end fn
    
    var x = add(2, 3)
    "#;
    
    let rust_code = transpile_with_optimization(source);
    assert!(rust_code.contains("5")); // Constant folded
}
```

### Performance Benchmarks

```rust
#[bench]
fn bench_ir_generation(b: &mut Bencher) {
    let source = generate_large_program(1000);
    b.iter(|| {
        let ast = parse(&source);
        generate_ir(&ast)
    });
}
```

## Migration Checklist

- [ ] Create `poly-ir` crate
- [ ] Define IR types
- [ ] Implement IR generator
- [ ] Add source map tracking
- [ ] Write IR tests
- [ ] Create optimizer framework
- [ ] Implement constant folding
- [ ] Implement dead code elimination
- [ ] Add optimizer tests
- [ ] Refactor CodeGen to use IR
- [ ] Add source map generation
- [ ] Update integration tests
- [ ] Implement inlining
- [ ] Add loop optimizations
- [ ] Performance benchmarking
- [ ] Update documentation
- [ ] Add examples
- [ ] Create migration guide

## Risks and Mitigations

### Risk 1: Breaking Changes

**Mitigation**: 
- Keep old codegen as fallback
- Feature flag for new IR
- Comprehensive test coverage

### Risk 2: Performance Regression

**Mitigation**:
- Benchmark before and after
- Profile hot paths
- Optimize critical sections

### Risk 3: Increased Complexity

**Mitigation**:
- Clear documentation
- Modular design
- Comprehensive tests

## Conclusion

Refactoring the transpiler to use an IR will significantly improve the Poly compiler's maintainability, extensibility, and performance. The phased approach minimizes risk while delivering incremental value.

The IR will enable:
- Better optimizations
- Improved debugging
- Easier feature additions
- Multiple backend support

This investment will pay dividends as the Poly language continues to evolve.

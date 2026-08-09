//! Abstract Syntax Tree definitions for the Poly language.

use poly_lexer::token::Span;

/// A node in the AST with source location information.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

/// The root of a Poly program.
#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Statement>,
}

/// A statement in Poly.
#[derive(Debug, Clone)]
pub enum Statement {
    /// Variable declaration: `var x: i32 = 0`
    VarDeclaration {
        name: String,
        ty: Option<TypeAnnotation>,
        value: Option<Expression>,
    },
    /// Let declaration: `let x: i32 = 0`
    LetDeclaration {
        name: String,
        ty: Option<TypeAnnotation>,
        value: Expression,
    },
    /// Constant declaration: `const MAX = 100`
    ConstDeclaration { name: String, value: Expression },
    /// Assignment: `x = 5` or `x += 5`
    Assignment {
        target: Expression,
        op: AssignmentOp,
        value: Expression,
    },
    /// Function declaration
    FunctionDeclaration(FunctionDecl),
    /// Struct declaration
    StructDeclaration(StructDecl),
    /// Enum declaration
    EnumDeclaration(EnumDecl),
    /// Trait declaration
    TraitDeclaration(TraitDecl),
    /// Impl declaration
    ImplDeclaration(ImplDecl),
    /// Module declaration
    ModuleDeclaration(ModuleDecl),
    /// Use declaration
    UseDeclaration(UseDecl),
    /// Type alias
    TypeDeclaration(TypeDecl),
    /// Expression statement
    ExpressionStatement(Expression),
    /// Return statement
    ReturnStatement(Option<Expression>),
    /// Break statement
    BreakStatement,
    /// Continue statement
    ContinueStatement,
    /// Put statement (output)
    PutStatement {
        no_newline: bool,
        expr: Expression,
        redirect: Option<Redirect>,
    },
    /// Error statement
    ErrorStatement(Expression),
    /// Warn statement
    WarnStatement(Expression),
    /// Info statement
    InfoStatement(Expression),
}

/// Redirect for file I/O.
#[derive(Debug, Clone)]
pub enum Redirect {
    /// Write: `> "file"`
    Write(Expression),
    /// Append: `>> "file"`
    Append(Expression),
}

/// Assignment operators.
#[derive(Debug, Clone)]
pub enum AssignmentOp {
    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    AmpEq,
    PipeEq,
    CaretEq,
    LtLtEq,
    GtGtEq,
}

/// Type annotation.
#[derive(Debug, Clone)]
pub enum TypeAnnotation {
    Named(String),
    Array(Box<TypeAnnotation>, Box<Expression>),
    Tuple(Vec<TypeAnnotation>),
    Vec(Box<TypeAnnotation>),
    Option(Box<TypeAnnotation>),
    Result(Box<TypeAnnotation>, Box<TypeAnnotation>),
    Reference(bool, Box<TypeAnnotation>), // mutability, inner type
    Pointer(Box<TypeAnnotation>),
    Nullable(Box<TypeAnnotation>),
    Function {
        params: Vec<TypeAnnotation>,
        ret: Box<TypeAnnotation>,
    },
}

/// An expression in Poly.
#[derive(Debug, Clone)]
pub enum Expression {
    /// Integer literal
    IntLiteral(String),
    /// Float literal
    FloatLiteral(String),
    /// String literal
    StringLiteral(String),
    /// Unicode string literal
    UnicodeStringLiteral(String),
    /// Boolean literal
    BoolLiteral(bool),
    /// Byte literal
    ByteLiteral(Vec<u8>),
    /// Identifier
    Identifier(String),
    /// Binary operation
    BinaryOp {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    /// Unary operation
    UnaryOp { op: UnaryOp, expr: Box<Expression> },
    /// Function call
    Call {
        func: Box<Expression>,
        args: Vec<Expression>,
    },
    /// Method call
    MethodCall {
        object: Box<Expression>,
        method: String,
        args: Vec<Expression>,
    },
    /// Index access
    Index {
        object: Box<Expression>,
        index: Box<Expression>,
    },
    /// Field access
    FieldAccess {
        object: Box<Expression>,
        field: String,
    },
    /// Parenthesized expression
    Parenthesized(Box<Expression>),
    /// If expression
    IfExpression {
        condition: Box<Expression>,
        then_block: Vec<Statement>,
        else_block: Option<Vec<Statement>>,
    },
    /// Match expression
    MatchExpression {
        scrutinee: Box<Expression>,
        arms: Vec<MatchArm>,
    },
    /// Closure
    Closure {
        params: Vec<Parameter>,
        body: Box<Expression>,
    },
    /// Array literal
    ArrayLiteral(Vec<Expression>),
    /// Tuple literal
    TupleLiteral(Vec<Expression>),
    /// Struct literal
    StructLiteral {
        name: String,
        fields: Vec<(String, Expression)>,
    },
    /// Enum variant
    EnumVariant {
        enum_name: String,
        variant: String,
        data: Option<Vec<Expression>>,
    },
    /// Loop range expression (loop: 1..3, 7, 19..21 step 2)
    LoopRange {
        ranges: Vec<LoopRangePart>,
        body: Vec<Statement>,
    },
    /// As expression (type cast)
    AsExpression {
        expr: Box<Expression>,
        ty: Box<TypeAnnotation>,
    },
    /// Try expression
    TryExpression(Box<Expression>),
    /// Get expression (input)
    GetExpression(Box<GetExpr>),
    /// Unsafe block
    UnsafeBlock(Vec<Statement>),
    /// Range expression
    Range {
        start: Box<Expression>,
        end: Box<Expression>,
        inclusive: bool,
    },
}

/// Binary operators.
#[derive(Debug, Clone)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// Unary operators.
#[derive(Debug, Clone)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Deref,
}

/// Match arm.
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expression>,
    pub body: MatchArmBody,
}

/// The body of a match arm: either a single expression or a block of statements.
#[derive(Debug, Clone)]
pub enum MatchArmBody {
    Expression(Expression),
    Block(Vec<Statement>),
}

/// Patterns in match expressions.
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Wildcard
    Wildcard,
    /// Literal pattern
    Literal(Expression),
    /// Identifier binding
    Identifier(String),
    /// Tuple pattern
    Tuple(Vec<Pattern>),
    /// Enum pattern (can have named fields too: Error(FileError::NotFound))
    Enum {
        enum_name: String,
        variant: String,
        inner: Option<Vec<Pattern>>,
    },
    /// Named field pattern: Foo { bar, baz }
    NamedFields {
        name: String,
        fields: Vec<(String, Pattern)>,
    },
    /// Range pattern
    Range {
        start: Box<Expression>,
        end: Box<Expression>,
        inclusive: bool,
    },
    /// @ binding
    Binding { name: String, pattern: Box<Pattern> },
}

/// A part of a loop range (either a range, a single value, or a range with step).
#[derive(Debug, Clone)]
pub enum LoopRangePart {
    Range {
        start: Box<Expression>,
        end: Box<Expression>,
        inclusive: bool,
        step: Option<Expression>,
    },
    Value(Box<Expression>),
}

/// Get expression (input from stdin/files).
#[derive(Debug, Clone)]
pub struct GetExpr {
    pub prompt: Option<Box<Expression>>,
    pub source: Option<Box<Expression>>, // input redirection: < "file"
    pub flags: Vec<GetFlag>,
    pub with_clause: Option<WithClause>,
}

/// Flags for the get command.
#[derive(Debug, Clone)]
pub enum GetFlag {
    Timeout(Expression),
    Default(Expression),
    Mask(Expression),
    Until(Expression),
    Bytes(Expression),
}

/// With clause for get command.
#[derive(Debug, Clone)]
pub enum WithClause {
    Validate(Expression),
    Complete(Expression),
    Encoding(Expression),
}

/// Function declaration.
#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Option<TypeAnnotation>,
    pub body: Option<Vec<Statement>>,
    pub is_async: bool,
}

/// Function parameter.
#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub ty: TypeAnnotation,
    pub default: Option<Expression>,
}

/// Struct declaration.
#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<StructField>,
    pub methods: Vec<FunctionDecl>,
}

/// Struct field.
#[derive(Debug, Clone)]
pub struct StructField {
    pub mutable: bool,
    pub name: String,
    pub ty: TypeAnnotation,
    pub default: Option<Expression>,
}

/// Enum declaration.
#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub methods: Vec<FunctionDecl>,
}

/// Enum variant.
#[derive(Debug, Clone)]
pub enum EnumVariant {
    Unit(String),
    Tuple(String, Vec<TypeAnnotation>),
    Struct(String, Vec<(String, TypeAnnotation)>),
}

/// Trait declaration.
#[derive(Debug, Clone)]
pub struct TraitDecl {
    pub name: String,
    pub methods: Vec<FunctionDecl>,
}

/// Impl declaration.
#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub trait_name: Option<String>,
    pub type_name: String,
    pub methods: Vec<FunctionDecl>,
}

/// Module declaration.
#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub name: String,
    pub statements: Vec<Statement>,
}

/// Use declaration.
#[derive(Debug, Clone)]
pub struct UseDecl {
    pub path: Vec<String>,
    pub alias: Option<String>,
}

/// Type declaration.
#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub name: String,
    pub ty: TypeAnnotation,
}

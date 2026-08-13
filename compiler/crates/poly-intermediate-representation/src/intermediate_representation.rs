//! Intermediate representation (intermediate representation) for the Poly compiler.
//!
//! The intermediate representation sits between the parser's AST and Rust code generation.  It keeps a
//! flat, structured view of a program so that optimization passes can rewrite
//! programs without knowing parser details, and so code generation never needs
//! to re-derive control-flow shapes from the AST.

/// Location of an intermediate representation node in the original Poly source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    /// 1-based line in the Poly source.
    pub line: usize,
    /// 1-based column in the Poly source.
    pub column: usize,
}

/// A complete intermediate representation program.
#[derive(Debug, Clone, Default)]
pub struct Program {
    pub functions: Vec<Function>,
    pub structs: Vec<Struct>,
    pub enums: Vec<Enum>,
    pub traits: Vec<Trait>,
    pub impls: Vec<Impl>,
    pub modules: Vec<Module>,
    pub constants: Vec<Constant>,
    pub type_aliases: Vec<TypeAlias>,
    pub uses: Vec<String>,
    pub main_body: Vec<Statement>,
}

/// intermediate representation function.
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub body: Vec<Statement>,
    pub is_async: bool,
    pub generics: Vec<GenericParam>,
    pub source_location: Option<SourceLocation>,
}

/// intermediate representation function parameter.
#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
}

/// Generic parameter (`T: Bound`).
#[derive(Debug, Clone)]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<String>,
}

/// intermediate representation struct.
#[derive(Debug, Clone)]
pub struct Struct {
    pub name: String,
    pub fields: Vec<Field>,
    pub methods: Vec<Function>,
    pub generics: Vec<GenericParam>,
    pub source_location: Option<SourceLocation>,
}

/// intermediate representation struct field.
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
}

/// intermediate representation enum.
#[derive(Debug, Clone)]
pub struct Enum {
    pub name: String,
    pub variants: Vec<Variant>,
    pub methods: Vec<Function>,
    pub source_location: Option<SourceLocation>,
}

/// intermediate representation enum variant.
#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Field>, // empty for unit variants
    pub is_struct: bool,
}

/// intermediate representation trait.
#[derive(Debug, Clone)]
pub struct Trait {
    pub name: String,
    pub methods: Vec<Function>,
    pub source_location: Option<SourceLocation>,
}

/// intermediate representation impl block.
#[derive(Debug, Clone)]
pub struct Impl {
    pub trait_name: Option<String>,
    pub type_name: String,
    pub methods: Vec<Function>,
    pub source_location: Option<SourceLocation>,
}

/// intermediate representation module.
#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub statements: Vec<Statement>,
}

/// intermediate representation constant declaration.
#[derive(Debug, Clone)]
pub struct Constant {
    pub name: String,
    pub value: Expr,
    pub source_location: Option<SourceLocation>,
}

/// intermediate representation type alias.
#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub name: String,
    pub ty: Type,
}

/// intermediate representation statement.
#[derive(Debug, Clone)]
pub enum Statement {
    VarDecl {
        name: String,
        ty: Option<Type>,
        value: Option<Expr>,
    },
    LetDecl {
        name: String,
        ty: Option<Type>,
        value: Expr,
    },
    Assignment {
        target: Expr,
        value: Expr,
    },
    Mutation {
        target: Expr,
        op: MutationOp,
        value: Option<Expr>,
    },
    Return(Option<Expr>),
    Break,
    Continue,
    Put {
        no_newline: bool,
        expr: Expr,
        redirect: Option<Redirect>,
    },
    Error(Expr),
    Warn(Expr),
    Info(Expr),
    Expression(Expr),
    If {
        condition: Expr,
        then_block: Vec<Statement>,
        else_block: Option<Vec<Statement>>,
        is_while: bool,
    },
    Match {
        scrutinee: Expr,
        arms: Vec<MatchArm>,
    },
    Block(Vec<Statement>),
    /// Nested function declared inside another function body.  Rust emits
    /// nested items as inner `fn` declarations.
    NestedFunction(Function),
}

/// intermediate representation mutation operator.
#[derive(Debug, Clone, Copy)]
pub enum MutationOp {
    Add,
    Sub,
    Inc,
    Dec,
}

/// intermediate representation file redirect for `put`.
#[derive(Debug, Clone)]
pub enum Redirect {
    Write(Expr),
    Append(Expr),
}

/// intermediate representation match arm.
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: MatchArmBody,
}

/// intermediate representation match arm body.
#[derive(Debug, Clone)]
pub enum MatchArmBody {
    Expression(Expr),
    Block(Vec<Statement>),
}

/// intermediate representation pattern.
#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard,
    Literal(Expr),
    Identifier(String),
    Tuple(Vec<Pattern>),
    Enum {
        enum_name: String,
        variant: String,
        inner: Option<Vec<Pattern>>,
    },
    NamedFields {
        name: String,
        fields: Vec<(String, Pattern)>,
    },
    Range {
        start: Expr,
        end: Expr,
        inclusive: bool,
    },
    Binding {
        name: String,
        pattern: Box<Pattern>,
    },
}

/// intermediate representation expression.
#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Identifier(String),
    BinaryOp {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
    },
    MethodCall {
        object: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    FieldAccess {
        object: Box<Expr>,
        field: String,
    },
    Parenthesized(Box<Expr>),
    If {
        condition: Box<Expr>,
        then_block: Vec<Statement>,
        else_block: Option<Vec<Statement>>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Closure {
        params: Vec<Parameter>,
        body: Box<Expr>,
    },
    Array(Vec<Expr>),
    Tuple(Vec<Expr>),
    Struct {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    Enum {
        enum_name: String,
        variant: String,
        data: Option<Vec<Expr>>,
    },
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
    },
    LoopRange {
        ranges: Vec<LoopRangePart>,
        body: Vec<Statement>,
    },
    /// Explicit for loop: `for variable in iterable` with a block body.
    ForLoop {
        variable: String,
        iterable: Box<Expr>,
        body: Vec<Statement>,
    },
    Try(Box<Expr>),
    As {
        expr: Box<Expr>,
        ty: Type,
    },
    Get(Box<GetExpr>),
    UnsafeBlock(Vec<Statement>),
}

/// intermediate representation literal.
#[derive(Debug, Clone)]
pub enum Literal {
    Int(String),
    Float(String),
    String(String),
    UnicodeString(String),
    Char(char),
    Bool(bool),
    Bytes(Vec<u8>),
}

/// intermediate representation binary operator.
#[derive(Debug, Clone, Copy, PartialEq)]
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

/// intermediate representation unary operator.
#[derive(Debug, Clone, Copy)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Deref,
}

/// intermediate representation loop range part.
#[derive(Debug, Clone)]
pub enum LoopRangePart {
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
        step: Option<Expr>,
    },
    Value(Box<Expr>),
}

/// intermediate representation `get` expression.
#[derive(Debug, Clone)]
pub struct GetExpr {
    pub prompt: Option<Box<Expr>>,
    pub source: Option<Box<Expr>>,
    pub flags: Vec<GetFlag>,
    pub with_clause: Option<WithClause>,
}

/// intermediate representation `get` flag.
#[derive(Debug, Clone)]
pub enum GetFlag {
    Timeout(Expr),
    Default(Expr),
    Mask(Expr),
    Until(Expr),
    Bytes(Expr),
    As(Type),
}

/// intermediate representation `get` with-clause.
#[derive(Debug, Clone)]
pub enum WithClause {
    Validate(Expr),
    Complete(Expr),
    Encoding(Expr),
}

/// intermediate representation type annotation.
#[derive(Debug, Clone)]
pub enum Type {
    Named(String),
    Array(Box<Type>, Box<Expr>),
    Tuple(Vec<Type>),
    Vec(Box<Type>),
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),
    Reference(bool, Box<Type>),
    Pointer(Box<Type>),
    Nullable(Box<Type>),
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
    },
    /// Generic named type: `Map<ustring, i32>`, `Box<Expr>`, `Foo<T>`.
    Generic {
        name: String,
        args: Vec<Type>,
    },
}

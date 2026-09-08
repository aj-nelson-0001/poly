//! Intermediate representation (intermediate representation) for the Poly compiler.
//!
//! The intermediate representation sits between the parser's AST and Rust code generation.  It keeps a
//! flat, structured view of a program so that optimization passes can rewrite
//! programs without knowing parser details, and so code generation never needs
//! to re-derive control-flow shapes from the AST.

/// Location of an intermediate representation node in the original Poly source.
///
/// Positions are byte offsets into the original source text.  Consumers that
/// need 1-based line/column pairs convert offsets with
/// `poly_lexer::diagnostics::line_col` while they still hold the source text;
/// the generator itself only sees the AST and cannot compute lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    /// Byte offset of the node's start in the original Poly source.
    pub byte_offset: usize,
}

/// A complete intermediate representation program.
#[derive(Debug, Clone, Default)]
pub struct Program {
    /// Module-scope declarations are separated from executable statements so
    /// codegen can emit valid target-language item order.
    pub functions: Vec<Function>,
    pub structs: Vec<Struct>,
    pub enums: Vec<Enum>,
    pub traits: Vec<Trait>,
    pub impls: Vec<Impl>,
    pub modules: Vec<Module>,
    pub constants: Vec<Constant>,
    pub type_aliases: Vec<TypeAlias>,
    pub uses: Vec<String>,
    /// Statements that become the generated entry point body.
    pub main_body: Vec<Statement>,
    /// Raw Rust blocks from `#rust ... #endrust`, emitted verbatim at
    /// the top level of the generated Rust file.
    pub top_level_rust_blocks: Vec<String>,
    /// Source byte offsets for `main_body`, element-for-element.  A `None`
    /// entry (or a shorter vector) means "no precise location" and codegen
    /// simply omits the mapping; the zip-based lookup never panics when the
    /// optimizer removes statements without touching this array.
    pub main_body_locations: Vec<Option<SourceLocation>>,
}

/// intermediate representation function.
#[derive(Debug, Clone)]
pub struct Function {
    /// The target-language symbol name; foreign calls are resolved by the
    /// native compiler, while Poly functions are generated from this field.
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub body: Vec<Statement>,
    /// Source byte offsets for `body`, element-for-element (see
    /// `Program::main_body_locations` for the desync guard).
    pub body_locations: Vec<Option<SourceLocation>>,
    /// Async is retained until codegen because it changes both the signature
    /// and the generated entry-point/runtime dependencies.
    pub is_async: bool,
    pub generics: Vec<GenericParam>,
    /// Optional source origin reserved for span-precise IR diagnostics.
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
    // Statements are deliberately target-neutral. Backends decide how a
    // supported operation lowers, while opaque foreign blocks remain visible
    // for selection but are never optimized as Poly syntax.
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
    /// Raw foreign-language block (`#rust`, `#c`, etc.), emitted by a
    /// target-specific backend.
    ForeignBlock {
        language: String,
        content: String,
    },
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
    TupleIndex {
        object: Box<Expr>,
        index: usize,
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
        variable: String,
        ranges: Vec<LoopRangePart>,
        body: Vec<Statement>,
    },
    /// Explicit for loop: `for variable in iterable` with a block body.
    ForLoop {
        variable: String,
        iterable: Box<Expr>,
        body: Vec<Statement>,
    },
    /// Infinite loop: `loop` ... `end loop`.
    InfiniteLoop(Vec<Statement>),
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
    /// Named types remain unresolved here so the checker/native compiler can
    /// own foreign signatures and user-defined target-language types.
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

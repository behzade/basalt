// ast.rs

// This file defines the Abstract Syntax Tree (AST) for the language.
// The parser will produce this structure from the token stream.

use crate::token::Token; // For representing operators in expressions.

// --- Core Nodes ---

/// The root node of an AST. A program is simply a sequence of statements.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

/// A wrapper for identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier(pub String);

/// A path, like `myMod::MyStruct`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Path(pub Vec<Identifier>);

/// Represents a block of statements, like `{ ... }`.
/// The last expression is the return value of the block.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockStatement {
    pub statements: Vec<Statement>,
}

// --- Type Representations ---

/// Represents a type annotation in the source code.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    // Simple type like `i64`, `MyStruct`.
    Ident(Identifier),
    // Generic type like `Array<i64>`.
    Generic { base: Identifier, params: Vec<Type> },
    // Array type `[T]`. A potential alternative to Generic.
    // For now, we'll stick to the `Array<T>` syntax.
}

// --- Statements ---

/// An enum representing all possible statements in the language.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// A `let` binding, e.g., `let x: i64 = 5;`
    Let {
        name: Identifier,
        value_type: Type,
        value: Expression,
        mutable: bool,
    },
    /// A `return` statement, e.g., `return 10;`
    Return { return_value: Expression },
    /// A statement that consists of a single expression, e.g., `my_func(5);`
    Expression { expression: Expression },
    /// A `struct` definition.
    StructDef {
        name: Identifier,
        fields: Vec<(Identifier, Type)>,
    },
    /// A `trait` definition.
    TraitDef {
        name: Identifier,
        methods: Vec<FunctionSignature>,
    },
    /// An `impl` block for a trait on a struct.
    Impl {
        trait_name: Path,
        struct_name: Identifier,
        methods: Vec<FunctionDeclaration>,
    },
    /// A named `fn` definition.
    FnDef { decl: FunctionDeclaration },
    /// An `effect` definition.
    EffectDef {
        name: Identifier,
        operations: Vec<FunctionSignature>,
    },
    /// A `handler` definition.
    HandlerDef {
        name: Identifier,
        effect: Path,
        handlers: Vec<FunctionDeclaration>,
    },
    /// An `enum` definition.
    EnumDef {
        name: Identifier,
        variants: Vec<EnumVariant>,
    },
}

// --- Expressions ---

/// An enum representing all possible expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Identifier(Identifier),
    IntegerLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    BooleanLiteral(bool),
    ArrayLiteral { elements: Vec<Expression> },
    StructLiteral { name: Path, fields: Vec<(Identifier, Expression)> },
    
    Prefix { operator: Token, right: Box<Expression> },
    Infix { left: Box<Expression>, operator: Token, right: Box<Expression> },
    
    If {
        condition: Box<Expression>,
        consequence: BlockStatement,
        alternative: Option<BlockStatement>,
    },
    While {
        condition: Box<Expression>,
        body: BlockStatement,
    },
    
    Block(BlockStatement),
    
    /// An anonymous function literal.
    FunctionLiteral {
        params: Vec<(Identifier, Type)>,
        return_type: Type,
        effects: Option<Vec<Path>>,
        body: BlockStatement,
    },
    
    Call {
        function: Box<Expression>, // Can be an identifier or another expression.
        arguments: Vec<Expression>,
    },
    
    Index { left: Box<Expression>, index: Box<Expression> },
    
    Handle {
        expression: Box<Expression>,
        handler: HandlerExpression,
    },
    
    Perform {
        effect: Path,
        operation: Identifier,
        arguments: Vec<Expression>,
    },
    
    Match {
        expression: Box<Expression>,
        arms: Vec<(Pattern, Expression)>,
    },

    /// An `extern fn` declaration.
    ExternFn { signature: FunctionSignature },
}

// --- Helper Structs for Statements and Expressions ---

/// Represents a named function's full declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDeclaration {
    pub name: Identifier,
    pub params: Vec<(Identifier, Type)>,
    pub return_type: Type,
    pub effects: Option<Vec<Path>>, // Effects this function can perform.
    pub body: BlockStatement,
}

/// Represents a function's signature, used in traits and effects.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSignature {
    pub name: Identifier,
    pub params: Vec<(Identifier, Type)>,
    pub return_type: Type,
    pub effects: Option<Vec<Path>>,
}

/// Represents the handler part of a `handle` expression.
#[derive(Debug, Clone, PartialEq)]
pub enum HandlerExpression {
    /// A named handler, e.g., `with myHandler`
    Named(Path),
    /// An inline handler, e.g., `with { ... }`
    Inline { handlers: Vec<FunctionDeclaration> },
}

/// Represents a variant in an `enum` definition.
#[derive(Debug, Clone, PartialEq)]
pub enum EnumVariant {
    /// A unit variant, e.g., `A`
    Unit(Identifier),
    /// A tuple variant, e.g., `B(i64)`
    Tuple(Identifier, Vec<Type>),
}

/// Represents a pattern in a `match` arm.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard, // `_`
    Identifier(Identifier),
    Literal(Expression), // For matching against literals like 1, "hello", true
    Enum { path: Path, patterns: Vec<Pattern> },
}


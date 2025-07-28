use crate::hir::Ty;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StaticId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(usize); // Represents an SSA virtual register.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(usize); // Represents a defined struct/array type.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldIndex(usize);


// --- Top-Level MIR Items ---

/// A top-level item in a MIR module. A full program is a `Vec<MirItem>`.
#[derive(Debug, Clone)]
pub enum MirItem {
    /// A function definition with its body.
    Function(MirFunction),
    /// A definition for static, global data.
    Static(MirStatic),
}

/// Represents a single, complete function in the MIR.
#[derive(Debug, Clone)]
pub struct MirFunction {
    pub name: String,
    pub id: FunctionId,
    pub signature: MirSignature,
    /// Storage for all local variables, indexed by `LocalId`.
    pub locals: Vec<Ty>,
    /// The body of the function.
    pub body: MirBlock,
}

/// Represents a function's parameters and return type.
#[derive(Debug, Clone)]
pub struct MirSignature {
    pub params: Vec<(LocalId, Ty)>,
    pub return_type: Ty,
}

/// Represents a global, static data item.
#[derive(Debug, Clone)]
pub struct MirStatic {
    pub name: String,
    pub id: StaticId,
    pub ty: Ty,
    /// The initial data for this static variable.
    pub data: Vec<u8>,
}

// --- MIR Core Structures ---

/// A block is a sequence of statements that produces a final value.
#[derive(Debug, Clone)]
pub struct MirBlock {
    pub stmts: Vec<MirStatement>,
    /// The final expression that provides the value for the block.
    pub tail_expr: Option<Box<MirExpr>>,
}

/// A statement in the MIR; primarily assignment.
#[derive(Debug, Clone)]
pub struct MirStatement {
    /// The location to store the result.
    pub place: MirPlace,
    /// The expression whose value will be stored.
    pub value: MirExpr,
}

/// Represents a location in memory (a variable, a field, etc.).
#[derive(Debug, Clone)]
pub enum MirPlace {
    /// A local variable.
    Local(LocalId),
    /// A field of a struct.
    Field {
        base: Box<MirPlace>,
        field: FieldIndex,
    },
    // Could also include `Index { ... }` for arrays.
}


// --- MIR Expression Tree ---

/// The core expression tree for the MIR. This is a canonical, simplified set of nodes.
#[derive(Debug, Clone)]
pub enum MirExpr {
    // --- Operands ---
    /// A literal value (e.g., 42, true, "hello").
    Literal(MirLiteral),
    /// A reference to a value in a place (e.g., reading a local variable).
    Operand(MirPlace),

    // --- Operations ---
    /// A standard binary operation (e.g., Add, Sub, Eq).
    BinaryOp {
        op: MirBinaryOp,
        left: Box<MirExpr>,
        right: Box<MirExpr>,
    },
    /// A standard unary operation (e.g., Not, Negate).
    UnaryOp {
        op: MirUnaryOp,
        operand: Box<MirExpr>,
    },
    /// A call to another function.
    Call {
        target: FunctionId,
        args: Vec<MirExpr>,
    },

    // --- Struct & Array Operations (for Wasm GC target) ---
    StructNew {
        type_id: TypeId,
        fields: Vec<(FieldIndex, MirExpr)>,
    },
    ArrayNew {
        type_id: TypeId, // The type of the array, e.g., `[i32; 10]`
        init_val: Box<MirExpr>,
        len: Box<MirExpr>,
    },
    ArrayLen(Box<MirExpr>),

    // --- Reference Operations ---
    RefCast {
        value: Box<MirExpr>,
        target_type: Ty,
    },

    // --- Control Flow (Tree-like) ---
    /// An if-else expression.
    If {
        cond: Box<MirExpr>,
        then_block: MirBlock,
        else_block: Option<MirBlock>,
    },
    /// An infinite loop. Must be broken out of with `Break`.
    Loop {
        body: MirBlock,
    },
    /// Breaks out of a loop.
    Break,
    /// Continues to the next iteration of a loop.
    Continue,
    /// Returns from the current function.
    Return(Option<Box<MirExpr>>),
    /// Represents code that is known to be unreachable.
    Unreachable,
}

// --- Helper Enums ---

#[derive(Debug, Clone, PartialEq)]
pub enum MirLiteral {
    Bool(bool),
    I32(i32),
    I64(i64),
    // ... other numeric types ...
    String(String), // Will be lowered to a pointer to static data.
    Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirBinaryOp {
    Add, Sub, Mul, Div,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirUnaryOp {
    Not,
    Negate,
}

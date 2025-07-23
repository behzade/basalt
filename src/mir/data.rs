//! src/mir/data.rs
//!
//! Defines the core data structures for the Mid-level Intermediate Representation (MIR).
//! The MIR simplifies control flow into a graph of basic blocks and makes memory
//! operations more explicit.

use crate::hir;
use crate::ast;
use std::collections::HashMap;

//================================================================================//
//                                Core Data Structures
//================================================================================//

/// A unique identifier for a basic block within a function.
pub type BasicBlockId = usize;

/// Represents the entire program in MIR form.
#[derive(Debug)]
pub struct MirProgram<'src> {
    pub functions: HashMap<&'src str, MirFunction<'src>>,
}

/// Represents a single function in MIR form.
#[derive(Debug)]
pub struct MirFunction<'src> {
    pub name: &'src str,
    pub params: Vec<LocalId>,
    pub return_type: hir::Ty<'src>,
    pub basic_blocks: Vec<BasicBlock<'src>>,
    pub locals: HashMap<LocalId, MirLocal<'src>>,
    pub next_local_id: LocalId,
}

/// A "basic block" is a straight line of code with no jumps in or out, except
/// at the beginning and end, respectively.
#[derive(Debug)]
pub struct BasicBlock<'src> {
    pub id: BasicBlockId,
    pub statements: Vec<Statement<'src>>,
    pub terminator: Terminator<'src>,
}

/// A local variable or temporary within a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub usize);

/// Information about a local variable.
#[derive(Debug)]
pub struct MirLocal<'src> {
    pub id: LocalId,
    pub ty: hir::Ty<'src>,
    pub is_param: bool,
}

//================================================================================//
//                              Statements & Rvalues
//================================================================================//

/// A statement within a basic block. MIR statements are simple assignments
/// and do not affect control flow.
#[derive(Debug)]
pub enum Statement<'src> {
    /// Assigns the value of an `Rvalue` to a `Place`.
    Assign(Place, Rvalue<'src>),
}

/// An "Rvalue" is an expression that produces a value.
/// These are the right-hand sides of assignments.
#[derive(Debug)]
pub enum Rvalue<'src> {
    /// Use an existing operand.
    Use(Operand<'src>),
    /// A binary operation.
    BinaryOp(ast::BinaryOp, Operand<'src>, Operand<'src>),
    /// A unary operation.
    UnaryOp(ast::UnaryOp, Operand<'src>),
    /// Create a reference to a place.
    Ref(Place),
}

/// A "place" is a location in memory, like a local variable or a field of a struct.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Place {
    pub local: LocalId,
    // Projection allows accessing fields, e.g., `_1.user.name`
    // For now, we'll keep it simple.
    // pub projection: Vec<ProjectionElem<'src>>,
}

/// An "operand" is a value used in an Rvalue. It's either a constant or
/// a copy of a value from a place.
#[derive(Debug)]
pub enum Operand<'src> {
    Constant(ast::Literal<'src>),
    Copy(Place),
}

//================================================================================//
//                                  Terminators
//================================================================================//

/// A "terminator" is an operation that ends a basic block, determining
/// which block to execute next.
#[derive(Debug)]
pub enum Terminator<'src> {
    /// Unconditional jump to another block.
    Goto {
        target: BasicBlockId,
    },
    /// Conditional jump. If the operand is true, jumps to `true_target`;
    /// otherwise, jumps to `false_target`.
    SwitchInt {
        discr: Operand<'src>,
        targets: Vec<(u64, BasicBlockId)>, // (value, target_block)
        otherwise: BasicBlockId,
    },
    /// A function call.
    Call {
        func: &'src str, // For now, simple function name.
        args: Vec<Operand<'src>>,
        destination: Place,
        target: BasicBlockId, // Block to jump to after the call.
    },
    /// An effect operation that captures the continuation.
    Perform {
        effect: &'src str,
        operation: &'src str,
        args: Vec<Operand<'src>>,
        destination: Place,
        continuation: BasicBlockId, // Block to resume after handler returns
    },
    /// Resume execution after an effect operation was handled.
    Resume {
        value: Operand<'src>,
        target: BasicBlockId,
    },
    /// Return from the function.
    Return,
    /// Should not be reachable.
    Unreachable,
} 
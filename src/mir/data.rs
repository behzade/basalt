//! src/mir/data.rs
//!
//! Defines the core data structures for the Mid-level Intermediate Representation (MIR).
//! The MIR uses structured control flow that maps directly to WebAssembly's nested
//! block, loop, and if instructions.

use crate::ast;
use crate::hir;
use std::collections::HashMap;

//================================================================================//
//                                Core Data Structures
//================================================================================//

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
    pub body: Vec<MirInstruction<'src>>,
    pub locals: HashMap<LocalId, MirLocal<'src>>,
    pub next_local_id: LocalId,
    /// Handler context that this function expects to receive
    pub handler_context: HandlerContext<'src>,
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

/// Represents the handler context for a function or expression
#[derive(Debug, Clone)]
pub struct HandlerContext<'src> {
    /// Stack of active handlers, from innermost to outermost
    pub handlers: Vec<HandlerEntry<'src>>,
}

/// A single handler entry in the handler stack
#[derive(Debug, Clone)]
pub struct HandlerEntry<'src> {
    /// The effect this handler handles
    pub effect: &'src str,
    /// The handler name or identifier
    pub handler: &'src str,
    /// Whether this handler was pushed by a handle expression (vs inherited from caller)
    pub is_local: bool,
}

impl<'src> HandlerContext<'src> {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Push a new handler onto the stack
    pub fn push_handler(&mut self, effect: &'src str, handler: &'src str) {
        self.handlers.push(HandlerEntry {
            effect,
            handler,
            is_local: true,
        });
    }

    /// Pop the topmost local handler from the stack
    pub fn pop_local_handler(&mut self) -> Option<HandlerEntry<'src>> {
        // Find the topmost local handler
        if let Some(index) = self.handlers.iter().rposition(|h| h.is_local) {
            Some(self.handlers.remove(index))
        } else {
            None
        }
    }

    /// Find the handler for a given effect (searches from top of stack)
    pub fn find_handler(&self, effect: &str) -> Option<&'src str> {
        self.handlers
            .iter()
            .rev()
            .find(|h| h.effect == effect)
            .map(|h| h.handler)
    }

    /// Clone the context for passing to called functions
    pub fn clone_for_call(&self) -> Self {
        Self {
            handlers: self.handlers.clone(),
        }
    }
}

//================================================================================//
//                              Structured Instructions
//================================================================================//

/// A structured instruction in MIR that maps directly to WebAssembly control flow.
#[derive(Debug)]
pub enum MirInstruction<'src> {
    /// Simple assignment statement
    Assign(Place, Rvalue<'src>),
    
    /// WebAssembly block instruction
    Block { body: Vec<MirInstruction<'src>> },
    
    /// WebAssembly loop instruction
    Loop { body: Vec<MirInstruction<'src>> },
    
    /// WebAssembly if/else instruction
    If { 
        condition: Operand<'src>, 
        then_block: Vec<MirInstruction<'src>>, 
        else_block: Vec<MirInstruction<'src>> 
    },
    
    /// WebAssembly br instruction - break from N-th enclosing Block or Loop
    Break(u32),
    
    /// WebAssembly br_if instruction - conditional break
    ConditionalBreak(Operand<'src>, u32),
    
    /// Function call
    Call { 
        func: &'src str, 
        args: Vec<Operand<'src>>, 
        destination: Place 
    },
    
    /// Effect operation using dynamic handler lookup
    Perform {
        effect: &'src str,
        operation: &'src str,
        args: Vec<Operand<'src>>,
        destination: Place,
    },
    
    /// Push a handler onto the dynamic handler stack
    PushHandler {
        effect: &'src str,
        handler: &'src str,
        body: Vec<MirInstruction<'src>>,
    },
    
    /// Pop a handler from the dynamic handler stack
    PopHandler,
    
    /// Resume execution after an effect operation was handled
    Resume {
        value: Operand<'src>,
    },
    
    /// Pattern matching for match expressions
    PatternMatch {
        scrutinee: Operand<'src>,
        arms: Vec<(Pattern<'src>, Vec<MirInstruction<'src>>)>,
        otherwise: Vec<MirInstruction<'src>>,
    },
    
    /// Return from the current function
    Return,
    
    /// Unreachable code (should never be executed)
    Unreachable,
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
    /// Create an array from a list of operands.
    Array(Vec<Operand<'src>>),
    /// Create a map from key-value pairs.
    Map(Vec<(Operand<'src>, Operand<'src>)>),
    /// Initialize a struct with field values.
    StructInit {
        path: &'src str,
        fields: HashMap<&'src str, Operand<'src>>,
    },
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
//                                  Patterns
//================================================================================//

/// A pattern used in match expressions for pattern matching.
#[derive(Debug, Clone)]
pub struct Pattern<'src> {
    pub kind: PatternKind<'src>,
    pub ty: hir::Ty<'src>,
}

/// The different kinds of patterns that can be matched against.
#[derive(Debug, Clone)]
pub enum PatternKind<'src> {
    /// A literal pattern, e.g., `1`, `"hello"`, `true`.
    Literal(ast::Literal<'src>),
    /// An identifier that binds the matched value to a new variable, e.g., `x`.
    Binding { name: &'src str, is_mut: bool },
    /// A path to an enum variant, e.g., `Option::Some(x)`.
    AdtVariant {
        path: &'src str,
        fields: Vec<Pattern<'src>>,
    },
    /// The wildcard pattern `_`.
    Wildcard,
}


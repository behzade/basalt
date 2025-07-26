//! src/mir/builder.rs
//!
//! A helper for constructing MIR for a function using structured control flow.

use super::data::*;
use crate::hir;
use std::collections::HashMap;

pub struct MirBuilder<'src> {
    pub body: Vec<MirInstruction<'src>>,
    pub locals: HashMap<LocalId, MirLocal<'src>>,
    next_local_id: usize,
    /// Dynamic handler context for this function
    handler_context: HandlerContext<'src>,
    /// Stack of active control flow blocks for label indexing
    /// Each entry is (block_type, depth) where depth is the nesting level
    control_flow_stack: Vec<(ControlFlowType, u32)>,
}

#[derive(Debug, Clone, Copy)]
enum ControlFlowType {
    Block,
    Loop,
}

impl<'src> MirBuilder<'src> {
    pub fn new() -> Self {
        Self {
            body: Vec::new(),
            locals: HashMap::new(),
            next_local_id: 0,
            handler_context: HandlerContext::new(),
            control_flow_stack: Vec::new(),
        }
    }

    /// Adds an instruction to the current body
    pub fn push_instruction(&mut self, instruction: MirInstruction<'src>) {
        self.body.push(instruction);
    }

    /// Pushes a control flow block onto the stack
    pub fn push_control_flow(&mut self, block_type: ControlFlowType) {
        let depth = self.control_flow_stack.len() as u32;
        self.control_flow_stack.push((block_type, depth));
    }

    /// Pops a control flow block from the stack
    pub fn pop_control_flow(&mut self) -> Option<(ControlFlowType, u32)> {
        self.control_flow_stack.pop()
    }

    /// Gets the label index for breaking from the N-th enclosing block/loop
    /// Returns the number of blocks to break out of
    pub fn get_break_label(&self, target_depth: u32) -> u32 {
        let current_depth = self.control_flow_stack.len() as u32;
        if target_depth >= current_depth {
            panic!("Invalid break target depth: {} >= {}", target_depth, current_depth);
        }
        current_depth - target_depth - 1
    }

    /// Pushes an effect handler onto the dynamic handler stack
    pub fn push_effect_handler(&mut self, effect: &'src str, handler: &'src str) {
        self.handler_context.push_handler(effect, handler);
    }

    /// Pops an effect handler from the dynamic handler stack
    pub fn pop_effect_handler(&mut self) -> Option<HandlerEntry<'src>> {
        self.handler_context.pop_local_handler()
    }

    /// Gets the current effect handler for a given effect (for static analysis)
    pub fn get_effect_handler(&self, effect: &str) -> Option<&'src str> {
        self.handler_context.find_handler(effect)
    }

    /// Gets the current handler context
    pub fn get_handler_context(&self) -> &HandlerContext<'src> {
        &self.handler_context
    }

    /// Sets the handler context (used when inheriting from caller)
    pub fn set_handler_context(&mut self, context: HandlerContext<'src>) {
        self.handler_context = context;
    }

    /// Allocates a new local variable.
    pub fn new_local(&mut self, ty: hir::Ty<'src>, is_param: bool) -> LocalId {
        let id = LocalId(self.next_local_id);
        self.next_local_id += 1;
        self.locals.insert(id, MirLocal { id, ty, is_param });
        id
    }

    /// Finalizes the build process, returning the constructed MirFunction.
    pub fn build(
        self,
        name: &'src str,
        params: Vec<LocalId>,
        return_type: hir::Ty<'src>,
    ) -> MirFunction<'src> {
        MirFunction {
            name,
            params,
            return_type,
            body: self.body,
            locals: self.locals,
            next_local_id: LocalId(self.next_local_id),
            handler_context: self.handler_context,
        }
    }
}


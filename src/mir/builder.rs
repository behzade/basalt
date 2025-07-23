//! src/mir/builder.rs
//!
//! A helper for constructing MIR for a function.

use super::data::*;
use crate::hir;
use std::collections::HashMap;

pub struct MirBuilder<'src> {
    pub basic_blocks: Vec<BasicBlock<'src>>,
    pub locals: HashMap<LocalId, MirLocal<'src>>,
    next_local_id: usize,
    current_block: BasicBlockId,
    /// Dynamic handler context for this function
    handler_context: HandlerContext<'src>,
}

impl<'src> MirBuilder<'src> {
    pub fn new() -> Self {
        let mut builder = Self {
            basic_blocks: Vec::new(),
            locals: HashMap::new(),
            next_local_id: 0,
            current_block: 0, // Placeholder, will be set by `new_basic_block`
            handler_context: HandlerContext::new(),
        };
        // Every function starts with at least one block.
        builder.current_block = builder.new_basic_block();
        builder
    }

    /// Creates a new, empty basic block and returns its ID.
    pub fn new_basic_block(&mut self) -> BasicBlockId {
        let id = self.basic_blocks.len();
        self.basic_blocks.push(BasicBlock {
            id,
            statements: Vec::new(),
            // All blocks start with an Unreachable terminator until properly closed.
            terminator: Terminator::Unreachable,
        });
        id
    }

    /// Switches the builder to work on a different basic block.
    pub fn switch_to_block(&mut self, block_id: BasicBlockId) {
        self.current_block = block_id;
    }

    /// Gets the current block ID.
    pub fn current_block(&self) -> BasicBlockId {
        self.current_block
    }

    /// Adds a statement to the current basic block.
    pub fn push_statement(&mut self, statement: Statement<'src>) {
        self.basic_blocks[self.current_block]
            .statements
            .push(statement);
    }

    /// Sets the terminator for the current basic block.
    pub fn set_terminator(&mut self, terminator: Terminator<'src>) {
        self.basic_blocks[self.current_block].terminator = terminator;
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
            basic_blocks: self.basic_blocks,
            locals: self.locals,
            next_local_id: LocalId(self.next_local_id),
            handler_context: self.handler_context,
        }
    }
} 
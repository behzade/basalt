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
    /// Stack of active effect handlers for continuation capture
    effect_handlers: Vec<(&'src str, &'src str)>, // (effect_name, handler_name)
}

impl<'src> MirBuilder<'src> {
    pub fn new() -> Self {
        let mut builder = Self {
            basic_blocks: Vec::new(),
            locals: HashMap::new(),
            next_local_id: 0,
            current_block: 0, // Placeholder, will be set by `new_basic_block`
            effect_handlers: Vec::new(),
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

    /// Pushes an effect handler onto the stack.
    pub fn push_effect_handler(&mut self, effect: &'src str, handler: &'src str) {
        self.effect_handlers.push((effect, handler));
    }

    /// Pops an effect handler from the stack.
    pub fn pop_effect_handler(&mut self) -> Option<(&'src str, &'src str)> {
        self.effect_handlers.pop()
    }

    /// Gets the current effect handler for a given effect.
    pub fn get_effect_handler(&self, effect: &str) -> Option<&'src str> {
        self.effect_handlers
            .iter()
            .rev() // Search from top of stack
            .find(|(e, _)| *e == effect)
            .map(|(_, h)| *h)
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
        }
    }
} 